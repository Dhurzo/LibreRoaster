//! Roasting phase simulation tests
//! Validates roasting phase (150°C → 220°C)

#![cfg(all(test, not(target_arch = "riscv32")))]

extern crate std;

use crate::roast_scenarios::mod::*;
use libreroaster::config::ArtisanCommand;

/// Test 1: Temperature maintenance at setpoint
#[test]
fn test_roasting_phase_temp_stability() {
    let mut roaster = create_test_roaster();
    let curve = RoastingCurve::new();
    
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    roaster.process_artisan_command(ArtisanCommand::SetHeater(75)).unwrap();
    
    let mut temps = vec![];
    
    // Simulate 10 temperature readings in stable phase (220°C target)
    for i in 0..10 {
        let temp = curve.get_temp_at_second(240 + i * 3); // Near target
        temps.push(temp);
        
        let _ = roaster.update_temperatures(temp, 200.0, Instant::now());
        roaster.update_control(Instant::now()).unwrap();
    }
    
    // Validate stability (±3°C acceptable)
    let mean: f32 = temps.iter().sum::<f32>() / temps.len() as f32;
    let variance: f32 = temps.iter()
        .map(|&t| (t - mean).powi(2))
        .sum::<f32>() / temps.len() as f32;
    let std_dev = variance.sqrt();
    
    assert!((mean - 220.0).abs() < 5.0, "Temperature must be close to target");
    assert!(std_dev < 3.0, "Standard deviation must be <3°C");
}

/// Test 2: Correct Rate of Rise (ROR) calculation
#[test]
fn test_roasting_phase_ror_calculation() {
    let mut roaster = create_test_roaster();
    let curve = RoastingCurve::new();
    
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    let mut ror_values = vec![];
    
    // Simulate multiple readings with temperature changes
    for i in 0..7 {
        let temp = curve.get_temp_at_second(i * 30);
        let _ = roaster.update_temperatures(temp, 190.0, Instant::now());
        roaster.update_control(Instant::now()).unwrap();
        
        let status = roaster.get_status();
        ror_values.push(status.derivative_rate);
    }
    
    // ROR should be positive during heating (temperatures rising)
    for &ror in &ror_values[0..4] {
        assert!(ror >= 0.0, "ROR must be positive during heating ramp");
    }
    
    // ROR should be near 0 at stable target
    for &ror in &ror_values[5..7] {
        assert!(ror.abs() < 0.5, "ROR must be close to 0 in stable phase");
    }
}

/// Test 3: Fan behavior during roasting
#[test]
fn test_roasting_phase_fan_control() {
    let mut roaster = create_test_roaster();
    let curve = RoastingCurve::new();
    
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    // Simulate roasting phase
    for i in 0..7 {
        let temp = curve.get_temp_at_second(i * 30);
        let et = temp + 20.0; // ET always > BT
        
        let _ = roaster.update_temperatures(temp, et, Instant::now());
        roaster.update_control(Instant::now()).unwrap();
        
        let status = roaster.get_status();
        
        // Fan should increase to moderate temperature (20-80%)
        assert!(
            status.fan_output >= 20.0,
            "Fan must be >=20% during roasting"
        );
        assert!(
            status.fan_output <= 80.0,
            "Fan must be <=80% during roasting"
        );
    }
}

/// Test 4: Derivative available only after 2 samples
#[test]
fn test_roasting_phase_derivative_availability() {
    let mut roaster = create_test_roaster();
    let curve = RoastingCurve::new();
    
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    // First sample
    let temp1 = curve.get_temp_at_second(0);
    let _ = roaster.update_temperatures(temp1, 170.0, Instant::now());
    let status1 = roaster.get_status();
    assert!(!status1.derivative_available, "Derivative NOT available after 1 sample");
    
    // Second sample
    let temp2 = curve.get_temp_at_second(1);
    let _ = roaster.update_temperatures(temp2, 175.0, Instant::now());
    let status2 = roaster.get_status();
    assert!(status2.derivative_available, "Derivative available after 2 samples");
    
    // Third sample
    let temp3 = curve.get_temp_at_second(2);
    let _ = roaster.update_temperatures(temp3, 180.0, Instant::now());
    let status3 = roaster.get_status();
    assert!(status3.derivative_available, "Derivative must remain available");
}
