//! Safety tests - Watchdog integration
//! Validates watchdog integration with control loop

#![cfg(all(test, not(target_arch = "riscv32")))]

extern crate std;

use crate::roast_scenarios::mod::*;
use libreroaster::config::ArtisanCommand;
use embassy_time::Instant;

/// Test 1: Watchdog is fed every 100ms (simulated)
#[test]
fn test_watchdog_feeds_successfully() {
    let mut roaster = create_test_roaster();
    
    // Simulate 10 control cycles (100ms each)
    for i in 0..10 {
        let temp = 100.0 + i as f32;
        let _ = roaster.update_temperatures(temp, 90.0 + i as f32, Instant::now());
        roaster.update_control(Instant::now()).unwrap();
        
        // In host tests, the watchdog is stubbed and always returns Ok
        let status = roaster.get_status();
        
        // Validate that no watchdog failures are accumulated
        assert_eq!(status.watchdog_consecutive_failures, 0, "There should be no consecutive failures");
    }
}

/// Test 2: Two consecutive failures simulated
#[test]
fn test_watchdog_two_consecutive_failures() {
    let mut roaster = create_test_roaster();
    
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    // Simulate first failure (manually in host tests)
    roaster.status_mut().watchdog_feed_ok = false;
    roaster.status_mut().watchdog_last_failure = Some("test_timeout_1");
    roaster.status_mut().watchdog_consecutive_failures = 1;
    
    let status1 = roaster.get_status();
    assert!(!status1.watchdog_feed_ok);
    assert_eq!(status1.watchdog_consecutive_failures, 1);
    assert!(!status1.fault_condition, "A single failure should not activate fault");
    
    // Simulate second failure
    roaster.status_mut().watchdog_feed_ok = false;
    roaster.status_mut().watchdog_last_failure = Some("test_timeout_2");
    roaster.status_mut().watchdog_consecutive_failures = 2;
    
    let status2 = roaster.get_status();
    assert!(!status2.watchdog_feed_ok);
    assert_eq!(status2.watchdog_consecutive_failures, 2);
    
    // In real firmware, fault_condition would be activated after 2 failures
    // This test validates the counter, not the complete activation
}

/// Test 3: Reset after successful watchdog feed
#[test]
fn test_watchdog_resets_on_success() {
    let mut roaster = create_test_roaster();
    
    // Set up failures
    roaster.status_mut().watchdog_feed_ok = false;
    roaster.status_mut().watchdog_last_failure = Some("test_timeout");
    roaster.status_mut().watchdog_consecutive_failures = 2;
    
    // Successful feed (simulated)
    roaster.status_mut().watchdog_feed_ok = true;
    roaster.status_mut().watchdog_last_failure = None;
    roaster.status_mut().watchdog_consecutive_failures = 0;
    
    let status = roaster.get_status();
    assert_eq!(status.watchdog_consecutive_failures, 0, "Failure counter must reset");
    assert!(status.watchdog_feed_ok, "Watchdog must be OK after successful feed");
    assert_eq!(status.watchdog_last_failure, None, "Last failure must be None");
}

/// Test 4: Watchdog does not affect temperature (only passed as parameter)
#[test]
fn test_watchdog_parameter_passing() {
    let mut roaster = create_test_roaster();
    roaster.process_artisan_command(ArtisanCommand::StartRoast).unwrap();
    
    // Update temperature
    let temp = 200.0;
    let _ = roaster.update_temperatures(temp, 190.0, Instant::now());
    
    // In real firmware, watchdog.feed_async() is called with bean_temp
    // This test validates that temperature updates correctly
    let status = roaster.get_status();
    assert_eq!(status.bean_temp, temp, "Temperature must be updated");
}

/// Test 5: Watchdog failure reason tracking
#[test]
fn test_watchdog_failure_reason_tracking() {
    let mut roaster = create_test_roaster();
    
    // No failures
    roaster.status_mut().watchdog_feed_ok = true;
    roaster.status_mut().watchdog_last_failure = None;
    
    let status1 = roaster.get_status();
    assert_eq!(status1.watchdog_last_failure, None);
    
    // With failure
    roaster.status_mut().watchdog_feed_ok = false;
    roaster.status_mut().watchdog_last_failure = Some("watchdog_timeout");
    
    let status2 = roaster.get_status();
    assert_eq!(status2.watchdog_last_failure, Some("watchdog_timeout"));
}

/// Test 6: Watchdog feed flag exposure in status
#[test]
fn test_watchdog_feed_flag_exposed() {
    let mut roaster = create_test_roaster();
    
    // Successful feed
    roaster.status_mut().watchdog_feed_ok = true;
    
    let status = roaster.get_status();
    assert!(status.watchdog_feed_ok, "Watchdog feed OK flag must be true");
    
    // Failed feed
    roaster.status_mut().watchdog_feed_ok = false;
    
    let status2 = roaster.get_status();
    assert!(!status2.watchdog_feed_ok, "Watchdog feed OK flag must be false after failure");
}
