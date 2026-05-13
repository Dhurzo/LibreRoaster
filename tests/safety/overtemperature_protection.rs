//! Safety tests - Overtemperature protection
//! Validates overtemperature protection behavior

#![cfg(all(test, not(target_arch = "riscv32")))]

extern crate std;

use crate::roast_scenarios::mod::*;
use libreroaster::config::{ArtisanCommand, RoasterState};
use embassy_time::Instant;

/// Test 1: Trigger at 260°C
#[test]
fn test_overtemperature_triggers_at_260c() {
    let mut roaster = create_test_roaster();
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    // Bring temperature below threshold
    let _ = roaster.update_temperatures(250.0, 240.0, Instant::now());
    let status = roaster.get_status();
    assert!(!status.fault_condition, "Fault condition must not be active at 250°C");
    
    // Cruzar threshold (OVERTEMP_THRESHOLD = 260.0)
    let _ = roaster.update_temperatures(260.0, 250.0, Instant::now());
    
    let status = roaster.get_status();
    assert!(status.fault_condition, "Must activate fault_condition at 260°C");
    assert_eq!(status.ssr_output, 0.0, "SSR must be turned off immediately");
    assert_eq!(status.fan_output, 100.0, "Fan must go to 100%");
}

/// Test 2: EmergencyStop state after overtemperature
#[test]
fn test_overtemperature_sets_emergency_state() {
    let mut roaster = create_test_roaster();
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    roaster.process_artisan_command(ArtisanCommand::SetHeater(75)).unwrap();
    
    // Activate overtemperature
    let _ = roaster.update_temperatures(270.0, 260.0, Instant::now());
    
    assert_eq!(roaster.get_state(), RoasterState::EmergencyStop);
    
    let status = roaster.get_status();
    assert!(!status.pid_enabled, "PID must be disabled");
    assert!(status.fault_condition, "Fault condition must be active");
}

/// Test 3: Prevents recovery without manual intervention
#[test]
fn test_overtemperature_requires_manual_reset() {
    let mut roaster = create_test_roaster();
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    // Activate overtemperature
    let _ = roaster.update_temperatures(270.0, 260.0, Instant::now());
    assert!(roaster.get_status().fault_condition);
    
    // Try to restart with START (must fail or do nothing)
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    assert!(roaster.get_status().fault_condition, "Fault must persist after overtemperature");
    
    // Should use RESET command to clear (simulated here manually)
    // Note: The current firmware may not have a RESET command; this is expected behavior
}

/// Test 4: No activation below threshold
#[test]
fn test_overtemperature_no_trigger_below_threshold() {
    let mut roaster = create_test_roaster();
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    // Increase temperature gradually to just below threshold
    for temp in [200.0, 220.0, 240.0, 259.0] {
        let _ = roaster.update_temperatures(temp, temp - 10.0, Instant::now());
        let status = roaster.get_status();
        assert!(!status.fault_condition, "Fault condition must not activate below 260°C");
        assert_eq!(roaster.get_state(), RoasterState::Heating, "Must remain in Heating");
    }
}

/// Test 5: Fan at maximum during overtemperature
#[test]
fn test_overtemperature_fan_max() {
    let mut roaster = create_test_roaster();
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    // Activar overtemperature
    let _ = roaster.update_temperatures(270.0, 260.0, Instant::now());
    
    let status = roaster.get_status();
    assert_eq!(status.fan_output, 100.0, "Fan must be at 100% after overtemperature");
}

/// Test 6: SSR turned off during overtemperature
#[test]
fn test_overtemperature_ssr_off() {
    let mut roaster = create_test_roaster();
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    roaster.process_artisan_command(ArtisanCommand::SetHeater(75)).unwrap();
    
    // Verify that SSR was on
    let status_before = roaster.get_status();
    assert!(status_before.ssr_output > 0.0, "SSR must be on before overtemperature");
    
    // Activate overtemperature
    let _ = roaster.update_temperatures(270.0, 260.0, Instant::now());
    
    let status_after = roaster.get_status();
    assert_eq!(status_after.ssr_output, 0.0, "SSR must be off after overtemperature");
}
