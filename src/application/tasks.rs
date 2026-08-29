//! Control-loop and dual-output Embassy tasks.
//!
//! Owns the two long-lived application tasks: `control_loop_task` (drain
//! commands, read sensors, update control, drive LEDC, feed the watchdog,
//! emit telemetry) and `dual_output_task` (route formatted output to the
//! active transport via the command multiplexer). The per-tick logic is
//! factored into `control_loop_tick`/`dual_output_tick` for host-side testing.

extern crate alloc;

use crate::application::service_container::{ContainerError, ServiceContainer};
#[cfg(any(feature = "instrumentation", feature = "test"))]
use crate::application::stage_instrumentation::GuardState;
use crate::application::stage_instrumentation::{StageName, StageReporter, WatchdogState};
use crate::config::SystemStatus;
use crate::error::AppError;
use crate::hardware::error_counters::try_send_output;
use crate::hardware::ledc_guard;
use crate::input::multiplexer::CommChannel;
use crate::logging::traceability::{
    trace_actuation, trace_guard, trace_telemetry, TraceId, TRACE_EVENT_MAX_LEN,
};
use crate::output::artisan::ArtisanFormatter;
use crate::output::artisan::MutableArtisanFormatter;
use crate::safety::regression;
use crate::safety::watchdog::WatchdogError;
use core::fmt::Write;
use embassy_executor::task;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Instant, Timer};
use heapless::String;
use log::{debug, info, warn};

type OutputChannel = Channel<
    embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
    String<TRACE_EVENT_MAX_LEN>,
    { crate::application::service_container::ARTISAN_OUTPUT_CHANNEL_SIZE },
>;

#[derive(Clone, Copy, Debug)]
enum ControlLoopStage {
    Idle,
    SensorRead,
    ControlUpdate,
    LedcWrite,
    WatchdogFeed,
    TelemetryEmit,
}

struct StageTracker {
    tick_start: Instant,
    current_stage: ControlLoopStage,
}

impl StageTracker {
    fn new() -> Self {
        Self {
            tick_start: Instant::now(),
            current_stage: ControlLoopStage::Idle,
        }
    }

    fn start_tick(&mut self, start: Instant) {
        self.tick_start = start;
        self.current_stage = ControlLoopStage::Idle;
    }

    fn set_stage(&mut self, stage: ControlLoopStage) {
        self.current_stage = stage;
    }

    fn elapsed(&self) -> Duration {
        Instant::now().saturating_duration_since(self.tick_start)
    }

    fn clear(&mut self) {
        self.current_stage = ControlLoopStage::Idle;
    }
}

#[derive(Clone, Copy, Debug)]
struct ControlUpdateSnapshot {
    desired_output: f32,
    applied_output: f32,
    fan_output: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct WatchdogSnapshot {
    feed_ok: bool,
    last_failure: Option<&'static str>,
    guard_timeouts: u16,
}

struct TickState {
    formatter: MutableArtisanFormatter,
    was_continuous: bool,
    /// L2: tracks the rising edge of `roast_logger::is_logging_active()` so
    /// the formatter epoch resets on START itself (sharing the roast
    /// logger's epoch), not just on the continuous-output rising edge
    /// (which also fires on pre-roast OT1/OT2 and would desynchronise
    /// the stream's `time_s` from the `#DUMP` `time_s`).
    was_roast_active: bool,
    last_guard_total_timeouts: u16,
    stage_tracker: StageTracker,
    stage_reporter: StageReporter,
    tick_trace_id: Option<TraceId>,
    prev_watchdog_state: WatchdogState,
    tick_app_error: Option<AppError>,
    sensor_err: Option<ContainerError>,
    consecutive_sensor_errors: u8,
    /// Timestamp of last telemetry emission (`None` → never emitted).
    /// Bug M1 (2026-07-25): the previous design throttled telemetry by tick
    /// count assuming every tick was exactly 100 ms. The control loop spends
    /// ≈ 190 ms waiting for the MAX31856 conversion every tick, so the real
    /// tick period is ~290 ms; a 10-tick gate therefore emitted every ≈ 2.9 s
    /// — telemetry 3× slower than documented and `#DUMP` drainage 3× slower
    /// than the comment claimed. We gate by elapsed wall-clock instead, so
    /// the rate is `DEFAULT_OUTPUT_INTERVAL_MS` regardless of how the tick
    /// budget is spent.
    last_telemetry_emit: Option<Instant>,
    // Bug V2-8: the roast epoch (`time_s` base for `#DUMP` and the ring
    // logger) is now OWNED by `RoastLogger` itself — set by its
    // `start_roast(now)`, called from `handle_start_roast`. The per-task
    // `roast_start: Option<Instant>` field and the rising-edge
    // `mark_continuous_started` are gone: capturing the epoch on the
    // continuous-telemetry rising edge was wrong (manual `OT1`/`OT2` also
    // fire that edge, shifting the time base by minutes) and the field was
    // never reset between roasts (the second roast inherited the first's
    // uptime). The logger's internal `start` fixes both.
}

impl TickState {
    fn new() -> Self {
        Self {
            formatter: MutableArtisanFormatter::new(),
            was_continuous: false,
            was_roast_active: false,
            last_guard_total_timeouts: ledc_guard::total_timeouts(),
            stage_tracker: StageTracker::new(),
            stage_reporter: StageReporter::new(),
            tick_trace_id: None,
            prev_watchdog_state: WatchdogState::None,
            tick_app_error: None,
            sensor_err: None,
            consecutive_sensor_errors: 0,
            last_telemetry_emit: None,
        }
    }

    // Bug V2-8: `mark_continuous_started` removed — the epoch is owned by
    // `RoastLogger::start_roast` (called from `handle_start_roast`), no
    // longer captured on the continuous-telemetry rising edge.
}

#[cfg(any(feature = "instrumentation", feature = "test"))]
fn report_stage_instrumentation(
    stage_reporter: &StageReporter,
    stage_name: StageName,
    elapsed_ms: u64,
    guard_timeout_happened: bool,
    watchdog_state: WatchdogState,
    output_channel: &Channel<
        embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
        String<TRACE_EVENT_MAX_LEN>,
        { crate::application::service_container::ARTISAN_OUTPUT_CHANNEL_SIZE },
    >,
) {
    let guard_state = if guard_timeout_happened {
        GuardState::Timeout
    } else {
        GuardState::Ok
    };
    if let Some(report) =
        stage_reporter.report_simple(stage_name, elapsed_ms, guard_state, watchdog_state)
    {
        try_send_output(output_channel, report);
    }
}

#[cfg(not(any(feature = "instrumentation", feature = "test")))]
fn report_stage_instrumentation(
    _stage_reporter: &StageReporter,
    _stage_name: StageName,
    _elapsed_ms: u64,
    _guard_timeout_happened: bool,
    _watchdog_state: WatchdogState,
    _output_channel: &Channel<
        embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
        String<TRACE_EVENT_MAX_LEN>,
        { crate::application::service_container::ARTISAN_OUTPUT_CHANNEL_SIZE },
    >,
) {
}

#[cfg(any(feature = "instrumentation", feature = "test"))]
fn report_stage_with_failure(
    stage_reporter: &StageReporter,
    stage_name: StageName,
    elapsed_ms: u64,
    guard_timeout_happened: bool,
    watchdog_state: WatchdogState,
    failure_marker: Option<&'static str>,
    output_channel: &Channel<
        embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
        String<TRACE_EVENT_MAX_LEN>,
        { crate::application::service_container::ARTISAN_OUTPUT_CHANNEL_SIZE },
    >,
) {
    let guard_state = if guard_timeout_happened {
        GuardState::Timeout
    } else {
        GuardState::Ok
    };
    if let Some(report) = stage_reporter.report(
        stage_name,
        elapsed_ms,
        guard_state,
        watchdog_state,
        failure_marker,
    ) {
        try_send_output(output_channel, report);
    }
}

#[cfg(not(any(feature = "instrumentation", feature = "test")))]
fn report_stage_with_failure(
    _stage_reporter: &StageReporter,
    _stage_name: StageName,
    _elapsed_ms: u64,
    _guard_timeout_happened: bool,
    _watchdog_state: WatchdogState,
    _failure_marker: Option<&'static str>,
    _output_channel: &Channel<
        embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
        String<TRACE_EVENT_MAX_LEN>,
        { crate::application::service_container::ARTISAN_OUTPUT_CHANNEL_SIZE },
    >,
) {
}

async fn drain_commands(tick_state: &mut TickState) {
    let cmd_channel = ServiceContainer::get_artisan_channel();
    // Bug L14 (2026-08-10): wire the multiplexer's 60 s idle failover into
    // the control loop. `is_idle`/`reset` were previously only exercised by
    // tests; the arrival-based reset inside `on_command_received` only fired
    // when a NEW command arrived, so a dead session kept the active channel
    // latched and the dual-output task kept writing (and timing out at 50 ms
    // per line) to a vanished host. This releases the channel after
    // `IDLE_TIMEOUT_SECS` of silence; the next command on either wire
    // re-activates it. On the host test build the mocked `Instant` reports
    // zero elapsed time, so the check is inert there (never idle once a
    // command was seen; a fresh `None` mux resets to `None` — a no-op).
    critical_section::with(|cs| {
        let multiplexer = ServiceContainer::get_multiplexer();
        let mut guard = multiplexer.borrow(cs).borrow_mut();
        if let Some(mux) = guard.as_mut() {
            if mux.is_idle() {
                mux.reset();
            }
        }
    });
    // Bug B26: the previous comment claimed a fallback pattern that does NOT
    // exist — when the artisan channel is full, `try_send` in
    // `transport_tasks::handle_parsed_command` returns Err, and the only
    // surfacing was `send_channel_full_error` emitting
    // `ERR channel_full command_dropped` to the host. The previous wording
    // ("fallback pattern... prevents silent drops") implied an internal
    // retry path that we never implemented. The host is now told explicitly
    // when a command is dropped due to backpressure, which is the contract
    // Artisan uses to retry. Further hardening (priority eviction of older
    // commands when STOP/EmergencyStop arrives) is documented but out of
    // scope for this audit — touch only when adding pre-emptive priority
    // support.
    // Audit M-X2 (2026-08-11): the per-tick rate-limit branch
    // (`cmds_this_tick > MAX_COMMANDS_PER_TICK` → `ERR rate_limited`) was
    // unreachable: `MAX_COMMANDS_PER_TICK == ARTISAN_CMD_CHANNEL_SIZE == 16`
    // (constants.rs), so a 16-slot channel can never deliver 17 commands in
    // one tick. Real backpressure is the channel capacity itself, surfaced to
    // the host as `ERR channel_full command_dropped` in transport_tasks.rs.
    // The misleading branch was removed; do NOT re-add unless the constant is
    // lowered below the channel size first.
    while let Ok(traced_command) = cmd_channel.try_receive() {
        if let crate::config::ArtisanCommand::RunRegression = traced_command.command {
            // Bug M9 (2026-07-26): the previous `continue` here skipped the
            // normal dispatch, so REG produced NO output — Artisan had zero
            // feedback. `request_regression()` starts the runner (embedded +
            // `regression` feature) or is a no-op stub (host); the command
            // then flows through the normal handler path so
            // `handle_run_regression` emits `OK regression_started` or
            // `ERR regression_disabled` via the output channel.
            regression::request_regression();
        }

        tick_state.tick_trace_id = Some(traced_command.trace_id);
        let output_channel = ServiceContainer::get_output_channel();

        let command_outcome = ServiceContainer::with_roaster_async(
            |roaster: &mut crate::control::roaster_control::RoasterControl| {
                let start_time = Instant::now();
                let result = roaster.process_artisan_command(traced_command.command);
                let latency = start_time.elapsed().as_micros() as u32;

                roaster.status_mut().command_latency_us = latency;
                if latency > roaster.status_mut().max_command_latency_us {
                    roaster.status_mut().max_command_latency_us = latency;
                }

                let status = roaster.get_status();
                (result, latency, status)
            },
        )
        .await;

        let mut latency_us = 0;
        let mut status_snapshot = SystemStatus::default();

        match command_outcome {
            Ok((result, latency, status)) => {
                latency_us = latency;
                status_snapshot = status;
                match result {
                    Ok(()) => {
                        debug!("Processed Artisan command successfully");
                        if let crate::config::ArtisanCommand::StatusReport = traced_command.command
                        {
                            let response =
                                ArtisanFormatter::format_status_response(&status_snapshot);

                            if let Ok(line) =
                                String::<TRACE_EVENT_MAX_LEN>::try_from(response.as_str())
                            {
                                try_send_output(output_channel, line);
                            } else {
                                // Emit ERR so Artisan sees explicit error rather than
                                // a silent truncation.
                                let mut msg = heapless::String::<TRACE_EVENT_MAX_LEN>::new();
                                let _ = msg.push_str("ERR status_too_long");
                                try_send_output(output_channel, msg);
                            }
                        } else if let crate::config::ArtisanCommand::ReadStatus =
                            traced_command.command
                        {
                            let response =
                                ArtisanFormatter::format_read_response_full(&status_snapshot);

                            if let Ok(line) =
                                String::<TRACE_EVENT_MAX_LEN>::try_from(response.as_str())
                            {
                                try_send_output(output_channel, line);
                            } else {
                                let mut msg = heapless::String::<TRACE_EVENT_MAX_LEN>::new();
                                let _ = msg.push_str("ERR status_too_long");
                                try_send_output(output_channel, msg);
                            }
                        }
                    }
                    Err(err) => {
                        warn!("Failed to process Artisan command: {:?}", err);
                        send_handler_error(output_channel, &err);
                    }
                }
            }
            Err(err) => {
                warn!("Control update container error: {:?}", err);
            }
        }

        trace_actuation(
            &traced_command,
            status_snapshot.ssr_output,
            status_snapshot.fan_output,
            latency_us,
            status_snapshot.saturation_active,
        );
    }
}

async fn read_sensors(
    tick_state: &mut TickState,
    guard_timeout_happened: bool,
    output_channel: &OutputChannel,
) {
    // Control loop now uses async read_sensors() - no longer blocks executor
    tick_state
        .stage_tracker
        .set_stage(ControlLoopStage::SensorRead);
    tick_state.sensor_err = ServiceContainer::roaster_async_sensor_read().await.err();
    let sensor_elapsed_ms = tick_state.stage_tracker.elapsed().as_millis();

    // Report stage instrumentation (watchdog state not yet known, use previous tick's state)
    report_stage_instrumentation(
        &tick_state.stage_reporter,
        StageName::SensorRead,
        sensor_elapsed_ms,
        guard_timeout_happened,
        tick_state.prev_watchdog_state,
        output_channel,
    );

    if tick_state.sensor_err.is_none() {
        // Audit MR-3 (2026-08-11): snapshot BT/ET with a SINGLE
        // `with_roaster_async` acquisition BEFORE the debug! macro. The
        // previous form put `ServiceContainer::read_bean_temperature().await`
        // and `…::read_env_temperature().await` directly in the macro args,
        // which (a) expanded to TWO extra async-mutex lock acquisitions per
        // tick (a 210 ms-worth of lock churn in an already lock-heavy tick),
        // and (b) instrumented the system by distorting the very timing the
        // Diagnostic/`instrumentation` build exists to measure. The macro
        // gate makes this a no-op at the production `Warn` filter, but the
        // `.await`-in-args trap is a latent hazard for future maintainers.
        // Audit CI (2026-08-11): `.unwrap_or` (NOT `.unwrap()`) — the
        // embedded-target clippy flagged the hand-rolled match as
        // `manual_unwrap_or`; `unwrap_or` is not `unwrap_used` (deny applies
        // only to `.unwrap()`).
        let (bt, et) = ServiceContainer::with_roaster_async(|roaster| {
            let status = roaster.get_status();
            (status.bean_temp, status.env_temp)
        })
        .await
        .unwrap_or((0.0, 0.0));
        debug!(
            "stage=SensorRead elapsed={}ms Sensors: BT: {:.1}°C, ET: {:.1}°C",
            sensor_elapsed_ms, bt, et
        );
    } else {
        warn!(
            "stage=SensorRead elapsed={}ms Sensor read error: {:?}",
            sensor_elapsed_ms, tick_state.sensor_err
        );
        // Note: ContainerError doesn't convert directly to AppError, so we log but don't capture
        // as AppError diagnostics. This is expected - sensor errors are infrastructure-level.
    }
}

async fn update_control_stage(
    tick_state: &mut TickState,
    guard_timeout_happened: bool,
    output_channel: &OutputChannel,
) -> (Option<ControlUpdateSnapshot>, Option<SystemStatus>) {
    // Do sync control update separately
    tick_state
        .stage_tracker
        .set_stage(ControlLoopStage::ControlUpdate);
    let mut tick_app_error: Option<AppError> = None;
    let control_snapshot = match ServiceContainer::with_roaster_async(
        |roaster: &mut crate::control::roaster_control::RoasterControl| match roaster
            .update_control(Instant::now())
        {
            Ok(output) => Some(ControlUpdateSnapshot {
                desired_output: roaster.last_desired_heater_output(),
                applied_output: output,
                fan_output: roaster.get_fan_speed(),
            }),
            Err(e) => {
                warn!("Control update error: {:?}", e);
                // Convert RoasterError to AppError for TRACE diagnostics
                tick_app_error = Some(AppError::from(e.clone()));
                None
            }
        },
    )
    .await
    {
        Ok(snapshot) => snapshot,
        Err(err) => {
            warn!("Control update container error: {:?}", err);
            None
        }
    };

    tick_state.tick_app_error = tick_app_error;

    let control_status: Option<SystemStatus> =
        match ServiceContainer::with_roaster_async(|roaster| roaster.get_status()).await {
            Ok(status) => Some(status),
            Err(err) => {
                warn!("Failed to capture status after control update: {:?}", err);
                None
            }
        };

    let control_elapsed_ms = tick_state.stage_tracker.elapsed().as_millis();

    // Report ControlUpdate stage instrumentation
    report_stage_instrumentation(
        &tick_state.stage_reporter,
        StageName::ControlUpdate,
        control_elapsed_ms,
        guard_timeout_happened,
        tick_state.prev_watchdog_state,
        output_channel,
    );

    if let Some(snapshot) = control_snapshot {
        if let Some(status) = control_status {
            debug!(
                "stage=ControlUpdate elapsed={}ms desired_ssr={:.1}% applied_ssr={:.1}% fan={:.1}% saturation_active={} integrator_clamped={} derivative_available={} derivative_rate={:.2}",
                control_elapsed_ms,
                snapshot.desired_output,
                snapshot.applied_output,
                snapshot.fan_output,
                status.saturation_active as u8,
                status.integrator_clamped as u8,
                status.derivative_available as u8,
                status.derivative_rate
            );
        } else {
            debug!(
                "stage=ControlUpdate elapsed={}ms desired_ssr={:.1}% applied_ssr={:.1}% fan={:.1}%",
                control_elapsed_ms,
                snapshot.desired_output,
                snapshot.applied_output,
                snapshot.fan_output
            );
        }
    } else if let Some(status) = control_status {
        debug!(
            "stage=ControlUpdate elapsed={}ms update failed saturation_active={} integrator_clamped={} derivative_available={}",
            control_elapsed_ms,
            status.saturation_active as u8,
            status.integrator_clamped as u8,
            status.derivative_available as u8
        );
    } else {
        debug!(
            "stage=ControlUpdate elapsed={}ms update failed",
            control_elapsed_ms
        );
    }

    (control_snapshot, control_status)
}

fn log_ledc_stage(
    tick_state: &mut TickState,
    guard_timeout_happened: bool,
    guard_total_timeouts: u16,
    control_status: Option<SystemStatus>,
    output_channel: &OutputChannel,
) {
    tick_state
        .stage_tracker
        .set_stage(ControlLoopStage::LedcWrite);
    let ledc_elapsed_ms = tick_state.stage_tracker.elapsed().as_millis();
    if guard_timeout_happened {
        if let Some(status) = control_status {
            debug!(
                "stage=LedcWrite elapsed={}ms guard_timeout_happened={} guard_timeouts={} saturation_active={} integrator_clamped={} derivative_available={}",
                ledc_elapsed_ms,
                guard_timeout_happened,
                guard_total_timeouts,
                status.saturation_active as u8,
                status.integrator_clamped as u8,
                status.derivative_available as u8
            );
        } else {
            debug!(
                "stage=LedcWrite elapsed={}ms guard_timeout_happened={} guard_timeouts={}",
                ledc_elapsed_ms, guard_timeout_happened, guard_total_timeouts
            );
        }
    } else {
        debug!(
            "stage=LedcWrite elapsed={}ms guard_timeout_happened={} guard_timeouts={}",
            ledc_elapsed_ms, guard_timeout_happened, guard_total_timeouts
        );
    }

    // Report LedcWrite stage instrumentation
    report_stage_instrumentation(
        &tick_state.stage_reporter,
        StageName::LedcWrite,
        ledc_elapsed_ms,
        guard_timeout_happened,
        tick_state.prev_watchdog_state,
        output_channel,
    );
}

/// Handle a single watchdog failure, updating roaster status and triggering
/// emergency shutdown if consecutive failures reach the threshold (2).
fn handle_watchdog_failure(
    roaster: &mut crate::control::roaster_control::RoasterControl,
    reason: &'static str,
    previous_watchdog_failure: Option<&'static str>,
    output_channel: &OutputChannel,
) {
    warn!("SAFETY WATCHDOGFEED fail: {}", reason);
    let status = roaster.status_mut();
    status.watchdog_feed_ok = false;
    status.watchdog_last_failure = Some(reason);
    status.watchdog_consecutive_failures = status.watchdog_consecutive_failures.saturating_add(1);
    let needs_emergency = status.watchdog_consecutive_failures >= 2;
    if previous_watchdog_failure != Some(reason) {
        let mut safety = String::<TRACE_EVENT_MAX_LEN>::new();
        let _ = write!(safety, "SAFETY WATCHDOG {}", reason);
        try_send_output(output_channel, safety);
    }
    let _ = status;
    if needs_emergency {
        let _ = roaster.emergency_shutdown("Watchdog failure");
    }
}

async fn feed_watchdog_stage(
    tick_state: &mut TickState,
    guard_timeout_happened: bool,
    guard_total_timeouts: u16,
    control_status: Option<SystemStatus>,
    output_channel: &OutputChannel,
) -> WatchdogSnapshot {
    tick_state
        .stage_tracker
        .set_stage(ControlLoopStage::WatchdogFeed);
    let watchdog_snapshot = match ServiceContainer::with_roaster_async(
        |roaster: &mut crate::control::roaster_control::RoasterControl| {
            // Read previous state with a short borrow to avoid borrowing conflicts
            let previous_watchdog_failure = {
                let status = roaster.status_mut();
                status.watchdog_last_failure
            };

            // Read bean_temp needed for the feed call
            let bean_temp = {
                let status = roaster.status_mut();
                status.bean_temp
            };

            match ServiceContainer::get_instance()
                .with_watchdog(|watchdog| watchdog.feed_async(bean_temp))
            {
                Ok(_) => {
                    let status = roaster.status_mut();
                    status.watchdog_feed_ok = true;
                    status.watchdog_last_failure = None;
                    status.watchdog_consecutive_failures = 0;
                }
                Err(ContainerError::Watchdog(err)) => {
                    handle_watchdog_failure(
                        roaster,
                        err.reason(),
                        previous_watchdog_failure,
                        output_channel,
                    );
                }
                Err(ContainerError::WatchdogUninitialized) => {
                    handle_watchdog_failure(
                        roaster,
                        WatchdogError::NotInitialized.reason(),
                        previous_watchdog_failure,
                        output_channel,
                    );
                }
                Err(err) => {
                    warn!("Watchdog container error: {:?}", err);
                }
            }

            // Re-borrow status for the remaining code after match arms that
            // may have dropped the borrow to call emergency_shutdown.
            let status = roaster.status_mut();
            status.ledc_guard_timeouts = guard_total_timeouts;
            if guard_timeout_happened {
                let mut guard_msg = String::<TRACE_EVENT_MAX_LEN>::new();
                let _ = guard_msg.push_str("SAFETY LEDC-GUARD timeout");
                try_send_output(output_channel, guard_msg);
            }

            WatchdogSnapshot {
                feed_ok: status.watchdog_feed_ok,
                last_failure: status.watchdog_last_failure,
                guard_timeouts: guard_total_timeouts,
            }
        },
    )
    .await
    {
        Ok(snapshot) => snapshot,
        Err(err) => {
            warn!("Watchdog container error: {:?}", err);
            WatchdogSnapshot::default()
        }
    };

    let watchdog_elapsed_ms = tick_state.stage_tracker.elapsed().as_millis();

    // Report WatchdogFeed stage instrumentation (now we know the watchdog state)
    let wd_state = if watchdog_snapshot.feed_ok {
        WatchdogState::Ok
    } else {
        WatchdogState::Fail
    };
    let failure_marker = watchdog_snapshot.last_failure;
    report_stage_with_failure(
        &tick_state.stage_reporter,
        StageName::WatchdogFeed,
        watchdog_elapsed_ms,
        guard_timeout_happened,
        wd_state,
        failure_marker,
        output_channel,
    );

    // Update previous watchdog state for next tick
    tick_state.prev_watchdog_state = wd_state;

    if watchdog_snapshot.last_failure.is_some() {
        if let Some(status) = control_status {
            debug!(
                "stage=WatchdogFeed elapsed={}ms guard_timeouts={} watchdog_feed_ok={} watchdog_failure={:?} saturation_active={} integrator_clamped={} derivative_available={}",
                watchdog_elapsed_ms,
                watchdog_snapshot.guard_timeouts,
                watchdog_snapshot.feed_ok,
                watchdog_snapshot.last_failure,
                status.saturation_active as u8,
                status.integrator_clamped as u8,
                status.derivative_available as u8
            );
        } else {
            debug!(
                "stage=WatchdogFeed elapsed={}ms guard_timeouts={} watchdog_feed_ok={} watchdog_failure={:?}",
                watchdog_elapsed_ms,
                watchdog_snapshot.guard_timeouts,
                watchdog_snapshot.feed_ok,
                watchdog_snapshot.last_failure
            );
        }
    } else {
        debug!(
            "stage=WatchdogFeed elapsed={}ms guard_timeouts={} watchdog_feed_ok={} watchdog_failure={:?}",
            watchdog_elapsed_ms,
            watchdog_snapshot.guard_timeouts,
            watchdog_snapshot.feed_ok,
            watchdog_snapshot.last_failure
        );
    }

    tick_state.last_guard_total_timeouts = guard_total_timeouts;

    watchdog_snapshot
}

async fn emit_telemetry_stage(
    tick_state: &mut TickState,
    guard_timeout_happened: bool,
    guard_total_timeouts: u16,
    watchdog_snapshot: &WatchdogSnapshot,
    tick_start: Instant,
    output_channel: &OutputChannel,
) -> Option<SystemStatus> {
    tick_state
        .stage_tracker
        .set_stage(ControlLoopStage::TelemetryEmit);
    let mut is_continuous_now = false;
    let mut status_for_output: Option<SystemStatus> = None;
    // BUG-08 (2026-08-21): the `#DUMP` ring-buffer feed used to be gated on
    // `is_continuous_enabled()` — the same flag as the spontaneous `#` line
    // emission. Now that telemetry is opt-in (`STREAM;ON`, off by default),
    // a full roast run without STREAM would otherwise lose its recoverable
    // log. Capture a SECOND snapshot driven by the roast-logger state so the
    // ring always fills while a roast is active, independent of the stream.
    let mut status_for_logger: Option<SystemStatus> = None;

    if let Err(err) = ServiceContainer::with_roaster_async(
        |roaster: &mut crate::control::roaster_control::RoasterControl| {
            is_continuous_now = roaster.get_output_manager().is_continuous_enabled();
            if is_continuous_now {
                status_for_output = Some(roaster.get_status());
            }
            if crate::logging::roast_logger::is_logging_active() {
                status_for_logger = Some(roaster.get_status());
            }
        },
    )
    .await
    {
        warn!("Telemetry capture failed: {:?}", err);
    }

    if is_continuous_now != tick_state.was_continuous {
        tick_state.formatter.reset();
        tick_state.was_continuous = is_continuous_now;
        // Bug V2-8: the roast epoch is now owned by `RoastLogger` (set by
        // its `start_roast(now)` from `handle_start_roast`). The per-task
        // rising-edge capture (`mark_continuous_started`) is gone — it fired
        // on manual OT1/OT2 too and was never reset between roasts.
    }

    // L2: also align the FORMATTER's epoch with the START event itself,
    // not only with the continuous-output rising edge. Otherwise manual
    // OT1/OT2 before START lets the formatter accumulate time while the
    // roast logger sits at 0s — start_streaming after the START produces
    // a #timestamp for the ARTISAN line that disagrees with the #DUMP
    // `time_s` base. Reset on the actual START transition only.
    let roast_active_now = crate::logging::roast_logger::is_logging_active();
    if roast_active_now && !tick_state.was_roast_active {
        tick_state.formatter.reset();
    }
    tick_state.was_roast_active = roast_active_now;

    // Bug M1 (2026-07-25): respect DEFAULT_OUTPUT_INTERVAL_MS (1000 ms) for
    // telemetry by checking elapsed wall-clock from the *last emission*
    // instead of a tick counter. The control loop spends ≈ 190 ms of every
    // tick waiting for MAX31856 conversion, so a tick-count gate emitted
    // every 2.9 s of real time and the `#DUMP` drain ran 3× slower than the
    // previous comment claimed.
    let should_emit = match tick_state.last_telemetry_emit {
        None => true,
        Some(last) => {
            tick_start.saturating_duration_since(last).as_millis()
                >= crate::config::constants::DEFAULT_OUTPUT_INTERVAL_MS
        }
    };
    if should_emit {
        tick_state.last_telemetry_emit = Some(tick_start);
    }

    // Bug B17: feed the ring-buffer roast logger ONLY on the 1 Hz telemetry
    // tick. The previous code ran `log_sample` on every ~100 ms control tick
    // (regardless of `should_emit`), so 256 samples covered ~25.6 s instead
    // of the intended ~256 s — a roast that survived a Disconnect/#DUMP
    // recovery lost the most recent data because the ring had already cycled.
    // BUG-08: the feed uses `status_for_logger` (driven by the roast-logger
    // state), NOT `status_for_output` (driven by the opt-in stream flag).
    if should_emit {
        if let Some(status) = status_for_logger {
            // Bug V2-8: the `time_s` column is derived INSIDE the logger from
            // its own epoch (`start`, set by `start_roast`) and this `now`.
            // The caller no longer owns the time base. M8: feed the `#DUMP`
            // `ror` column in °C/min (the unit the column header declares),
            // not the internal °C/s — `LogSampleData.ror` is documented in
            // roast_logger.rs as "°C/min".
            //
            // Bug DRA-1 (2026-07-26): the buffer used to store raw INTERNAL
            // °C (and °C/min RoR) regardless of the active display scale,
            // while the live stream (`ArtisanFormatter`) converts to the
            // host's scale. After a Disconnect/#DUMP recovery in °F mode the
            // dump showed °C values — 1.8×+32 off from the live curve.
            // Apply the same conversion as the live formatter so dump and
            // stream agree.
            let ts = &status.temperature_settings;
            let bt = ts.convert_to_display(status.bean_temp);
            let et = ts.convert_to_display(status.env_temp);
            let target = ts.convert_to_display(status.target_temp);
            // M8 established °C/min for the ror column; in °F mode mirror the
            // live formatter (°C/s × 9/5 × 60 = °F/min).
            let ror = if ts.is_fahrenheit() {
                status.derivative_rate * (9.0 / 5.0) * 60.0
            } else {
                status.derivative_rate * 60.0
            };
            crate::logging::roast_logger::log_sample(
                crate::logging::roast_logger::LogSampleData {
                    bt,
                    et,
                    heater: status.ssr_output,
                    fan: status.fan_output,
                    target,
                    ror,
                },
                tick_start,
            );
        }

        if let Some(status) = status_for_output {
            let line = tick_state.formatter.format(&status);

            match line {
                Ok(formatted_line) => {
                    if let Ok(s) = String::<TRACE_EVENT_MAX_LEN>::try_from(formatted_line.as_str())
                    {
                        try_send_output(output_channel, s);
                    }
                }
                Err(e) => {
                    debug!("Formatter error: {:?}", e);
                }
            }
        }
    }

    // Bug V2-7: drain `#DUMP` rows OUTSIDE the 1 Hz `should_emit` gate so a
    // full roast (up to LOG_CAPACITY rows) drains in ~6 s, not ~256 s. The
    // previous design popped one row per 1 Hz tick and dropped the row
    // silently if `try_send` failed (channel full). Here we drain up to
    // `MAX_DUMP_ROWS_PER_TICK` rows per 100 ms tick and RE-PUSH a row to
    // the front of the deque if the output channel is full, so no row is
    // lost. `with_roaster_async` is `.await`-able but its closure is sync —
    // we take+send+repush via three short lock acquisitions.
    const MAX_DUMP_ROWS_PER_TICK: usize = 4;
    for _ in 0..MAX_DUMP_ROWS_PER_TICK {
        let row_opt = ServiceContainer::with_roaster_async(|roaster| roaster.take_dump_row())
            .await
            .ok()
            .flatten();
        let Some(row) = row_opt else { break };
        // Audit H-5 (2026-08-11): dump rows are `String<DUMP_ROW_CAPACITY=128>`
        // (roast_logger.rs) while the output channel messages are
        // `String<TRACE_EVENT_MAX_LEN=256>` — widen here. Infallible by
        // construction (128 < 256); the else arm is defensive only.
        let Ok(msg) = String::<TRACE_EVENT_MAX_LEN>::try_from(row.as_str()) else {
            let _ =
                ServiceContainer::with_roaster_async(|roaster| roaster.push_dump_row_front(row))
                    .await;
            break;
        };
        if try_send_output(output_channel, msg) {
            continue;
        }
        // Channel full — the counter was bumped by try_send_output; re-push
        // the row to the FRONT so the next tick emits it again (FIFO order
        // preserved; the row itself is not lost, only delayed).
        let _ =
            ServiceContainer::with_roaster_async(|roaster| roaster.push_dump_row_front(row)).await;
        break;
    }

    let telemetry_elapsed_ms = tick_state.stage_tracker.elapsed().as_millis();

    // Report TelemetryEmit stage instrumentation
    let telemetry_wd = if watchdog_snapshot.feed_ok {
        WatchdogState::Ok
    } else {
        WatchdogState::Fail
    };
    report_stage_instrumentation(
        &tick_state.stage_reporter,
        StageName::TelemetryEmit,
        telemetry_elapsed_ms,
        guard_timeout_happened,
        telemetry_wd,
        output_channel,
    );

    if let Some(status) = status_for_output {
        debug!(
            "stage=TelemetryEmit elapsed={}ms telemetry_sent={} guard_timeout={} watchdog_failure={:?} saturation_active={} integrator_clamped={} derivative_available={}",
            telemetry_elapsed_ms,
            true,
            guard_timeout_happened,
            watchdog_snapshot.last_failure,
            status.saturation_active as u8,
            status.integrator_clamped as u8,
            status.derivative_available as u8
        );
    } else {
        debug!(
            "stage=TelemetryEmit elapsed={}ms telemetry_sent={} guard_timeout={} watchdog_failure={:?}",
            telemetry_elapsed_ms,
            status_for_output.is_some(),
            guard_timeout_happened,
            watchdog_snapshot.last_failure
        );
    }

    if let Some(trace_id) = tick_state.tick_trace_id {
        trace_telemetry(
            trace_id,
            guard_timeout_happened,
            guard_total_timeouts,
            watchdog_snapshot.feed_ok,
            tick_state.tick_app_error.as_ref(),
        );
        trace_guard(
            trace_id,
            guard_timeout_happened,
            guard_total_timeouts,
            watchdog_snapshot.feed_ok,
            watchdog_snapshot.last_failure,
            tick_state.tick_app_error.as_ref(),
        );
        tick_state.tick_trace_id = None;
    }

    status_for_output
}

fn finalize_tick(
    tick_state: &mut TickState,
    control_snapshot: Option<ControlUpdateSnapshot>,
    guard_timeout_happened: bool,
    watchdog_snapshot: &WatchdogSnapshot,
) {
    tick_state.stage_tracker.clear();

    let desired_delta = if let Some(snapshot) = control_snapshot {
        snapshot.desired_output - snapshot.applied_output
    } else {
        0.0
    };
    let tick_elapsed_ms = tick_state.stage_tracker.elapsed().as_millis();
    debug!(
        "tick complete elapsed={}ms guard_timeout={} watchdog_failure={:?} desired-applied_delta={:.1}%",
        tick_elapsed_ms,
        guard_timeout_happened,
        watchdog_snapshot.last_failure,
        desired_delta
    );
}

/// BUG-06 (2026-08-21): drive the status LED once per tick (embedded-only).
///
/// The pattern is decided by the pure `status_led` module (host-tested);
/// the GPIO write goes through the single owner stored in the service
/// container. Blink phase is locked to the boot-epoch clock, so no per-task
/// toggle state is needed.
#[cfg(target_arch = "riscv32")]
async fn update_status_led_stage() {
    use crate::hardware::status_led::{led_on, pattern_for};
    use esp_hal::gpio::Level;

    let elapsed_ms = Instant::now().as_millis();
    let on = ServiceContainer::with_roaster_async(|roaster| {
        let status = roaster.get_status();
        led_on(
            pattern_for(status.state, status.fault_condition),
            elapsed_ms,
        )
    })
    .await
    .unwrap_or(false);

    ServiceContainer::with_status_led(|led| {
        led.set_level(if on { Level::High } else { Level::Low });
    });
}

/// Execute one iteration of the control loop tick.
///
/// Extracted from `control_loop_task` for testability. Contains the full tick
/// sequence: drain commands → read sensors → error checks → control update →
/// LEDC write → watchdog feed → telemetry emit → finalize.
///
/// Does NOT include the `Timer::after(CONTROL_LOOP_PERIOD_MS)` — that's the
/// caller's responsibility.
async fn control_loop_tick(tick_state: &mut TickState, output_channel: &OutputChannel) {
    let tick_start = Instant::now();
    tick_state.stage_tracker.start_tick(tick_start);
    tick_state.tick_app_error = None;

    let guard_total_timeouts = ledc_guard::total_timeouts();
    let guard_timeout_happened = guard_total_timeouts != tick_state.last_guard_total_timeouts;

    drain_commands(tick_state).await;

    read_sensors(tick_state, guard_timeout_happened, output_channel).await;

    // C3: Track consecutive sensor errors — trigger emergency shutdown at threshold.
    if tick_state.sensor_err.is_some() {
        tick_state.consecutive_sensor_errors =
            tick_state.consecutive_sensor_errors.saturating_add(1);
        if tick_state.consecutive_sensor_errors
            >= crate::config::constants::MAX_CONSECUTIVE_SENSOR_ERRORS
        {
            warn!(
                "SAFETY SENSOR-ERR: {} consecutive sensor read errors — emergency shutdown",
                crate::config::constants::MAX_CONSECUTIVE_SENSOR_ERRORS
            );
            let _ = ServiceContainer::with_roaster_async(|roaster| {
                let _ = roaster.emergency_shutdown("Consecutive sensor errors");
            })
            .await;
        }
    } else {
        tick_state.consecutive_sensor_errors = 0;
    }

    // C4: Check comms read error thresholds — trigger emergency shutdown if exceeded.
    if crate::hardware::error_counters::any_comms_error_threshold_exceeded() {
        warn!("SAFETY COMMS-ERR: comms read error threshold exceeded — emergency shutdown");
        let _ = ServiceContainer::with_roaster_async(|roaster| {
            let _ = roaster.emergency_shutdown("Comms read error threshold exceeded");
        })
        .await;
    }

    let (control_snapshot, control_status) =
        update_control_stage(tick_state, guard_timeout_happened, output_channel).await;

    log_ledc_stage(
        tick_state,
        guard_timeout_happened,
        guard_total_timeouts,
        control_status,
        output_channel,
    );

    let watchdog_snapshot = feed_watchdog_stage(
        tick_state,
        guard_timeout_happened,
        guard_total_timeouts,
        control_status,
        output_channel,
    )
    .await;

    if let Some(e) = tick_state.sensor_err.take() {
        // Bug #11: this error originates from `roaster_async_sensor_read`
        // (set into `tick_state.sensor_err` earlier in the tick), not from
        // a ServiceContainer access. The previous message pointed at the
        // wrong subsystem during debugging.
        info!("Sensor read error in control loop: {:?}", e);
    }

    let _status_for_output = emit_telemetry_stage(
        tick_state,
        guard_timeout_happened,
        guard_total_timeouts,
        &watchdog_snapshot,
        tick_start,
        output_channel,
    )
    .await;

    #[cfg(target_arch = "riscv32")]
    update_status_led_stage().await;

    finalize_tick(
        tick_state,
        control_snapshot,
        guard_timeout_happened,
        &watchdog_snapshot,
    );
}

/// Long-lived Embassy task running the control loop until the device resets.
///
/// Spawns `control_loop_tick` on `CONTROL_LOOP_PERIOD_MS` cadence; the safety
/// watchdogs and emergency-shutdown paths live inside the tick.
#[task]
pub async fn control_loop_task() {
    info!("Roaster control loop started - Artisan+ integration ACTIVE");

    let mut tick_state = TickState::new();
    let _start_time = Instant::now();
    let output_channel = ServiceContainer::get_output_channel();

    loop {
        control_loop_tick(&mut tick_state, output_channel).await;
        // Bug L10 (2026-08-10): reference the constant instead of the raw
        // literal — `CHARGE_SAMPLE_TICK_DIV` derives from
        // `CONTROL_LOOP_TICK_MS = CONTROL_LOOP_PERIOD_MS + …`, so a constant
        // change silently shifted the charge window while the loop kept the
        // stale `100` here.
        Timer::after(Duration::from_millis(
            crate::config::constants::CONTROL_LOOP_PERIOD_MS as u64,
        ))
        .await;
    }
}

fn send_handler_error(
    output_channel: &Channel<
        embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
        String<TRACE_EVENT_MAX_LEN>,
        { crate::application::service_container::ARTISAN_OUTPUT_CHANNEL_SIZE },
    >,
    error: &crate::control::RoasterError,
) {
    let mut message = String::<TRACE_EVENT_MAX_LEN>::new();
    let _ = message.push_str("ERR handler_failed ");
    let _ = message.push_str(error.message_token());
    // Append error source for diagnostics (e.g. ":fault_condition_active", ":ssr_cycle_busy")
    if let Some(source) = error.source() {
        let _ = core::write!(&mut message, ":{}", source);
    }
    try_send_output(output_channel, message);
}

/// Process one message from the output channel.
///
/// Extracted from `dual_output_task` for testability. Reads the output channel,
/// looks up the active transport via the multiplexer, appends CRLF, and writes
/// to the selected USB or UART driver.
///
/// Does NOT include the `Timer::after(5ms)` — that's the caller's responsibility.
///
/// Bug E2 (2026-08-03): drains up to `MAX_MESSAGES_PER_TICK` per invocation
/// instead of exactly one. A `#DUMP` backlog (up to 256 rows pushed at 4 rows
/// per control tick) used to monopolize the channel: with one `try_receive`
/// per 5 ms tick every SAFETY/ERR/STATUS message produced behind the dump was
/// dropped silently (`try_send` on a full channel). Draining up to 4 messages
/// per 5 ms tick still bounds the USB/UART write blocking per tick while
/// clearing the dump backlog ~4× faster, cutting the silent-drop window.
const MAX_MESSAGES_PER_TICK: usize = 4;

async fn dual_output_tick(output_channel: &OutputChannel) {
    for _ in 0..MAX_MESSAGES_PER_TICK {
        let Ok(data) = output_channel.try_receive() else {
            break;
        };
        let (channel, data_to_write) = critical_section::with(|cs| {
            let multiplexer = ServiceContainer::get_multiplexer();
            let mut guard = multiplexer.borrow(cs).borrow_mut();
            if let Some(mux) = guard.as_mut() {
                let active_channel = mux.get_active_channel();
                let bytes = append_crlf(data.as_str());
                (active_channel, Some(bytes))
            } else {
                (CommChannel::None, None)
            }
        });

        if let Some(bytes) = data_to_write {
            // Audit H-1 (2026-08-11): write failures used to vanish (`let _`).
            // The message is already dequeued — no retry is possible — but the
            // failure must now be counted (per transport) and logged (bounded:
            // one warn! per failed write, at most one per 5 ms tick, and only
            // while a transport is genuinely failing). The counters are
            // exposed via `hardware::error_counters`.
            match channel {
                CommChannel::Usb => {
                    if let Err(e) =
                        crate::hardware::usb_cdc::driver::usb_cdc_write_bytes(&bytes).await
                    {
                        crate::hardware::error_counters::increment_usb_write_failure();
                        warn!(
                            "USB output write failed: {:?} ({:?} total)",
                            e,
                            crate::hardware::error_counters::usb_write_failure_count()
                        );
                    }
                }
                CommChannel::Uart => {
                    if let Err(e) = crate::hardware::uart::driver::uart_write_bytes(&bytes).await {
                        crate::hardware::error_counters::increment_uart_write_failure();
                        warn!(
                            "UART output write failed: {:?} ({:?} total)",
                            e,
                            crate::hardware::error_counters::uart_write_failure_count()
                        );
                    }
                }
                CommChannel::None => {}
            }
        }
    }
}

/// Long-lived Embassy task forwarding the Artisan output channel to USB/UART.
///
/// Drains up to `MAX_MESSAGES_PER_TICK` lines per 5 ms invocation, appending
/// CRLF and writing to the active transport selected by the multiplexer.
#[task]
pub async fn dual_output_task() {
    info!("Dual output task started - USB CDC + UART");

    let output_channel = ServiceContainer::get_output_channel();

    loop {
        dual_output_tick(output_channel).await;
        Timer::after(Duration::from_millis(5)).await;
    }
}

fn append_crlf(payload: &str) -> heapless::Vec<u8, 300> {
    // Audit M-R4 (2026-08-11): the output channel carries at most
    // `String<TRACE_EVENT_MAX_LEN=256>` messages, so payload + CRLF is
    // ≤ 258 bytes. The previous `Vec<u8, 1024>` burned 4× the needed stack
    // per message (up to 4 KB churn per output tick on the task stack).
    let mut bytes = heapless::Vec::<u8, 300>::new();
    if bytes.extend_from_slice(payload.as_bytes()).is_ok() {
        let _ = bytes.extend_from_slice(b"\r\n");
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{StubFan, StubHeater};
    use crate::control::RoasterControl;
    use crate::hardware::sensors::SensorConversionHub;
    use crate::input::ArtisanInput;
    use futures::executor::block_on;

    // ── Helpers ─────────────────────────────────────────────────────────

    fn build_test_roaster() -> RoasterControl {
        RoasterControl::new(
            Box::new(StubHeater::new()),
            Box::new(StubFan::new()),
            SensorConversionHub::new(),
        )
        .expect("RoasterControl should build")
    }

    fn init_container_with_roaster(roaster: RoasterControl) {
        ServiceContainer::init_roaster(roaster);
        let _ = ArtisanInput::new().map(ServiceContainer::init_artisan_input);
    }

    fn drain_all_channels() {
        while ServiceContainer::get_artisan_channel()
            .try_receive()
            .is_ok()
        {}
        while ServiceContainer::get_output_channel().try_receive().is_ok() {}
    }

    // ── append_crlf ────────────────────────────────────────────────────

    #[test]
    fn test_append_crlf_appends_single_terminator() {
        let payload = "READ,120.3,150.5,75.0,25.0";
        let bytes = append_crlf(payload);

        let output = core::str::from_utf8(&bytes).expect("Output should be valid UTF-8");
        assert_eq!(output, "READ,120.3,150.5,75.0,25.0\r\n");
    }

    #[test]
    fn test_append_crlf_empty_payload() {
        let bytes = append_crlf("");
        let output = core::str::from_utf8(&bytes).expect("Output should be valid UTF-8");
        assert_eq!(output, "\r\n");
    }

    // ── TickState ───────────────────────────────────────────────────────

    #[test]
    fn test_tick_state_new_initializes_defaults() {
        let state = TickState::new();
        assert_eq!(state.consecutive_sensor_errors, 0);
        assert!(state.sensor_err.is_none());
        assert!(state.tick_app_error.is_none());
        assert!(state.tick_trace_id.is_none());
        assert!(!state.was_continuous);
    }

    #[test]
    fn test_tick_state_stage_tracker_starts_at_idle() {
        let tick_state = TickState::new();
        assert!(matches!(
            tick_state.stage_tracker.current_stage,
            ControlLoopStage::Idle
        ));
    }

    // ── StageTracker ───────────────────────────────────────────────────

    #[test]
    fn test_stage_tracker_elapsed_increases_over_time() {
        let mut tracker = StageTracker::new();
        let start = Instant::now();
        tracker.start_tick(start);
        let e1 = tracker.elapsed();
        let e2 = tracker.elapsed();
        assert!(e2.as_micros() >= e1.as_micros());
    }

    #[test]
    fn test_stage_tracker_clear_resets_stage() {
        let mut tracker = StageTracker::new();
        tracker.set_stage(ControlLoopStage::SensorRead);
        assert!(matches!(
            tracker.current_stage,
            ControlLoopStage::SensorRead
        ));
        tracker.clear();
        assert!(matches!(tracker.current_stage, ControlLoopStage::Idle));
    }

    #[test]
    fn test_stage_tracker_stage_transitions() {
        let mut tracker = StageTracker::new();
        tracker.set_stage(ControlLoopStage::SensorRead);
        tracker.set_stage(ControlLoopStage::ControlUpdate);
        tracker.set_stage(ControlLoopStage::WatchdogFeed);
        assert!(matches!(
            tracker.current_stage,
            ControlLoopStage::WatchdogFeed
        ));
    }

    // ── send_handler_error ─────────────────────────────────────────────

    #[test]
    fn test_send_handler_error_produces_err_message() {
        use crate::control::RoasterError;
        let _guard = crate::application::tasks::tests::acquire_test_lock();
        drain_all_channels();

        let roaster = build_test_roaster();
        init_container_with_roaster(roaster);

        let error = RoasterError::InvalidState {
            source: Some("test_error_source"),
        };
        let output_channel = ServiceContainer::get_output_channel();

        send_handler_error(output_channel, &error);

        let messages: Vec<_> = (0..10)
            .filter_map(|_| output_channel.try_receive().ok())
            .collect();
        let err_msg = messages
            .iter()
            .find(|m| m.contains("ERR"))
            .map(|s| s.as_str().to_string());

        assert!(
            err_msg.is_some(),
            "Expected ERR message in output channel, got: {:?}",
            messages
        );
        let msg = err_msg.unwrap();
        assert!(
            msg.contains("handler_failed"),
            "ERR message should contain handler_failed: {}",
            msg
        );
    }

    // ── drain_commands / command processing ────────────────────────────

    #[test]
    fn drain_commands_processes_status_command() {
        let _guard = crate::application::tasks::tests::acquire_test_lock();
        let mut roaster = build_test_roaster();
        roaster
            .update_temperatures(100.0, 120.0, Instant::now())
            .expect("temps");
        init_container_with_roaster(roaster);
        drain_all_channels();

        let traced = crate::logging::traceability::TracedCommand {
            command: crate::config::ArtisanCommand::StatusReport,
            trace_id: crate::logging::traceability::TraceId::next(),
            channel: crate::input::multiplexer::CommChannel::None,
        };
        let _ = ServiceContainer::get_artisan_channel().try_send(traced);

        let mut tick_state = TickState::new();
        block_on(async {
            drain_commands(&mut tick_state).await;
        });

        let messages: Vec<_> = (0..10)
            .filter_map(|_| ServiceContainer::get_output_channel().try_receive().ok())
            .collect();
        assert!(
            !messages.is_empty(),
            "STATUS command should produce output messages"
        );
    }

    #[test]
    fn drain_commands_processes_read_command() {
        let _guard = crate::application::tasks::tests::acquire_test_lock();
        let mut roaster = build_test_roaster();
        roaster
            .update_temperatures(120.3, 150.5, Instant::now())
            .expect("temps");
        init_container_with_roaster(roaster);
        drain_all_channels();

        let traced = crate::logging::traceability::TracedCommand {
            command: crate::config::ArtisanCommand::ReadStatus,
            trace_id: crate::logging::traceability::TraceId::next(),
            channel: crate::input::multiplexer::CommChannel::None,
        };
        let _ = ServiceContainer::get_artisan_channel().try_send(traced);

        let mut tick_state = TickState::new();
        block_on(async {
            drain_commands(&mut tick_state).await;
        });

        let messages: Vec<_> = (0..10)
            .filter_map(|_| ServiceContainer::get_output_channel().try_receive().ok())
            .collect();
        assert!(
            !messages.is_empty(),
            "READ command should produce output messages"
        );
    }

    // ── handle_watchdog_failure ────────────────────────────────────────

    #[test]
    fn watchdog_failure_increments_counter_and_flags_status() {
        let mut roaster = build_test_roaster();
        let output_channel = ServiceContainer::get_output_channel();
        roaster.status_mut().watchdog_consecutive_failures = 0;

        handle_watchdog_failure(&mut roaster, "test_failure", None, output_channel);

        let status = roaster.get_status();
        assert!(!status.watchdog_feed_ok);
        assert_eq!(status.watchdog_last_failure, Some("test_failure"));
        assert_eq!(status.watchdog_consecutive_failures, 1);
    }

    #[test]
    fn watchdog_consecutive_failures_trigger_emergency() {
        let mut roaster = build_test_roaster();
        let output_channel = ServiceContainer::get_output_channel();
        roaster.status_mut().watchdog_consecutive_failures = 1;

        handle_watchdog_failure(
            &mut roaster,
            "second_failure",
            Some("first_failure"),
            output_channel,
        );

        assert!(roaster.safety().is_emergency_active());
    }

    // ── finalize_tick ──────────────────────────────────────────────────

    #[test]
    fn test_finalize_tick_clears_stage_tracker() {
        let _guard = crate::application::tasks::tests::acquire_test_lock();
        let roaster = build_test_roaster();
        init_container_with_roaster(roaster);
        drain_all_channels();

        let mut tick_state = TickState::new();
        tick_state.last_guard_total_timeouts = 5;
        tick_state.prev_watchdog_state = WatchdogState::Fail;

        let control_snapshot = ControlUpdateSnapshot {
            desired_output: 50.0,
            applied_output: 45.0,
            fan_output: 30.0,
        };
        let watchdog_snapshot = WatchdogSnapshot {
            feed_ok: true,
            last_failure: None,
            guard_timeouts: 3,
        };

        tick_state
            .stage_tracker
            .set_stage(ControlLoopStage::TelemetryEmit);
        finalize_tick(
            &mut tick_state,
            Some(control_snapshot),
            false,
            &watchdog_snapshot,
        );

        // finalize_tick clears the stage tracker back to Idle
        assert!(matches!(
            tick_state.stage_tracker.current_stage,
            ControlLoopStage::Idle
        ));
        // finalize_tick does NOT modify last_guard_total_timeouts
        // (that is updated by feed_watchdog_stage, not finalize_tick)
        assert_eq!(tick_state.last_guard_total_timeouts, 5);
        // finalize_tick does NOT modify prev_watchdog_state
        assert_eq!(tick_state.prev_watchdog_state, WatchdogState::Fail);
    }

    // ── Full control_loop_tick integration tests ───────────────────────

    #[test]
    fn control_loop_tick_empty_tick_completes() {
        let _guard = acquire_test_lock();
        let roaster = build_test_roaster();
        init_container_with_roaster(roaster);
        drain_all_channels();

        let mut tick_state = TickState::new();
        let output_channel = ServiceContainer::get_output_channel();

        block_on(async {
            control_loop_tick(&mut tick_state, output_channel).await;
        });

        // Tick completes without panic. Under `test` feature, stage instrumentation
        // emits 5 STAGE reports (SensorRead, ControlUpdate, LedcWrite, WatchdogFeed,
        // TelemetryEmit). No command/message output expected.
        let stage_count = drain_stage_reports(output_channel);
        assert_eq!(
            stage_count, 5,
            "Empty tick should emit exactly 5 stage reports, got {}",
            stage_count
        );
    }

    // ── L3: full pipeline with simulated sensors (wall clock) ──────────
    // Audit L3 (2026-08-11): exercises the REAL control-loop pipeline on
    // host — `control_loop_tick` with a `SensorConversionHub` backed by
    // `SimulatedSensorSource` (advances by wall clock), READ commands
    // flowing in through the artisan channel and TC4 responses out through
    // the output channel. Gated behind `simulated-sensors`; the embedded
    // 210 ms MAX31856 wait does not exist on this path, so ticks are fast
    // and real time drives the curve. This is a smoke run (short window of
    // the default medium roast curve), not a full roast — the full roast is
    // covered deterministically at L1 in tests/full_roast_verification.rs.
    #[cfg(all(test, feature = "simulated-sensors", not(target_arch = "riscv32")))]
    #[test]
    fn control_loop_tick_simulated_sensors_full_pipeline() {
        let _guard = acquire_test_lock();
        let roaster = build_test_roaster();
        init_container_with_roaster(roaster);
        // The real boot wires the watchdog feeder (app_builder.rs); the
        // container helper does not — without it the second tick would trip
        // the 2-consecutive-failures emergency on `WatchdogUninitialized`.
        ServiceContainer::get_instance()
            .init_watchdog(crate::safety::watchdog::WatchdogFeeder::initialize().expect("wd"));
        drain_all_channels();

        let mut tick_state = TickState::new();
        let output_channel = ServiceContainer::get_output_channel();

        let mut read_responses: usize = 0;
        for tick in 0..40u32 {
            if tick % 5 == 0 {
                // Artisan polls READ every ~250 ms simulated.
                let traced = crate::logging::traceability::TracedCommand {
                    command: crate::config::ArtisanCommand::ReadStatus,
                    trace_id: crate::logging::traceability::TraceId::next(),
                    channel: crate::input::multiplexer::CommChannel::None,
                };
                let _ = ServiceContainer::get_artisan_channel().try_send(traced);
            }

            block_on(async {
                control_loop_tick(&mut tick_state, output_channel).await;
            });

            // Drain everything (STAGE reports + protocol responses).
            while let Ok(msg) = output_channel.try_receive() {
                if msg.contains(',') && msg.split(',').count() >= 5 {
                    read_responses += 1;
                }
            }

            // Real sleep so the simulated source's wall clock advances.
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        // ~2 s of wall clock → the default curve ramps 25 → ~29 °C.
        let status =
            block_on(async { ServiceContainer::with_roaster_async(|r| r.get_status()).await })
                .expect("roaster initialized");
        assert!(
            status.bean_temp > 26.0 && status.bean_temp < 80.0,
            "simulated BT must advance along the ramp after ~2 s, got {:.1}",
            status.bean_temp
        );
        assert!(
            !status.fault_condition
                && status.ssr_hardware_status != crate::config::constants::SsrHardwareStatus::Error,
            "no fault may develop in the pipeline smoke run (fault={}, state={:?})",
            status.fault_condition,
            status.state
        );
        assert!(
            read_responses >= 4,
            "READ commands must produce TC4 responses through the pipeline, got {}",
            read_responses
        );
    }

    #[test]
    fn control_loop_tick_processes_read_command() {
        let _guard = acquire_test_lock();
        let mut roaster = build_test_roaster();
        roaster
            .update_temperatures(120.3, 150.5, Instant::now())
            .expect("temps");
        init_container_with_roaster(roaster);
        drain_all_channels();

        let traced = crate::logging::traceability::TracedCommand {
            command: crate::config::ArtisanCommand::ReadStatus,
            trace_id: crate::logging::traceability::TraceId::next(),
            channel: crate::input::multiplexer::CommChannel::None,
        };
        let _ = ServiceContainer::get_artisan_channel().try_send(traced);

        let mut tick_state = TickState::new();
        let output_channel = ServiceContainer::get_output_channel();

        block_on(async {
            control_loop_tick(&mut tick_state, output_channel).await;
        });

        let messages = drain_non_stage_output(output_channel);
        assert!(
            !messages.is_empty(),
            "READ command should produce TC4 output after full tick, got 0 messages"
        );
        let has_read_response = messages
            .iter()
            .any(|msg| msg.contains(',') && msg.split(',').count() >= 5);
        assert!(
            has_read_response,
            "Expected READ response in output channel, got: {:?}",
            messages
        );
    }

    #[test]
    fn control_loop_tick_processes_status_command() {
        let _guard = acquire_test_lock();
        let mut roaster = build_test_roaster();
        roaster
            .update_temperatures(100.0, 120.0, Instant::now())
            .expect("temps");
        init_container_with_roaster(roaster);
        drain_all_channels();

        // Send START command to transition out of Idle state.
        let traced = crate::logging::traceability::TracedCommand {
            command: crate::config::ArtisanCommand::StartRoast,
            trace_id: crate::logging::traceability::TraceId::next(),
            channel: crate::input::multiplexer::CommChannel::None,
        };
        let _ = ServiceContainer::get_artisan_channel().try_send(traced);

        let mut tick_state = TickState::new();
        let output_channel = ServiceContainer::get_output_channel();

        block_on(async {
            control_loop_tick(&mut tick_state, output_channel).await;
        });
        // Drain stage reports
        let msgs = drain_non_stage_output(output_channel);

        // Verify roaster state advanced
        let status =
            block_on(async { ServiceContainer::with_roaster_async(|r| r.get_status()).await })
                .expect("status read");
        // The START command was processed — PID was enabled before the tick ran it.
        assert!(
            !msgs.is_empty(),
            "START command should produce trace output after full tick, got 0 non-stage messages"
        );
        // Default state is Idle — after START command dispatch it should no longer be Idle
        // (even if sensor/watchdog issues later push it to Error).
        assert_ne!(
            status.state,
            crate::config::constants::RoasterState::Idle,
            "Roaster should have left Idle state after START + full tick, got {:?}",
            status.state
        );
        // STATUS command emits a 20-field CSV line plus TRACE lines.
        let has_status_line = msgs.iter().any(|msg| {
            msg.split(',').count() >= 15
                && msg.chars().all(|c| {
                    c == '.' || c == ',' || c == '-' || c.is_ascii_digit() || c.is_alphabetic()
                })
        });
        let has_trace_line = msgs.iter().any(|msg| msg.contains("actuation"));
        assert!(
            has_status_line || has_trace_line,
            "Expected STATUS-related output, got: {:?}",
            msgs
        );
    }

    #[test]
    fn control_loop_tick_sensor_and_control_advance() {
        let _guard = acquire_test_lock();
        let mut roaster = build_test_roaster();
        roaster
            .update_temperatures(25.0, 30.0, Instant::now())
            .expect("temps");
        init_container_with_roaster(roaster);
        drain_all_channels();

        // Start a roast to enable control
        let traced = crate::logging::traceability::TracedCommand {
            command: crate::config::ArtisanCommand::StartRoast,
            trace_id: crate::logging::traceability::TraceId::next(),
            channel: crate::input::multiplexer::CommChannel::None,
        };
        let _ = ServiceContainer::get_artisan_channel().try_send(traced);

        let mut tick_state = TickState::new();
        let output_channel = ServiceContainer::get_output_channel();

        block_on(async {
            control_loop_tick(&mut tick_state, output_channel).await;
        });
        let msgs = drain_non_stage_output(output_channel);

        let _status =
            block_on(async { ServiceContainer::with_roaster_async(|r| r.get_status()).await })
                .expect("status read");
        assert!(
            !msgs.is_empty(),
            "START command should produce trace output after full tick, got {} non-stage messages",
            msgs.len()
        );
    }

    #[test]
    fn control_loop_three_consecutive_ticks_no_panic() {
        let _guard = acquire_test_lock();
        let roaster = build_test_roaster();
        init_container_with_roaster(roaster);
        drain_all_channels();

        let mut tick_state = TickState::new();
        let output_channel = ServiceContainer::get_output_channel();

        block_on(async {
            for _ in 0..3 {
                control_loop_tick(&mut tick_state, output_channel).await;
            }
        });

        // All three ticks completed without panic.
        // Tick state should have valid accumulated guard timeouts.
        assert!(
            tick_state.last_guard_total_timeouts < u16::MAX, // not saturated
            "Guard timeouts should be within normal range after 3 ticks"
        );
    }

    #[test]
    fn control_loop_tick_comms_error_threshold_triggers_emergency() {
        let _guard = acquire_test_lock();
        let roaster = build_test_roaster();
        init_container_with_roaster(roaster);
        drain_all_channels();

        // Push UART error count past threshold
        for _ in 0..crate::hardware::error_counters::MAX_COMMS_READ_ERRORS {
            crate::hardware::error_counters::increment_uart_error_count();
        }
        assert!(
            crate::hardware::error_counters::any_comms_error_threshold_exceeded(),
            "Comms error threshold should be exceeded after max increments"
        );

        // Reset UART counter so test leaves clean state
        let _cleanup = ResetCommsOnDrop;

        let mut tick_state = TickState::new();
        let output_channel = ServiceContainer::get_output_channel();

        block_on(async {
            control_loop_tick(&mut tick_state, output_channel).await;
        });

        // Verify emergency shutdown was triggered
        let status =
            block_on(async { ServiceContainer::with_roaster_async(|r| r.get_status()).await })
                .expect("status read");
        assert!(
            status.fault_condition,
            "Emergency shutdown should activate after comms errors exceed threshold"
        );
    }

    #[test]
    fn control_loop_multi_tick_commands_drained_across_ticks() {
        let _guard = acquire_test_lock();
        let roaster = build_test_roaster();
        init_container_with_roaster(roaster);
        drain_all_channels();

        // Fill the command channel with READ and STATUS commands
        for cmd in &[
            crate::config::ArtisanCommand::ReadStatus,
            crate::config::ArtisanCommand::StatusReport,
            crate::config::ArtisanCommand::ReadStatus,
        ] {
            let traced = crate::logging::traceability::TracedCommand {
                command: *cmd,
                trace_id: crate::logging::traceability::TraceId::next(),
                channel: crate::input::multiplexer::CommChannel::None,
            };
            let _ = ServiceContainer::get_artisan_channel().try_send(traced);
        }

        let mut tick_state = TickState::new();
        let output_channel = ServiceContainer::get_output_channel();

        // Run 3 ticks — commands should be drained across ticks
        let mut total_outputs = 0usize;
        block_on(async {
            for _ in 0..3 {
                control_loop_tick(&mut tick_state, output_channel).await;
                total_outputs += drain_non_stage_output(output_channel).len();
            }
        });

        assert!(
            total_outputs > 0,
            "Commands queued before ticks should produce output across 3 ticks, got {}",
            total_outputs
        );
    }

    // ── Dual output tick tests ─────────────────────────────────────────

    #[test]
    fn dual_output_tick_empty_channel_noop() {
        let _guard = acquire_test_lock();
        let output_channel = ServiceContainer::get_output_channel();
        drain_all_channels();

        block_on(async {
            dual_output_tick(output_channel).await;
        });

        // No message was in the channel, so nothing should have been consumed.
        assert!(output_channel.try_receive().is_err());
    }

    #[test]
    fn dual_output_tick_consumes_output_message() {
        let _guard = acquire_test_lock();

        // Init multiplexer so channel routing works
        ServiceContainer::init_multiplexer();
        let output_channel = ServiceContainer::get_output_channel();
        drain_all_channels();

        // Send a test message
        let _ = output_channel
            .try_send(heapless::String::try_from("READ,120.3,150.5,75.0,25.0").unwrap());

        // Message should be in the channel before tick
        assert!(
            output_channel.try_receive().is_ok(),
            "Message should be available before dual_output_tick"
        );

        // But wait — try_receive removes it! Need to put it back.
        let _ = output_channel
            .try_send(heapless::String::try_from("READ,120.3,150.5,75.0,25.0").unwrap());

        block_on(async {
            dual_output_tick(output_channel).await;
        });

        // After tick, the message should have been consumed
        assert!(
            output_channel.try_receive().is_err(),
            "Message should be consumed from output channel after dual_output_tick"
        );
    }

    #[test]
    fn dual_output_tick_multiple_messages_processed() {
        let _guard = acquire_test_lock();
        ServiceContainer::init_multiplexer();
        let output_channel = ServiceContainer::get_output_channel();
        drain_all_channels();

        // Send 3 messages
        for i in 0..3u8 {
            let msg = heapless::String::try_from(alloc::format!("MSG{}", i).as_str()).unwrap();
            try_send_output(output_channel, msg);
        }

        // Process all 3 with separate ticks
        block_on(async {
            for _ in 0..3 {
                dual_output_tick(output_channel).await;
            }
        });

        // All should be consumed
        assert!(
            output_channel.try_receive().is_err(),
            "All 3 messages should be consumed after 3 dual_output_tick calls"
        );
    }

    // ── RAII helper for comms error cleanup ────────────────────────────

    struct ResetCommsOnDrop;

    impl Drop for ResetCommsOnDrop {
        fn drop(&mut self) {
            crate::hardware::error_counters::reset_uart_error_count();
            crate::hardware::error_counters::reset_usb_error_count();
        }
    }

    // ── Test lock for ServiceContainer tests ───────────────────────────

    static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn acquire_test_lock() -> std::sync::MutexGuard<'static, ()> {
        let guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        TEST_LOCK.clear_poison();
        guard
    }

    /// Helper: drain output channel and return only non-stage-report messages.
    /// Stage reports start with "STAGE," — the full tick emits 5 of them per tick
    /// under the `test` feature (SensorRead, ControlUpdate, LedcWrite, WatchdogFeed, TelemetryEmit).
    fn drain_non_stage_output(
        channel: &OutputChannel,
    ) -> Vec<heapless::String<TRACE_EVENT_MAX_LEN>> {
        let mut messages = Vec::new();
        while let Ok(msg) = channel.try_receive() {
            if !msg.starts_with("STAGE,") {
                messages.push(msg);
            }
        }
        messages
    }

    /// Helper: count stage report messages in the output channel.
    fn drain_stage_reports(channel: &OutputChannel) -> usize {
        let mut count = 0;
        while let Ok(msg) = channel.try_receive() {
            if msg.starts_with("STAGE,") {
                count += 1;
            }
        }
        count
    }

    // ── Bug P7 (2026-08-03): comms read errors are channel-aware ──────────

    #[test]
    fn should_count_read_error_only_active_channel() {
        use crate::hardware::transport_tasks::should_count_read_error;

        // Only the multiplexer's ACTIVE channel counts toward the
        // MAX_COMMS_READ_ERRORS emergency threshold.
        assert!(should_count_read_error(CommChannel::Usb, CommChannel::Usb));
        assert!(should_count_read_error(
            CommChannel::Uart,
            CommChannel::Uart
        ));
        assert!(
            !should_count_read_error(CommChannel::Usb, CommChannel::Uart),
            "P7: a dead UART line must not count during a USB session"
        );
        assert!(
            !should_count_read_error(CommChannel::Uart, CommChannel::Usb),
            "P7: a dead USB line must not count during a UART session"
        );
        assert!(
            !should_count_read_error(CommChannel::None, CommChannel::Uart),
            "P7: no active session → nothing counts"
        );
        assert!(!should_count_read_error(
            CommChannel::None,
            CommChannel::Usb
        ));
    }

    // ── Bug P8 (2026-08-03): garbage must not hijack the multiplexer ───────

    #[test]
    fn garbage_line_does_not_hijack_channel() {
        use crate::hardware::transport_tasks::{send_parse_error, TransportConfig};
        use crate::input::parser::ParseError;

        let _guard = acquire_test_lock();
        // Fresh multiplexer — active channel starts as `None`.
        ServiceContainer::init_multiplexer();
        drain_all_channels();

        let config = TransportConfig {
            name: "UART",
            channel: CommChannel::Uart,
            ..TransportConfig::default()
        };

        // Boot-window garbage that fails to parse must NOT activate UART.
        block_on(async {
            send_parse_error(ParseError::UnknownCommand, CommChannel::Uart, &config).await;
        });

        let active = critical_section::with(|cs| {
            ServiceContainer::get_multiplexer()
                .borrow(cs)
                .borrow()
                .as_ref()
                .map(|mux| mux.get_active_channel())
        });
        assert_eq!(
            active,
            Some(CommChannel::None),
            "P8: a parse error in the None window must not hijack the channel"
        );
        // With no active channel there is nowhere to reply — nothing emitted.
        assert!(
            ServiceContainer::get_output_channel()
                .try_receive()
                .is_err(),
            "P8: no ERR may be emitted while no channel is active"
        );
    }
}
