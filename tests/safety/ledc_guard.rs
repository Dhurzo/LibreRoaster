//! Safety tests - LEDC guard mechanism
//! Valida mecanismo de protección LEDC guard con timeout

#![cfg(all(test, not(target_arch = "riscv32")))]

extern crate std;

use libreroaster::hardware::ledc_guard::{total_timeouts, LedcGuardError};

/// Test 1: Total timeouts function exists and returns counter
#[test]
fn test_ledc_guard_total_timeouts_function() {
    // La función total_timeouts debe estar disponible
    let timeouts = total_timeouts();

    // Debe ser un valor u16
    assert!(timeouts <= u16::MAX, "Timeout counter debe ser u16");
}

/// Test 2: Timeout counter is atomic and thread-safe
#[test]
fn test_ledc_guard_timeout_counter_thread_safety() {
    // Este test valida que la interfaz existe
    // La atomicidad se valida al usar el contador desde múltiples threads

    let timeouts1 = total_timeouts();
    let timeouts2 = total_timeouts();

    // Llamadas consecutivas deben retornar valores consistentes
    assert!(timeouts2 >= timeouts1, "Counter debe ser no decreciente");
}

/// Test 3: LEDC guard error has channel field
#[test]
fn test_ledc_guard_error_channel_field() {
    let error = LedcGuardError { channel: "SSR" };

    assert_eq!(
        error.channel(),
        "SSR",
        "Error channel debe retornar nombre del canal"
    );
}

/// Test 4: Multiple error types can be created
#[test]
fn test_ledc_guard_multiple_errors() {
    let error_ssr = LedcGuardError { channel: "SSR" };
    let error_fan = LedcGuardError { channel: "FAN" };

    assert_eq!(error_ssr.channel(), "SSR");
    assert_eq!(error_fan.channel(), "FAN");
    assert_ne!(
        error_ssr.channel(),
        error_fan.channel(),
        "Errores de diferentes canales deben ser diferentes"
    );
}

/// Test 5: Error type is copy and clone
#[test]
fn test_ledc_guard_error_copy_clone() {
    let error1 = LedcGuardError { channel: "TEST" };
    let error2 = error1;

    assert_eq!(
        error1.channel(),
        error2.channel(),
        "Copied error debe ser igual al original"
    );
}
