extern crate alloc;

use crate::application::service_container::{ContainerError, ServiceContainer};
use crate::application::stage_instrumentation::{
    GuardState, StageName, StageReporter, WatchdogState,
};
use crate::config::SystemStatus;
use crate::hardware::ledc_guard;
use crate::input::multiplexer::CommChannel;
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

    fn current_stage(&self) -> ControlLoopStage {
        self.current_stage
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

#[task]
pub async fn control_loop_task() {
    info!("Roaster control loop started - Artisan+ integration ACTIVE");

    if let Err(err) = ServiceContainer::ensure_async_roaster_initialized_from_sync().await {
        warn!("Failed to initialize async roaster storage: {:?}", err);
    }

    let mut formatter = MutableArtisanFormatter::new();
    let _start_time = Instant::now();
    let cmd_channel = ServiceContainer::get_artisan_channel();
    let output_channel = ServiceContainer::get_output_channel();
    let mut was_continuous = false;
    let mut last_guard_total_timeouts = ledc_guard::total_timeouts();
    let mut stage_tracker = StageTracker::new();
    let stage_reporter = StageReporter::new();
    // Track previous tick's watchdog state for stage instrumentation (not yet known in first tick)
    let mut prev_watchdog_state = WatchdogState::None;

    loop {
        let tick_start = Instant::now();
        stage_tracker.start_tick(tick_start);
        let current_time = tick_start;

        let guard_total_timeouts = ledc_guard::total_timeouts();
        let guard_timeout_happened = guard_total_timeouts != last_guard_total_timeouts;

        while let Ok(command) = cmd_channel.try_receive() {
            if let crate::config::ArtisanCommand::RunRegression = command {
                regression::request_regression();
                continue;
            }
            let output_channel = ServiceContainer::get_output_channel();

            let _ = ServiceContainer::with_roaster_async(
                |roaster: &mut crate::control::roaster_refactored::RoasterControl| {
                    let start_time = Instant::now();
                    let result = roaster.process_artisan_command(command);
                    let latency = start_time.elapsed().as_micros() as u32;

                    roaster.status_mut().command_latency_us = latency;
                    if latency > roaster.status_mut().max_command_latency_us {
                        roaster.status_mut().max_command_latency_us = latency;
                    }

                    match result {
                        Ok(()) => {
                            debug!("Processed Artisan command successfully");

                            if let crate::config::ArtisanCommand::StatusReport = command {
                                let status = roaster.get_status();
                                let response = ArtisanFormatter::format_status_response(&status);

                                if let Ok(line) = String::<128>::try_from(response.as_str()) {
                                    let _ = output_channel.try_send(line);
                                }
                            } else if let crate::config::ArtisanCommand::ReadStatus = command {
                                let status = roaster.get_status();
                                // Use full READ response with 4 values per current protocol
                                let response = ArtisanFormatter::format_read_response_full(&status);

                                if let Ok(line) = String::<128>::try_from(response.as_str()) {
                                    let _ = output_channel.try_send(line);
                                }
                            }
                        }
                        Err(err) => {
                            warn!("Failed to process Artisan command: {:?}", err);
                            send_handler_error(output_channel, &err);
                        }
                    }
                },
            );
        }

        // Control loop now uses async read_sensors() - no longer blocks executor
        // Using the roaster_async_sensor_read method that takes ownership, calls async, returns it
        stage_tracker.set_stage(ControlLoopStage::SensorRead);
        let sensor_err = ServiceContainer::roaster_async_sensor_read().await.err();
        let sensor_elapsed_ms = stage_tracker.elapsed().as_millis();

        // Report stage instrumentation (watchdog state not yet known, use previous tick's state)
        let sensor_guard = if guard_timeout_happened {
            GuardState::Timeout
        } else {
            GuardState::Ok
        };
        if let Some(report) = stage_reporter.report_simple(
            StageName::SensorRead,
            sensor_elapsed_ms,
            sensor_guard,
            prev_watchdog_state,
        ) {
            let _ = output_channel.try_send(report);
        }

        if sensor_err.is_none() {
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
                sensor_elapsed_ms, sensor_err
            );
        }

        // Do sync control update separately
        stage_tracker.set_stage(ControlLoopStage::ControlUpdate);
        let control_snapshot = match ServiceContainer::with_roaster_async(
            |roaster: &mut crate::control::roaster_refactored::RoasterControl| match roaster
                .update_control(current_time)
            {
                Ok(output) => Some(ControlUpdateSnapshot {
                    desired_output: roaster.last_desired_heater_output(),
                    applied_output: output,
                    fan_output: roaster.get_fan_speed(),
                }),
                Err(e) => {
                    warn!("Control update error: {:?}", e);
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

        let control_status: Option<SystemStatus> =
            match ServiceContainer::with_roaster_async(|roaster| roaster.get_status()).await {
                Ok(status) => Some(status),
                Err(err) => {
                    warn!("Failed to capture status after control update: {:?}", err);
                    None
                }
            };

        let control_elapsed_ms = stage_tracker.elapsed().as_millis();

        // Report ControlUpdate stage instrumentation
        let control_guard = if guard_timeout_happened {
            GuardState::Timeout
        } else {
            GuardState::Ok
        };
        if let Some(report) = stage_reporter.report_simple(
            StageName::ControlUpdate,
            control_elapsed_ms,
            control_guard,
            prev_watchdog_state,
        ) {
            let _ = output_channel.try_send(report);
        }

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

        stage_tracker.set_stage(ControlLoopStage::LedcWrite);
        let ledc_elapsed_ms = stage_tracker.elapsed().as_millis();
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
        let ledc_guard = if guard_timeout_happened {
            GuardState::Timeout
        } else {
            GuardState::Ok
        };
        if let Some(report) = stage_reporter.report_simple(
            StageName::LedcWrite,
            ledc_elapsed_ms,
            ledc_guard,
            prev_watchdog_state,
        ) {
            let _ = output_channel.try_send(report);
        }

        stage_tracker.set_stage(ControlLoopStage::WatchdogFeed);
        let watchdog_snapshot = match ServiceContainer::with_roaster_async(
            |roaster: &mut crate::control::roaster_refactored::RoasterControl| {
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
                        let mut safety = String::<128>::new();
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
                    let mut guard_msg = String::<128>::new();
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

        let watchdog_elapsed_ms = stage_tracker.elapsed().as_millis();

        // Report WatchdogFeed stage instrumentation (now we know the watchdog state)
        let wd_guard = if guard_timeout_happened {
            GuardState::Timeout
        } else {
            GuardState::Ok
        };
        let wd_state = if watchdog_snapshot.feed_ok {
            WatchdogState::Ok
        } else {
            WatchdogState::Fail
        };
        let failure_marker = watchdog_snapshot.last_failure;
        if let Some(report) = stage_reporter.report(
            StageName::WatchdogFeed,
            watchdog_elapsed_ms,
            wd_guard,
            wd_state,
            failure_marker,
        ) {
            let _ = output_channel.try_send(report);
        }

        // Update previous watchdog state for next tick
        prev_watchdog_state = wd_state;

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

        if let Some(e) = sensor_err {
            info!("Service container error in control loop: {:?}", e);
        }

        last_guard_total_timeouts = guard_total_timeouts;

        let mut is_continuous_now = false;
        let mut status_for_output = None;

        stage_tracker.set_stage(ControlLoopStage::TelemetryEmit);
        if let Err(err) = ServiceContainer::with_roaster_async(
            |roaster: &mut crate::control::roaster_refactored::RoasterControl| {
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

        if is_continuous_now != was_continuous {
            formatter.reset();
            was_continuous = is_continuous_now;
        }

        if let Some(status) = status_for_output {
            let line = formatter.format(&status);

            match line {
                Ok(formatted_line) => {
                    if let Ok(s) = heapless::String::try_from(formatted_line.as_str()) {
                        let _ = output_channel.try_send(s);
                    }
                }
                Err(e) => {
                    debug!("Formatter error: {:?}", e);
                }
            }
        }

        let telemetry_elapsed_ms = stage_tracker.elapsed().as_millis();

        // Report TelemetryEmit stage instrumentation
        let telemetry_guard = if guard_timeout_happened {
            GuardState::Timeout
        } else {
            GuardState::Ok
        };
        let telemetry_wd = if watchdog_snapshot.feed_ok {
            WatchdogState::Ok
        } else {
            WatchdogState::Fail
        };
        if let Some(report) = stage_reporter.report_simple(
            StageName::TelemetryEmit,
            telemetry_elapsed_ms,
            telemetry_guard,
            telemetry_wd,
        ) {
            let _ = output_channel.try_send(report);
        }

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

        stage_tracker.clear();

        let desired_delta = if let Some(snapshot) = control_snapshot {
            snapshot.desired_output - snapshot.applied_output
        } else {
            0.0
        };
        let tick_elapsed_ms = stage_tracker.elapsed().as_millis();
        debug!(
            "tick complete elapsed={}ms guard_timeout={} watchdog_failure={:?} desired-applied_delta={:.1}%",
            tick_elapsed_ms,
            guard_timeout_happened,
            watchdog_snapshot.last_failure,
            desired_delta
        );

        Timer::after(Duration::from_millis(100)).await;
    }
}

fn send_handler_error(
    output_channel: &Channel<
        embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
        String<128>,
        { crate::application::service_container::ARTISAN_OUTPUT_CHANNEL_SIZE },
    >,
    error: &crate::control::RoasterError,
) {
    let mut message = String::<128>::new();
    let _ = message.push_str("ERR handler_failed ");
    let _ = message.push_str(error.message_token());

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

fn append_crlf(payload: &str) -> alloc::vec::Vec<u8> {
    let mut bytes = payload.as_bytes().to_vec();
    bytes.extend_from_slice(b"\r\n");
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
