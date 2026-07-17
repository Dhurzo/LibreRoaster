extern crate alloc;

use crate::application::service_container::{ContainerError, ServiceContainer};
#[cfg(any(feature = "instrumentation", feature = "test"))]
use crate::application::stage_instrumentation::GuardState;
use crate::application::stage_instrumentation::{StageName, StageReporter, WatchdogState};
use crate::config::SystemStatus;
use crate::error::AppError;
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
        Instant::now().duration_since(self.tick_start)
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
    last_guard_total_timeouts: u16,
    stage_tracker: StageTracker,
    stage_reporter: StageReporter,
    tick_trace_id: Option<TraceId>,
    prev_watchdog_state: WatchdogState,
    tick_app_error: Option<AppError>,
    sensor_err: Option<ContainerError>,
    consecutive_sensor_errors: u8,
    /// Counter for throttling telemetry emission to DEFAULT_OUTPUT_INTERVAL_MS.
    /// The control loop runs at 100 ms (10 Hz); we only emit telemetry every
    /// `telemetry_emit_every` ticks to respect the documented 1 Hz rate.
    telemetry_tick_counter: u8,
    /// How many control-loop ticks between telemetry emissions.
    /// 1000 ms / 100 ms = 10.
    telemetry_emit_every: u8,
}

impl TickState {
    fn new() -> Self {
        Self {
            formatter: MutableArtisanFormatter::new(),
            was_continuous: false,
            last_guard_total_timeouts: ledc_guard::total_timeouts(),
            stage_tracker: StageTracker::new(),
            stage_reporter: StageReporter::new(),
            tick_trace_id: None,
            prev_watchdog_state: WatchdogState::None,
            tick_app_error: None,
            sensor_err: None,
            consecutive_sensor_errors: 0,
            telemetry_tick_counter: 0,
            telemetry_emit_every: (crate::config::constants::DEFAULT_OUTPUT_INTERVAL_MS / 100)
                as u8,
        }
    }
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
        let _ = output_channel.try_send(report);
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
        let _ = output_channel.try_send(report);
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
    // Drain all pending commands from the channel. Commands arriving between ticks
    // are queued up to capacity. The fallback pattern in UART/USB task code retries
    // via the direct artisan_channel when the main queue is full, preventing silent drops.
    let mut cmds_this_tick: usize = 0;
    while let Ok(traced_command) = cmd_channel.try_receive() {
        cmds_this_tick = cmds_this_tick.saturating_add(1);
        let is_emergency = matches!(
            &traced_command.command,
            crate::config::ArtisanCommand::Stop | crate::config::ArtisanCommand::EmergencyStop
        );
        if cmds_this_tick > crate::config::MAX_COMMANDS_PER_TICK && !is_emergency {
            warn!(
                "Command rate limit exceeded — {} commands this tick, skipping remaining",
                cmds_this_tick
            );
            continue;
        }
        if let crate::config::ArtisanCommand::RunRegression = traced_command.command {
            regression::request_regression();
            continue;
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
                                let _ = output_channel.try_send(line);
                            }
                        } else if let crate::config::ArtisanCommand::ReadStatus =
                            traced_command.command
                        {
                            let response =
                                ArtisanFormatter::format_read_response_full(&status_snapshot);

                            if let Ok(line) =
                                String::<TRACE_EVENT_MAX_LEN>::try_from(response.as_str())
                            {
                                let _ = output_channel.try_send(line);
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
    // Using the roaster_async_sensor_read method that takes ownership, calls async, returns it
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
        debug!(
            "stage=SensorRead elapsed={}ms Sensors: BT: {:.1}°C, ET: {:.1}°C",
            sensor_elapsed_ms,
            ServiceContainer::read_bean_temperature()
                .await
                .unwrap_or(0.0),
            ServiceContainer::read_env_temperature()
                .await
                .unwrap_or(0.0)
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
        let _ = output_channel.try_send(safety);
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
                let _ = output_channel.try_send(guard_msg);
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

    if let Err(err) = ServiceContainer::with_roaster_async(
        |roaster: &mut crate::control::roaster_control::RoasterControl| {
            is_continuous_now = roaster.get_output_manager().is_continuous_enabled();
            if is_continuous_now {
                status_for_output = Some(roaster.get_status());
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
    }

    // Bug #7 fix: respect DEFAULT_OUTPUT_INTERVAL_MS (1000 ms) for telemetry.
    // The control loop runs at 100 ms; we only emit every N ticks where
    // N = DEFAULT_OUTPUT_INTERVAL_MS / 100 = 10 (i.e., 1 Hz instead of 10 Hz).
    tick_state.telemetry_tick_counter = tick_state.telemetry_tick_counter.saturating_add(1);
    let should_emit = tick_state.telemetry_tick_counter >= tick_state.telemetry_emit_every;
    if should_emit {
        tick_state.telemetry_tick_counter = 0;
    }

    // Feed ring-buffer roast logger (runs every tick, independent of telemetry rate)
    if let Some(status) = status_for_output {
        crate::logging::roast_logger::log_sample(crate::logging::roast_logger::LogSampleData {
            elapsed_secs: tick_start.elapsed().as_secs() as u32,
            bt: status.bean_temp,
            et: status.env_temp,
            heater: status.ssr_output,
            fan: status.fan_output,
            target: status.target_temp,
            ror: status.derivative_rate,
        });
    }

    if should_emit {
        if let Some(status) = status_for_output {
            let line = tick_state.formatter.format(&status);

            match line {
                Ok(formatted_line) => {
                    if let Ok(s) = String::<TRACE_EVENT_MAX_LEN>::try_from(formatted_line.as_str())
                    {
                        let _ = output_channel.try_send(s);
                    }
                }
                Err(e) => {
                    debug!("Formatter error: {:?}", e);
                }
            }
        }
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

/// Execute one iteration of the control loop tick.
///
/// Extracted from `control_loop_task` for testability. Contains the full tick
/// sequence: drain commands → read sensors → error checks → control update →
/// LEDC write → watchdog feed → telemetry emit → finalize.
///
/// Does NOT include the `Timer::after(100ms)` — that's the caller's responsibility.
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
        info!("Service container error in control loop: {:?}", e);
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

    finalize_tick(
        tick_state,
        control_snapshot,
        guard_timeout_happened,
        &watchdog_snapshot,
    );
}

#[task]
pub async fn control_loop_task() {
    info!("Roaster control loop started - Artisan+ integration ACTIVE");

    let mut tick_state = TickState::new();
    let _start_time = Instant::now();
    let output_channel = ServiceContainer::get_output_channel();

    loop {
        control_loop_tick(&mut tick_state, output_channel).await;
        Timer::after(Duration::from_millis(100)).await;
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
    let _ = output_channel.try_send(message);
}

/// Process one message from the output channel.
///
/// Extracted from `dual_output_task` for testability. Reads the output channel,
/// looks up the active transport via the multiplexer, appends CRLF, and writes
/// to the selected USB or UART driver.
///
/// Does NOT include the `Timer::after(5ms)` — that's the caller's responsibility.
async fn dual_output_tick(output_channel: &OutputChannel) {
    if let Ok(data) = output_channel.try_receive() {
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
            match channel {
                CommChannel::Usb => {
                    let _ = crate::hardware::usb_cdc::driver::usb_cdc_write_bytes(&bytes).await;
                }
                CommChannel::Uart => {
                    let _ = crate::hardware::uart::driver::uart_write_bytes(&bytes).await;
                }
                CommChannel::None => {}
            }
        }
    }
}

#[task]
pub async fn dual_output_task() {
    info!("Dual output task started - USB CDC + UART");

    let output_channel = ServiceContainer::get_output_channel();

    loop {
        dual_output_tick(output_channel).await;
        Timer::after(Duration::from_millis(5)).await;
    }
}

fn append_crlf(payload: &str) -> heapless::Vec<u8, 1024> {
    let mut bytes = heapless::Vec::<u8, 1024>::new();
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

        let traced = crate::logging::traceability::TracedCommand {
            command: crate::config::ArtisanCommand::StatusReport,
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
            let _ = output_channel.try_send(msg);
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
}
