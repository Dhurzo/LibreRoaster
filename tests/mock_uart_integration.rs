#![cfg(all(test, not(target_arch = "riscv32")))]
#![allow(deprecated)]

extern crate std;

use std::boxed::Box;
use std::string::String as StdString;
use std::vec::Vec;

use critical_section;
use critical_section::RawRestoreState;
use embassy_time::Instant;
use heapless::String;

use libreroaster::application::service_container::ServiceContainer;
use libreroaster::config::{ArtisanCommand, RoasterState, SystemStatus};
use libreroaster::control::RoasterControl;
use libreroaster::hardware::uart::tasks::process_command_data;
use libreroaster::logging::traceability::TRACE_EVENT_MAX_LEN;
use libreroaster::output::artisan::ArtisanFormatter;

#[path = "common/mod.rs"]
mod tests_common;
use tests_common::{build_test_control, init_test_service_container, StubFan, StubHeater};

struct TestCriticalSection;

critical_section::set_impl!(TestCriticalSection);

unsafe impl critical_section::Impl for TestCriticalSection {
    unsafe fn acquire() -> RawRestoreState {
        false
    }

    unsafe fn release(_restore_state: RawRestoreState) {}
}

#[allow(dead_code)]
fn build_control() -> RoasterControl {
    build_test_control(Box::new(StubHeater::new()), Box::new(StubFan::new()))
}

fn reset_channels() {
    let cmd_channel = ServiceContainer::get_artisan_channel();
    while cmd_channel.try_receive().is_ok() {}

    let output_channel = ServiceContainer::get_output_channel();
    while output_channel.try_receive().is_ok() {}
}

fn collect_output() -> Vec<StdString> {
    let output_channel = ServiceContainer::get_output_channel();
    let mut messages = Vec::new();

    while let Ok(msg) = output_channel.try_receive() {
        if msg.as_str().starts_with("TRACE,") {
            continue;
        }
        messages.push(StdString::from(msg.as_str()));
    }

    messages
}

fn collect_commands() -> Vec<ArtisanCommand> {
    let channel = ServiceContainer::get_artisan_channel();
    let mut commands = Vec::new();

    while let Ok(cmd) = channel.try_receive() {
        commands.push(cmd.command);
    }

    commands
}

fn drain_and_process_commands() {
    loop {
        let command = {
            let channel = ServiceContainer::get_artisan_channel();
            match channel.try_receive() {
                Ok(cmd) => cmd.command,
                Err(_) => break,
            }
        };

        let output_channel = ServiceContainer::get_output_channel();
        let _ = ServiceContainer::with_roaster(|roaster| {
            match roaster.process_artisan_command(command) {
                Ok(()) => {
                    if let ArtisanCommand::ReadStatus = command {
                        let status = roaster.get_status();
                        let response = ArtisanFormatter::format_read_response(
                            &status,
                            roaster.get_fan_speed(),
                        );
                        if let Ok(line) = String::<TRACE_EVENT_MAX_LEN>::try_from(response.as_str())
                        {
                            let _ = output_channel.try_send(line);
                        }
                    }
                }
                Err(err) => {
                    let mut message = String::<TRACE_EVENT_MAX_LEN>::new();
                    let _ = message.push_str("ERR handler_failed ");
                    let _ = message.push_str(err.message_token());
                    let _ = output_channel.try_send(message);
                }
            }
        });
    }
}

fn current_status() -> SystemStatus {
    ServiceContainer::with_roaster(|roaster| roaster.get_status())
        .expect("Roaster should be initialized")
}

fn continuous_output_enabled() -> bool {
    ServiceContainer::with_roaster(|roaster| roaster.get_output_manager().is_continuous_enabled())
        .expect("Roaster should be initialized")
}

fn assert_status_unchanged(before: SystemStatus, after: SystemStatus) {
    assert_eq!(before.state, after.state);
    assert_eq!(before.bean_temp, after.bean_temp);
    assert_eq!(before.env_temp, after.env_temp);
    assert_eq!(before.target_temp, after.target_temp);
    assert_eq!(before.ssr_output, after.ssr_output);
    assert_eq!(before.fan_output, after.fan_output);
    assert_eq!(before.pid_enabled, after.pid_enabled);
    assert_eq!(before.artisan_control, after.artisan_control);
    assert_eq!(before.fault_condition, after.fault_condition);
    assert_eq!(before.ssr_hardware_status, after.ssr_hardware_status);
}

#[test]
fn read_command_emits_expected_response() {
    init_test_service_container();
    reset_channels();

    ServiceContainer::with_roaster(|roaster| {
        roaster
            .update_temperatures(101.2, 98.7, Instant::now())
            .expect("Temperature update should succeed");
    })
    .expect("Roaster should be initialized");

    process_command_data(b"READ\r");
    drain_and_process_commands();

    let outputs = collect_output();
    assert_eq!(outputs.len(), 1, "Expected a single READ response");

    let expected = ServiceContainer::with_roaster(|roaster| {
        let status = roaster.get_status();
        ArtisanFormatter::format_read_response(&status, roaster.get_fan_speed())
    })
    .expect("Roaster should be initialized");

    assert_eq!(outputs[0], expected.as_str());
    assert!(
        collect_commands().is_empty(),
        "No commands should remain queued"
    );
}

#[test]
fn start_ot1_io3_stop_sequence_updates_state() {
    init_test_service_container();
    reset_channels();

    process_command_data(b"START\r");
    drain_and_process_commands();

    let started = current_status();
    assert!(continuous_output_enabled());
    assert!(started.pid_enabled);
    assert_eq!(started.state, RoasterState::Heating);
    assert!(
        collect_output().is_empty(),
        "START should not emit ERR output"
    );

    process_command_data(b"OT1 60\r");
    drain_and_process_commands();

    let heater = current_status();
    assert_eq!(heater.ssr_output, 60.0);
    assert!(heater.artisan_control);
    assert!(!heater.pid_enabled);
    assert!(
        collect_output().is_empty(),
        "OT1 should not emit ERR output"
    );

    process_command_data(b"IO3 40\r");
    drain_and_process_commands();

    let fan = current_status();
    assert_eq!(fan.fan_output, 40.0);
    assert!(fan.artisan_control);
    assert!(
        collect_output().is_empty(),
        "IO3 should not emit ERR output"
    );

    process_command_data(b"STOP\r");
    drain_and_process_commands();

    let stopped = current_status();
    assert!(!continuous_output_enabled());
    assert_eq!(stopped.ssr_output, 0.0);
    assert_eq!(stopped.fan_output, 100.0);
    assert!(!stopped.pid_enabled);
    assert!(!stopped.artisan_control);
    assert_eq!(stopped.state, RoasterState::Idle);
    assert!(
        collect_output().is_empty(),
        "STOP should not emit ERR output"
    );
}

#[test]
fn error_paths_emit_err_without_side_effects() {
    init_test_service_container();
    reset_channels();

    let baseline_status = current_status();
    let baseline_streaming = continuous_output_enabled();

    process_command_data(b"BOGUS\r");
    process_command_data(b"OT1 150\r");
    process_command_data(b"IO3 abc\r");
    drain_and_process_commands();

    let outputs = collect_output();
    assert_eq!(outputs.len(), 3, "Expected three ERR outputs");
    assert_eq!(outputs[0], "ERR unknown_command unknown_command");
    assert_eq!(outputs[1], "ERR out_of_range out_of_range");
    assert_eq!(outputs[2], "ERR invalid_value invalid_value");

    assert!(
        collect_commands().is_empty(),
        "No commands should be enqueued"
    );

    let after_status = current_status();
    let after_streaming = continuous_output_enabled();
    assert_status_unchanged(baseline_status, after_status);
    assert_eq!(baseline_streaming, after_streaming);
}
