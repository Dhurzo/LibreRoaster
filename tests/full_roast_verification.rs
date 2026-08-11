#![cfg(all(test, feature = "test", not(target_arch = "riscv32")))]
//! Full-roast verification suite (L1 — deterministic, no hardware).
//!
//! Simulates a COMPLETE coffee roast at the `RoasterControl` level with
//! EXPLICIT timestamps (`Instant::now()` anchor + 310 ms offsets — the real
//! `CONTROL_LOOP_TICK_MS` cadence), answering the question: "would this
//! firmware support a full roast, phase by phase?"
//!
//! Design rules (why the curves look like they do):
//! - Every tick pairs `update_temperatures(t)` with `update_control(t)`
//!   (same instant) so the 1 s stale-sensor backstop never misfires.
//! - Positive BT slope is capped at ~0.16 °C/s (9.7 °C/min) so the
//!   rate-of-rise guards (0.5 °C/s filtered, 3 consecutive) never trip on a
//!   healthy roast; the charge DIP is a fast FALL (negative slope) — both
//!   RoR guards only trip on positive rate (sensor.rs `check_bt_rate` /
//!   `check_rate_of_rise`).
//! - `ReadStatus` is injected every 40 ticks (12.4 s simulated) to mirror
//!   Artisan's ~1 s READ polling; without it the COMMS_IDLE_TIMEOUT_MS
//!   backstop (15 s) would fire by design on an unattended session.
//! - The base timestamp anchors on `Instant::now()` (real) because
//!   `process_artisan_command` records `last_command_received_at_ms` from
//!   the REAL clock; a synthetic base far from real time would trip
//!   comms-idle instantly.
//!
//! Run:  cargo test --test full_roast_verification --features test

#![allow(clippy::expect_used, clippy::unwrap_used)]

extern crate std;

use std::sync::Mutex;

use embassy_time::{Duration, Instant};

use libreroaster::common::{StubFan, StubHeater};
use libreroaster::config::constants::{
    CHARGE_DROP_THRESHOLD_C, COOLING_RELEASE_BEAN_TEMP_C, MAX_BT_RATE_OF_RISE, MAX_ROAST_TIME_SECS,
    OVERTEMP_THRESHOLD,
};
use libreroaster::config::{
    ArtisanCommand, FanProfile, ProfileSetpoint, RoastProfile, RoasterState,
};
use libreroaster::control::roaster_control::RoasterControl;
use libreroaster::hardware::sensors::SensorConversionHub;
use libreroaster::output::artisan::ArtisanFormatter;

// Tick cadence: the real embedded loop is CONTROL_LOOP_TICK_MS ≈ 310-330 ms
// (100 ms timer + ~210 ms MAX31856 conversion wait). 310 ms keeps every
// time-derived backstop honest (stale 1 s, charge window ~3.1 s,
// comms-idle 15 s, max-roast 30 min).
const TICK_MS: u64 = 310;
/// Inject a READ every N ticks to keep the 15 s comms-idle backstop away.
const READ_EVERY_TICKS: u64 = 40; // 12.4 s simulated

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

/// One tick at an explicit instant: temps + control at the SAME time.
/// Returns Err on the first emergency (update_control propagates it).
fn tick_at(ctrl: &mut RoasterControl, bt: f32, et: f32, t: Instant) -> Result<f32, ()> {
    ctrl.update_temperatures(bt, et, t).map_err(|_| ())?;
    ctrl.update_control(t).map_err(|_| ())
}

/// Simulated-clock tick number → timestamp (anchored on real `now`).
fn tick_time(t0: Instant, n: u64) -> Instant {
    t0 + Duration::from_millis(TICK_MS * n)
}

/// Artisan's ~1 s READ polling, modeled in the SYNTHETIC clock domain.
/// `process_artisan_command` stamps `last_command_received_at_ms` with the
/// REAL clock; in a fast synthetic loop that would let the 15 s comms-idle
/// backstop fire regardless of polling. A real READ at synthetic time `t`
/// would reset that counter to `t` — so we patch it to match.
fn poll_read(ctrl: &mut RoasterControl, t: Instant) {
    ctrl.process_artisan_command(ArtisanCommand::ReadStatus)
        .expect("READ must be accepted");
    ctrl.status_mut().last_command_received_at_ms = t.as_millis();
}

/// Run `n` ticks of `bt(n)` / `et(n)` (curve as fn of tick index), keeping
/// comms-idle away by injecting READs. Returns the first tick's Err, or Ok
/// after all ticks. Asserts no emergency latched along the way.
fn run_ticks(
    ctrl: &mut RoasterControl,
    t0: Instant,
    n: u64,
    mut bt: impl FnMut(u64) -> f32,
    mut et: impl FnMut(u64) -> f32,
) -> Result<(), ()> {
    for i in 0..n {
        let t = tick_time(t0, i);
        if i % READ_EVERY_TICKS == 0 {
            poll_read(ctrl, t);
        }
        tick_at(ctrl, bt(i), et(i), t)?;
        assert!(
            !ctrl.safety().is_emergency_active(),
            "tick {i}: emergency must not fire during a healthy roast phase"
        );
    }
    Ok(())
}

// ── Curve models (per-tick samples; slopes chosen per the design rules) ───

/// Preheat: 25 → 180 °C at +0.12 °C/tick (0.39 °C/s), then hold.
/// (RoR guards are inactive in Preheating — roaster_control.rs:778 — but we
/// stay under the threshold anyway to prove a clean ramp.)
fn preheat_bt(n: u64) -> f32 {
    (25.0 + 0.12 * n as f32).min(180.0)
}

/// Roast: charge dip 150 → 143 °C over 10 ticks (~2.3 °C/s FALLING — the
/// physical 2-3 °C/s signature, negative slope so RoR guards ignore it),
/// then a steady 0.05 °C/tick rise (9.7 °C/min) to ~225 °C.
fn roast_bt(n: u64) -> f32 {
    let dip = if n < 10 {
        150.0 - 0.7 * n as f32
    } else {
        143.0
    };
    if n < 10 {
        dip
    } else {
        dip + 0.05 * (n - 10) as f32
    }
}

fn roast_et(n: u64) -> f32 {
    roast_bt(n) + 15.0
}

/// Cooling: 220 → 55 °C at -0.5 °C/tick (1.6 °C/s falling).
fn cooling_bt(n: u64) -> f32 {
    (220.0 - 0.5 * n as f32).max(55.0)
}

/// The standard 3-setpoint roast profile used by the full-roast scenarios.
fn medium_profile() -> RoastProfile {
    let mut profile = RoastProfile::new();
    for (t, temp) in [(0u32, 150.0f32), (300, 200.0), (480, 230.0)] {
        profile
            .setpoints
            .push(ProfileSetpoint {
                time_secs: t,
                temperature: temp,
            })
            .expect("setpoint");
    }
    profile
}

fn medium_fan_profile() -> FanProfile {
    let mut profile = FanProfile::new();
    for (t, fan) in [(0u32, 30u8), (300, 50), (480, 70)] {
        profile
            .setpoints
            .push(libreroaster::config::FanSetpoint {
                time_secs: t,
                fan_speed: fan,
            })
            .expect("fan setpoint");
    }
    profile
}

/// Drain the shared output channel (static, cross-test global) collecting
/// any `#CHARGE` notifications; returns true if at least one arrived.
fn drain_charge_notifications() -> bool {
    let channel =
        libreroaster::application::service_container::ServiceContainer::get_output_channel();
    let mut saw_charge = false;
    while let Ok(msg) = channel.try_receive() {
        if msg.starts_with("#CHARGE dt=") {
            saw_charge = true;
        }
    }
    saw_charge
}

// ═══════════════════════════════════════════════════════════════════════════
// P3 — PREHEAT
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn s1_preheat_reaches_target_and_holds() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();
    let t0 = Instant::now();

    ctrl.process_artisan_command(ArtisanCommand::Preheat(180.0))
        .expect("preheat");
    assert_eq!(ctrl.get_state(), RoasterState::Preheating);
    assert_eq!(ctrl.get_status().target_temp, 180.0);
    assert!(ctrl.get_status().pid_enabled);

    // ~7.2 simulated minutes: ramp 25→180 then a short hold (~33 s — well
    // under the 120 s probe-stuck window, which is armed while the PID is
    // away from the target margin during the ramp).
    run_ticks(&mut ctrl, t0, 1400, preheat_bt, |n| preheat_bt(n) - 5.0)
        .expect("preheat must run without emergencies");

    let s = ctrl.get_status();
    assert!(
        (s.bean_temp - 180.0).abs() <= 0.5,
        "preheat must reach target, BT={:.1}",
        s.bean_temp
    );
    assert_eq!(ctrl.get_state(), RoasterState::Preheating);
    assert!(!s.fault_condition, "no fault during preheat");

    // READ stays well-formed at the end of preheat (5-field or the 8-field
    // PID variant, per the TC4 contract).
    let response = ArtisanFormatter::format_read_response_full(&s);
    assert!(
        matches!(response.split(',').count(), 5 | 8),
        "READ must be 5 or 8 fields, got {}",
        response.split(',').count()
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// P4 — CHARGE DETECTION (bean drop)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn s1b_charge_dip_detected_and_notified() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();
    let t0 = Instant::now();

    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");

    // Pre-charge: drain any stale #CHARGE from other (parallel) tests.
    drain_charge_notifications();

    // 20 ticks: dip 150 → 143 (10 ticks) + a few rising ticks.
    for i in 0..20u64 {
        let t = tick_time(t0, i);
        let bt = roast_bt(i);
        tick_at(&mut ctrl, bt, bt + 15.0, t).expect("charge dip must not trip safety");
        if i == 5 {
            // After 5 samples the window needs drop > CHARGE_DROP_THRESHOLD_C.
            assert!(
                !ctrl.get_status().charge_detected,
                "charge must NOT fire before the full {CHARGE_DROP_THRESHOLD_C} °C drop accumulates"
            );
        }
    }

    assert!(
        ctrl.get_status().charge_detected,
        "the {} °C charge dip must be detected within the ~3.1 s window",
        CHARGE_DROP_THRESHOLD_C + 1.0
    );
    assert!(
        drain_charge_notifications(),
        "#CHARGE notification must be emitted to the output channel"
    );
    assert!(!ctrl.safety().is_emergency_active());
}

// ═══════════════════════════════════════════════════════════════════════════
// P5 + P6 + P8 — PROFILE FOLLOWING, MANUAL+PID, RoR THROUGH FIRST CRACK
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn s2_profile_fan_and_ror_across_full_roast() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();
    let t0 = Instant::now();

    // Load PROFILE + FANPROFILE exactly as Artisan does, then START.
    libreroaster::input::parser::store_profile(medium_profile());
    ctrl.process_artisan_command(ArtisanCommand::SetProfile)
        .expect("profile");
    libreroaster::input::parser::fan_profile_store(medium_fan_profile());
    ctrl.process_artisan_command(ArtisanCommand::SetFanProfile)
        .expect("fan profile");
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");
    assert_eq!(ctrl.get_state(), RoasterState::Heating);

    // ── Run ~9 simulated minutes with mid-flight window assertions ──────
    // BT window n → (elapsed_secs, expected fan %, expected profile target):
    //   n≈100  → 31 s   → fan 30  (profile t=0)
    //   n≈1070 → 332 s  → BT≈196 °C (first crack) → RoR check
    //   n≈1100 → 341 s  → fan 50  (profile t=300)
    //   n≈1600 → 496 s  → fan 70  (profile t=480)
    let mut fan_seen_at_100 = None;
    let mut fan_seen_at_1100 = None;
    let mut fan_seen_at_1600 = None;
    let mut ror_at_fc = None;
    for i in 0..1700u64 {
        let t = tick_time(t0, i);
        if i % READ_EVERY_TICKS == 0 {
            poll_read(&mut ctrl, t);
        }
        tick_at(&mut ctrl, roast_bt(i), roast_et(i), t).expect("roast must complete cleanly");
        assert!(
            !ctrl.safety().is_emergency_active(),
            "tick {i}: no backstop may fire during a healthy roast"
        );
        match i {
            100 => fan_seen_at_100 = Some(ctrl.get_status().fan_output),
            1070 => {
                ror_at_fc = Some(ctrl.get_status().derivative_rate);
                assert!(
                    ctrl.get_status().derivative_available,
                    "derivative must be available at first crack"
                );
            }
            1100 => fan_seen_at_1100 = Some(ctrl.get_status().fan_output),
            1600 => fan_seen_at_1600 = Some(ctrl.get_status().fan_output),
            _ => {}
        }
    }

    // ── Profile target follows the setpoints ────────────────────────────
    // update_control advances target_temp via target_at(elapsed)
    // (roaster_control.rs:1796-1799); after 527 s the final setpoint rules.
    assert!(
        ctrl.get_status().target_temp >= 225.0,
        "profile target must reach the final setpoint (230), got {:.1}",
        ctrl.get_status().target_temp
    );

    // ── Fan follows FANPROFILE (30 → 50 → 70) ───────────────────────────
    // FANPROFILE is LINEARLY INTERPOLATED between setpoints with
    // (interp + 0.5) as u8 rounding (constants.rs:367-370). The elapsed
    // floor is `(t - real_profile_start) / 1000`; the real anchor sits
    // ε ∈ (0, 1 s] after t0, so elapsed at n is one second SHORT of
    // 310·n/1000:
    //   n=100 → 30 s   → between (0,30)/(300,50): 30 + 20·30/300 = 32
    //   n=1100 → 340 s → between (300,50)/(480,70): 50 + 20·40/180 ≈ 54.44 → 54
    //   n=1600 → 495 s → past 480 s: 70 (last setpoint)
    assert_eq!(
        fan_seen_at_100,
        Some(32.0),
        "fan at ~31 s must interpolate to 32 %"
    );
    assert_eq!(
        fan_seen_at_1100,
        Some(54.0),
        "fan at ~341 s must interpolate to 54 %"
    );
    assert_eq!(
        fan_seen_at_1600,
        Some(70.0),
        "fan at ~496 s must reach the final setpoint 70 %"
    );
    assert!(
        ctrl.get_status().fan_output >= libreroaster::config::constants::FAN_MIN_SAFETY_PCT,
        "fan interlock floor must hold"
    );

    // ── RoR through the first-crack window (BT ≈ 196 °C) ─────────────────
    let ror = ror_at_fc.expect("RoR sampled at first crack");
    assert!(
        ror > 0.0 && ror < MAX_BT_RATE_OF_RISE,
        "RoR at first crack must be a healthy positive rate < {MAX_BT_RATE_OF_RISE} °C/s, got {ror:.3}"
    );

    // ── End-of-roast state + STATUS wire line ────────────────────────────
    let s = ctrl.get_status();
    assert!(
        s.bean_temp >= 215.0,
        "BT must reach ~225 at roast end, got {:.1}",
        s.bean_temp
    );
    assert_eq!(ctrl.get_state(), RoasterState::Heating);
    assert!(!s.fault_condition);
    let status_line = ArtisanFormatter::format_status_response(&s);
    assert_eq!(
        status_line.split(',').count(),
        20,
        "STATUS must stay 20 fields"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// P7 — SAFETY BACKSTOPS MID-ROAST (one scenario per backstop, deterministic)
// ═══════════════════════════════════════════════════════════════════════════

/// Shared prelude: START + a few ticks so the roast is genuinely active.
fn roast_active(t0: Instant) -> (RoasterControl, Instant) {
    let mut ctrl = build_control();
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");
    for i in 0..20u64 {
        let t = tick_time(t0, i);
        tick_at(&mut ctrl, roast_bt(i), roast_et(i), t).expect("prelude ticks");
    }
    (ctrl, t0)
}

/// Assert the full emergency posture: latch + Error + heater 0 (via the
/// next tick forcing the off-write) + fan 100.
fn assert_emergency_posture(ctrl: &mut RoasterControl, t: Instant) {
    assert!(ctrl.safety().is_emergency_active(), "latch must be active");
    assert_eq!(ctrl.get_state(), RoasterState::Error);
    assert!(ctrl.get_status().fault_condition);
    // One more tick to let the forced writes land.
    let _ = ctrl.update_control(t + Duration::from_millis(TICK_MS));
    assert_eq!(ctrl.get_status().ssr_output, 0.0, "heater must be 0 %");
    assert_eq!(ctrl.get_status().fan_output, 100.0, "fan must be 100 %");
}

#[test]
fn s3_overtemp_mid_roast_trips() {
    let _guard = acquire_lock();
    let (mut ctrl, t0) = roast_active(Instant::now());

    let t = tick_time(t0, 21);
    let result = ctrl.update_temperatures(OVERTEMP_THRESHOLD + 1.0, 200.0, t);
    assert!(result.is_err(), "BT ≥ OVERTEMP_THRESHOLD must reject");
    assert_emergency_posture(&mut ctrl, t);
}

#[test]
fn s3_nan_pv_mid_roast_trips() {
    let _guard = acquire_lock();
    let (mut ctrl, t0) = roast_active(Instant::now());

    let t = tick_time(t0, 21);
    ctrl.status_mut().bean_temp = f32::NAN;
    let result = ctrl.update_control(t);
    assert!(result.is_err(), "NaN PV must trip the emergency");
    assert_emergency_posture(&mut ctrl, t);
}

#[test]
fn s3_stale_sensor_mid_roast_trips() {
    let _guard = acquire_lock();
    let (mut ctrl, t0) = roast_active(Instant::now());

    // Advance 2 s WITHOUT feeding temps → stale > TEMP_VALIDITY_TIMEOUT_MS.
    let t = tick_time(t0, 21) + Duration::from_secs(2);
    let result = ctrl.update_control(t);
    assert!(result.is_err(), "stale sensor must trip the emergency");
    assert_emergency_posture(&mut ctrl, t);
}

#[test]
fn s3_comms_idle_mid_roast_trips() {
    let _guard = acquire_lock();
    let (mut ctrl, t0) = roast_active(Instant::now());

    // No commands for > 15 s simulated → comms-idle backstop fires.
    // (READs are deliberately NOT injected here.)
    let t = tick_time(t0, 60); // 18.6 s after the last command (START)
    let _ = tick_at(&mut ctrl, roast_bt(60), roast_et(60), t);
    assert_emergency_posture(&mut ctrl, t);
}

#[test]
fn s3_max_roast_time_mid_roast_trips() {
    let _guard = acquire_lock();
    let (mut ctrl, t0) = roast_active(Instant::now());

    // Run past MAX_ROAST_TIME_SECS (30 min) with READs keeping comms-idle
    // away — ONLY the max-roast-time cap can fire here.
    let total = (MAX_ROAST_TIME_SECS as u64 * 1000) / TICK_MS + 30; // ≈ 5800 ticks
    let mut fired = false;
    for i in 20..total {
        let t = tick_time(t0, i);
        if i % READ_EVERY_TICKS == 0 {
            poll_read(&mut ctrl, t);
        }
        let bt = (roast_bt(20) + 0.05 * (i - 20) as f32).min(240.0);
        match tick_at(&mut ctrl, bt, bt + 15.0, t) {
            Ok(_) => {}
            Err(()) => {
                fired = true;
                break;
            }
        }
        assert!(
            !ctrl.safety().is_emergency_active(),
            "no backstop may fire before the 30 min cap"
        );
    }
    assert!(fired, "MAX_ROAST_TIME_SECS cap must trip at ~30 min");
    assert_emergency_posture(&mut ctrl, tick_time(t0, total));
}

#[test]
fn s3_probe_stuck_manual_flat_bt_trips() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();
    let t0 = Instant::now();

    // Manual mode (no PID): OT1 60 energizes the heater; BT frozen at 80 °C.
    ctrl.process_artisan_command(ArtisanCommand::SetHeater(60))
        .expect("manual heater");
    let mut fired = false;
    for i in 0..(121u64 * 1000 / TICK_MS) {
        // 391 ticks = 121 s simulated: probe-stuck fires at 120 s flat BT.
        let t = tick_time(t0, i);
        if i % READ_EVERY_TICKS == 0 {
            poll_read(&mut ctrl, t);
        }
        if tick_at(&mut ctrl, 80.0, 85.0, t).is_err() {
            fired = true;
            break;
        }
    }
    assert!(
        fired,
        "flat BT with heater on must trip probe-stuck at 120 s"
    );
    assert_emergency_posture(&mut ctrl, tick_time(t0, 400));
}

// ═══════════════════════════════════════════════════════════════════════════
// P9 + P10 — STOP, COOLDOWN RELEASE, SECOND ROAST
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn s4_stop_cooldown_and_release() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();
    let t0 = Instant::now();

    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");
    // Roast to BT ≈ 220 (n=1550 ≈ 8 min).
    run_ticks(&mut ctrl, t0, 1550, roast_bt, roast_et).expect("roast must run clean");

    // Plain STOP (OFF semantics — the V2-1 sanctioned recovery) keeps the
    // roaster recoverable; cooling_active arms the 100 % fan.
    ctrl.process_artisan_command(ArtisanCommand::Stop)
        .expect("stop");
    let s = ctrl.get_status();
    assert_eq!(s.ssr_output, 0.0, "heater must be 0 right after STOP");
    assert_eq!(
        ctrl.get_state(),
        RoasterState::Idle,
        "plain STOP stays recoverable"
    );

    // Cooldown: fan must stay 100 % until BT < COOLING_RELEASE_BEAN_TEMP_C.
    let mut fan_held_100 = true;
    let mut released = false;
    for i in 0..400u64 {
        let t = tick_time(t0, 1600 + i);
        let bt = cooling_bt(i);
        tick_at(&mut ctrl, bt, bt - 10.0, t).expect("cooling ticks");
        let fan = ctrl.get_status().fan_output;
        if bt >= COOLING_RELEASE_BEAN_TEMP_C {
            if fan < 99.0 {
                fan_held_100 = false;
            }
        } else if fan < 99.0 {
            released = true;
            break;
        }
    }
    assert!(
        fan_held_100,
        "fan must hold 100 % while BT ≥ COOLING_RELEASE_BEAN_TEMP_C"
    );
    assert!(
        released,
        "fan must release once BT < COOLING_RELEASE_BEAN_TEMP_C"
    );
    assert!(
        !ctrl.safety().is_emergency_active(),
        "plain STOP must not latch"
    );
}

#[test]
fn s5_two_consecutive_full_roasts() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();
    let t0 = Instant::now();

    // ── Roast #1 ─────────────────────────────────────────────────────────
    libreroaster::input::parser::store_profile(medium_profile());
    ctrl.process_artisan_command(ArtisanCommand::SetProfile)
        .expect("profile");
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start #1");
    assert_eq!(ctrl.get_state(), RoasterState::Heating);

    // Charge dip → detected.
    for i in 0..20u64 {
        let t = tick_time(t0, i);
        tick_at(&mut ctrl, roast_bt(i), roast_et(i), t).expect("dip");
    }
    assert!(
        ctrl.get_status().charge_detected,
        "roast #1 must detect charge"
    );

    // ~5 simulated minutes of rise.
    for i in 20..1000u64 {
        let t = tick_time(t0, i);
        if i % READ_EVERY_TICKS == 0 {
            poll_read(&mut ctrl, t);
        }
        tick_at(&mut ctrl, roast_bt(i), roast_et(i), t).expect("roast #1");
    }

    // STOP (plain) + cooldown below the release threshold.
    ctrl.process_artisan_command(ArtisanCommand::Stop)
        .expect("stop #1");
    for i in 0..400u64 {
        let t = tick_time(t0, 1000 + i);
        let bt = cooling_bt(i);
        tick_at(&mut ctrl, bt, bt - 10.0, t).expect("cooling #1");
        if bt < COOLING_RELEASE_BEAN_TEMP_C && ctrl.get_status().fan_output < 99.0 {
            break;
        }
    }
    assert_eq!(
        ctrl.get_state(),
        RoasterState::Idle,
        "roast #1 must return to Idle"
    );

    // ── Roast #2: everything must be clean ───────────────────────────────
    // Pending dump queue empty (cleared by the previous START... and any
    // residual from roast #1's own START is long drained).
    assert!(
        ctrl.take_dump_row().is_none(),
        "dump queue must be empty before roast #2"
    );
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start #2");
    assert_eq!(ctrl.get_state(), RoasterState::Heating);
    let s = ctrl.get_status();
    assert!(!s.charge_detected, "charge state must reset between roasts");
    assert!(
        s.target_temp >= 145.0 && s.target_temp <= 160.0,
        "roast #2 must reload the profile's t=0 target, got {:.1}",
        s.target_temp
    );
    assert!(s.pid_enabled, "PID must be re-armed for roast #2");

    // Charge dip detected AGAIN (the detector re-arms per roast). The drum
    // must first warm back up from the cooldown (~55 °C) to charge temp
    // (~150 °C) — a 0.1 °C/tick ramp (~5 min), keeping the RoR guards away
    // from the 55→150 °C step a naive timeline would produce.
    for i in 0..1000u64 {
        let t = tick_time(t0, 1500 + i);
        if i % READ_EVERY_TICKS == 0 {
            poll_read(&mut ctrl, t);
        }
        let bt = (55.0 + 0.1 * i as f32).min(150.0);
        tick_at(&mut ctrl, bt, bt + 15.0, t).expect("drum warm-up #2");
        assert!(
            !ctrl.safety().is_emergency_active(),
            "tick {}: drum warm-up must not trip any backstop",
            1500 + i
        );
    }
    assert!(
        (ctrl.get_status().bean_temp - 150.0).abs() <= 0.5,
        "drum must reach charge temp before the dip"
    );
    for i in 0..20u64 {
        let t = tick_time(t0, 2500 + i);
        if i % READ_EVERY_TICKS == 0 {
            poll_read(&mut ctrl, t);
        }
        tick_at(&mut ctrl, roast_bt(i), roast_et(i), t).expect("dip #2");
    }
    assert!(
        ctrl.get_status().charge_detected,
        "roast #2 must detect its own charge dip"
    );

    // READ after the second roast is still well-formed.
    let response = ArtisanFormatter::format_read_response_full(&ctrl.get_status());
    assert!(
        matches!(response.split(',').count(), 5 | 8),
        "READ must be 5 or 8 fields, got {}",
        response.split(',').count()
    );
}
