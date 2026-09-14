//! Fault injection scenarios for SOLID-02 verification — Bug M2 rewrite
//! (2026-07-26).
//!
//! The previous harness built a `SystemStatus` FROM the scenario expectations
//! (`status_for_scenario`) and then asserted that the status matched those
//! expectations — a tautology that verified the helper, not production code.
//! These tests now inject real faults into a real `RoasterControl` (stub
//! heater/fan) and assert on the resulting state, exercising the actual
//! production paths: overtemp detection, the STOP/emergency latch, and the
//! host recovery door.
//!
//! Run with: cargo test --test fault_injection_scenarios --features regression --target x86_64-unknown-linux-gnu

#![cfg(all(test, not(target_arch = "riscv32"), feature = "regression"))]

extern crate std;

use embassy_time::Instant;
use libreroaster::common::{StubFan, StubHeater};
use libreroaster::config::RoasterState;
use libreroaster::control::RoasterControl;
use libreroaster::hardware::sensors::SensorConversionHub;
use std::boxed::Box;

/// Build a stub `RoasterControl` for fault-injection scenarios.
fn make_control() -> RoasterControl {
    RoasterControl::new(
        Box::new(StubHeater::new()),
        Box::new(StubFan::new()),
        SensorConversionHub::new(),
    )
    .expect("test control should build")
}

/// Run one sensor-update + control-tick pair at the given instant.
fn tick(ctrl: &mut RoasterControl, bt: f32, et: f32, now: Instant) {
    ctrl.update_temperatures(bt, et, now).expect("temps");
    let _ = ctrl.update_control(now);
}

/// GD-03-class fault: BT above the overtemp threshold must latch an emergency
/// (state Error, fault flag set, heater cut to 0%, fan forced to 100%).
#[test]
fn overtemp_triggers_emergency_shutdown() {
    use libreroaster::config::constants::OVERTEMP_THRESHOLD;

    let mut ctrl = make_control();
    let now = Instant::from_millis(1_000);

    // Inject BT above the overtemp threshold.
    let result = ctrl.update_temperatures(OVERTEMP_THRESHOLD + 10.0, 25.0, now);

    assert!(result.is_err(), "overtemp must return Err");
    assert_eq!(ctrl.get_state(), RoasterState::Error);
    assert!(ctrl.get_status().fault_condition);
    assert_eq!(ctrl.get_status().ssr_output, 0.0);
    assert_eq!(ctrl.get_status().fan_output, 100.0);
}

/// WD-03-class fault: the emergency latch must PERSIST across ticks — a
/// healthy temperature reading after the fault must not silently clear it.
#[test]
fn emergency_latch_persists_across_ticks() {
    use libreroaster::config::constants::OVERTEMP_THRESHOLD;

    let mut ctrl = make_control();
    let t0 = Instant::from_millis(1_000);
    let _ = ctrl.update_temperatures(OVERTEMP_THRESHOLD + 10.0, 25.0, t0);

    // Healthy temps afterwards — the latch must hold.
    let t1 = t0 + embassy_time::Duration::from_millis(200);
    tick(&mut ctrl, 150.0, 25.0, t1);
    assert!(
        ctrl.get_status().fault_condition,
        "latched emergency must survive a healthy tick"
    );
    assert_eq!(ctrl.get_state(), RoasterState::Error);
}

/// V2-1-class recovery: after a latched emergency, the host's OFF (`Stop`)
/// is the sanctioned recovery door and must clear the latch back to Idle.
#[test]
fn stop_recovers_after_emergency() {
    use libreroaster::config::ArtisanCommand;

    let mut ctrl = make_control();
    ctrl.process_artisan_command(ArtisanCommand::EmergencyStop)
        .expect("emergency stop");

    assert!(ctrl.get_status().fault_condition);
    assert_eq!(ctrl.get_state(), RoasterState::Error);

    // OFF clears the latch (unconditional host recovery door).
    ctrl.process_artisan_command(ArtisanCommand::Stop)
        .expect("off");
    assert!(!ctrl.get_status().fault_condition);
    assert_eq!(ctrl.get_state(), RoasterState::Idle);
}

/// CM-03-class fault: a STOP mid-roast arms the cooldown fan and zeroes the
/// heater — the very next tick must not annihilate the cooling airflow.
#[test]
fn stop_arms_cooldown_fan_on_next_tick() {
    use libreroaster::config::ArtisanCommand;

    let mut ctrl = make_control();
    let t0 = Instant::from_millis(1_000);
    tick(&mut ctrl, 205.0, 200.0, t0); // hot roast, near-cooling temperature

    ctrl.process_artisan_command(ArtisanCommand::Stop)
        .expect("stop");

    let t1 = t0 + embassy_time::Duration::from_millis(200);
    tick(&mut ctrl, 204.0, 199.0, t1);
    assert_eq!(
        ctrl.get_status().fan_output,
        100.0,
        "cooldown latch must keep the fan at 100% the tick after STOP"
    );
    assert_eq!(ctrl.get_status().ssr_output, 0.0);
}
