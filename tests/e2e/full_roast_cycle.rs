//! End-to-end tests - Full roast cycle simulation
//! Validates complete roast cycle with all phases

#![cfg(all(test, not(target_arch = "riscv32")))]

extern crate std;

use crate::roast_scenarios::mod::*;
use crate::roast_scenarios::{HeatingCurve, RoastingCurve, CoolingCurve};
use libreroaster::config::{ArtisanCommand, RoasterState};
use embassy_time::Instant;

/// Test 1: Full roast cycle (Start → Heat → Roast → Stop → Cool)
#[test]
fn test_full_roast_cycle() {
    let mut roaster = create_test_roaster();
    
    // Phase 1: Start roast
    assert_eq!(roaster.get_state(), RoasterState::Idle);
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    assert_eq!(roaster.get_state(), RoasterState::Heating);
    
    // Phase 2: Heating (use heating curve)
    let heating = HeatingCurve::new();
    for i in 0..=10 {
        let temp = heating.get_temp_at_second(i);
        let _ = roaster.update_temperatures(temp, temp - 10.0, Instant::now());
        roaster.update_control(Instant::now()).unwrap();
    }
    
    // Phase 3: Roasting (use roasting curve)
    let roasting = RoastingCurve::new();
    for i in 0..5 {
        let temp = roasting.get_temp_at_second(i * 30);
        let _ = roaster.update_temperatures(temp, temp - 20.0, Instant::now());
        roaster.update_control(Instant::now()).unwrap();
    }
    
    // Phase 4: Stop and cooling (use cooling curve)
    roaster.process_artisan_command(ArtisanCommand::EmergencyStop).unwrap();
    assert_eq!(roaster.get_state(), RoasterState::EmergencyStop);
    
    let cooling = CoolingCurve::new();
    for i in 0..3 {
        let temp = cooling.get_temp_at_second(i * 30);
        let _ = roaster.update_temperatures(temp, temp - 20.0, Instant::now());
        roaster.update_control(Instant::now()).unwrap();
    }
    
    // Fase 5: Final cooling → Idle
    for _ in 0..2 {
        let _ = roaster.update_temperatures(45.0, 40.0, Instant::now());
        roaster.update_control(Instant::now()).unwrap();
    }
    
    // Validate final transition
    assert!(
        matches!(roaster.get_state(), RoasterState::Cooling | RoasterState::Idle),
        "Must be in Cooling or Idle at the end"
    );
    
    // Validate final state
    let status = roaster.get_status();
    assert_eq!(status.ssr_output, 0.0, "SSR debe estar apagado al final");
}

/// Test 2: Manual control sequence (OT1, IO3)
#[test]
fn test_manual_control_sequence() {
    let mut roaster = create_test_roaster();
    
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    // Sequence: Set heater → Set fan → Verify
    roaster.process_artisan_command(ArtisanCommand::SetHeater(60)).unwrap();
    let status1 = roaster.get_status();
    assert_eq!(status1.ssr_output, 60.0, "SSR debe estar al 60%");
    
    roaster.process_artisan_command(ArtisanCommand::SetFan(40)).unwrap();
    let status2 = roaster.get_status();
    assert_eq!(status2.fan_output, 40.0, "Fan debe estar al 40%");
    
    roaster.process_artisan_command(ArtisanCommand::SetHeater(75)).unwrap();
    let status3 = roaster.get_status();
    assert_eq!(status3.ssr_output, 75.0, "SSR debe estar al 75%");
    
    roaster.process_artisan_command(ArtisanCommand::SetFan(60)).unwrap();
    let status4 = roaster.get_status();
    assert_eq!(status4.fan_output, 60.0, "Fan debe estar al 60%");
}

/// Test 3: START command enables PID
#[test]
fn test_start_enables_pid() {
    let mut roaster = create_test_roaster();
    
    let status_before = roaster.get_status();
    assert!(!status_before.pid_enabled, "PID debe estar deshabilitado inicialmente");
    
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    let status_after = roaster.get_status();
    assert!(status_after.pid_enabled, "PID debe estar habilitado tras START");
}

/// Test 4: STOP command disables PID and resets outputs
#[test]
fn test_stop_disables_pid() {
    let mut roaster = create_test_roaster();
    
    // Start and heat
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    let _ = roaster.update_temperatures(150.0, 140.0, Instant::now());
    roaster.update_control(Instant::now()).unwrap();
    
    let status_before = roaster.get_status();
    assert!(status_before.pid_enabled, "PID debe estar habilitado");
    assert!(status_before.ssr_output > 0.0, "SSR debe estar encendido");
    
    // STOP
    roaster.process_artisan_command(ArtisanCommand::EmergencyStop).unwrap();
    
    let status_after = roaster.get_status();
    assert!(!status_after.pid_enabled, "PID debe estar deshabilitado tras STOP");
    assert_eq!(status_after.ssr_output, 0.0, "SSR debe estar apagado tras STOP");
    assert_eq!(status_after.fan_output, 100.0, "Fan debe estar al 100% tras STOP");
}

/// Test 5: READ command returns correct format
#[test]
fn test_read_command_format() {
    let mut roaster = create_test_roaster();
    
    // Update temperatures
    let _ = roaster.update_temperatures(200.0, 190.0, Instant::now());
    
    // READ command (simulated - READ is not really ArtisanCommand, this is an interface test)
    let status = roaster.get_status();
    
    // Validate fields have reasonable values
    assert!(status.bean_temp > 0.0, "BT debe ser positivo");
    assert!(status.env_temp > 0.0, "ET debe ser positivo");
    assert!(status.ssr_output >= 0.0 && status.ssr_output <= 100.0, "SSR debe estar en rango");
    assert!(status.fan_output >= 0.0 && status.fan_output <= 100.0, "Fan debe estar en rango");
}

/// Test 6: Temperature validation prevents invalid values
#[test]
fn test_temperature_validation() {
    let mut roaster = create_test_roaster();
    
    // Attempt to update with invalid temperature
    let result = roaster.update_temperatures(400.0, 380.0, Instant::now());
    
    // Should return error for out-of-range temperature
    assert!(result.is_err(), "Out-of-range temperature must cause error");
}
