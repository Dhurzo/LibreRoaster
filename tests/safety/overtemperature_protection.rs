//! Safety tests - Overtemperature protection
//! Valida protección contra sobrecalentamiento

#![cfg(all(test, not(target_arch = "riscv32")))]

extern crate std;

use crate::roast_scenarios::mod::*;
use libreroaster::config::{ArtisanCommand, RoasterState};
use embassy_time::Instant;

/// Test 1: Activación a 260°C
#[test]
fn test_overtemperature_triggers_at_260c() {
    let mut roaster = create_test_roaster();
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    // Llevar temperatura por debajo de threshold
    let _ = roaster.update_temperatures(250.0, 240.0, Instant::now());
    let status = roaster.get_status();
    assert!(!status.fault_condition, "Fault condition no debe estar activo a 250°C");
    
    // Cruzar threshold (OVERTEMP_THRESHOLD = 260.0)
    let _ = roaster.update_temperatures(260.0, 250.0, Instant::now());
    
    let status = roaster.get_status();
    assert!(status.fault_condition, "Debe activar fault_condition a 260°C");
    assert_eq!(status.ssr_output, 0.0, "SSR debe apagarse inmediatamente");
    assert_eq!(status.fan_output, 100.0, "Fan debe ir al 100%");
}

/// Test 2: Estado EmergencyStop tras overtemperature
#[test]
fn test_overtemperature_sets_emergency_state() {
    let mut roaster = create_test_roaster();
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    roaster.process_artisan_command(ArtisanCommand::SetHeater(75)).unwrap();
    
    // Activar overtemperature
    let _ = roaster.update_temperatures(270.0, 260.0, Instant::now());
    
    assert_eq!(roaster.get_state(), RoasterState::EmergencyStop);
    
    let status = roaster.get_status();
    assert!(!status.pid_enabled, "PID debe estar deshabilitado");
    assert!(status.fault_condition, "Fault condition debe estar activo");
}

/// Test 3: Prevención de recuperación sin intervención
#[test]
fn test_overtemperature_requires_manual_reset() {
    let mut roaster = create_test_roaster();
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    // Activar overtemperature
    let _ = roaster.update_temperatures(270.0, 260.0, Instant::now());
    assert!(roaster.get_status().fault_condition);
    
    // Intentar reiniciar con START (debe fallar o no hacer nada)
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    assert!(roaster.get_status().fault_condition, "Fault debe persistir tras overtemperature");
    
    // Debe usar comando RESET para limpiar (simulado aquí manualmente)
    // Nota: El firmware actual puede no tener comando RESET, esto es un comportamiento esperado
}

/// Test 4: No activación por debajo de threshold
#[test]
fn test_overtemperature_no_trigger_below_threshold() {
    let mut roaster = create_test_roaster();
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    // Aumentar temperatura gradualmente hasta justo antes de threshold
    for temp in [200.0, 220.0, 240.0, 259.0] {
        let _ = roaster.update_temperatures(temp, temp - 10.0, Instant::now());
        let status = roaster.get_status();
        assert!(!status.fault_condition, "Fault condition no debe activar debajo de 260°C");
        assert_eq!(roaster.get_state(), RoasterState::Heating, "Debe seguir en Heating");
    }
}

/// Test 5: Fan al máximo en overtemperature
#[test]
fn test_overtemperature_fan_max() {
    let mut roaster = create_test_roaster();
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    // Activar overtemperature
    let _ = roaster.update_temperatures(270.0, 260.0, Instant::now());
    
    let status = roaster.get_status();
    assert_eq!(status.fan_output, 100.0, "Fan debe estar al 100% tras overtemperature");
}

/// Test 6: SSR apagado en overtemperature
#[test]
fn test_overtemperature_ssr_off() {
    let mut roaster = create_test_roaster();
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    roaster.process_artisan_command(ArtisanCommand::SetHeater(75)).unwrap();
    
    // Verificar que SSR estaba encendido
    let status_before = roaster.get_status();
    assert!(status_before.ssr_output > 0.0, "SSR debe estar encendido antes de overtemperature");
    
    // Activar overtemperature
    let _ = roaster.update_temperatures(270.0, 260.0, Instant::now());
    
    let status_after = roaster.get_status();
    assert_eq!(status_after.ssr_output, 0.0, "SSR debe estar apagado tras overtemperature");
}
