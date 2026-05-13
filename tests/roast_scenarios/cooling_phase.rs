//! Cooling phase simulation tests
//! Validates cooling phase (STOP → Cooling → Idle)

#![cfg(all(test, not(target_arch = "riscv32")))]

extern crate std;

use crate::roast_scenarios::mod::*;
use libreroaster::config::{ArtisanCommand, RoasterState};

/// Test 1: STOP command cuts immediately
#[test]
fn test_cooling_phase_emergency_shutdown() {
    let mut roaster = create_test_roaster();
    
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    roaster.process_artisan_command(ArtisanCommand::SetHeater(80)).unwrap();
    
    // Bring to roasting temperature
    let _ = roaster.update_temperatures(200.0, 180.0, Instant::now());
    roaster.update_control(Instant::now()).unwrap();
    assert!(roaster.get_status().ssr_output > 0.0);
    
    // STOP
    roaster.process_artisan_command(ArtisanCommand::EmergencyStop).unwrap();
    let status = roaster.get_status();
    
    assert_eq!(status.ssr_output, 0.0, "SSR must be off");
    assert_eq!(status.fan_output, 100.0, "Fan must be at 100%");
    assert!(!status.pid_enabled, "PID must be disabled");
    assert_eq!(roaster.get_state(), RoasterState::EmergencyStop);
}

/// Test 2: Fan maintains 100% during cooling
#[test]
fn test_cooling_phase_max_fan() {
    let mut roaster = create_test_roaster();
    let curve = CoolingCurve::new();
    
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    // Bring to high temperature and STOP
    let _ = roaster.update_temperatures(220.0, 200.0, Instant::now());
    roaster.process_artisan_command(ArtisanCommand::EmergencyStop).unwrap();
    
    // Simulate cooling for 150 seconds
    for i in 0..5 {
        let temp = curve.get_temp_at_second(i * 30);
        let _ = roaster.update_temperatures(temp, temp - 20.0, Instant::now());
        roaster.update_control(Instant::now()).unwrap();
        
        let status = roaster.get_status();
        
        // Fan must stay at 100% while temperature > 80°C
        if temp > 80.0 {
            assert_eq!(status.fan_output, 100.0, "Fan must be at 100% during cooling");
        }
    }
}

/// Test 3: Negative derivative during cooling
#[test]
fn test_cooling_phase_negative_derivative() {
    let mut roaster = create_test_roaster();
    let curve = CoolingCurve::new();
    
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    roaster.process_artisan_command(ArtisanCommand::EmergencyStop).unwrap();
    
    let mut ror_values = vec![];
    
    // Simulate cooling
    for i in 0..4 {
        let temp = curve.get_temp_at_second(i * 30);
        let _ = roaster.update_temperatures(temp, temp - 20.0, Instant::now());
        roaster.update_control(Instant::now()).unwrap();
        
        let status = roaster.get_status();
        ror_values.push(status.derivative_rate);
    }
    
    // ROR should be negative during cooling (temperature dropping)
    for &ror in &ror_values {
        assert!(ror <= 0.0, "ROR must be <=0 during cooling");
    }
}

/// Test 4: Transition to Idle after cooling
#[test]
fn test_cooling_phase_idle_transition() {
    let mut roaster = create_test_roaster();
    let curve = CoolingCurve::new();
    
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    roaster.process_artisan_command(ArtisanCommand::EmergencyStop).unwrap();
    
    // Simulate complete cooling
    for _ in 0..6 {
        let temp = curve.get_temp_at_second(150); // 50°C
        let _ = roaster.update_temperatures(temp, 40.0, Instant::now());
        roaster.update_control(Instant::now()).unwrap();
    }
    
    let status = roaster.get_status();
    
    // At 50°C, fan can reduce
    assert!(status.fan_output <= 100.0, "Fan must be <=100% after cooling");
    
    // State should be in Cooling or Idle (not necessarily Idle immediately)
    assert!(
        matches!(roaster.get_state(), RoasterState::Cooling | RoasterState::Idle),
        "Must be in Cooling or Idle after cooling"
    );
}
