//! Heating phase simulation tests
//! Valida fase de calentamiento (Idle → Heating → Stable)

#![cfg(all(test, not(target_arch = "riscv32")))]

extern crate std;

use crate::roast_scenarios::mod::*;
use libreroaster::config::{ArtisanCommand, RoasterState};

/// Test 1: Transición correcta de estado durante calentamiento
#[test]
fn test_heating_phase_state_transitions() {
    let mut roaster = create_test_roaster();
    let curve = HeatingCurve::new();
    
    // Estado inicial: Idle
    assert_eq!(roaster.get_state(), RoasterState::Idle);
    
    // Iniciar tueste
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    assert_eq!(roaster.get_state(), RoasterState::Heating);
    
    // Simular rampa de temperatura (0-10 segundos)
    for i in 0..=10 {
        let temp = curve.get_temp_at_second(i);
        let _ = roaster.update_temperatures(temp, temp - 10.0, Instant::now());
        roaster.update_control(Instant::now()).unwrap();
        
        // Validar que el calentador está activo
        let status = roaster.get_status();
        assert!(status.ssr_output >= 0.0, "SSR debe estar entre 0-100%");
        assert!(status.ssr_output <= 100.0, "SSR debe estar entre 0-100%");
    }
    
    // Al alcanzar 150°C, debe estabilizarse
    let status = roaster.get_status();
    // El estado debe seguir en Heating o transicionar a Stable
    assert!(
        matches!(roaster.get_state(), RoasterState::Heating | RoasterState::Stable),
        "Debe estar en Heating o Stable al alcanzar target"
    );
}

/// Test 2: Respuesta de PID durante calentamiento
#[test]
fn test_heating_phase_pid_response() {
    let mut roaster = create_test_roaster();
    let curve = HeatingCurve::new();
    
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    let mut integrator_values = vec![];
    
    // Simular rampa de temperatura
    for i in 0..=10 {
        let temp = curve.get_temp_at_second(i);
        let _ = roaster.update_temperatures(temp, temp - 10.0, Instant::now());
        roaster.update_control(Instant::now()).unwrap();
        
        let status = roaster.get_status();
        integrator_values.push(status.integrator_value);
        
        // Validar que el integrador está creciendo (acumulando error)
        if i > 0 {
            assert!(
                integrator_values[i] >= integrator_values[i - 1],
                "Integrador debe crecer durante rampa de calentamiento"
            );
        }
    }
    
    // Validar que no hay saturación durante calentamiento normal
    let status = roaster.get_status();
    assert!(!status.integrator_clamped, "Integrador no debe estar clamped en rampa normal");
    assert!(!status.saturation_active, "No debe haber saturación en rampa normal");
}

/// Test 3: Fan debe mantenerse bajo durante calentamiento
#[test]
fn test_heating_phase_fan_behavior() {
    let mut roaster = create_test_roaster();
    let curve = HeatingCurve::new();
    
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    // Simular rampa de temperatura
    for i in 0..=10 {
        let temp = curve.get_temp_at_second(i);
        let _ = roaster.update_temperatures(temp, temp - 10.0, Instant::now());
        roaster.update_control(Instant::now()).unwrap();
        
        let status = roaster.get_status();
        
        // Fan debe mantenerse bajo (0-30%) durante calentamiento
        assert!(
            status.fan_output <= 30.0,
            "Fan debe estar bajo (<=30%) durante calentamiento"
        );
    }
}

/// Test 4: SSR output debe aumentar gradualmente
#[test]
fn test_heating_phase_ssr_ramp() {
    let mut roaster = create_test_roaster();
    let curve = HeatingCurve::new();
    
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    let mut ssr_outputs = vec![];
    
    // Simular rampa de temperatura
    for i in 0..=10 {
        let temp = curve.get_temp_at_second(i);
        let _ = roaster.update_temperatures(temp, temp - 10.0, Instant::now());
        roaster.update_control(Instant::now()).unwrap();
        
        let status = roaster.get_status();
        ssr_outputs.push(status.ssr_output);
    }
    
    // Validar que SSR output aumentó durante rampa
    // (puede no ser monótono si PID modula)
    let initial_avg = ssr_outputs[0..3].iter().sum::<f32>() / 3.0;
    let final_avg = ssr_outputs[7..=10].iter().sum::<f32>() / 4.0;
    
    assert!(
        final_avg >= initial_avg,
        "SSR output debe ser mayor al final de la rampa"
    );
}
