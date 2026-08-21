#![cfg(all(test, feature = "test", not(target_arch = "riscv32")))]
#![allow(deprecated)]

extern crate std;

#[path = "common/mod.rs"]
mod tests_common;

use libreroaster::config::{ArtisanCommand, RoasterState, SsrHardwareStatus, SystemStatus};
use libreroaster::control::{RoasterControl, RoasterError};
use libreroaster::output::artisan::ArtisanFormatter;
use std::boxed::Box;
use tests_common::{build_test_control, StubFan, StubHeater};

fn build_control() -> RoasterControl {
    let heater = StubHeater::new();
    heater.set_status(SsrHardwareStatus::Available);

    build_test_control(Box::new(heater), Box::new(StubFan::new()))
}

#[test]
fn start_stop_idempotent() {
    let mut control = build_control();

    assert!(!control.get_output_manager().is_continuous_enabled());

    control
        .process_artisan_command(ArtisanCommand::StartRoast)
        .expect("first START succeeds");

    let started = control.get_status();
    // BUG-08 (2026-08-21): telemetry is opt-in — START must NOT enable the
    // spontaneous stream (it used to, and its `#` lines could occupy the
    // READ response slot).
    assert!(!control.get_output_manager().is_continuous_enabled());
    assert!(started.pid_enabled);
    assert_eq!(started.state, RoasterState::Heating);

    control
        .process_artisan_command(ArtisanCommand::StartRoast)
        .expect("duplicate START leaves session intact");

    let restarted = control.get_status();
    assert!(!control.get_output_manager().is_continuous_enabled());
    assert_eq!(restarted.target_temp, started.target_temp);
    assert!(restarted.pid_enabled);

    // Opt-in stream: STREAM;ON enables it explicitly…
    control
        .process_artisan_command(ArtisanCommand::SetStreaming(true))
        .expect("STREAM;ON succeeds");
    assert!(control.get_output_manager().is_continuous_enabled());

    control
        .process_artisan_command(ArtisanCommand::EmergencyStop)
        .expect("STOP zeros outputs and disables streaming");

    let stopped = control.get_status();
    // …and STOP clears it (session-scoped).
    assert!(!control.get_output_manager().is_continuous_enabled());
    assert_eq!(stopped.ssr_output, 0.0);
    assert_eq!(stopped.fan_output, 100.0);
    assert!(!stopped.pid_enabled);
    assert!(!stopped.artisan_control);

    control
        .process_artisan_command(ArtisanCommand::EmergencyStop)
        .expect("STOP remains idempotent");

    let stopped_again = control.get_status();
    assert_eq!(stopped_again.ssr_output, 0.0);
    assert_eq!(stopped_again.fan_output, 100.0);
    assert!(!stopped_again.pid_enabled);
    assert!(!stopped_again.artisan_control);
}

#[test]
fn manual_bounds_and_reset() {
    let mut control = build_control();

    let heater_err = control.process_artisan_command(ArtisanCommand::SetHeater(150));
    assert!(matches!(
        heater_err,
        Err(RoasterError::InvalidState { source: _ })
    ));
    let status_after_invalid = control.get_status();
    assert_eq!(status_after_invalid.ssr_output, 0.0);

    let fan_err = control.process_artisan_command(ArtisanCommand::SetFan(150));
    assert!(matches!(
        fan_err,
        Err(RoasterError::InvalidState { source: _ })
    ));
    let status_after_fan_invalid = control.get_status();
    assert_eq!(status_after_fan_invalid.fan_output, 0.0);

    control
        .process_artisan_command(ArtisanCommand::SetHeater(80))
        .expect("manual heater within bounds should apply");
    let status_manual = control.get_status();
    assert!(status_manual.artisan_control);
    assert!(!status_manual.pid_enabled);
    assert_eq!(status_manual.ssr_output, 80.0);

    control
        .process_artisan_command(ArtisanCommand::SetFan(60))
        .expect("manual fan within bounds should apply");
    let status_manual_fan = control.get_status();
    assert_eq!(status_manual_fan.fan_output, 60.0);
    // BUG-08 (2026-08-21): slider commands must NOT auto-enable the
    // spontaneous telemetry stream (opt-in via STREAM;ON only).
    assert!(!control.get_output_manager().is_continuous_enabled());

    control
        .process_artisan_command(ArtisanCommand::EmergencyStop)
        .expect("STOP should reset manual values");
    let stopped = control.get_status();
    assert_eq!(stopped.ssr_output, 0.0);
    assert_eq!(stopped.fan_output, 100.0);
    assert!(!stopped.artisan_control);
    assert!(!stopped.pid_enabled);
    assert!(!control.get_output_manager().is_continuous_enabled());
}

#[test]
fn read_response_deterministic() {
    let mut status = SystemStatus {
        state: RoasterState::Stable,
        bean_temp: 150.56,
        env_temp: 120.34,
        target_temp: 200.0,
        ssr_output: 75.67,
        fan_output: 25.43,
        pid_enabled: true,
        artisan_control: false,
        fault_condition: false,
        ssr_hardware_status: SsrHardwareStatus::Available,
        ssr_last_duty_delta_ticks: 0,
        ssr_retry_count: 0,
        ssr_cycle_guard_busy_until_ms: 0,
        ..SystemStatus::default()
    };

    let first = ArtisanFormatter::format_read_response(&status, status.fan_output);
    let second = ArtisanFormatter::format_read_response(&status, status.fan_output);

    assert_eq!(first, second);
    assert_eq!(first, "120.3,150.6,75.7,25.4");
    assert_eq!(first.split(',').count(), 4);

    status.ssr_output = 0.0;
    status.fan_output = 0.0;
    status.env_temp = 0.0;
    status.bean_temp = 0.0;

    let zeroed = ArtisanFormatter::format_read_response(&status, 0.0);
    assert_eq!(zeroed, "0.0,0.0,0.0,0.0");
    assert_eq!(zeroed.split(',').count(), 4);
}

#[test]
fn read_response_boundaries() {
    let status = SystemStatus {
        state: RoasterState::Stable,
        bean_temp: 100.0,
        env_temp: 0.0,
        target_temp: 200.0,
        ssr_output: 100.0,
        fan_output: 0.0,
        pid_enabled: true,
        artisan_control: false,
        fault_condition: false,
        ssr_hardware_status: SsrHardwareStatus::Available,
        ssr_last_duty_delta_ticks: 0,
        ssr_retry_count: 0,
        ssr_cycle_guard_busy_until_ms: 0,
        ..SystemStatus::default()
    };

    let response = ArtisanFormatter::format_read_response(&status, status.fan_output);
    assert_eq!(response, "0.0,100.0,100.0,0.0");
    assert_eq!(response.split(',').count(), 4);
}
