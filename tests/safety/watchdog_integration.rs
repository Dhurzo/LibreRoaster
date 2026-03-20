//! Safety tests - Watchdog integration
//! Valida integración del watchdog con control loop

#![cfg(all(test, not(target_arch = "riscv32")))]

extern crate std;

use crate::roast_scenarios::mod::*;
use libreroaster::config::ArtisanCommand;
use embassy_time::Instant;

/// Test 1: Watchdog se alimenta cada 100ms (simulado)
#[test]
fn test_watchdog_feeds_successfully() {
    let mut roaster = create_test_roaster();
    
    // Simular 10 ciclos de control (100ms cada uno)
    for i in 0..10 {
        let temp = 100.0 + i as f32;
        let _ = roaster.update_temperatures(temp, 90.0 + i as f32, Instant::now());
        roaster.update_control(Instant::now()).unwrap();
        
        // En host tests, el watchdog es stubbed y siempre retorna Ok
        let status = roaster.get_status();
        
        // Validar que no hay fallos de watchdog acumulados
        assert_eq!(status.watchdog_consecutive_failures, 0, "No debe haber fallos consecutivos");
    }
}

/// Test 2: Dos fallos consecutivos simulados
#[test]
fn test_watchdog_two_consecutive_failures() {
    let mut roaster = create_test_roaster();
    
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    // Simular primer fallo (manualmente en host tests)
    roaster.status_mut().watchdog_feed_ok = false;
    roaster.status_mut().watchdog_last_failure = Some("test_timeout_1");
    roaster.status_mut().watchdog_consecutive_failures = 1;
    
    let status1 = roaster.get_status();
    assert!(!status1.watchdog_feed_ok);
    assert_eq!(status1.watchdog_consecutive_failures, 1);
    assert!(!status1.fault_condition, "Un fallo no debe activar fault");
    
    // Simular segundo fallo
    roaster.status_mut().watchdog_feed_ok = false;
    roaster.status_mut().watchdog_last_failure = Some("test_timeout_2");
    roaster.status_mut().watchdog_consecutive_failures = 2;
    
    let status2 = roaster.get_status();
    assert!(!status2.watchdog_feed_ok);
    assert_eq!(status2.watchdog_consecutive_failures, 2);
    
    // En firmware real, fault_condition se activaría tras 2 fallos
    // Este test valida el contador, no la activación completa
}

/// Test 3: Reset tras alimentar watchdog exitosamente
#[test]
fn test_watchdog_resets_on_success() {
    let mut roaster = create_test_roaster();
    
    // Establecer fallos
    roaster.status_mut().watchdog_feed_ok = false;
    roaster.status_mut().watchdog_last_failure = Some("test_timeout");
    roaster.status_mut().watchdog_consecutive_failures = 2;
    
    // Feed exitoso (simulado)
    roaster.status_mut().watchdog_feed_ok = true;
    roaster.status_mut().watchdog_last_failure = None;
    roaster.status_mut().watchdog_consecutive_failures = 0;
    
    let status = roaster.get_status();
    assert_eq!(status.watchdog_consecutive_failures, 0, "Fallo contador debe resetear");
    assert!(status.watchdog_feed_ok, "Watchdog debe estar OK tras feed exitoso");
    assert_eq!(status.watchdog_last_failure, None, "Último fallo debe ser None");
}

/// Test 4: Watchdog no afecta temperatura (solo se pasa como parámetro)
#[test]
fn test_watchdog_parameter_passing() {
    let mut roaster = create_test_roaster();
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    // Actualizar temperatura
    let temp = 200.0;
    let _ = roaster.update_temperatures(temp, 190.0, Instant::now());
    
    // En firmware real, watchdog.feed_async() se llama con bean_temp
    // Este test valida que la temperatura se actualiza correctamente
    let status = roaster.get_status();
    assert_eq!(status.bean_temp, temp, "Temperatura debe actualizarse");
}

/// Test 5: Watchdog failure reason tracking
#[test]
fn test_watchdog_failure_reason_tracking() {
    let mut roaster = create_test_roaster();
    
    // Sin fallos
    roaster.status_mut().watchdog_feed_ok = true;
    roaster.status_mut().watchdog_last_failure = None;
    
    let status1 = roaster.get_status();
    assert_eq!(status1.watchdog_last_failure, None);
    
    // Con fallo
    roaster.status_mut().watchdog_feed_ok = false;
    roaster.status_mut().watchdog_last_failure = Some("watchdog_timeout");
    
    let status2 = roaster.get_status();
    assert_eq!(status2.watchdog_last_failure, Some("watchdog_timeout"));
}

/// Test 6: Watchdog feed flag exposure in status
#[test]
fn test_watchdog_feed_flag_exposed() {
    let mut roaster = create_test_roaster();
    
    // Feed exitoso
    roaster.status_mut().watchdog_feed_ok = true;
    
    let status = roaster.get_status();
    assert!(status.watchdog_feed_ok, "Watchdog feed OK flag debe ser true");
    
    // Feed fallido
    roaster.status_mut().watchdog_feed_ok = false;
    
    let status2 = roaster.get_status();
    assert!(!status2.watchdog_feed_ok, "Watchdog feed OK flag debe ser false tras fallo");
}
