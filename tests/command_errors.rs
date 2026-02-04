#![cfg(all(test, not(target_arch = "riscv32")))]

extern crate std;

use std::string::String as StdString;
use std::vec::Vec;

use libreroaster::application::service_container::ServiceContainer;
use libreroaster::config::ArtisanCommand;
use libreroaster::hardware::uart::tasks::process_command_data;

fn reset_channels() {
    let cmd_channel = ServiceContainer::get_artisan_channel();
    while cmd_channel.try_receive().is_ok() {}

    let output_channel = ServiceContainer::get_output_channel();
    while output_channel.try_receive().is_ok() {}
}

fn collect_commands() -> Vec<ArtisanCommand> {
    let channel = ServiceContainer::get_artisan_channel();
    let mut commands = Vec::new();

    while let Ok(cmd) = channel.try_receive() {
        commands.push(cmd);
    }

    commands
}

fn collect_output() -> Vec<StdString> {
    let output_channel = ServiceContainer::get_output_channel();
    let mut messages = Vec::new();

    while let Ok(msg) = output_channel.try_receive() {
        messages.push(StdString::from(msg.as_str()));
    }

    messages
}

fn assert_err_tokens(output: &str, expected_code: &str, expected_message: &str) {
    let parts: Vec<&str> = output.split_whitespace().collect();
    assert_eq!(parts.len(), 3, "ERR output should have 3 tokens");
    assert_eq!(parts[0], "ERR", "ERR prefix missing");
    assert_eq!(parts[1], expected_code, "Unexpected error code");
    assert_eq!(parts[2], expected_message, "Unexpected error message");
}

#[test]
fn empty_command_emits_err() {
    reset_channels();

    process_command_data(b"\r");

    let outputs = collect_output();
    assert_eq!(outputs.len(), 1, "Expected one ERR for empty command");
    assert_err_tokens(&outputs[0], "invalid_value", "empty_command");
    assert!(
        collect_commands().is_empty(),
        "No commands should be enqueued"
    );

    // Final UART output will append CRLF
    assert!(format!("{}\r\n", outputs[0]).ends_with("\r\n"));
}

#[test]
fn unknown_command_is_rejected() {
    reset_channels();

    process_command_data(b"BOGUS\r");

    let outputs = collect_output();
    assert_eq!(outputs.len(), 1, "Unknown command should emit ERR");
    assert_err_tokens(&outputs[0], "unknown_command", "unknown_command");
    assert!(
        collect_commands().is_empty(),
        "Unknown commands must not reach channel"
    );
}

#[test]
fn out_of_range_setpoints_error_without_side_effects() {
    reset_channels();

    process_command_data(b"OT1 150\r");
    process_command_data(b"IO3 200\r");

    let outputs = collect_output();
    assert_eq!(
        outputs.len(),
        2,
        "Each out-of-range command should emit ERR"
    );
    for output in &outputs {
        assert_err_tokens(output, "out_of_range", "out_of_range");
    }
    assert!(
        collect_commands().is_empty(),
        "Out-of-range inputs must not enqueue commands"
    );
}

#[test]
fn malformed_values_emit_invalid_value_err() {
    reset_channels();

    process_command_data(b"OT1 abc\r");

    let outputs = collect_output();
    assert_eq!(outputs.len(), 1, "Malformed value should emit ERR");
    assert_err_tokens(&outputs[0], "invalid_value", "invalid_value");
    assert!(
        collect_commands().is_empty(),
        "Malformed commands should be dropped"
    );
}

#[test]
fn valid_commands_pass_through_without_err() {
    reset_channels();

    process_command_data(b"READ\r");
    process_command_data(b"OT1 50\r");
    process_command_data(b"IO3 75\r");

    let outputs = collect_output();
    assert!(
        outputs.is_empty(),
        "Valid commands should not emit ERR lines"
    );

    let commands = collect_commands();
    assert_eq!(commands.len(), 3, "All valid commands should reach channel");
    assert!(matches!(commands[0], ArtisanCommand::ReadStatus));
    assert!(matches!(commands[1], ArtisanCommand::SetHeater(50)));
    assert!(matches!(commands[2], ArtisanCommand::SetFan(75)));
}
