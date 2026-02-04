//! USB CDC driver tests
//!
//! Tests for USB CDC command processing, error handling, and channel routing
//! using the mock USB driver.
//!
//! # Test Coverage (10 tests)
//!
//! - Complete command processing (READ, OT1, IO3)
//! - Empty/invalid command handling
//! - Buffer overflow handling
//! - Command routing by channel
//! - Error routing by channel

#![cfg(all(test, not(target_arch = "riscv32")))]
#![allow(non_snake_case)]

extern crate std;

use std::println;
use std::string::String as StdString;
use std::vec::Vec;

use libreroaster::application::service_container::ServiceContainer;
use libreroaster::config::ArtisanCommand;
use libreroaster::input::multiplexer::CommChannel;

// Use the mock USB driver from sibling file
#[path = "mock_usb_driver.rs"]
mod mock_usb_driver;

/// Reset all channels before each test
fn reset_channels() {
    let cmd_channel = ServiceContainer::get_artisan_channel();
    while cmd_channel.try_receive().is_ok() {}

    let output_channel = ServiceContainer::get_output_channel();
    while output_channel.try_receive().is_ok() {}
}

/// Collect commands from the artisan channel
fn collect_commands() -> Vec<ArtisanCommand> {
    let channel = ServiceContainer::get_artisan_channel();
    let mut commands = Vec::new();

    while let Ok(cmd) = channel.try_receive() {
        commands.push(cmd);
    }

    commands
}

/// Collect output messages from the output channel
fn collect_output() -> Vec<StdString> {
    let output_channel = ServiceContainer::get_output_channel();
    let mut messages = Vec::new();

    while let Ok(msg) = output_channel.try_receive() {
        messages.push(StdString::from(msg.as_str()));
    }

    messages
}

/// Assert that error output has expected format
fn assert_err_tokens(output: &str, expected_code: &str, expected_message: &str) {
    let parts: Vec<&str> = output.split_whitespace().collect();
    assert_eq!(parts.len(), 3, "ERR output should have 3 tokens");
    assert_eq!(parts[0], "ERR", "ERR prefix missing");
    assert_eq!(parts[1], expected_code, "Unexpected error code");
    assert_eq!(parts[2], expected_message, "Unexpected error message");
}

/// ========================================================================
// ============================================================================
// COMMAND PROCESSING TESTS
// ============================================================================
/// ========================================================================

/// TEST-USB-01: Complete READ command processing via USB
#[test]
fn test_usb_read_command_processing() {
    println!("TEST-USB-01: Complete READ command processing via USB");

    reset_channels();

    // Simulate USB reader task receiving READ command
    // The actual processing is done by process_usb_command_data
    // which is conditionally compiled with "test" feature
    libreroaster::hardware::usb_cdc::tasks::process_usb_command_data(b"READ\r");

    // Verify command was received
    let commands = collect_commands();
    assert_eq!(commands.len(), 1, "Should have 1 command");
    assert!(
        matches!(commands[0], ArtisanCommand::ReadStatus),
        "Should be ReadStatus command"
    );

    // Verify no errors
    let outputs = collect_output();
    assert!(
        outputs.is_empty(),
        "No errors should be emitted for valid command"
    );

    println!("   ✅ READ command correctly processed via USB CDC");
}

/// TEST-USB-02: Complete OT1 command processing via USB
#[test]
fn test_usb_ot1_command_processing() {
    println!("TEST-USB-02: Complete OT1 command processing via USB");

    reset_channels();

    // Simulate USB reader task receiving OT1 command
    libreroaster::hardware::usb_cdc::tasks::process_usb_command_data(b"OT1 75\r");

    // Verify command was received
    let commands = collect_commands();
    assert_eq!(commands.len(), 1, "Should have 1 command");
    assert!(
        matches!(commands[0], ArtisanCommand::SetHeater(75)),
        "Should be SetHeater(75)"
    );

    // Verify no errors
    let outputs = collect_output();
    assert!(
        outputs.is_empty(),
        "No errors should be emitted for valid command"
    );

    println!("   ✅ OT1 command correctly processed via USB CDC");
}

/// TEST-USB-03: Complete IO3 command processing via USB
#[test]
fn test_usb_io3_command_processing() {
    println!("TEST-USB-03: Complete IO3 command processing via USB");

    reset_channels();

    // Simulate USB reader task receiving IO3 command
    libreroaster::hardware::usb_cdc::tasks::process_usb_command_data(b"IO3 50\r");

    // Verify command was received
    let commands = collect_commands();
    assert_eq!(commands.len(), 1, "Should have 1 command");
    assert!(
        matches!(commands[0], ArtisanCommand::SetFan(50)),
        "Should be SetFan(50)"
    );

    // Verify no errors
    let outputs = collect_output();
    assert!(
        outputs.is_empty(),
        "No errors should be emitted for valid command"
    );

    println!("   ✅ IO3 command correctly processed via USB CDC");
}

/// ========================================================================
// ============================================================================
// EMPTY/INVALID COMMAND TESTS
// ============================================================================
/// ========================================================================

/// TEST-USB-04: Empty command via USB emits ERR
#[test]
fn test_usb_empty_command_emits_err() {
    println!("TEST-USB-04: Empty command via USB emits ERR");

    reset_channels();

    // Process empty command (just CR)
    libreroaster::hardware::usb_cdc::tasks::process_usb_command_data(b"\r");

    let outputs = collect_output();
    assert_eq!(outputs.len(), 1, "Expected one ERR for empty command");
    assert_err_tokens(&outputs[0], "invalid_value", "empty_command");
    assert!(
        collect_commands().is_empty(),
        "No commands should be enqueued"
    );

    println!("   ✅ Empty USB command correctly emits ERR");
}

/// TEST-USB-05: Unknown command via USB is rejected
#[test]
fn test_usb_unknown_command_rejected() {
    println!("TEST-USB-05: Unknown command via USB is rejected");

    reset_channels();

    // Process unknown command
    libreroaster::hardware::usb_cdc::tasks::process_usb_command_data(b"BOGUS\r");

    let outputs = collect_output();
    assert_eq!(outputs.len(), 1, "Unknown command should emit ERR");
    assert_err_tokens(&outputs[0], "unknown_command", "unknown_command");
    assert!(
        collect_commands().is_empty(),
        "Unknown commands must not reach channel"
    );

    println!("   ✅ Unknown USB command correctly rejected with ERR");
}

/// TEST-USB-06: Out-of-range values emit ERR
#[test]
fn test_usb_out_of_range_emits_err() {
    println!("TEST-USB-06: Out-of-range values emit ERR");

    reset_channels();

    // OT1 with value > 100
    libreroaster::hardware::usb_cdc::tasks::process_usb_command_data(b"OT1 150\r");

    // IO3 with value > 100
    libreroaster::hardware::usb_cdc::tasks::process_usb_command_data(b"IO3 200\r");

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

    println!("   ✅ Out-of-range USB values correctly emit ERR");
}

/// ========================================================================
// ============================================================================
// BUFFER OVERFLOW TESTS
// ============================================================================
/// ========================================================================

/// TEST-USB-07: Buffer overflow handling via USB
#[test]
fn test_usb_buffer_overflow_handling() {
    println!("TEST-USB-07: Buffer overflow handling via USB");

    reset_channels();

    // Create command longer than buffer (64 bytes)
    let long_command = b"A".repeat(70);
    let mut long_command_with_cr = long_command.clone();
    long_command_with_cr.push(b'\r');

    // Process should handle overflow gracefully
    // The mock should trigger buffer overflow error
    // which gets converted to ERR message
    libreroaster::hardware::usb_cdc::tasks::process_usb_command_data(&long_command_with_cr);

    // Should emit error for overflow
    let outputs = collect_output();
    assert_eq!(outputs.len(), 1, "Should emit error for overflow");
    assert_err_tokens(&outputs[0], "invalid_value", "invalid_value");

    println!("   ✅ USB buffer overflow correctly handled with ERR");
}

/// ========================================================================
// ============================================================================
// CHANNEL ROUTING TESTS
// ============================================================================
/// ========================================================================

/// TEST-USB-08: Commands routed to correct channel
#[test]
fn test_usb_commands_routed_by_channel() {
    println!("TEST-USB-08: Commands routed by channel");

    reset_channels();

    // Simulate USB channel activation via multiplexer
    let multiplexer = ServiceContainer::get_multiplexer();
    critical_section::with(|cs| {
        let mut guard = multiplexer.borrow(cs).borrow_mut();
        if let Some(mux) = guard.as_mut() {
            // Activate USB channel
            mux.should_process_command(CommChannel::Usb);
        }
    });

    // Process command
    libreroaster::hardware::usb_cdc::tasks::process_usb_command_data(b"READ\r");

    // Verify command was received
    let commands = collect_commands();
    assert_eq!(commands.len(), 1, "Command should be routed");
    assert!(matches!(commands[0], ArtisanCommand::ReadStatus));

    println!("   ✅ USB commands correctly routed to artisan channel");
}

/// TEST-USB-09: Errors routed to correct channel
#[test]
fn test_usb_errors_routed_by_channel() {
    println!("TEST-USB-09: Errors routed by channel");

    reset_channels();

    // Simulate USB channel activation
    let multiplexer = ServiceContainer::get_multiplexer();
    critical_section::with(|cs| {
        let mut guard = multiplexer.borrow(cs).borrow_mut();
        if let Some(mux) = guard.as_mut() {
            mux.should_process_command(CommChannel::Usb);
        }
    });

    // Process invalid command
    libreroaster::hardware::usb_cdc::tasks::process_usb_command_data(b"BOGUS\r");

    // Verify error was emitted to output channel
    let outputs = collect_output();
    assert_eq!(outputs.len(), 1, "Error should be routed");
    assert_err_tokens(&outputs[0], "unknown_command", "unknown_command");

    println!("   ✅ USB errors correctly routed to output channel");
}

/// ========================================================================
// ============================================================================
// INTEGRATION TESTS
// ============================================================================
/// ========================================================================

/// TEST-USB-10: Complete USB command → response flow
#[test]
fn test_usb_complete_flow() {
    println!("TEST-USB-10: Complete USB command → response flow");

    reset_channels();

    // Step 1: Simulate USB channel activation
    let multiplexer = ServiceContainer::get_multiplexer();
    critical_section::with(|cs| {
        let mut guard = multiplexer.borrow(cs).borrow_mut();
        if let Some(mux) = guard.as_mut() {
            let accepted = mux.should_process_command(CommChannel::Usb);
            assert!(accepted, "USB channel should be activated");
        }
    });

    // Step 2: Send READ command via USB
    libreroaster::hardware::usb_cdc::tasks::process_usb_command_data(b"READ\r");

    // Step 3: Verify command received
    let commands = collect_commands();
    assert_eq!(commands.len(), 1, "READ command should be received");
    assert!(matches!(commands[0], ArtisanCommand::ReadStatus));

    // Step 4: Verify no errors
    let outputs = collect_output();
    assert!(outputs.is_empty(), "No errors should occur");

    println!("   ✅ Complete USB CDC flow verified");
}

/// TEST-USB-11: Multiple valid USB commands
#[test]
fn test_usb_multiple_valid_commands() {
    println!("TEST-USB-11: Multiple valid USB commands");

    reset_channels();

    // Activate USB channel once
    let multiplexer = ServiceContainer::get_multiplexer();
    critical_section::with(|cs| {
        let mut guard = multiplexer.borrow(cs).borrow_mut();
        if let Some(mux) = guard.as_mut() {
            mux.should_process_command(CommChannel::Usb);
        }
    });

    // Send multiple commands
    libreroaster::hardware::usb_cdc::tasks::process_usb_command_data(b"READ\r");
    libreroaster::hardware::usb_cdc::tasks::process_usb_command_data(b"OT1 60\r");
    libreroaster::hardware::usb_cdc::tasks::process_usb_command_data(b"IO3 40\r");

    // Verify all commands received
    let commands = collect_commands();
    assert_eq!(commands.len(), 3, "All valid commands should reach channel");
    assert!(matches!(commands[0], ArtisanCommand::ReadStatus));
    assert!(matches!(commands[1], ArtisanCommand::SetHeater(60)));
    assert!(matches!(commands[2], ArtisanCommand::SetFan(40)));

    println!("   ✅ Multiple USB commands processed correctly");
}

/// TEST-USB-12: Invalid commands don't block valid ones
#[test]
fn test_usb_invalid_commands_dont_block() {
    println!("TEST-USB-12: Invalid USB commands don't block valid ones");

    reset_channels();

    // Activate USB channel
    let multiplexer = ServiceContainer::get_multiplexer();
    critical_section::with(|cs| {
        let mut guard = multiplexer.borrow(cs).borrow_mut();
        if let Some(mux) = guard.as_mut() {
            mux.should_process_command(CommChannel::Usb);
        }
    });

    // Send invalid then valid command
    libreroaster::hardware::usb_cdc::tasks::process_usb_command_data(b"BOGUS\r");
    libreroaster::hardware::usb_cdc::tasks::process_usb_command_data(b"READ\r");

    // Verify valid command was still received
    let commands = collect_commands();
    assert_eq!(
        commands.len(),
        1,
        "Valid command should be received despite invalid one"
    );
    assert!(matches!(commands[0], ArtisanCommand::ReadStatus));

    // Verify error was emitted for invalid command
    let outputs = collect_output();
    assert_eq!(
        outputs.len(),
        1,
        "Error for invalid command should be emitted"
    );

    println!("   ✅ Invalid USB commands correctly isolated");
}

/// TEST-USB-13: USB command with malformed value
#[test]
fn test_usb_malformed_value() {
    println!("TEST-USB-13: USB command with malformed value");

    reset_channels();

    // Activate USB channel
    let multiplexer = ServiceContainer::get_multiplexer();
    critical_section::with(|cs| {
        let mut guard = multiplexer.borrow(cs).borrow_mut();
        if let Some(mux) = guard.as_mut() {
            mux.should_process_command(CommChannel::Usb);
        }
    });

    // Send command with invalid numeric value
    libreroaster::hardware::usb_cdc::tasks::process_usb_command_data(b"OT1 abc\r");

    let outputs = collect_output();
    assert_eq!(outputs.len(), 1, "Malformed value should emit ERR");
    assert_err_tokens(&outputs[0], "invalid_value", "invalid_value");
    assert!(
        collect_commands().is_empty(),
        "Malformed commands should be dropped"
    );

    println!("   ✅ USB malformed values correctly rejected");
}

/// TEST-USB-14: USB partial command accumulation
#[test]
fn test_usb_partial_command_accumulation() {
    println!("TEST-USB-14: USB partial command accumulation");

    reset_channels();

    // Simulate streaming input - partial command
    libreroaster::hardware::usb_cdc::tasks::process_usb_command_data(b"REA");
    libreroaster::hardware::usb_cdc::tasks::process_usb_command_data(b"D\r");

    // Should accumulate to complete command
    let commands = collect_commands();
    assert_eq!(commands.len(), 1, "Partial commands should accumulate");
    assert!(matches!(commands[0], ArtisanCommand::ReadStatus));

    println!("   ✅ USB partial command accumulation works correctly");
}

/// TEST-USB-15: USB command without CR terminator
#[test]
fn test_usb_command_without_terminator() {
    println!("TEST-USB-15: USB command without CR terminator");

    reset_channels();

    // Send command without CR - should not process
    libreroaster::hardware::usb_cdc::tasks::process_usb_command_data(b"READ");

    // No command should be received
    let commands = collect_commands();
    assert!(
        commands.is_empty(),
        "Command without CR should not be processed"
    );

    // No error should be emitted (just waiting for more data)
    let outputs = collect_output();
    assert!(outputs.is_empty(), "No error for incomplete command");

    println!("   ✅ USB command without terminator correctly held");
}
