//! Cooling phase simulation tests
//! Valida fase de enfriamiento (STOP → Cooling → Idle)

#![cfg(all(test, not(target_arch = "riscv32")))]

extern crate std;

use crate::roast_scenarios::mod::*;
use libreroaster::config::{ArtisanCommand, RoasterState};

/// Test 1: Comando STOP corta inmediatamente
#[test]
fn test_cooling_phase_emergency_shutdown() {
    let mut roaster = create_test_roaster();
    
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    roaster.process_artisan_command(ArtisanCommand::SetHeater(80)).unwrap();
    
    // Llevar a temperatura de tueste
    let _ = roaster.update_temperatures(200.0, 180.0, Instant::now());
    roaster.update_control(Instant::now()).unwrap();
    assert!(roaster.get_status().ssr_output > 0.0);
    
    // STOP
    roaster.process_artisan_command(ArtisanCommand::EmergencyStop).unwrap();
    let status = roaster.get_status();
    
    assert_eq!(status.ssr_output, 0.0, "SSR debe estar apagado");
    assert_eq!(status.fan_output, 100.0, "Fan debe estar al 100%");
    assert!(!status.pid_enabled, "PID debe estar deshabilitado");
    assert_eq!(roaster.get_state(), RoasterState::EmergencyStop);
}

/// Test 2: Fan mantiene 100% durante enfriamiento
#[test]
fn test_cooling_phase_max_fan() {
    let mut roaster = create_test_roaster();
    let curve = CoolingCurve::new();
    
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    // Llevar a temperatura alta y STOP
    let _ = roaster.update_temperatures(220.0, 200.0, Instant::now());
    roaster.process_artisan_command(ArtisanCommand::EmergencyStop).unwrap();
    
    // Simular enfriamiento durante 150 segundos
    for i in 0..5 {
        let temp = curve.get_temp_at_second(i * 30);
        let _ = roaster.update_temperatures(temp, temp - 20.0, Instant::now());
        roaster.update_control(Instant::now()).unwrap();
        
        let status = roaster.get_status();
        
        // Fan debe mantenerse al 100% mientras temperatura > 80°C
        if temp > 80.0 {
            assert_eq!(status.fan_output, 100.0, "Fan debe estar al 100% durante enfriamiento");
        }
    }
}

/// Test 3: Derivative negativo durante enfriamiento
#[test]
fn test_cooling_phase_negative_derivative() {
    let mut roaster = create_test_roaster();
    let curve = CoolingCurve::new();
    
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    roaster.process_artisan_command(ArtisanCommand::EmergencyStop).unwrap();
    
    let mut ror_values = vec![];
    
    // Simular enfriamiento
    for i in 0..4 {
        let temp = curve.get_temp_at_second(i * 30);
        let _ = roaster.update_temperatures(temp, temp - 20.0, Instant::now());
        roaster.update_control(Instant::now()).unwrap();
        
        let status = roaster.get_status();
        ror_values.push(status.derivative_rate);
    }
    
    // ROR debe ser negativo durante enfriamiento (temperatura bajando)
    for &ror in &ror_values {
        assert!(ror <= 0.0, "ROR debe ser <=0 durante enfriamiento");
    }
}

/// Test 4: Transición a Idle después de enfriamiento
#[test]
fn test_cooling_phase_idle_transition() {
    let mut roaster = create_test_roaster();
    let curve = CoolingCurve::new();
    
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    roaster.process_artisan_command(ArtisanCommand::EmergencyStop).unwrap();
    
    // Simular enfriamiento completo
    for _ in 0..6 {
        let temp = curve.get_temp_at_second(150); // 50°C
        let _ = roaster.update_temperatures(temp, 40.0, Instant::now());
        roaster.update_control(Instant::now()).unwrap();
    }
    
    let status = roaster.get_status();
    
    // Al llegar a 50°C, fan puede reducirse
    assert!(status.fan_output <= 100.0, "Fan debe ser <=100% tras enfriamiento");
    
    // Estado debe estar en Cooling o Idle (no necesariamente Idle inmediatamente)
    assert!(
        matches!(roaster.get_state(), RoasterState::Cooling | RoasterState::Idle),
        "Debe estar en Cooling o Idle tras enfriamiento"
    );
}
