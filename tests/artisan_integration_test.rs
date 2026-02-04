//! Integration tests for Parser + Formatter flow
//!
//! These tests verify the complete Artisan+ protocol flow:
//! - Command parsing (READ, OT1, IO3, START, STOP)
//! - Status formatting (Artisan CSV, READ response)
//! - End-to-end command → parse → format → response flow
//!
//! # Running Tests
//!
//! ```bash
//! # Run with ESP toolchain
//! cargo test --test artisan_integration_test
//!
//! # Run with custom test runner
//! cargo test --test artisan_integration_test --features test
//! ```

#![cfg(all(test, not(target_arch = "riscv32")))]

extern crate std;

use std::println;

use libreroaster::config::{ArtisanCommand, RoasterState, SsrHardwareStatus, SystemStatus};
use libreroaster::input::parser::{parse_artisan_command, ParseError};
use libreroaster::output::artisan::{ArtisanFormatter, MutableArtisanFormatter};
use libreroaster::output::OutputFormatter;

/// Helper to create test SystemStatus
fn create_test_status() -> SystemStatus {
    SystemStatus {
        state: RoasterState::Stable,
        bean_temp: 150.5,
        env_temp: 120.3,
        target_temp: 200.0,
        ssr_output: 75.0,
        fan_output: 50.0,
        pid_enabled: true,
        artisan_control: false,
        fault_condition: false,
        ssr_hardware_status: SsrHardwareStatus::Available,
    }
}

/// Helper to create minimal SystemStatus for ROR tests
fn create_minimal_status(bean_temp: f32, env_temp: f32, ssr_output: f32) -> SystemStatus {
    SystemStatus {
        bean_temp,
        env_temp,
        ssr_output,
        ..create_test_status()
    }
}

/// TEST-INT-01: READ command parsing and response formatting
#[test]
fn test_read_command_flow() {
    println!("TEST-INT-01: READ command flow");

    // 1. Parse READ command
    let parse_result = parse_artisan_command("READ");
    assert!(
        parse_result.is_ok(),
        "READ command should parse successfully"
    );

    let command = parse_result.unwrap();
    match command {
        ArtisanCommand::ReadStatus => {
            println!("   ✅ Correctly parsed as ReadStatus");
        }
        _ => {
            panic!("Expected ReadStatus, got {:?}", command);
        }
    }

    // 2. Format READ response
    let status = create_test_status();
    let fan_speed = 25.0;

    let response = ArtisanFormatter::format_read_response(&status, fan_speed);
    let expected = "120.3,150.5,75.0,25.0"; // ET,BT,Power,Fan

    assert_eq!(
        response, expected,
        "READ response should match expected format"
    );
    println!("   ✅ READ response formatted correctly: {}", response);

    // 3. Parse and verify all fields
    let parts: Vec<&str> = response.split(',').collect();
    assert_eq!(
        parts.len(),
        4,
        "READ response should have 4 comma-separated values"
    );

    assert_eq!(parts[0], "120.3", "First field should be ET");
    assert_eq!(parts[1], "150.5", "Second field should be BT");
    assert_eq!(parts[2], "75.0", "Third field should be Power");
    assert_eq!(parts[3], "25.0", "Fourth field should be Fan speed");

    println!("   ✅ All READ response fields verified");
}

/// TEST-INT-02: OT1 (heater control) command parsing
#[test]
fn test_ot1_command_flow() {
    println!("TEST-INT-02: OT1 command flow");

    // Test various OT1 values
    let test_cases = [
        ("OT1 0", 0),
        ("OT1 50", 50),
        ("OT1 75", 75),
        ("OT1 100", 100),
    ];

    for (input, expected_value) in test_cases {
        let result = parse_artisan_command(input);

        assert!(result.is_ok(), "OT1 {} should parse successfully", input);

        match result.unwrap() {
            ArtisanCommand::SetHeater(value) => {
                assert_eq!(
                    value, expected_value,
                    "OT1 {} should parse to heater {}",
                    input, expected_value
                );
                println!("   ✅ OT1 {} → SetHeater({})", input, value);
            }
            _ => {
                panic!("Expected SetHeater, got {:?}", result.unwrap());
            }
        }
    }

    // Test boundary and invalid values
    let invalid_result = parse_artisan_command("OT1 150");
    assert!(invalid_result.is_err(), "OT1 150 should fail (value > 100)");

    let invalid_result = parse_artisan_command("OT1");
    assert!(invalid_result.is_err(), "OT1 without value should fail");

    println!("   ✅ OT1 validation works correctly");
}

/// TEST-INT-03: IO3 (fan control) command parsing
#[test]
fn test_io3_command_flow() {
    println!("TEST-INT-03: IO3 command flow");

    // Test various IO3 values
    let test_cases = [("IO3 0", 0), ("IO3 50", 50), ("IO3 100", 100)];

    for (input, expected_value) in test_cases {
        let result = parse_artisan_command(input);

        assert!(result.is_ok(), "IO3 {} should parse successfully", input);

        match result.unwrap() {
            ArtisanCommand::SetFan(value) => {
                assert_eq!(
                    value, expected_value,
                    "IO3 {} should parse to fan {}",
                    input, expected_value
                );
                println!("   ✅ IO3 {} → SetFan({})", input, value);
            }
            _ => {
                panic!("Expected SetFan, got {:?}", result.unwrap());
            }
        }
    }

    // Test invalid values
    let invalid_result = parse_artisan_command("IO3 150");
    assert!(invalid_result.is_err(), "IO3 150 should fail (value > 100)");

    println!("   ✅ IO3 validation works correctly");
}

/// TEST-INT-04: Full command pipeline - parse multiple commands
#[test]
fn test_full_command_pipeline() {
    println!("TEST-INT-04: Full command pipeline");

    // Define command sequence
    let commands = [
        ("READ", ArtisanCommand::ReadStatus),
        ("START", ArtisanCommand::StartRoast),
        ("OT1 80", ArtisanCommand::SetHeater(80)),
        ("IO3 60", ArtisanCommand::SetFan(60)),
        ("STOP", ArtisanCommand::EmergencyStop),
    ];

    // Parse each command and verify
    for (input, expected) in commands {
        let result = parse_artisan_command(input);

        assert!(result.is_ok(), "Command '{}' should parse", input);
        assert_eq!(
            result.unwrap(),
            expected,
            "Command '{}' should parse to expected type",
            input
        );
        println!("   ✅ '{}' → {:?}", input, expected);
    }

    // After parsing, test that formatter can create output
    let status = create_test_status();
    let formatter = ArtisanFormatter::new();

    let csv_result = formatter.format(&status);
    assert!(csv_result.is_ok(), "Formatter should work after parsing");

    let csv = csv_result.unwrap();
    let parts: Vec<&str> = csv.split(',').collect();
    assert_eq!(parts.len(), 5, "CSV should have 5 fields");

    println!("   ✅ Formatter produces valid CSV after parsing commands");
}

/// TEST-INT-05: ROR calculation across multiple reads
#[test]
fn test_ror_calculation_across_reads() {
    println!("TEST-INT-05: ROR calculation across reads");

    let mut formatter = MutableArtisanFormatter::new();

    // Sequence of readings with increasing BT
    // BT: 100 → 102 → 104 → 106 → 108
    // Expected ROR for each: 0.0 → 2.0 → 2.0 → 2.0 → 2.0
    let readings = [100.0, 102.0, 104.0, 106.0, 108.0];
    let expected_ror = [0.0, 2.0, 2.0, 2.0, 2.0];

    for (i, (bt, expected)) in readings.iter().zip(expected_ror.iter()).enumerate() {
        let status = create_minimal_status(*bt, 120.0, 50.0);

        let result = formatter.format(&status);
        assert!(
            result.is_ok(),
            "Reading {} should format successfully",
            i + 1
        );

        let output = result.unwrap();
        let parts: Vec<&str> = output.split(',').collect();
        assert_eq!(parts.len(), 5, "Reading {} should have 5 fields", i + 1);

        // Parse ROR field (index 3)
        let ror_value: f32 = parts[3].parse().expect("ROR should be parseable");

        // Allow small floating point tolerance
        assert!(
            (ror_value - expected).abs() < 0.1,
            "Reading {} ROR should be ~{:.1}, got {:.2}",
            i + 1,
            expected,
            ror_value
        );

        println!(
            "   ✅ Reading {} (BT={}): ROR = {} °C/s",
            i + 1,
            bt,
            ror_value
        );
    }

    println!("   ✅ ROR calculation verified across multiple readings");
}

/// TEST-INT-06: Error handling in integration flow
#[test]
fn test_error_handling_integration() {
    println!("TEST-INT-06: Error handling integration");

    // Test invalid commands
    let invalid_commands = [
        ("INVALID", ParseError::UnknownCommand),
        ("", ParseError::EmptyCommand),
        ("OT1 150", ParseError::OutOfRange),
        ("IO3 150", ParseError::OutOfRange),
        ("OT1 abc", ParseError::InvalidValue),
    ];

    for (input, expected_error) in invalid_commands {
        let result = parse_artisan_command(input);

        assert!(result.is_err(), "'{}' should fail to parse", input);

        match result.unwrap_err() {
            error => {
                // Check error type matches
                match (&error, &expected_error) {
                    (ParseError::UnknownCommand, ParseError::UnknownCommand) => {
                        println!("   ✅ '{}' → UnknownCommand", input);
                    }
                    (ParseError::EmptyCommand, ParseError::EmptyCommand) => {
                        println!("   ✅ '{}' → EmptyCommand", input);
                    }
                    (ParseError::InvalidValue, ParseError::InvalidValue) => {
                        println!("   ✅ '{}' → InvalidValue", input);
                    }
                    (ParseError::OutOfRange, ParseError::OutOfRange) => {
                        println!("   ✅ '{}' → OutOfRange", input);
                    }
                    _ => {
                        panic!(
                            "Wrong error type for '{}': got {:?}, expected {:?}",
                            input, error, expected_error
                        );
                    }
                }
            }
        }
    }

    println!("   ✅ Error handling works correctly across all cases");
}

/// TEST-INT-07: Artisan CSV format validation
#[test]
fn test_artisan_csv_format() {
    println!("TEST-INT-07: Artisan CSV format validation");

    let status = create_test_status();
    let formatter = ArtisanFormatter::new();

    let result = formatter.format(&status);
    assert!(result.is_ok(), "Format should succeed");

    let csv = result.unwrap();
    let parts: Vec<&str> = csv.split(',').collect();

    assert_eq!(
        parts.len(),
        5,
        "CSV should have 5 fields: time,ET,BT,ROR,Gas"
    );

    // Verify field structure
    assert!(!parts[0].is_empty(), "Time field should not be empty");
    assert!(
        parts[0].starts_with(|c: char| c.is_ascii_digit()),
        "Time should start with digit"
    );

    // ET field (index 1)
    let et: f32 = parts[1].parse().expect("ET should be parseable");
    assert_eq!(et, 120.3, "ET should match input");

    // BT field (index 2)
    let bt: f32 = parts[2].parse().expect("BT should be parseable");
    assert_eq!(bt, 150.5, "BT should match input");

    // ROR field (index 3)
    let ror: f32 = parts[3].parse().expect("ROR should be parseable");
    assert!(ror >= 0.0, "ROR should be non-negative");

    // Gas field (index 4)
    let gas: f32 = parts[4].parse().expect("Gas should be parseable");
    assert_eq!(gas, 75.0, "Gas should match SSR output");

    println!("   ✅ CSV format validated: {}", csv);
    println!(
        "   ✅ Fields: time={}, ET={}, BT={}, ROR={}, Gas={}",
        parts[0], parts[1], parts[2], parts[3], parts[4]
    );
}

/// TEST-INT-08: Complete READ → Parse → Format → Response flow
#[test]
fn test_complete_flow() {
    println!("TEST-INT-08: Complete READ → Parse → Format → Response flow");

    // Step 1: Simulate receiving a command from Artisan
    let incoming_command = "READ";

    // Step 2: Parse the command
    let parsed = parse_artisan_command(incoming_command);
    assert!(parsed.is_ok(), "Should parse READ command");

    let command = parsed.unwrap();
    assert!(matches!(command, ArtisanCommand::ReadStatus));

    // Step 3: Create status for response
    let status = create_test_status();

    // Step 4: Format response
    let response = ArtisanFormatter::format_read_response(&status, 25.0);

    // Step 5: Verify response format
    assert!(response.contains("120.3"), "Should contain ET");
    assert!(response.contains("150.5"), "Should contain BT");
    assert!(response.contains("75.0"), "Should contain Power");
    assert!(response.contains("25.0"), "Should contain Fan speed");

    let parts: Vec<&str> = response.split(',').collect();
    assert_eq!(parts.len(), 4, "Response should have 4 fields");

    println!(
        "   ✅ Complete flow: '{}' → {:?} → '{}'",
        incoming_command, command, response
    );
    println!("   ✅ All stages executed successfully");
}
