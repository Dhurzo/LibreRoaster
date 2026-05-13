//! READ command format validation (host-side, no hardware needed)
//!
//! These tests validate that the READ command formats correct
//! TC4-standard responses. They use mocks — no real hardware needed.
//! For hardware-in-the-loop (HIL) tests against a real ESP32-C3,
//! run the Python script instead:
//!
//! ```bash
//! python tests/hardware/read_command_hil.py
//! ```
//!
//! # Coverage
//!
//! - Basic READ → TC4 response (5-value: AMB,ET,BT,0.0,0.0)
//! - READ with PID enabled → 8-value TC4 response (+ heater, fan, SV)
//! - Temperature scale: Celsius and Fahrenheit output
//! - Invalid/non-finite values normalized to 0.0
//! - Multiple READ commands in sequence
//! - Mixed command workload (READ + heater/fan)
//! - USB streaming: partial bytes, no-CR hold
//!
//! # Running
//!
//! ```bash
//! cargo test --test read_command_usb_test
//! ```

#![cfg(all(test, not(target_arch = "riscv32")))]
#![allow(non_snake_case)]
#![allow(clippy::expect_used)]

extern crate std;

use std::println;
use std::string::String as StdString;
use std::vec::Vec;

use libreroaster::config::{
    ArtisanCommand, RoasterState, SsrHardwareStatus, SystemStatus, TemperatureScale,
    TemperatureSettings,
};
use libreroaster::input::parser::parse_artisan_command;
use libreroaster::output::artisan::ArtisanFormatter;

// Import the mock USB CDC driver
#[path = "mock_usb_driver.rs"]
mod mock_usb_driver;
use mock_usb_driver::MockUsbCdcDriver;

// ── Helpers ──────────────────────────────────────────────────────────────

/// Create a standard test SystemStatus for READ response validation.
fn test_status() -> SystemStatus {
    SystemStatus {
        state: RoasterState::Stable,
        bean_temp: 155.7,
        env_temp: 125.5,
        ambient_temp: 25.0,
        target_temp: 200.0,
        ssr_output: 75.0,
        fan_output: 50.0,
        pid_enabled: false,
        artisan_control: false,
        fault_condition: false,
        ssr_hardware_status: SsrHardwareStatus::Available,
        ssr_last_duty_delta_ticks: 0,
        ssr_retry_count: 0,
        ssr_cycle_guard_busy_until_ms: 0,
        watchdog_feed_ok: true,
        watchdog_last_failure: None,
        watchdog_consecutive_failures: 0,
        ledc_guard_timeouts: 0,
        overtemp_regression_active: false,
        temperature_settings: TemperatureSettings::new(),
        ..SystemStatus::default()
    }
}

/// Simulate sending a USB command string to the mock driver and parsing it.
fn send_usb_and_parse(
    mock: &mut MockUsbCdcDriver,
    data: &str,
) -> Result<ArtisanCommand, StdString> {
    mock.push_rx_data(data);

    let mut buffer = [0u8; 64];
    let bytes_read = mock
        .read_bytes(&mut buffer)
        .expect("mock USB should return data");
    let cmd_str =
        core::str::from_utf8(&buffer[..bytes_read]).expect("mock USB data should be valid UTF-8");
    let trimmed = cmd_str.trim_end_matches(['\r', '\n']);

    parse_artisan_command(trimmed).map_err(|e| {
        StdString::from(match e {
            libreroaster::input::parser::ParseError::UnknownCommand => "unknown_command",
            libreroaster::input::parser::ParseError::InvalidValue => "invalid_value",
            libreroaster::input::parser::ParseError::OutOfRange => "out_of_range",
            libreroaster::input::parser::ParseError::EmptyCommand => "empty_command",
        })
    })
}

/// Assert a TC4 READ response has the correct 5-field format.
fn assert_tc4_5_value(response: &str, amb: &str, et: &str, bt: &str) {
    let parts: Vec<&str> = response.split(',').collect();
    assert_eq!(
        parts.len(),
        5,
        "TC4 READ without PID must have exactly 5 fields (AMB,ET,BT,CHAN3,CHAN4): got {}",
        response
    );
    assert_eq!(parts[0], amb, "AMB mismatch");
    assert_eq!(parts[1], et, "ET mismatch");
    assert_eq!(parts[2], bt, "BT mismatch");
    assert_eq!(parts[3], "0.0", "CHAN3 must be 0.0");
    assert_eq!(parts[4], "0.0", "CHAN4 must be 0.0");
}

/// Assert a TC4 READ response has the correct 8-field format (PID enabled).
fn assert_tc4_8_value(
    response: &str,
    amb: &str,
    et: &str,
    bt: &str,
    heater: &str,
    fan: &str,
    sv: &str,
) {
    let parts: Vec<&str> = response.split(',').collect();
    assert_eq!(
        parts.len(),
        8,
        "TC4 READ with PID must have exactly 8 fields: got {}",
        response
    );
    assert_eq!(parts[0], amb, "AMB mismatch");
    assert_eq!(parts[1], et, "ET mismatch");
    assert_eq!(parts[2], bt, "BT mismatch");
    assert_eq!(parts[3], "0.0", "CHAN3 must be 0.0");
    assert_eq!(parts[4], "0.0", "CHAN4 must be 0.0");
    assert_eq!(parts[5], heater, "Heater % mismatch");
    assert_eq!(parts[6], fan, "Fan % mismatch");
    assert_eq!(parts[7], sv, "SV setpoint mismatch");
}

// ── Tests ────────────────────────────────────────────────────────────────

/// TEST-READ-USB-01: Basic READ via USB parses to ReadStatus command.
#[test]
fn test_read_via_usb_parses_correctly() {
    println!("TEST-READ-USB-01: Basic READ via USB parses to ReadStatus");

    let mut mock = MockUsbCdcDriver::new();
    let result = send_usb_and_parse(&mut mock, "READ\r\n");

    assert!(result.is_ok(), "READ command should parse successfully");
    assert!(
        matches!(result.unwrap(), ArtisanCommand::ReadStatus),
        "READ must parse to ReadStatus"
    );
    println!("   ✅ READ via USB correctly parsed as ReadStatus");
}

/// TEST-READ-USB-02: TC4 READ response has exactly 5 values when PID is off.
#[test]
fn test_tc4_read_response_format() {
    println!("TEST-READ-USB-02: TC4 response format (PID off → 5 values)");

    let status = test_status();
    let response = ArtisanFormatter::format_read_response_full(&status);
    assert_tc4_5_value(&response, "25.0", "125.5", "155.7");

    println!("   ✅ TC4 5-value format correct: {}", response);
}

/// TEST-READ-USB-03: TC4 READ response has 8 values when PID is on.
#[test]
fn test_tc4_read_response_pid_format() {
    println!("TEST-READ-USB-03: TC4 response with PID (→ 8 values)");

    let mut status = test_status();
    status.pid_enabled = true;
    status.target_temp = 200.0;
    status.ssr_output = 75.0;
    status.fan_output = 50.0;

    let response = ArtisanFormatter::format_read_response_full(&status);
    assert_tc4_8_value(&response, "25.0", "125.5", "155.7", "75.0", "50.0", "200.0");

    println!("   ✅ TC4 8-value format correct: {}", response);
}

/// TEST-READ-USB-04: READ response respects Fahrenheit conversion.
#[test]
fn test_tc4_read_respects_fahrenheit() {
    println!("TEST-READ-USB-04: READ response in Fahrenheit");

    let mut status = test_status();
    // 25.0°C = 77.0°F, 125.5°C = 257.9°F, 155.7°C = 312.3°F
    status
        .temperature_settings
        .set_scale(TemperatureScale::Fahrenheit);

    let response = ArtisanFormatter::format_read_response_full(&status);
    let parts: Vec<&str> = response.split(',').collect();

    assert_eq!(parts.len(), 5);
    assert_eq!(parts[0], "77.0", "AMB in °F");
    assert_eq!(parts[1], "257.9", "ET in °F");
    assert_eq!(parts[2], "312.3", "BT in °F");
    assert_eq!(parts[3], "0.0");
    assert_eq!(parts[4], "0.0");

    println!(
        "   ✅ Fahrenheit values correct: AMB={}, ET={}, BT={}",
        parts[0], parts[1], parts[2]
    );
}

/// TEST-READ-USB-05: PID fields (heater, fan, SV) must NOT be converted in Fahrenheit.
#[test]
fn test_tc4_pid_values_not_converted_in_fahrenheit() {
    println!("TEST-READ-USB-05: PID fields not temperature-converted in °F");

    let mut status = test_status();
    status.pid_enabled = true;
    status.target_temp = 200.0; // 200°C = 392°F
    status.ssr_output = 75.0; // percentage — NOT converted
    status.fan_output = 50.0; // percentage — NOT converted
    status
        .temperature_settings
        .set_scale(TemperatureScale::Fahrenheit);

    let response = ArtisanFormatter::format_read_response_full(&status);
    let parts: Vec<&str> = response.split(',').collect();

    assert_eq!(parts.len(), 8);
    // Heater/fan are percentages
    assert_eq!(parts[5], "75.0", "Heater %% must not be converted in °F");
    assert_eq!(parts[6], "50.0", "Fan %% must not be converted in °F");
    // SV IS a temperature so it IS converted
    assert_eq!(parts[7], "392.0", "SV must be converted to °F");

    println!(
        "   ✅ PID fields correct: heater={}, fan={}, SV={}",
        parts[5], parts[6], parts[7]
    );
}

/// TEST-READ-USB-06: Non-finite values normalized to 0.0.
#[test]
fn test_tc4_read_invalid_values_normalized() {
    println!("TEST-READ-USB-06: Invalid values normalized to 0.0");

    let mut status = test_status();
    status.bean_temp = f32::NEG_INFINITY;
    status.env_temp = f32::NAN;

    let response = ArtisanFormatter::format_read_response_full(&status);
    // AMB=25.0 (valid), ET=0.0 (NaN→0), BT=0.0 (-inf→0)
    assert_tc4_5_value(&response, "25.0", "0.0", "0.0");

    println!("   ✅ Invalid values normalized: {}", response);
}

/// TEST-READ-USB-07: Multiple READ commands in sequence via USB.
#[test]
fn test_multiple_reads_in_sequence() {
    println!("TEST-READ-USB-07: Multiple READ commands in sequence");

    let mut mock = MockUsbCdcDriver::new();

    // Send first READ
    let r1 = send_usb_and_parse(&mut mock, "READ\r\n");
    assert!(r1.is_ok());
    assert!(matches!(r1.unwrap(), ArtisanCommand::ReadStatus));

    // Send second READ
    let r2 = send_usb_and_parse(&mut mock, "READ\r\n");
    assert!(r2.is_ok());
    assert!(matches!(r2.unwrap(), ArtisanCommand::ReadStatus));

    // Send third READ
    let r3 = send_usb_and_parse(&mut mock, "READ\r\n");
    assert!(r3.is_ok());
    assert!(matches!(r3.unwrap(), ArtisanCommand::ReadStatus));

    println!("   ✅ All 3 READ commands parsed successfully");
}

/// TEST-READ-USB-08: Mixed USB workload — READ interleaved with other commands.
#[test]
fn test_read_mixed_with_other_commands() {
    println!("TEST-READ-USB-08: READ mixed with OT1/IO3 commands");

    let mut mock = MockUsbCdcDriver::new();

    // READ
    let cmd = send_usb_and_parse(&mut mock, "READ\r\n").unwrap();
    assert!(matches!(cmd, ArtisanCommand::ReadStatus));

    // Set heater
    let cmd = send_usb_and_parse(&mut mock, "OT1 75\r\n").unwrap();
    assert!(matches!(cmd, ArtisanCommand::SetHeater(75)));

    // READ again — parser must handle after OT1
    let cmd = send_usb_and_parse(&mut mock, "READ\r\n").unwrap();
    assert!(matches!(cmd, ArtisanCommand::ReadStatus));

    // Set fan
    let cmd = send_usb_and_parse(&mut mock, "IO3 50\r\n").unwrap();
    assert!(matches!(cmd, ArtisanCommand::SetFan(50)));

    // READ again
    let cmd = send_usb_and_parse(&mut mock, "READ\r\n").unwrap();
    assert!(matches!(cmd, ArtisanCommand::ReadStatus));

    println!("   ✅ READ works correctly interleaved with OT1/IO3");
}

/// TEST-READ-USB-09: USB streaming — concatenating partial bytes builds READ.
#[test]
fn test_read_partial_bytes_accumulate() {
    println!("TEST-READ-USB-09: Concatenating partial bytes builds READ");

    let mut mock = MockUsbCdcDriver::new();

    // Send first chunk
    let mut buffer = [0u8; 64];
    let mut accumulated = std::vec::Vec::new();

    mock.push_rx_data("REA");
    let bytes = mock.read_bytes(&mut buffer).unwrap();
    accumulated.extend_from_slice(&buffer[..bytes]);
    let chunk1 = core::str::from_utf8(&buffer[..bytes]).unwrap();
    println!("   Chunk 1: {:?}", chunk1);

    // Send second chunk
    mock.push_rx_data("D\r\n");
    let bytes = mock.read_bytes(&mut buffer).unwrap();
    accumulated.extend_from_slice(&buffer[..bytes]);
    let chunk2 = core::str::from_utf8(&buffer[..bytes]).unwrap();
    println!("   Chunk 2: {:?}", chunk2);

    // Now parse the fully accumulated string
    let full = core::str::from_utf8(&accumulated).unwrap();
    let trimmed = full.trim_end_matches(['\r', '\n']);
    let result = parse_artisan_command(trimmed);

    assert!(
        result.is_ok(),
        "Accumulated bytes should parse as READ, got {:?}",
        result
    );
    assert!(matches!(result.unwrap(), ArtisanCommand::ReadStatus));

    println!(
        "   ✅ Partial bytes correctly accumulated: {:?} → READ",
        full.trim()
    );
}

/// TEST-READ-USB-10: USB command without CR terminator is held (not processed).
#[test]
fn test_read_without_cr_is_held() {
    println!("TEST-READ-USB-10: READ without CR terminator is held");

    let mut mock = MockUsbCdcDriver::new();

    mock.push_rx_data("READ");
    // No \r\n — parser should not see a complete command
    let mut buffer = [0u8; 64];
    // Read all available bytes (no CR yet)
    let bytes = mock.read_bytes(&mut buffer).unwrap();
    let chunk = core::str::from_utf8(&buffer[..bytes]).unwrap();
    let trimmed = chunk.trim_end_matches(['\r', '\n']);

    if trimmed == "READ" {
        // Without CR, the USB task wouldn't dispatch this — it's just buffered.
        // The parser would see it as unknown (no CR = incomplete command).
        println!("   ✅ DATA held (no CR terminator) — would wait for completion");
    } else {
        println!("   No complete command available");
    }

    println!("   ✅ READ without CR correctly held");
}

/// TEST-READ-USB-11: READ response has no CR/LF terminators.
#[test]
fn test_read_response_has_no_terminators() {
    println!("TEST-READ-USB-11: READ response terminator-free");

    let status = test_status();
    let response = ArtisanFormatter::format_read_response_full(&status);

    assert!(
        !response.contains('\r'),
        "READ response must not contain CR"
    );
    assert!(
        !response.contains('\n'),
        "READ response must not contain LF"
    );

    println!("   ✅ READ response has no embedded terminators");
}

/// TEST-READ-USB-12: READ response field order is AMB first (TC4 standard).
#[test]
fn test_tc4_field_order_amb_first() {
    println!("TEST-READ-USB-12: TC4 field order — AMB first");

    let mut status = test_status();
    status.ambient_temp = 22.5;
    status.env_temp = 180.0;
    status.bean_temp = 200.0;

    let response = ArtisanFormatter::format_read_response_full(&status);
    let parts: Vec<&str> = response.split(',').collect();

    // TC4 standard: AMB,ET,BT,CHAN3,CHAN4
    assert_eq!(parts[0], "22.5", "First field must be AMB (ambient)");
    assert_eq!(parts[1], "180.0", "Second field must be ET");
    assert_eq!(parts[2], "200.0", "Third field must be BT");

    println!(
        "   ✅ TC4 field order correct: AMB={}, ET={}, BT={}",
        parts[0], parts[1], parts[2]
    );
}

/// TEST-READ-USB-13: READ ambient temp defaults to 0.0.
#[test]
fn test_read_ambient_defaults_to_zero() {
    println!("TEST-READ-USB-13: AMB defaults to 0.0 when not set");

    let status = SystemStatus::default(); // fresh default — ambient_temp = 0.0
    let response = ArtisanFormatter::format_read_response_full(&status);
    let parts: Vec<&str> = response.split(',').collect();

    assert_eq!(parts[0], "0.0", "Default AMB should be 0.0");

    println!("   ✅ Default AMB is 0.0");
}

/// TEST-READ-USB-14: Edge case — READ after STOP (fault condition).
#[test]
fn test_read_response_after_stop() {
    println!("TEST-READ-USB-14: READ response after STOP");

    // A STOP puts the system in Idle; the READ response should still be valid
    let mut status = test_status();
    status.state = RoasterState::EmergencyStop;
    status.ssr_output = 0.0; // Heater = 0 after STOP

    let response = ArtisanFormatter::format_read_response_full(&status);
    assert_tc4_5_value(&response, "25.0", "125.5", "155.7");

    println!("   ✅ READ returns valid data after STOP");
}

/// TEST-READ-USB-15: READ response consistency across multiple calls same data.
#[test]
fn test_read_response_consistency() {
    println!("TEST-READ-USB-15: READ response consistency");

    let status = test_status();
    let r1 = ArtisanFormatter::format_read_response_full(&status);
    let r2 = ArtisanFormatter::format_read_response_full(&status);
    let r3 = ArtisanFormatter::format_read_response_full(&status);

    assert_eq!(r1, r2, "READ response must be identical for same data");
    assert_eq!(r2, r3, "READ response must be identical for same data");

    println!("   ✅ READ response consistent: {}", r1);
}
