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
            let status = roaster.status_mut();
            let previous_watchdog_failure = status.watchdog_last_failure;
            let mut report_watchdog_failure = |reason: &'static str| {
                warn!("SAFETY WATCHDOGFEED fail: {}", reason);
                status.watchdog_feed_ok = false;
                status.watchdog_last_failure = Some(reason);
                status.watchdog_consecutive_failures =
                    status.watchdog_consecutive_failures.saturating_add(1);
                if status.watchdog_consecutive_failures >= 2 {
                    status.fault_condition = true;
                }
                if previous_watchdog_failure != Some(reason) {
                    let mut safety = String::<TRACE_EVENT_MAX_LEN>::new();
                    let _ = write!(safety, "SAFETY WATCHDOG {}", reason);
                    let _ = output_channel.try_send(safety);
                }
            };

            match ServiceContainer::get_instance()
                .with_watchdog(|watchdog| watchdog.feed_async(status.bean_temp))
            {
                Ok(_) => {
                    status.watchdog_feed_ok = true;
                    status.watchdog_last_failure = None;
                    status.watchdog_consecutive_failures = 0;
                }
                Err(ContainerError::Watchdog(err)) => {
                    report_watchdog_failure(err.reason());
                }
                Err(ContainerError::WatchdogUninitialized) => {
                    report_watchdog_failure(WatchdogError::NotInitialized.reason());
                }
                Err(err) => {
                    warn!("Watchdog container error: {:?}", err);
                }
            }

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

    if let Some(status) = status_for_output {
        let line = tick_state.formatter.format(&status);

        match line {
            Ok(formatted_line) => {
                if let Ok(s) = String::<TRACE_EVENT_MAX_LEN>::try_from(formatted_line.as_str()) {
                    let _ = output_channel.try_send(s);
                }
            }
            Err(e) => {
                debug!("Formatter error: {:?}", e);
            }
        }

        // Feed ring-buffer roast logger
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

#[task]
pub async fn control_loop_task() {
    info!("Roaster control loop started - Artisan+ integration ACTIVE");

    if let Err(err) = ServiceContainer::ensure_async_roaster_initialized_from_sync().await {
        warn!("Failed to initialize async roaster storage: {:?}", err);
    }

    let mut tick_state = TickState::new();
    let _start_time = Instant::now();
    let output_channel = ServiceContainer::get_output_channel();

    loop {
        let tick_start = Instant::now();
        tick_state.stage_tracker.start_tick(tick_start);
        tick_state.tick_app_error = None;

        let guard_total_timeouts = ledc_guard::total_timeouts();
        let guard_timeout_happened = guard_total_timeouts != tick_state.last_guard_total_timeouts;

        drain_commands(&mut tick_state).await;

        read_sensors(&mut tick_state, guard_timeout_happened, &output_channel).await;

        let (control_snapshot, control_status) =
            update_control_stage(&mut tick_state, guard_timeout_happened, &output_channel).await;

        log_ledc_stage(
            &mut tick_state,
            guard_timeout_happened,
            guard_total_timeouts,
            control_status,
            &output_channel,
        );

        let watchdog_snapshot = feed_watchdog_stage(
            &mut tick_state,
            guard_timeout_happened,
            guard_total_timeouts,
            control_status,
            &output_channel,
        )
        .await;

        if let Some(e) = tick_state.sensor_err.take() {
            info!("Service container error in control loop: {:?}", e);
        }

        let _status_for_output = emit_telemetry_stage(
            &mut tick_state,
            guard_timeout_happened,
            guard_total_timeouts,
            &watchdog_snapshot,
            tick_start,
            &output_channel,
        )
        .await;

        finalize_tick(
            &mut tick_state,
            control_snapshot,
            guard_timeout_happened,
            &watchdog_snapshot,
        );

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

#[task]
pub async fn dual_output_task() {
    info!("Dual output task started - USB CDC + UART");

    let output_channel = ServiceContainer::get_output_channel();

    loop {
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
                        if let Some(usb) = crate::hardware::usb_cdc::driver::get_usb_cdc_driver() {
                            let _ = usb.write_bytes(&bytes).await;
                        }
                    }
                    CommChannel::Uart => {
                        if let Some(uart) = crate::hardware::uart::driver::get_uart_driver() {
                            let _ = uart.write_bytes(&bytes).await;
                        }
                    }
                    CommChannel::None => {}
                }
            }
        }

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
    use super::append_crlf;

    #[test]
    fn test_append_crlf_appends_single_terminator() {
        let payload = "READ,120.3,150.5,75.0,25.0";
        let bytes = append_crlf(payload);

        let output = core::str::from_utf8(&bytes).expect("Output should be valid UTF-8");
        assert_eq!(output, "READ,120.3,150.5,75.0,25.0\r\n");
    }
}
