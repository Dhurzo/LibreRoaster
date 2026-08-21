#![cfg(all(test, feature = "test", not(target_arch = "riscv32")))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

extern crate alloc;
extern crate std;

use std::boxed::Box;
use std::sync::Mutex;

use embassy_time::{Duration, Instant};

use libreroaster::common::{StubFan, StubHeater};
use libreroaster::config::constants::{RoasterState, COMMS_IDLE_TIMEOUT_MS, MAX_ROAST_TIME_SECS};
use libreroaster::config::{ArtisanCommand, ProfileSetpoint, RoastProfile, SystemStatus};
use libreroaster::control::roaster_control::RoasterControl;
use libreroaster::hardware::sensors::SensorConversionHub;

static TEST_MUTEX: Mutex<()> = Mutex::new(());

fn acquire_lock() -> std::sync::MutexGuard<'static, ()> {
    let guard = TEST_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    TEST_MUTEX.clear_poison();
    guard
}

fn build_control() -> RoasterControl {
    RoasterControl::new(
        Box::new(StubHeater::new()),
        Box::new(StubFan::new()),
        SensorConversionHub::new(),
    )
    .expect("RoasterControl should build")
}

/// Single tick: set temps + update control at the same instant (avoids stale-data timeout).
fn tick_now(ctrl: &mut RoasterControl, bt: f32, et: f32) -> f32 {
    let now = Instant::now();
    ctrl.update_temperatures(bt, et, now).expect("temps");
    ctrl.update_control(now).expect("update")
}

/// Single tick at an explicit instant (for deterministic clock advancement).
fn tick_now_at(ctrl: &mut RoasterControl, bt: f32, et: f32, now: Instant) -> f32 {
    ctrl.update_temperatures(bt, et, now).expect("temps");
    ctrl.update_control(now).expect("update")
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. CHARGE DETECTION RESET BETWEEN ROASTS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn charge_detection_resets_after_stop_roast() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    tick_now(&mut ctrl, 25.0, 30.0);
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");

    tick_now(&mut ctrl, 180.0, 200.0);

    ctrl.process_artisan_command(ArtisanCommand::Stop)
        .expect("stop");

    let status = ctrl.get_status();
    assert!(
        !status.charge_detected,
        "charge_detected must reset after STOP"
    );
}

#[test]
fn charge_detection_works_on_second_roast() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    tick_now(&mut ctrl, 25.0, 30.0);
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("roast 1");
    ctrl.process_artisan_command(ArtisanCommand::Stop)
        .expect("stop 1");

    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("roast 2");

    // Single tick minimizes real-time gap between StartRoast (which sets
    // last_command_received_at_ms = Instant::now()) and the control tick.
    // The comms idle check compares Instant::now()-based timestamps and
    // can trigger spuriously if multiple ticks accumulate real-world delay.
    tick_now(&mut ctrl, 180.0, 200.0);

    assert!(
        !ctrl.safety().is_emergency_active(),
        "Second roast should not have emergency"
    );
    assert_eq!(ctrl.get_state(), RoasterState::Heating);
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. MAX ROAST TIME SAFETY BACKSTOP
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn max_roast_time_triggers_emergency_shutdown() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    tick_now(&mut ctrl, 150.0, 180.0);
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");

    // Jump far into the future — profile_start_time was set at start,
    // now elapsed_secs exceeds MAX_ROAST_TIME_SECS
    let far_future_ms = (MAX_ROAST_TIME_SECS as u64 + 60) * 1000;
    let far_future = Instant::now() + Duration::from_millis(far_future_ms);

    // Use tick_with_stale_sensor: sensor at t=0, control at far_future.
    // This will trigger both stale sensor and max roast time.
    ctrl.update_temperatures(200.0, 220.0, far_future)
        .expect("temps");
    let _ = ctrl.update_control(far_future);

    // Audit MT-1 (2026-08-11): the previous
    // `is_emergency_active() || fault_condition` assert was tautological —
    // `emergency_shutdown()` latches *both* facts atomically, so the OR
    // added no discriminating power and a *different* emergency (e.g. a
    // plain stale-sensor trip masking a broken MAX_ROAST_TIME gate) would
    // have passed silently. The strong form requires both: the latched
    // emergency AND the Error state (`emergency_shutdown` always sets
    // both). This deliberately-constructed `far_future` tick genuinely
    // trips overlapping backstops (stale sensor + max-roast); the
    // isolation of each mechanism is provided by the *negative* test
    // `roast_within_max_time_does_not_trigger_emergency` below plus the
    // dedicated stale-sensor test (separator 7).
    assert!(
        ctrl.safety().is_emergency_active()
            && ctrl.get_status().state == libreroaster::config::RoasterState::Error,
        "Roast exceeding MAX_ROAST_TIME_SECS should latch emergency + state==Error"
    );
}

#[test]
fn roast_within_max_time_does_not_trigger_emergency() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    tick_now(&mut ctrl, 150.0, 180.0);
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");

    // Immediately tick — well within limits, same instant so no stale data
    tick_now(&mut ctrl, 200.0, 220.0);

    assert!(
        !ctrl.safety().is_emergency_active(),
        "Roast within MAX_ROAST_TIME should not trigger emergency"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. COMMS IDLE TIMEOUT SAFETY
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn comms_idle_timeout_triggers_emergency_during_roast() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    tick_now(&mut ctrl, 150.0, 180.0);
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");

    // last_command_received_at_ms was set at start (Instant::now().as_millis()).
    // We advance millis-since-boot by COMMS_IDLE_TIMEOUT_MS + 5000.
    let idle_ms = COMMS_IDLE_TIMEOUT_MS + 5000;
    let future = Instant::now() + Duration::from_millis(idle_ms);
    ctrl.update_temperatures(200.0, 220.0, future)
        .expect("temps");
    let _ = ctrl.update_control(future);

    assert!(
        ctrl.safety().is_emergency_active()
            && ctrl.get_status().state == libreroaster::config::RoasterState::Error,
        "Comms idle timeout should latch emergency + state==Error during Heating state"
    );
}

#[test]
fn comms_idle_timeout_does_not_trigger_when_idle() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    let idle_ms = COMMS_IDLE_TIMEOUT_MS + 5000;
    let future = Instant::now() + Duration::from_millis(idle_ms);
    ctrl.update_temperatures(25.0, 30.0, future).expect("temps");
    let _ = ctrl.update_control(future);

    assert!(
        !ctrl.safety().is_emergency_active(),
        "Comms idle should NOT trigger emergency when Idle"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. FAULT RECOVERY: STOP AFTER EMERGENCY
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn full_emergency_recovery_cycle() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    tick_now(&mut ctrl, 150.0, 180.0);
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");

    ctrl.emergency_shutdown("test").expect_err("emergency");

    assert_eq!(ctrl.get_state(), RoasterState::Error);
    assert!(ctrl.get_status().fault_condition);
    assert!(ctrl.safety().is_emergency_active());

    // Bug C3 doctrine (2026-07-25): `ArtisanCommand::Stop` (token "OFF") is the
    // *unconditional* recovery door for the host. A latched emergency with no
    // reachable recovery used to brick the device until a power cycle, because
    // `RoasterCommand::StopRoast` (the only un-latch path) had no producer in
    // production code. OFF now clears the latch first and then runs the
    // normal stop, so the device always returns to `Idle` on the visible
    // Artisan button.
    ctrl.process_artisan_command(ArtisanCommand::Stop)
        .expect("OFF is the recovery path; must not panic");

    // After OFF: latch released, fault cleared, state back to Idle.
    assert!(
        !ctrl.safety().is_emergency_active(),
        "OFF must release the emergency latch (host recovery route)"
    );
    assert!(
        !ctrl.get_status().fault_condition,
        "OFF must clear the fault flag (host recovery route)"
    );
    assert_eq!(ctrl.get_state(), RoasterState::Idle);

    tick_now(&mut ctrl, 25.0, 30.0);
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("new roast after recovery");

    assert_eq!(ctrl.get_state(), RoasterState::Heating);
}

#[test]
fn fault_condition_blocks_heater_but_allows_read() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    tick_now(&mut ctrl, 150.0, 180.0);
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");
    ctrl.emergency_shutdown("test").expect_err("emergency");

    assert!(
        ctrl.process_artisan_command(ArtisanCommand::SetHeater(50))
            .is_err(),
        "Heater must be rejected during fault"
    );
    assert!(
        ctrl.process_artisan_command(ArtisanCommand::ReadStatus)
            .is_ok(),
        "READ must work during fault"
    );
    assert!(
        ctrl.process_artisan_command(ArtisanCommand::EmergencyStop)
            .is_ok(),
        "EmergencyStop must work during fault"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. MODE TRANSITIONS: PID → MANUAL → PID
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn pid_to_manual_back_to_pid_resets_integrator() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    tick_now(&mut ctrl, 25.0, 30.0);
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start with PID");
    assert!(
        ctrl.get_status().pid_enabled,
        "PID should be on after start"
    );

    // Drive the integrator with a target *close* to the current temperature,
    // so the PID is NOT pinned at the output_max rail. Bug B6 conditional
    // anti-windup now stops integrating once the predictive MV hits the
    // controller's own clamp; the previous test used DEFAULT_TARGET_TEMP with
    // BT=25 → error ≈ 200 → P-term alone pins MV to 100% → integrator is
    // (correctly) held at 0 by anti-windup. We instead use a small error so
    // MV stays inside [0,100] and the integrator actually accumulates. Target
    // must be inside `is_valid_target_temp`'s 50..=300 °C window.
    ctrl.process_artisan_command(ArtisanCommand::SetTargetTemp(60.0))
        .expect("close target");
    for _ in 0..5 {
        tick_now(&mut ctrl, 25.0, 30.0);
    }

    let integrator_before_manual = ctrl.get_status().integrator_value;
    assert!(
        integrator_before_manual > 0.0,
        "Integrator should have accumulated during PID mode with a small error (B6 anti-windup holds it when pinned to the rail), got {}",
        integrator_before_manual
    );

    // SSR cycle guard is 100ms. Sleep past it so the manual command is accepted.
    std::thread::sleep(std::time::Duration::from_millis(150));
    ctrl.process_artisan_command(ArtisanCommand::SetHeater(50))
        .expect("manual");
    assert!(ctrl.get_status().artisan_control);

    // Re-enable PID — this calls enable() which resets integrator to 0.0
    std::thread::sleep(std::time::Duration::from_millis(150));
    ctrl.process_artisan_command(ArtisanCommand::SetTargetTemp(200.0))
        .expect("re-enable PID");
    assert!(ctrl.get_status().pid_enabled);

    // Verify integrator was reset: read directly from the PID controller
    // (not from status, which is only updated on update_control tick)
    let integrator_after_reenable = ctrl.dispatch().pid_integrator_value();
    assert!(
        integrator_after_reenable.abs() < 0.001,
        "Integrator should be zero immediately after PID re-enable, got {}",
        integrator_after_reenable
    );
}

#[test]
fn manual_heater_then_stop_clears_manual_state() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    tick_now(&mut ctrl, 150.0, 180.0);
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");
    ctrl.process_artisan_command(ArtisanCommand::SetHeater(80))
        .expect("manual heater");

    tick_now(&mut ctrl, 150.0, 180.0);
    assert!(ctrl.get_status().artisan_control);

    ctrl.process_artisan_command(ArtisanCommand::Stop)
        .expect("stop");

    let status = ctrl.get_status();
    assert!(!status.artisan_control);
    assert!(!status.pid_enabled);
    assert_eq!(status.ssr_output, 0.0);
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. SSR SLEW RATE LIMITING
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn ssr_slew_rate_limits_rapid_increase() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    tick_now(&mut ctrl, 150.0, 180.0);
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");
    ctrl.process_artisan_command(ArtisanCommand::SetHeater(100))
        .expect("heater 100%");

    let output_t0 = tick_now(&mut ctrl, 150.0, 180.0);
    let output_t1 = tick_now(&mut ctrl, 150.0, 180.0);

    // Each tick is ~0ms apart (Instant::now()), so slew should be 0 or very small
    // If ticks were 200ms apart: max step = 50.0 * 0.2 = 10.0%
    let delta = (output_t1 - output_t0).abs();
    assert!(
        delta <= 15.0,
        "SSR slew Δ={:.1}% between consecutive ticks should be bounded",
        delta
    );
}

#[test]
fn ssr_immediate_zero_on_stop() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    tick_now(&mut ctrl, 150.0, 180.0);
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");
    ctrl.process_artisan_command(ArtisanCommand::SetHeater(80))
        .expect("heater 80%");

    for _ in 0..5 {
        tick_now(&mut ctrl, 150.0, 180.0);
    }

    ctrl.process_artisan_command(ArtisanCommand::Stop)
        .expect("stop");

    assert_eq!(
        ctrl.get_status().ssr_output,
        0.0,
        "SSR must go to 0 immediately on STOP"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. SENSOR TIMEOUT EMERGENCY
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn stale_sensor_reading_triggers_emergency() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    // Use a single time source to avoid Instant::now() drift issues.
    // Set all times relative to a fixed future point so embassy duration_since
    // never sees a negative delta.
    let base = Instant::now() + Duration::from_secs(10);

    ctrl.update_temperatures(150.0, 180.0, base).expect("temps");
    // StartRoast calls Instant::now() internally for profile_start_time and
    // last_command_received_at_ms. We cannot control those, but we CAN ensure
    // our control_time is far enough in the future that duration_since works.
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");
    ctrl.update_control(base).expect("first tick ok");

    // Advance 2 seconds WITHOUT updating temperatures → stale > 1000ms.
    let t_stale = base + Duration::from_secs(2);
    let _ = ctrl.update_control(t_stale);

    assert!(
        ctrl.safety().is_emergency_active()
            && ctrl.get_status().state == libreroaster::config::RoasterState::Error,
        "Stale sensor data (>1000ms) should latch emergency + state==Error during roast"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 8. PROFILE-FOLLOWING
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn profile_target_tracks_setpoints_over_time() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    let profile = RoastProfile {
        setpoints: heapless::Vec::from_slice(&[
            ProfileSetpoint {
                time_secs: 0,
                temperature: 100.0,
            },
            ProfileSetpoint {
                time_secs: 60,
                temperature: 150.0,
            },
            ProfileSetpoint {
                time_secs: 120,
                temperature: 200.0,
            },
        ])
        .expect("setpoints"),
    };
    libreroaster::input::parser::store_profile(profile);
    ctrl.process_artisan_command(ArtisanCommand::SetProfile)
        .expect("set profile");

    tick_now(&mut ctrl, 100.0, 120.0);
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");

    let status = ctrl.get_status();
    assert!(
        (status.target_temp - 100.0).abs() < 1.0,
        "Initial target should be ~100°C from profile, got {}",
        status.target_temp
    );
}

#[test]
fn profile_with_invalid_temperature_is_rejected() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    let bad_profile = RoastProfile {
        setpoints: heapless::Vec::from_slice(&[
            ProfileSetpoint {
                time_secs: 0,
                temperature: 50.0,
            },
            ProfileSetpoint {
                time_secs: 60,
                temperature: 999.0,
            },
        ])
        .expect("setpoints"),
    };
    libreroaster::input::parser::store_profile(bad_profile);
    let result = ctrl.process_artisan_command(ArtisanCommand::SetProfile);
    assert!(
        result.is_err(),
        "Profile with temp=999°C should be rejected"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 9. OVERTEMP EMERGENCY
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn overtemp_bt_triggers_emergency() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    let result = ctrl.update_temperatures(300.0, 25.0, Instant::now());
    assert!(result.is_err());
    assert_eq!(ctrl.get_state(), RoasterState::Error);
    assert!(ctrl.get_status().fault_condition);
}

#[test]
fn overtemp_et_triggers_emergency() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    let result = ctrl.update_temperatures(25.0, 270.0, Instant::now());
    assert!(result.is_err());
    assert!(ctrl.get_status().fault_condition);
}

#[test]
fn exactly_at_overtemp_threshold_triggers_emergency() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    let result = ctrl.update_temperatures(
        libreroaster::config::constants::OVERTEMP_THRESHOLD,
        25.0,
        Instant::now(),
    );
    assert!(
        result.is_err(),
        "OVERTEMP_THRESHOLD should trigger emergency"
    );
}

#[test]
fn overtemp_just_below_threshold_does_not_trigger() {
    // Audit MT-2 (2026-08-11): the overtemp suite only covered `>=`
    // OVERTEMP_THRESHOLD. Add the negative boundary: `threshold - 1.0` must
    // NOT trip (severity is decided at the `>=` boundary in
    // controllers/sensor.rs), and repeated sub-threshold samples must not
    // accumulate into a trip.
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    let below = libreroaster::config::constants::OVERTEMP_THRESHOLD - 1.0;
    for _ in 0..8 {
        let result = ctrl.update_temperatures(below, 200.0, Instant::now());
        assert!(
            result.is_ok(),
            "temperature {below}°C (1° below the cutoff) must NOT trip"
        );
        assert!(
            !ctrl.get_status().fault_condition,
            "no fault may accumulate from sub-threshold samples"
        );
        assert_ne!(
            ctrl.get_state(),
            RoasterState::Error,
            "state must not latch to Error below the cutoff"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 10. SSR NOT DETECTED → ZERO OUTPUT
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn ssr_not_detected_forces_zero_output_in_manual_mode() {
    let _guard = acquire_lock();
    let heater = StubHeater::new();
    let fan = StubFan::new();
    // Audit H-8 (2026-08-11): the original `if status.ssr_hardware_status !=
    // Available` guard made the assert unreachable (StubHeater defaults to
    // Available), so the "stuck/unknown SSR must gate output to zero" rule
    // never ran. Mirror T6 in safety_injection_midroast_tests.rs: force Error
    // (BEFORE the move into RoasterControl), assert unconditionally.
    heater.set_status(libreroaster::config::constants::SsrHardwareStatus::Error);
    let mut ctrl = RoasterControl::new(Box::new(heater), Box::new(fan), SensorConversionHub::new())
        .expect("build");

    tick_now(&mut ctrl, 150.0, 180.0);
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");
    ctrl.process_artisan_command(ArtisanCommand::SetHeater(80))
        .expect("manual 80%");

    tick_now(&mut ctrl, 150.0, 180.0);

    let status = ctrl.get_status();
    assert_eq!(
        status.ssr_hardware_status,
        libreroaster::config::constants::SsrHardwareStatus::Error,
        "precondition: SSR must be reported as Error after set_status"
    );
    assert_eq!(
        status.ssr_output, 0.0,
        "manual control with SSR not Available must output 0 %"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 11. PREHEAT FLOW
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn preheat_enters_correct_state_with_target() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    ctrl.process_artisan_command(ArtisanCommand::Preheat(200.0))
        .expect("preheat");

    assert_eq!(ctrl.get_state(), RoasterState::Preheating);
    assert_eq!(ctrl.get_status().target_temp, 200.0);
    assert!(ctrl.get_status().pid_enabled);
}

#[test]
fn preheat_disables_continuous_output() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    ctrl.process_artisan_command(ArtisanCommand::Preheat(180.0))
        .expect("preheat");

    assert!(
        !ctrl.get_output_manager().is_continuous_enabled(),
        "Preheat should disable continuous output"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 12. UP/DOWN HEATER COMMANDS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn up_command_increments_heater() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    tick_now(&mut ctrl, 150.0, 180.0);

    // Sleep to clear SSR guard before StartRoast
    std::thread::sleep(std::time::Duration::from_millis(150));
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");

    // Sleep past SSR guard so SetHeater is accepted.
    std::thread::sleep(std::time::Duration::from_millis(150));
    ctrl.process_artisan_command(ArtisanCommand::SetHeater(50))
        .expect("base 50%");

    // Bug #8 fix: SetHeater stores manual_heater=50 (ssr_output is
    // slew-rate-limited separately). UP must use manual_heater as the
    // baseline — NOT ssr_output — so the operator's manual setting is
    // honoured regardless of where the slew limiter currently is.
    let manual_before_up = ctrl.dispatch().artisan_manual_heater();
    assert_eq!(
        manual_before_up, 50.0,
        "manual_heater should be exactly 50 after SetHeater(50)"
    );
    assert!(
        ctrl.get_status().ssr_output < 50.0,
        "SSR output should be slew-limited below 50"
    );

    std::thread::sleep(std::time::Duration::from_millis(150));
    ctrl.process_artisan_command(ArtisanCommand::IncreaseHeater)
        .expect("UP");

    let manual_after_up = ctrl.dispatch().artisan_manual_heater();
    assert!(
        (manual_after_up - manual_before_up - 5.0).abs() < 0.1,
        "UP should add 5 to manual_heater: before={:.1}, after={:.1}",
        manual_before_up,
        manual_after_up
    );
    assert_eq!(
        manual_after_up, 55.0,
        "manual_heater must end at exactly 55 after UP from 50"
    );
}

#[test]
fn down_command_at_zero_stays_at_zero() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    tick_now(&mut ctrl, 150.0, 180.0);
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");
    ctrl.process_artisan_command(ArtisanCommand::DecreaseHeater)
        .expect("DOWN from 0");

    let manual_heater = ctrl.dispatch().artisan_manual_heater();
    assert_eq!(manual_heater, 0.0);
}

// ═══════════════════════════════════════════════════════════════════════════
// 13. PID CHANNEL SELECTION
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn pid_channel_1_uses_env_temp_as_pv() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    ctrl.process_artisan_command(ArtisanCommand::SetPidChannel(1))
        .expect("ch1");
    tick_now(&mut ctrl, 150.0, 200.0);
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");
    tick_now(&mut ctrl, 150.0, 200.0);

    let status = ctrl.get_status();
    assert!(
        (status.pv - 200.0).abs() < 0.1,
        "PV should be ET (200.0) when channel=1, got {}",
        status.pv
    );
}

#[test]
fn pid_channel_2_uses_bean_temp_as_pv() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    tick_now(&mut ctrl, 150.0, 200.0);
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");
    tick_now(&mut ctrl, 150.0, 200.0);

    let status = ctrl.get_status();
    assert!(
        (status.pv - 150.0).abs() < 0.1,
        "PV should be BT (150.0) when channel=2, got {}",
        status.pv
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 14. EMERGENCY FAN BEHAVIOR
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn emergency_shutdown_sets_fan_to_100_percent() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    ctrl.emergency_shutdown("test").expect_err("emergency");

    assert!(
        ctrl.get_status().fan_output >= 99.0,
        "Fan must go to 100% during emergency, got {}",
        ctrl.get_status().fan_output
    );
}

#[test]
fn stop_sets_fan_to_100_percent_for_cooling() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    tick_now(&mut ctrl, 150.0, 180.0);
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");
    ctrl.process_artisan_command(ArtisanCommand::SetFan(30))
        .expect("fan 30%");
    tick_now(&mut ctrl, 150.0, 180.0);
    ctrl.process_artisan_command(ArtisanCommand::Stop)
        .expect("stop");

    assert!(ctrl.get_status().fan_output >= 99.0);
}

// ═══════════════════════════════════════════════════════════════════════════
// 14b. BUG B3 — Cooldown fan latch survives the next update_control tick
// ═══════════════════════════════════════════════════════════════════════════
// The pre-existing `stop_sets_fan_to_100_percent_for_cooling` test never ran
// a tick after STOP, so it passed even though `update_control` immediately
// overwrote the fan to 0% via `artisan_manual_fan()` (cleared by STOP's
// `clear_manual`). This test runs the tick and asserts the cooldown latch.

#[test]
fn stop_cooldown_fan_survives_next_tick() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    // Hold BT stable at a hot value across all ticks so the rate-of-rise
    // guard stays at 0 °C/s and never arms an emergency. (A 25→200 °C jump
    // in a single tick produces a hundreds-thousands °C/s derivative that
    // counts up `ror_exceeded` over ticks and shadows the cooldown latch
    // with an emergency — which also forces fan=100% for the wrong reason.)
    tick_now(&mut ctrl, 200.0, 220.0);
    tick_now(&mut ctrl, 200.0, 220.0);
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");
    tick_now(&mut ctrl, 200.0, 220.0);
    ctrl.process_artisan_command(ArtisanCommand::SetFan(30))
        .expect("fan 30%");
    tick_now(&mut ctrl, 200.0, 220.0);
    // Confirm no emergency was armed by the BT spike — prove the latch, not a
    // latched emergency, is what's keeping the fan at 100%.
    assert!(
        !ctrl.safety().is_emergency_active(),
        "precondition: no emergency armed by BT warm-up"
    );
    ctrl.process_artisan_command(ArtisanCommand::Stop)
        .expect("stop");
    assert!(
        ctrl.get_status().fan_output >= 99.0,
        "STOP must set fan to 100% (immediate effect)"
    );

    // The single tick that the original test omitted — this is where B3 lived.
    tick_now(&mut ctrl, 200.0, 220.0);
    assert!(
        !ctrl.safety().is_emergency_active(),
        "B3 must hold fan: emergency not armed, latch is the active mechanism"
    );
    assert!(
        ctrl.get_status().fan_output >= 99.0,
        "Cooldown fan latch (B3): fan must stay at 100% on the tick after STOP, got {}",
        ctrl.get_status().fan_output
    );

    // And must keep holding for subsequent ticks while BT stays hot.
    tick_now(&mut ctrl, 200.0, 220.0);
    assert!(
        !ctrl.safety().is_emergency_active(),
        "B3 must persist: still no emergency across subsequent hot ticks"
    );
    assert!(
        ctrl.get_status().fan_output >= 99.0,
        "Cooldown fan latch (B3) must persist across ticks while BT is hot"
    );
}

#[test]
fn cooldown_latch_releases_when_bean_cools_below_threshold() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    // Hold BT hot and stable so no rate-of-rise emergency fires (see
    // `stop_cooldown_fan_survives_next_tick` for the spike rationale).
    tick_now(&mut ctrl, 200.0, 220.0);
    tick_now(&mut ctrl, 200.0, 220.0);
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");
    tick_now(&mut ctrl, 200.0, 220.0);
    ctrl.process_artisan_command(ArtisanCommand::Stop)
        .expect("stop");
    // Hot BT keeps the latch armed.
    tick_now(&mut ctrl, 200.0, 220.0);
    assert!(
        !ctrl.safety().is_emergency_active(),
        "precondition: latch (not emergency) holds the cooldown fan"
    );
    assert!(ctrl.get_status().fan_output >= 99.0);

    // Operator's manual fan setting is in effect once the beans cool.
    ctrl.process_artisan_command(ArtisanCommand::SetFan(40))
        .expect("fan 40%");
    // Big drop in one tick would also spike the roRate. Drop gradually:
    // 200 → 130 → 70 → 50 keeps each step under the guard's threshold.
    tick_now(&mut ctrl, 130.0, 150.0);
    assert!(
        ctrl.get_status().fan_output >= 99.0,
        "BT=130 is still above 60°C — latch stays armed, got {}",
        ctrl.get_status().fan_output
    );
    tick_now(&mut ctrl, 70.0, 80.0);
    assert!(
        ctrl.get_status().fan_output >= 99.0,
        "BT=70 is still above 60°C — latch stays armed, got {}",
        ctrl.get_status().fan_output
    );
    tick_now(&mut ctrl, 50.0, 60.0);
    assert!(
        !ctrl.safety().is_emergency_active(),
        "B3 release: emergency must NOT be armed after cooling below threshold"
    );
    assert!(
        ctrl.get_status().fan_output <= 41.0 && ctrl.get_status().fan_output >= 39.0,
        "Cooldown latch (B3) must release when BT < 60°C, got {}",
        ctrl.get_status().fan_output
    );
}

#[test]
fn start_roast_drops_cooldown_latch() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    // Keep BT stable and hot throughout so the rate-of-rise guard stays at 0
    // and never arms an emergency that would shadow the cooldown behaviour.
    tick_now(&mut ctrl, 200.0, 220.0);
    tick_now(&mut ctrl, 200.0, 220.0);
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start 1");
    tick_now(&mut ctrl, 200.0, 220.0);
    assert!(
        !ctrl.safety().is_emergency_active(),
        "precondition: no emergency armed during first roast"
    );
    ctrl.process_artisan_command(ArtisanCommand::Stop)
        .expect("stop 1");
    tick_now(&mut ctrl, 200.0, 220.0);
    assert!(
        !ctrl.safety().is_emergency_active(),
        "post-STOP cooldown must not be confused with an emergency"
    );
    assert!(ctrl.get_status().fan_output >= 99.0, "cooldown armed");

    // New roast start overrides the cooldown latch. We must NOT issue any
    // manual command before START2 (the cooldown state owns the fan); the
    // latch drop must come purely from the START2 path.
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start 2");
    tick_now(&mut ctrl, 200.0, 220.0);
    assert!(
        !ctrl.safety().is_emergency_active(),
        "B3: START2 must drop cooldown without arming emergency, got emergency=true"
    );
    assert!(
        ctrl.get_status().fan_output < 99.0,
        "Cooldown latch (B3) must drop on new roast start, got {}",
        ctrl.get_status().fan_output
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 14c. BUG B4 — OT2 (fan) command must NOT disable PID or drop the heater
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn fan_command_does_not_disable_pid() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    tick_now(&mut ctrl, 25.0, 30.0);
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");
    tick_now(&mut ctrl, 180.0, 200.0);

    let pid_enabled_before = ctrl.get_status().pid_enabled;
    assert!(pid_enabled_before, "PID should be on after START");

    // SetFan maps to OT2/IO3 in Artisan's protocol (manual fan).
    ctrl.process_artisan_command(ArtisanCommand::SetFan(50))
        .expect("fan 50%");
    tick_now(&mut ctrl, 180.0, 200.0);

    let status = ctrl.get_status();
    assert!(
        status.pid_enabled,
        "B4: fan command must NOT disable PID (Spec F4.8), got pid_enabled=false"
    );
    assert!(
        !status.artisan_control,
        "B4: fan command must NOT flip artisan_control to true, got true"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 14d. BUG B5 — Tuning PID gains mid-roast must NOT silently disable the PID
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn set_pid_gains_keeps_pid_enabled() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    // Bug fix (2026-08-10): `tick_now` uses REAL wall-clock time and the host
    // time driver has 1 µs resolution, so two consecutive ticks can share a
    // tick (slew dt == 0 → applied output 0) and `update_pid_control` only
    // recomputes once `pid_cycle_time_ms` (100 ms) has elapsed since the last
    // computation. Both effects made the final `mv > 0` assert flaky depending
    // on scheduler timing. Drive the clock explicitly (+200 ms per tick) so
    // the slew always advances and the PID cycle gate is deterministically
    // open.
    let t0 = Instant::now();
    tick_now_at(&mut ctrl, 25.0, 30.0, t0);
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");
    let t1 = t0 + Duration::from_millis(200);
    tick_now_at(&mut ctrl, 180.0, 200.0, t1);

    assert!(ctrl.get_status().pid_enabled, "PID on after START");

    // Artisan PID dialog: PID;T;kp;ki;kd → SetPidGain.
    ctrl.process_artisan_command(ArtisanCommand::SetPidGain(2.5, 0.3, 0.08))
        .expect("pid gains");

    // The bug: status.pid_enabled stayed true but the controller was rebuilt
    // with enabled=false, so compute_output returned 0.0. A tick must show
    // the PID actively computing (non-zero desired output when below target).
    assert!(
        ctrl.get_status().pid_enabled,
        "B5: SetPidGain must NOT silently disable the PID"
    );

    // Drive the PID with a target well above the current PV so it must
    // produce a positive MV (prove it's actually still computing, not a 0-V stub).
    ctrl.process_artisan_command(ArtisanCommand::SetTargetTemp(230.0))
        .expect("target");
    let t2 = t1 + Duration::from_millis(200);
    tick_now_at(&mut ctrl, 180.0, 200.0, t2);
    // MV reflects the controller's desired output (see status.mv doc comment).
    assert!(
        ctrl.get_status().mv > 0.0,
        "B5: PID must keep producing a non-zero MV after a gain change, got MV={}",
        ctrl.get_status().mv
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 15. NaN PV DURING ROAST
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn nan_pv_triggers_emergency_during_roast() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    tick_now(&mut ctrl, 150.0, 180.0);
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");

    {
        let status = ctrl.status_mut();
        status.bean_temp = f32::NAN;
    }

    let _ = ctrl.update_control(Instant::now());

    assert!(
        ctrl.safety().is_emergency_active()
            && ctrl.get_status().state == libreroaster::config::RoasterState::Error,
        "NaN PV should latch emergency + state==Error"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 16. UNITS COMMAND
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn units_command_switches_to_fahrenheit() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();
    ctrl.process_artisan_command(ArtisanCommand::Units(true))
        .expect("fahrenheit");
    assert!(ctrl.get_status().temperature_settings.is_fahrenheit());
}

#[test]
fn units_command_switches_back_to_celsius() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();
    ctrl.process_artisan_command(ArtisanCommand::Units(true))
        .expect("fahrenheit");
    ctrl.process_artisan_command(ArtisanCommand::Units(false))
        .expect("celsius");
    assert!(!ctrl.get_status().temperature_settings.is_fahrenheit());
}

// ═══════════════════════════════════════════════════════════════════════════
// 17. PID OUTPUT LIMITS AND CYCLE TIME
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn pid_output_limits_are_applied() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();
    ctrl.process_artisan_command(ArtisanCommand::SetPidOutputLimits(10.0, 80.0))
        .expect("limits");
    let s = ctrl.get_status();
    assert_eq!(s.pid_output_min, 10.0);
    assert_eq!(s.pid_output_max, 80.0);
}

#[test]
fn pid_cycle_time_is_respected() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();
    ctrl.process_artisan_command(ArtisanCommand::SetPidCycleTime(500))
        .expect("ct");
    assert_eq!(ctrl.get_status().pid_cycle_time_ms, 500);
}

// ═══════════════════════════════════════════════════════════════════════════
// 18. SYSTEM STATUS SNAPSHOT CONSISTENCY
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn system_status_snapshot_is_consistent() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    tick_now(&mut ctrl, 150.0, 180.0);
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");

    let s1: SystemStatus = ctrl.get_status();
    let s2: SystemStatus = ctrl.get_status();

    assert_eq!(s1.bean_temp, s2.bean_temp);
    assert_eq!(s1.env_temp, s2.env_temp);
    assert_eq!(s1.state, s2.state);
    assert_eq!(s1.pid_enabled, s2.pid_enabled);
}

// ═══════════════════════════════════════════════════════════════════════════
// 19. DOUBLE START COMMAND
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn double_start_does_not_cause_duplicate_heating() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    tick_now(&mut ctrl, 25.0, 30.0);
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("first");
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("second");

    assert_eq!(ctrl.get_state(), RoasterState::Heating);
    tick_now(&mut ctrl, 25.0, 30.0);
    assert_eq!(ctrl.get_state(), RoasterState::Heating);
}

// ═══════════════════════════════════════════════════════════════════════════
// 20. EMERGENCY FORCES HEATER OFF
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn emergency_forces_heater_off() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();
    ctrl.emergency_shutdown("test").expect_err("emergency");
    assert_eq!(ctrl.get_status().ssr_output, 0.0);
}

// ═══════════════════════════════════════════════════════════════════════════
// 21. SET TARGET TEMP VALIDATION
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn set_target_temp_valid_range_accepted() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();
    ctrl.process_artisan_command(ArtisanCommand::SetTargetTemp(200.0))
        .expect("valid");
    assert_eq!(ctrl.get_status().target_temp, 200.0);
}

#[test]
fn set_target_temp_out_of_range_rejected() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();
    assert!(ctrl
        .process_artisan_command(ArtisanCommand::SetTargetTemp(999.0))
        .is_err());
}

#[test]
fn set_target_temp_nan_rejected() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();
    assert!(ctrl
        .process_artisan_command(ArtisanCommand::SetTargetTemp(f32::NAN))
        .is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// 14e. BUG B7 — A single transient sensor fault must NOT latch an emergency
// ═══════════════════════════════════════════════════════════════════════════
// `RoasterControl::update_temperatures` poisons bean_temp/env_temp with NaN
// on the FIRST faulted sample. The PID downstream treats NaN PV as a faulted
// sensor and triggers a latched `emergency_shutdown` on the same tick. B7
// makes the poisoning conditional on `consecutive_fault_count >=
// SENSOR_FAULT_DEBOUNCE`, holding the last valid value until the fault is
// confirmed persistent — matching the F4.11 debouncer that already protects
// `fault_condition`.

#[test]
fn single_sensor_fault_does_not_latch_emergency() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();

    // Stable hot roast so no rate-of-rise emergency interferes.
    tick_now(&mut ctrl, 200.0, 220.0);
    tick_now(&mut ctrl, 200.0, 220.0);
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");
    tick_now(&mut ctrl, 200.0, 220.0);
    assert!(
        !ctrl.safety().is_emergency_active(),
        "precondition: no emergency armed at start of B7 test"
    );

    // Inject aSensorFault directly: simulate one transient SPI glitch by
    // calling `update_temperatures` with bean_fault set. Pre-B7 this would
    // set `status.bean_temp = NaN`, and the subsequent `update_control`
    // would call `emergency_shutdown("Sensor fault (NaN/infinite)")`.
    // Per B7, the value is held (NOT NaN) and no emergency is armed.
    use libreroaster::hardware::sensors::conversion::SensorFault;
    let transient_fault = SensorFault {
        fault_detected: true,
        ..SensorFault::default()
    };
    let now = Instant::now();
    ctrl.update_temperatures_with_fault(200.0, 220.0, transient_fault, SensorFault::default(), now)
        .expect("faulted read does not error");
    // Run the control tick that would have caught a NaN PV.
    let _ = ctrl.update_control(now);

    assert!(
        !ctrl.safety().is_emergency_active(),
        "B7: a single transient fault must NOT latch an emergency, got emergency=true"
    );
    assert!(
        !ctrl.get_status().fault_condition,
        "B7: a single fault must not set fault_condition (F4.11 debouncer protects it)"
    );
}
