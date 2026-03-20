//! End-to-end tests - Full roast cycle simulation
//! Valida ciclo completo de tueste con todas las fases

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
    
    // Fase 1: Iniciar tueste
    assert_eq!(roaster.get_state(), RoasterState::Idle);
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    assert_eq!(roaster.get_state(), RoasterState::Heating);
    
    // Fase 2: Calentamiento (usar curva de heating)
    let heating = HeatingCurve::new();
    for i in 0..=10 {
        let temp = heating.get_temp_at_second(i);
        let _ = roaster.update_temperatures(temp, temp - 10.0, Instant::now());
        roaster.update_control(Instant::now()).unwrap();
    }
    
    // Fase 3: Tueste (usar curva de roasting)
    let roasting = RoastingCurve::new();
    for i in 0..5 {
        let temp = roasting.get_temp_at_second(i * 30);
        let _ = roaster.update_temperatures(temp, temp - 20.0, Instant::now());
        roaster.update_control(Instant::now()).unwrap();
    }
    
    // Fase 4: Stop y enfriamiento (usar curva de cooling)
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
    
    // Validar transición final
    assert!(
        matches!(roaster.get_state(), RoasterState::Cooling | RoasterState::Idle),
        "Debe estar en Cooling o Idle al final"
    );
    
    // Validar estado final
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
    
    // Iniciar y calentar
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
    
    // Actualizar temperaturas
    let _ = roaster.update_temperatures(200.0, 190.0, Instant::now());
    
    // READ command (simulado - en realidad READ no es ArtisanCommand, esto es un test de interfaz)
    let status = roaster.get_status();
    
    // Validar que los campos tienen valores razonables
    assert!(status.bean_temp > 0.0, "BT debe ser positivo");
    assert!(status.env_temp > 0.0, "ET debe ser positivo");
    assert!(status.ssr_output >= 0.0 && status.ssr_output <= 100.0, "SSR debe estar en rango");
    assert!(status.fan_output >= 0.0 && status.fan_output <= 100.0, "Fan debe estar en rango");
}

/// Test 6: Temperature validation prevents invalid values
#[test]
fn test_temperature_validation() {
    let mut roaster = create_test_roaster();
    
    // Intentar actualizar con temperatura inválida
    let result = roaster.update_temperatures(400.0, 380.0, Instant::now());
    
    // Debe retornar error por temperatura fuera de rango
    assert!(result.is_err(), "Temperatura fuera de rango debe causar error");
}
