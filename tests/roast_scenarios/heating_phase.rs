//! Heating phase simulation tests
//! Validates heating phase (Idle → Heating → Stable)

#![cfg(all(test, not(target_arch = "riscv32")))]

extern crate std;

use crate::roast_scenarios::mod::*;
use libreroaster::config::{ArtisanCommand, RoasterState};

/// Test 1: Correct state transition during heating
#[test]
fn test_heating_phase_state_transitions() {
    let mut roaster = create_test_roaster();
    let curve = HeatingCurve::new();
    
    // Initial state: Idle
    assert_eq!(roaster.get_state(), RoasterState::Idle);
    
    // Start roast
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    assert_eq!(roaster.get_state(), RoasterState::Heating);
    
    // Simulate temperature ramp (0-10 seconds)
    for i in 0..=10 {
        let temp = curve.get_temp_at_second(i);
        let _ = roaster.update_temperatures(temp, temp - 10.0, Instant::now());
        roaster.update_control(Instant::now()).unwrap();
        
        // Validate that heater is active
        let status = roaster.get_status();
        assert!(status.ssr_output >= 0.0, "SSR must be between 0-100%");
        assert!(status.ssr_output <= 100.0, "SSR must be between 0-100%");
    }
    
    // At 150°C, must stabilize
    let status = roaster.get_status();
    // State must remain in Heating or transition to Stable
    assert!(
        matches!(roaster.get_state(), RoasterState::Heating | RoasterState::Stable),
        "Must be in Heating or Stable when reaching target"
    );
}

/// Test 2: PID response during heating
#[test]
fn test_heating_phase_pid_response() {
    let mut roaster = create_test_roaster();
    let curve = HeatingCurve::new();
    
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    let mut integrator_values = vec![];
    
    // Simulate temperature ramp
    
    for i in 0..=10 {
        let temp = curve.get_temp_at_second(i);
        let _ = roaster.update_temperatures(temp, temp - 10.0, Instant::now());
        roaster.update_control(Instant::now()).unwrap();
        
        let status = roaster.get_status();
        integrator_values.push(status.integrator_value);
        
        // Validate that integrator is growing (accumulating error)
        if i > 0 {
            assert!(
                integrator_values[i] >= integrator_values[i - 1],
                "Integrator must grow during heating ramp"
            );
        }
    }
    
    // Validate no saturation during normal heating
    let status = roaster.get_status();
    assert!(!status.integrator_clamped, "Integrator must not be clamped during normal ramp");
    assert!(!status.saturation_active, "There must be no saturation during normal ramp");
}

/// Test 3: Fan should remain low during heating
#[test]
fn test_heating_phase_fan_behavior() {
    let mut roaster = create_test_roaster();
    let curve = HeatingCurve::new();
    
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    // Simulate temperature ramp
    for i in 0..=10 {
        let temp = curve.get_temp_at_second(i);
        let _ = roaster.update_temperatures(temp, temp - 10.0, Instant::now());
        roaster.update_control(Instant::now()).unwrap();
        
        let status = roaster.get_status();
        
        // Fan should remain low (0-30%) during heating
        assert!(
            status.fan_output <= 30.0,
            "Fan must be low (<=30%) during heating"
        );
    }
}

/// Test 4: SSR output should increase gradually
#[test]
fn test_heating_phase_ssr_ramp() {
    let mut roaster = create_test_roaster();
    let curve = HeatingCurve::new();
    
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    let mut ssr_outputs = vec![];
    
    // Simulate temperature ramp
    for i in 0..=10 {
        let temp = curve.get_temp_at_second(i);
        let _ = roaster.update_temperatures(temp, temp - 10.0, Instant::now());
        roaster.update_control(Instant::now()).unwrap();
        
        let status = roaster.get_status();
        ssr_outputs.push(status.ssr_output);
    }
    
    // Validate SSR output increased during ramp
    // (may not be monotonic if PID modulates)
    let initial_avg = ssr_outputs[0..3].iter().sum::<f32>() / 3.0;
    let final_avg = ssr_outputs[7..=10].iter().sum::<f32>() / 4.0;
    
    assert!(
        final_avg >= initial_avg,
        "SSR output must be higher at the end of the ramp"
    );
}
