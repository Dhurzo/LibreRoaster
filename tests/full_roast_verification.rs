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
    CHARGE_DROP_THRESHOLD_C, COOLING_RELEASE_BEAN_TEMP_C, MAX_BT_RATE_OF_RISE,
    MAX_BT_RATE_OF_RISE_HARD, MAX_ROAST_TIME_SECS, OVERTEMP_THRESHOLD,
    ROR_EXCEEDED_CONSECUTIVE_LIMIT, ROR_SOFT_DEBOUNCE_LIMIT,
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

/// Drain the shared output channel collecting every non-TRACE line, in
/// arrival order. Callers hold TEST_MUTEX, so no other test in this binary
/// can interleave on the channel.
fn drain_output_lines() -> Vec<std::string::String> {
    let channel =
        libreroaster::application::service_container::ServiceContainer::get_output_channel();
    let mut lines = Vec::new();
    while let Ok(msg) = channel.try_receive() {
        if msg.starts_with("TRACE,") {
            continue;
        }
        lines.push(msg.as_str().to_string());
    }
    lines
}

/// Send a slider/actuator command interleaved with the synthetic-clock tick
/// loop. `process_artisan_command` stamps `last_command_received_at_ms` from
/// the REAL clock (the same reason `poll_read` patches); a slider arriving
/// mid-flight at real time would otherwise open a multi-hundred-second
/// comms-idle gap on the next synthetic tick. Patch the stamp back to the
/// tick's synthetic time — what a real Artisan session would register.
fn send_slider(ctrl: &mut RoasterControl, cmd: ArtisanCommand, t: Instant) {
    ctrl.process_artisan_command(cmd).expect("slider command");
    ctrl.status_mut().last_command_received_at_ms = t.as_millis();
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
    // Audit A-TC4-C (2026-08-12): manual mode is TWO-STAGE — at ~120 s the
    // firmware emits `ERR probe_stuck_warning` on the wire WITHOUT latching
    // (a legitimately slow finish can hold BT <1 °C for 2 min at low duty);
    // the real latch lands at ~300 s and announces itself with
    // `ERR safety_fault Probe stuck`.
    ctrl.process_artisan_command(ArtisanCommand::SetHeater(60))
        .expect("manual heater");
    drain_output_lines(); // clear stale lines from earlier tests in this binary

    // 120 s ≈ tick 388, 300 s ≈ tick 968 (TICK_MS = 310).
    const WARN_TICKS: u64 = 120 * 1000 / TICK_MS + 5; // 392
    const LATCH_TICKS: u64 = 300 * 1000 / TICK_MS + 5; // 972
    let mut fired = false;
    for i in 0..LATCH_TICKS {
        let t = tick_time(t0, i);
        if i % READ_EVERY_TICKS == 0 {
            poll_read(&mut ctrl, t);
        }
        if tick_at(&mut ctrl, 80.0, 85.0, t).is_err() {
            fired = true;
            break;
        }
        if i == WARN_TICKS {
            // Just past the 120 s warning threshold: the warning must be on
            // the wire, the latch still disarmed.
            assert!(
                !ctrl.safety().is_emergency_active(),
                "manual mode must not latch at the ~120 s warning threshold"
            );
            assert!(
                drain_output_lines()
                    .iter()
                    .any(|l| l == "ERR probe_stuck_warning"),
                "the two-stage warning must be emitted on the wire"
            );
        }
    }
    assert!(
        fired,
        "flat BT with heater on must latch probe-stuck at ~300 s"
    );
    assert_emergency_posture(&mut ctrl, tick_time(t0, LATCH_TICKS + 2));
    assert!(
        drain_output_lines()
            .iter()
            .any(|l| l.starts_with("ERR safety_fault Probe stuck")),
        "the manual-mode latch must announce itself on the wire"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// A-TC4-D — LIGHT ROAST verification (Audit A-TC4-D, 2026-08-12)
// ═══════════════════════════════════════════════════════════════════════════
//
// A normal Artisan light roast: preheat ET ≈ 200 °C, charge dip to ~95 °C,
// an aggressive turnaround (RoR peak 15-30 °C/min), a declining development
// phase, drop at BT ≈ 200 °C around 8-10 min. Both Artisan drive modes are
// covered — the modern software-PID slider stream (manual mode) and the
// legacy firmware-PID path — plus the boundary tests around the two-tier
// rate-of-rise guard.
//
// Slopes used here stay ≤ 0.19 °C/s (11.4 °C/min) for the healthy phases —
// comfortably under the 0.5 °C/s soft guard threshold (constants.rs), and
// the boundary tests below drive the intentional spikes.

#[test]
fn light_roast_software_pid_full_flow() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();
    let t0 = Instant::now();

    // ── Preheat by slider (manual mode — Artisan's modern default) ──────
    // Empty-drum preheat: OT1 80 + IO3 40; BT climbs 0.08 °C/tick
    // (0.26 °C/s) from 25 to 200 °C (~11 simulated min).
    ctrl.process_artisan_command(ArtisanCommand::SetHeater(80))
        .expect("OT1 80");
    ctrl.process_artisan_command(ArtisanCommand::SetFan(40))
        .expect("IO3 40");
    drain_output_lines(); // clear stale lines left by earlier tests in this binary
    for i in 0..2200u64 {
        let t = tick_time(t0, i);
        if i % READ_EVERY_TICKS == 0 {
            poll_read(&mut ctrl, t);
        }
        let bt = (25.0 + 0.08 * i as f32).min(200.0);
        tick_at(&mut ctrl, bt, bt + 10.0, t).expect("preheat must run clean");
    }
    assert_eq!(
        ctrl.get_state(),
        RoasterState::Idle,
        "manual mode stays Idle"
    );

    // ── Charge: 200 → 95 °C in 10 ticks (10.5 °C/tick falling) ──────────
    // Manual mode keeps state = Idle, so the charge detector is gated out
    // (roaster_control.rs charge gate) and no #CHARGE may appear on the wire.
    let charge_start = 2200u64;
    for i in 0..10u64 {
        let t = tick_time(t0, charge_start + i);
        // Artisan polls READ ~1/s even through the charge window — without
        // it the 15 s comms-idle backstop would (correctly) fire here.
        poll_read(&mut ctrl, t);
        let bt = 200.0 - 10.5 * i as f32;
        tick_at(&mut ctrl, bt, bt + 10.0, t).expect("charge dip");
    }
    assert!(
        !ctrl.get_status().charge_detected,
        "manual mode must not run the charge detector"
    );
    assert!(
        !drain_charge_notifications(),
        "#CHARGE must never be emitted in a manual slider session"
    );

    // ── Development: 95 → 202 °C at 0.06 °C/tick with Artisan's slider ──
    // stream: heater steps down, airflow steps up (light-roast profile).
    for i in 0..1800u64 {
        let t = tick_time(t0, charge_start + 10 + i);
        if i % READ_EVERY_TICKS == 0 {
            poll_read(&mut ctrl, t);
        }
        match i {
            0 => {
                send_slider(&mut ctrl, ArtisanCommand::SetHeater(100), t);
            }
            600 => {
                send_slider(&mut ctrl, ArtisanCommand::SetHeater(75), t);
                send_slider(&mut ctrl, ArtisanCommand::SetFan(60), t);
            }
            1200 => {
                send_slider(&mut ctrl, ArtisanCommand::SetHeater(50), t);
                send_slider(&mut ctrl, ArtisanCommand::SetFan(80), t);
            }
            _ => {}
        }
        let bt = 95.0 + 0.06 * i as f32;
        tick_at(&mut ctrl, bt, bt + 10.0, t).expect("development must run clean");
    }

    // ── Drop: OT1;0 (what Artisan sends at roast end) ───────────────────
    ctrl.process_artisan_command(ArtisanCommand::SetHeater(0))
        .expect("OT1 0");
    let t = tick_time(t0, charge_start + 10 + 1800);
    tick_at(&mut ctrl, 203.0, 213.0, t).expect("post-drop tick");

    let s = ctrl.get_status();
    assert_eq!(s.ssr_output, 0.0, "OT1;0 must cut the heater");
    assert!(
        !ctrl.safety().is_emergency_active(),
        "a clean light roast must not latch anything"
    );
    assert_eq!(ctrl.get_state(), RoasterState::Idle);
    assert!(
        !s.pid_enabled,
        "software-PID mode never enables the firmware PID"
    );
    // READ stays the 5-field TC4 format through the whole session.
    let response = ArtisanFormatter::format_read_response_full(&s);
    assert_eq!(response.split(',').count(), 5, "READ must stay 5-field");
    // The wire carried no error lines during the whole session.
    assert!(
        !drain_output_lines().iter().any(|l| l.starts_with("ERR ")),
        "a clean software-PID light roast must emit no ERR lines"
    );
}

#[test]
fn light_roast_firmware_pid_full_flow() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();
    let t0 = Instant::now();

    // Legacy TC4 PID path: PID;ON (=START) then PID;SV;200.
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("START");
    ctrl.process_artisan_command(ArtisanCommand::SetTargetTemp(200.0))
        .expect("PID;SV;200");
    assert_eq!(ctrl.get_state(), RoasterState::Heating);
    assert!(ctrl.get_status().pid_enabled);
    assert_eq!(ctrl.get_status().target_temp, 200.0);
    drain_output_lines(); // clear stale lines left by earlier tests in this binary

    // ── Drum warm-up under PID: 25 → 200 °C at 0.06 °C/tick ─────────────
    // (no dip — charge detection only triggers on a real drop).
    for i in 0..2920u64 {
        let t = tick_time(t0, i);
        if i % READ_EVERY_TICKS == 0 {
            poll_read(&mut ctrl, t);
        }
        let bt = (25.0 + 0.06 * i as f32).min(200.0);
        tick_at(&mut ctrl, bt, bt + 10.0, t).expect("warm-up");
    }
    assert!(
        !ctrl.get_status().charge_detected,
        "no charge may be detected without a dip"
    );

    // ── Charge: 200 → 95 °C — the detector IS armed in Heating state ────
    let charge_start = 2920u64;
    for i in 0..10u64 {
        let t = tick_time(t0, charge_start + i);
        // Artisan keeps polling READ through the charge window; without it
        // the comms-idle backstop would fire instead of the charge detection.
        poll_read(&mut ctrl, t);
        let bt = 200.0 - 10.5 * i as f32;
        tick_at(&mut ctrl, bt, bt + 10.0, t).expect("charge dip");
    }
    assert!(
        ctrl.get_status().charge_detected,
        "firmware-PID mode must detect the light-roast charge dip"
    );
    assert!(
        drain_charge_notifications(),
        "#CHARGE must be emitted in firmware-PID mode"
    );

    // ── Development + drop: 95 → 203 °C at 0.06 °C/tick ─────────────────
    for i in 0..1800u64 {
        let t = tick_time(t0, charge_start + 10 + i);
        if i % READ_EVERY_TICKS == 0 {
            poll_read(&mut ctrl, t);
        }
        let bt = 95.0 + 0.06 * i as f32;
        tick_at(&mut ctrl, bt, bt + 10.0, t).expect("development");
    }

    // READ carries the 8-field PID variant while PID is on.
    let s = ctrl.get_status();
    let response = ArtisanFormatter::format_read_response_full(&s);
    assert_eq!(
        response.split(',').count(),
        8,
        "READ with PID on must be 8-field, got: {response}"
    );
    assert!(
        !ctrl.safety().is_emergency_active(),
        "a clean firmware-PID light roast must not latch anything"
    );

    // ── Roast end = PID;OFF ─────────────────────────────────────────────
    ctrl.process_artisan_command(ArtisanCommand::Stop)
        .expect("PID;OFF");
    let t = tick_time(t0, charge_start + 10 + 1800 + 1);
    tick_at(&mut ctrl, 203.0, 213.0, t).expect("post-off tick");
    let s = ctrl.get_status();
    assert_eq!(s.ssr_output, 0.0, "PID;OFF must cut the heater");
    assert!(
        !s.fault_condition,
        "PID;OFF (roast end) must NOT arm the safety latch"
    );
    assert!(
        !drain_output_lines()
            .iter()
            .any(|l| l.starts_with("ERR safety_fault")),
        "a clean firmware-PID light roast must emit no safety faults"
    );
}

// ── Boundary tests: the two-tier rate-of-rise guard ────────────────────────
//
// The guard is fed by `refresh_bt_guard_derivative` (IIR alpha 0.3, sensor.rs)
// on the BT-only derivative. For a constant input rate r the filtered value
// converges as r·(1 − 0.7ⁿ) per tick, so the filtered signal crosses the
// 0.5 °C/s soft band ~4-6 ticks into a spike and the 1.0 °C/s hard band
// ~3-4 ticks into a fast one. The tests below use 0.186 °C/tick = 0.6 °C/s,
// 0.217 °C/tick = 0.7 °C/s and 0.465 °C/tick = 1.5 °C/s at TICK_MS = 310.

#[test]
fn light_roast_boundary_manual_mode_ror_guard_disarmed() {
    let _guard = acquire_lock();
    let mut ctrl = build_control();
    let t0 = Instant::now();

    ctrl.process_artisan_command(ArtisanCommand::SetHeater(80))
        .expect("OT1 80");

    // 0.6 °C/s for 16 ticks (~5 s) — enough to trip the 3-tick rule if the
    // guard were armed. In pure-manual mode (pid_enabled = false) the arm
    // gate (roaster_control.rs) keeps it disarmed by design.
    let mut bt = 100.0f32;
    for i in 0..16u64 {
        let t = tick_time(t0, i);
        if i % READ_EVERY_TICKS == 0 {
            poll_read(&mut ctrl, t);
        }
        bt += 0.186;
        tick_at(&mut ctrl, bt, bt + 10.0, t).expect("manual spike");
    }
    assert!(
        !ctrl.safety().is_emergency_active(),
        "manual mode must keep the RoR guard disarmed"
    );
    assert_eq!(ctrl.get_state(), RoasterState::Idle);
}

#[test]
fn light_roast_boundary_turnaround_does_not_trip_firmware_pid() {
    // The key light-roast false-trip: a ~3 s, 0.6 °C/s turnaround right after
    // charge in firmware-PID mode. With the old single-tier 3-tick rule the
    // filtered derivative crossed 0.5 °C/s ~6 ticks in and latched ~2 ticks
    // later — a false emergency on a healthy aggressive light roast. The
    // soft band (ROR_SOFT_DEBOUNCE_LIMIT = 12) tolerates the brief spike.
    let _guard = acquire_lock();
    let mut ctrl = build_control();
    let t0 = Instant::now();

    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("START");
    ctrl.process_artisan_command(ArtisanCommand::SetTargetTemp(200.0))
        .expect("SV 200");

    // Seed the derivative pipeline with 4 healthy ticks (slope 0.05 °C/tick).
    let mut bt = 90.0f32;
    for i in 0..4u64 {
        let t = tick_time(t0, i);
        poll_read(&mut ctrl, t);
        tick_at(&mut ctrl, bt, bt + 10.0, t).expect("seed");
    }

    // Turnaround spike: 0.6 °C/s (soft band + 0.1) for 10 ticks (~3.1 s).
    // The per-tick step is derived FROM the production soft threshold so the
    // test keeps pinning the false-trip scenario if HIL calibration ever
    // moves MAX_BT_RATE_OF_RISE.
    let spike_step = (MAX_BT_RATE_OF_RISE + 0.1) * (TICK_MS as f32 / 1000.0);
    for i in 4..14u64 {
        let t = tick_time(t0, i);
        poll_read(&mut ctrl, t);
        bt += spike_step;
        tick_at(&mut ctrl, bt, bt + 10.0, t).expect("turnaround spike");
    }
    assert!(
        !ctrl.safety().is_emergency_active(),
        "a ~3 s light-roast turnaround must not trip the soft band"
    );

    // The roast continues cleanly afterwards at a healthy slope.
    for i in 14..40u64 {
        let t = tick_time(t0, i);
        if i % READ_EVERY_TICKS == 0 {
            poll_read(&mut ctrl, t);
        }
        bt += 0.05;
        tick_at(&mut ctrl, bt, bt + 10.0, t).expect("post-turnaround");
    }
    assert!(
        !ctrl.safety().is_emergency_active(),
        "the roast must stay clean after the tolerated spike"
    );
}

#[test]
fn light_roast_boundary_hard_runaway_trips_firmware_pid() {
    // A genuine runaway: 1.5 °C/s sustained. The filtered derivative crosses
    // the 1.0 °C/s hard band ~4 ticks into the spike and the FAST 3-tick
    // debounce latches ~2 ticks later — the hard-band protection is not
    // degraded by the two-tier change.
    let _guard = acquire_lock();
    let mut ctrl = build_control();
    let t0 = Instant::now();

    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("START");
    ctrl.process_artisan_command(ArtisanCommand::SetTargetTemp(200.0))
        .expect("SV 200");

    let mut bt = 90.0f32;
    for i in 0..4u64 {
        let t = tick_time(t0, i);
        poll_read(&mut ctrl, t);
        tick_at(&mut ctrl, bt, bt + 10.0, t).expect("seed");
    }

    let mut fired = false;
    let hard_step = (MAX_BT_RATE_OF_RISE_HARD + 0.5) * (TICK_MS as f32 / 1000.0); // 1.5 °C/s
                                                                                  // Enter the hard band ~4 spike ticks in (IIR alpha 0.3); the FAST
                                                                                  // 3-tick debounce then latches — bound the loop on the production
                                                                                  // consecutive limit plus margin so a regressed debounce fails loudly.
    for i in 4..(4 + ROR_EXCEEDED_CONSECUTIVE_LIMIT as u64 + 12) {
        let t = tick_time(t0, i);
        poll_read(&mut ctrl, t);
        bt += hard_step;
        if tick_at(&mut ctrl, bt, bt + 10.0, t).is_err() {
            fired = true;
            break;
        }
    }
    assert!(fired, "a sustained hard-band runaway must still latch");
    assert_emergency_posture(&mut ctrl, tick_time(t0, 40));
    assert!(
        drain_output_lines()
            .iter()
            .any(|l| l.starts_with("ERR safety_fault Bean temperature rate-of-rise exceeded")),
        "the hard-band latch must announce itself on the wire"
    );
}

#[test]
fn light_roast_boundary_sustained_soft_band_trips_firmware_pid() {
    // A marginal-but-SUSTAINED climb (0.7 °C/s) must still abort: the
    // filtered derivative enters the soft band ~6 ticks in and the 12-tick
    // debounce latches ~10 ticks later (~5 s sustained).
    let _guard = acquire_lock();
    let mut ctrl = build_control();
    let t0 = Instant::now();

    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("START");
    ctrl.process_artisan_command(ArtisanCommand::SetTargetTemp(200.0))
        .expect("SV 200");

    let mut bt = 90.0f32;
    for i in 0..4u64 {
        let t = tick_time(t0, i);
        poll_read(&mut ctrl, t);
        tick_at(&mut ctrl, bt, bt + 10.0, t).expect("seed");
    }

    let mut fired = false;
    let soft_step = (MAX_BT_RATE_OF_RISE + 0.2) * (TICK_MS as f32 / 1000.0); // 0.7 °C/s
                                                                             // Enters the soft band ~3 spike ticks in (IIR alpha 0.3); the extended
                                                                             // 12-tick debounce then latches — bound the loop on the production
                                                                             // debounce limit plus margin.
    for i in 4..(4 + ROR_SOFT_DEBOUNCE_LIMIT as u64 + 12) {
        let t = tick_time(t0, i);
        poll_read(&mut ctrl, t);
        bt += soft_step;
        if tick_at(&mut ctrl, bt, bt + 10.0, t).is_err() {
            fired = true;
            break;
        }
    }
    assert!(
        fired,
        "a SUSTAINED soft-band climb must latch after the extended debounce"
    );
    assert_emergency_posture(&mut ctrl, tick_time(t0, 40));
}

#[test]
fn light_roast_boundary_slow_finish_does_not_trip_probe_stuck() {
    // A very slow manual light finish: RoR 0.05 °C/s (3 °C/min) with the
    // heater on. BT moves 6 °C per 120 s — far above the 1 °C probe-stuck
    // variation — so neither the 120 s warning nor the 300 s manual latch
    // may fire over a 400 s window (A-TC4-C two-stage detector).
    let _guard = acquire_lock();
    let mut ctrl = build_control();
    let t0 = Instant::now();

    ctrl.process_artisan_command(ArtisanCommand::SetHeater(40))
        .expect("OT1 40");
    drain_output_lines(); // clear stale lines left by earlier tests in this binary

    let mut bt = 195.0f32;
    for i in 0..1300u64 {
        // 403 simulated seconds — past the 300 s manual latch threshold.
        let t = tick_time(t0, i);
        if i % READ_EVERY_TICKS == 0 {
            poll_read(&mut ctrl, t);
        }
        bt += 0.0155; // 0.05 °C/s
        tick_at(&mut ctrl, bt, bt + 10.0, t).expect("slow finish");
    }
    assert!(
        !ctrl.safety().is_emergency_active(),
        "a slow finish with live BT must not latch probe-stuck"
    );
    let lines = drain_output_lines();
    assert!(
        !lines.iter().any(|l| l == "ERR probe_stuck_warning"),
        "a live probe must not even reach the warning stage"
    );
    assert!(
        !lines.iter().any(|l| l.starts_with("ERR safety_fault")),
        "no safety fault on a healthy slow finish"
    );
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
