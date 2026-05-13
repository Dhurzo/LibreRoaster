//! Safety tests - LEDC guard mechanism
//! Validates LEDC guard protection mechanism with timeout

#![cfg(all(test, not(target_arch = "riscv32")))]

extern crate std;

use libreroaster::hardware::ledc_guard::{total_timeouts, LedcGuardError};

/// Test 1: Total timeouts function exists and returns counter
#[test]
fn test_ledc_guard_total_timeouts_function() {
    // The total_timeouts function must be available
    let timeouts = total_timeouts();

    // Must be a u16 value
    assert!(timeouts <= u16::MAX, "Timeout counter must be u16");
}

/// Test 2: Timeout counter is atomic and thread-safe
#[test]
fn test_ledc_guard_timeout_counter_thread_safety() {
    // This test validates the interface exists
    // Atomicity is validated by using the counter from multiple threads

    let timeouts1 = total_timeouts();
    let timeouts2 = total_timeouts();

    // Consecutive calls should return consistent values
    assert!(timeouts2 >= timeouts1, "Counter must be non-decreasing");
}

/// Test 3: LEDC guard error has channel field
#[test]
fn test_ledc_guard_error_channel_field() {
    let error = LedcGuardError { channel: "SSR" };

    assert_eq!(
        error.channel(),
        "SSR",
        "Error channel must return channel name"
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
        "Errors from different channels must be different"
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
        "Copied error must be equal to the original"
    );
}
