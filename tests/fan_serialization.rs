#![cfg(all(test, not(target_arch = "riscv32")))]

extern crate std;

use libreroaster::config::{ArtisanCommand, RoasterState, SystemStatus};
use libreroaster::control::traits::Fan;
use libreroaster::control::{RoasterControl, RoasterError};
use std::boxed::Box;
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
#[path = "common/mod.rs"]
mod tests_common;
use tests_common::{build_test_control, StubHeater};

/// Thread-local tracking of guard state for serialization verification
static GUARD_BUSY: AtomicBool = AtomicBool::new(false);

/// Stub fan that tracks calls for serialization verification
#[derive(Default)]
struct StubFanWithTracking {
    speed: f32,
    call_count: Cell<u32>,
    guard_conflicts: Cell<u32>,
}

impl StubFanWithTracking {
    fn new() -> Self {
        Self::default()
    }
}

impl Fan for StubFanWithTracking {
    fn set_speed(&mut self, duty: f32) -> Result<(), RoasterError> {
        // Check for guard serialization - if already busy, that's a conflict
        if GUARD_BUSY.load(Ordering::Acquire) {
            self.guard_conflicts.set(self.guard_conflicts.get() + 1);
        }

        // Simulate acquiring the LEDC guard
        GUARD_BUSY.store(true, Ordering::Release);

        self.call_count.set(self.call_count.get() + 1);
        self.speed = duty;

        // Simulate releasing the guard
        GUARD_BUSY.store(false, Ordering::Release);

        Ok(())
    }

    fn get_speed(&self) -> f32 {
        self.speed
    }
}

/// Build RoasterControl with tracking fan
fn build_control_with_tracking_fan() -> RoasterControl {
    build_test_control(
        Box::new(StubHeater::new()),
        Box::new(StubFanWithTracking::new()),
    )
}

#[test]
fn fan_telemetry_reflects_applied_duty() {
    let mut control = build_control_with_tracking_fan();

    // Set fan to 50%
    control
        .process_artisan_command(ArtisanCommand::SetFan(50))
        .expect("SetFan should succeed");

    let status = control.get_status();
    // The applied speed should match what was requested (50%)
    // With real LEDC bus, this would be the actual applied percentage
    assert_eq!(status.fan_output, 50.0);

    // Set fan to 75%
    control
        .process_artisan_command(ArtisanCommand::SetFan(75))
        .expect("SetFan should succeed");

    let status = control.get_status();
    assert_eq!(status.fan_output, 75.0);

    // Set fan to 25%
    control
        .process_artisan_command(ArtisanCommand::SetFan(25))
        .expect("SetFan should succeed");

    let status = control.get_status();
    assert_eq!(status.fan_output, 25.0);
}

#[test]
fn fan_output_updated_after_set_speed() {
    let mut control = build_control_with_tracking_fan();

    // Set fan to 60%
    control
        .process_artisan_command(ArtisanCommand::SetFan(60))
        .expect("SetFan should succeed");

    let status = control.get_status();
    // Verify status reports the applied fan output
    assert_eq!(status.fan_output, 60.0);

    // Set to 0%
    control
        .process_artisan_command(ArtisanCommand::SetFan(0))
        .expect("SetFan should succeed");

    let status = control.get_status();
    assert_eq!(status.fan_output, 0.0);
}

#[test]
fn update_control_reports_applied_fan_speed() {
    let mut control = build_control_with_tracking_fan();

    // Start a roast which uses update_control
    control
        .process_artisan_command(ArtisanCommand::StartRoast)
        .expect("StartRoast should succeed");

    // Manually call update_control to simulate the control loop
    use embassy_time::Instant;
    let now = Instant::from_millis(1000);

    // Get initial status
    let initial_status = control.get_status();
    assert_eq!(initial_status.fan_output, 0.0);

    // Set fan manually first
    control
        .process_artisan_command(ArtisanCommand::SetFan(40))
        .expect("SetFan should succeed");

    // Call update_control - it should use the manual fan setting
    let _ = control.update_control(now);

    let status = control.get_status();
    // The fan output should reflect the applied speed (40%)
    assert_eq!(status.fan_output, 40.0);
}

#[test]
fn system_status_fan_output_matches_controller() {
    let mut control = build_control_with_tracking_fan();

    // Test multiple fan speed changes
    for expected_speed in [10.0, 20.0, 30.0, 50.0, 80.0, 100.0] {
        let cmd = ArtisanCommand::SetFan(expected_speed as u8);
        control
            .process_artisan_command(cmd)
            .expect("SetFan should succeed");

        let status = control.get_status();
        assert_eq!(
            status.fan_output, expected_speed,
            "fan_output should match applied speed {}",
            expected_speed
        );
    }
}

#[test]
fn stop_resets_fan_output() {
    let mut control = build_control_with_tracking_fan();

    // Set fan to some value
    control
        .process_artisan_command(ArtisanCommand::SetFan(70))
        .expect("SetFan should succeed");

    let status = control.get_status();
    assert_eq!(status.fan_output, 70.0);

    // Emergency stop should reset fan output
    control
        .process_artisan_command(ArtisanCommand::EmergencyStop)
        .expect("EmergencyStop should succeed");

    let status = control.get_status();
    assert_eq!(status.fan_output, 0.0);
}

#[test]
fn fan_bounds_validation() {
    let mut control = build_control_with_tracking_fan();

    // Invalid value > 100 should fail
    let result = control.process_artisan_command(ArtisanCommand::SetFan(150));
    assert!(matches!(result, Err(RoasterError::InvalidState)));

    // After failed command, status should remain unchanged
    let status = control.get_status();
    assert_eq!(status.fan_output, 0.0);
}
