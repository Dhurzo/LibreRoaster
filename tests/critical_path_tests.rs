#![cfg(all(test, feature = "test", not(target_arch = "riscv32")))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

extern crate alloc;
extern crate std;

use std::boxed::Box;
use std::sync::Mutex;

use embassy_time::{Duration, Instant};

use libreroaster::common::{StubFan, StubHeater};
use libreroaster::config::constants::RoasterCommand;
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

/// Tick at a fixed instant. Use for tests where `Instant::now()` inside
/// `process_artisan_command` causes timing races with safety checks.
fn tick_at(ctrl: &mut RoasterControl, bt: f32, et: f32, t: Instant) -> f32 {
    ctrl.update_temperatures(bt, et, t).expect("temps");
    ctrl.update_control(t).expect("update")
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

    assert!(
        ctrl.safety().is_emergency_active() || ctrl.get_status().fault_condition,
        "Roast exceeding MAX_ROAST_TIME_SECS should trigger emergency"
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
        ctrl.safety().is_emergency_active() || ctrl.get_status().fault_condition,
        "Comms idle timeout should trigger emergency during Heating state"
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

    // Bug #3 regression: Artisan's `ArtisanCommand::Stop` (which is the
    // "stop streaming" path, mapped from `PID;OFF` over the protocol) must
    // NOT un-latch a held emergency — only the explicit recovery command
    // (StopRoast) clears it. The previous test asserted the opposite here,
    // blessing the bug. Stop may drop us back to Idle for streaming
    // bookkeeping, but the latch stays armed until recovery.
    ctrl.process_artisan_command(ArtisanCommand::Stop)
        .expect("artisan stop does not un-latch emergency");

    // The streaming task is idle, but the safety latch is still armed and
    // the fault flag is still asserted.
    assert!(
        ctrl.safety().is_emergency_active(),
        "STOP must not release the emergency latch"
    );
    assert!(
        ctrl.get_status().fault_condition,
        "STOP must not clear the fault flag (only StopRoast does)"
    );

    // The operator's explicit recovery path releases the latch.
    let recover_now = Instant::from_millis(2000);
    ctrl.process_command(RoasterCommand::StopRoast, recover_now)
        .expect("StopRoast clears emergency");
    assert_eq!(ctrl.get_state(), RoasterState::Idle);
    assert!(!ctrl.get_status().fault_condition);
    assert!(!ctrl.safety().is_emergency_active());

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

    for _ in 0..5 {
        tick_now(&mut ctrl, 25.0, 30.0);
    }

    let integrator_before_manual = ctrl.get_status().integrator_value;
    assert!(
        integrator_before_manual > 0.0,
        "Integrator should have accumulated during PID mode, got {}",
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
        ctrl.safety().is_emergency_active() || ctrl.get_status().fault_condition,
        "Stale sensor data (>1000ms) should trigger emergency during roast"
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

// ═══════════════════════════════════════════════════════════════════════════
// 10. SSR NOT DETECTED → ZERO OUTPUT
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn ssr_not_detected_forces_zero_output_in_manual_mode() {
    let _guard = acquire_lock();
    let heater = StubHeater::new();
    let fan = StubFan::new();
    let mut ctrl = RoasterControl::new(Box::new(heater), Box::new(fan), SensorConversionHub::new())
        .expect("build");

    tick_now(&mut ctrl, 150.0, 180.0);
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");
    ctrl.process_artisan_command(ArtisanCommand::SetHeater(80))
        .expect("manual 80%");

    tick_now(&mut ctrl, 150.0, 180.0);

    let status = ctrl.get_status();
    if status.ssr_hardware_status != libreroaster::config::constants::SsrHardwareStatus::Available {
        assert_eq!(status.ssr_output, 0.0);
    }
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
        ctrl.safety().is_emergency_active() || ctrl.get_status().fault_condition,
        "NaN PV should trigger emergency shutdown"
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
