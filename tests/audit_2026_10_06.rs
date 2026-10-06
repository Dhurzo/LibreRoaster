//! Regression tests for the 2026-10-06 audit (A1..A10). Harness copied
//! from tests/diff_features.rs. Formerly: Differentiation features (plan DIFF-2026-10-05): CHARGE/DROP markers,
//! RoR-follow, READ extra channels, step-test autotune, and the
//! cross-feature safety fuzz test. Every task appends its tests above the
//! marker line at the end of this file.
#![cfg(all(test, feature = "test", not(target_arch = "riscv32")))]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
// Harness helpers are shared by every task; some are unused until later tasks.
#![allow(dead_code)]

extern crate std;

use std::sync::Mutex;

use embassy_time::{Duration, Instant};
use libreroaster::common::{StubFan, StubHeater};
use libreroaster::config::ArtisanCommand;
use libreroaster::control::roaster_control::RoasterControl;
use libreroaster::hardware::sensors::SensorConversionHub;

static TEST_MUTEX: Mutex<()> = Mutex::new(());
fn lock() -> std::sync::MutexGuard<'static, ()> {
    let g = TEST_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
    TEST_MUTEX.clear_poison();
    g
}
/// Real embedded cadence: sample stamped BEFORE the 210 ms wait, control after it; period ≈ 320 ms.
const PERIOD_MS: u64 = 320;
const CONV_MS: u64 = 215;

struct Sim {
    c: RoasterControl,
    t0: Instant,
    n: u64,
    ema_bt: Option<f32>,
    ema_et: Option<f32>,
    first_fault: Option<u64>,
}
impl Sim {
    fn new() -> Self {
        let c = RoasterControl::new(
            Box::new(StubHeater::new()),
            Box::new(StubFan::new()),
            SensorConversionHub::new(),
        )
        .expect("build");
        Self {
            c,
            t0: Instant::now(),
            n: 0,
            ema_bt: None,
            ema_et: None,
            first_fault: None,
        }
    }
    fn now(&self) -> Instant {
        self.t0 + Duration::from_millis(self.n * PERIOD_MS)
    }
    fn secs(&self) -> f32 {
        (self.n * PERIOD_MS) as f32 / 1000.0
    }
    /// Command at the synthetic instant; patches the real-clock comms-idle stamp.
    fn cmd(&mut self, c: ArtisanCommand) -> bool {
        let ok = self.c.process_artisan_command(c).is_ok();
        let t = self.now();
        self.c.status_mut().last_command_received_at_ms = t.as_millis();
        ok
    }
    /// One production-shaped tick with raw temps (EMA α=0.2 as the hub does).
    fn tick(&mut self, bt_raw: f32, et_raw: f32) {
        let a = 0.2;
        let bt = self.ema_bt.map_or(bt_raw, |p| a * bt_raw + (1.0 - a) * p);
        let et = self.ema_et.map_or(et_raw, |p| a * et_raw + (1.0 - a) * p);
        self.ema_bt = Some(bt);
        self.ema_et = Some(et);
        let ts = self.now();
        let _ = self.c.update_temperatures(bt, et, ts);
        let _ = self.c.update_control(ts + Duration::from_millis(CONV_MS));
        if self.first_fault.is_none() && self.c.safety().is_emergency_active() {
            self.first_fault = Some(self.n);
        }
        self.n += 1;
    }
    /// Run for `secs`, READ every 2 s (Artisan-like polling).
    fn run(&mut self, secs: f32, mut bt: impl FnMut(f32) -> f32, mut et: impl FnMut(f32) -> f32) {
        let ticks = (secs * 1000.0 / PERIOD_MS as f32) as u64;
        for _ in 0..ticks {
            if self.n.is_multiple_of(6) {
                self.cmd(ArtisanCommand::ReadStatus);
            }
            let s = self.secs();
            self.tick(bt(s), et(s));
        }
    }
    fn fault_at_s(&self) -> Option<f32> {
        self.first_fault.map(|n| (n * PERIOD_MS) as f32 / 1000.0)
    }
}
/// Deterministic pseudo-noise in [-1, 1].
#[allow(dead_code)]
struct Lcg(u64);
#[allow(dead_code)]
impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f32 / (1u64 << 31) as f32) * 2.0 - 1.0
    }
}

use libreroaster::input::parser::parse_artisan_command;

/// Seconds per Sim tick.
const DT: f32 = PERIOD_MS as f32 / 1000.0;

/// Closed-loop plant helper: BT reacts to the heater the firmware actually
/// applied. dBT/dt = 0.004·u − 0.001·(BT − 25) (°C/s). ET = BT + 20.
/// READ every 6 ticks keeps the comms-idle guard quiet.
fn plant_run(s: &mut Sim, secs: f32, bt: &mut f32, mut each: impl FnMut(&Sim, f32)) {
    let ticks = (secs / DT) as u64;
    for _ in 0..ticks {
        if s.n.is_multiple_of(6) {
            s.cmd(ArtisanCommand::ReadStatus);
        }
        s.tick(*bt, *bt + 20.0);
        let u = s.c.get_status().ssr_output;
        *bt += (0.004 * u - 0.001 * (*bt - 25.0)) * DT;
        each(s, *bt);
    }
}

/// Parse a wire line and feed it to the controller like the transport does.
fn wire(s: &mut Sim, line: &str) -> bool {
    match parse_artisan_command(line) {
        Ok(cmd) => s.cmd(cmd),
        Err(_) => false,
    }
}

/// PID;ON, SV 200, RoR profile 15→10→6 °C/min, 600 s plant preheat, CHARGE
/// (BT jumps to 95). Returns the charge time (s).
fn ror_roast(s: &mut Sim, bt: &mut f32) -> f32 {
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    assert!(s.cmd(ArtisanCommand::PidOn));
    let now = s.now();
    s.c.set_profile_start_for_test(now);
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(200.0)));
    assert!(wire(s, "RORPROFILE;0,15;300,10;600,6"));
    plant_run(s, 600.0, bt, |_, _| {});
    assert!(s.cmd(ArtisanCommand::Charge(None)));
    *bt = 95.0;
    s.secs()
}

// ── A1 (X1): a frozen BT probe must be caught while RoR-follow ramps ──

#[test]
fn a1_frozen_bt_during_ror_follow_latches_like_plain_pid() {
    let _g = lock();
    let mut s = Sim::new();
    let mut bt = 150.0f32;
    let tc = ror_roast(&mut s, &mut bt);
    let mut frozen: Option<f32> = None;
    let mut et = bt + 20.0;
    for _ in 0..((1200.0 / DT) as u64) {
        if s.n.is_multiple_of(6) {
            s.cmd(ArtisanCommand::ReadStatus);
        }
        let shown = if s.secs() - tc >= 120.0 {
            *frozen.get_or_insert(bt)
        } else {
            bt
        };
        s.tick(shown, et);
        if s.fault_at_s().is_some() {
            break;
        }
        let u = s.c.get_status().ssr_output;
        bt += (0.004 * u - 0.001 * (bt - 25.0)) * DT;
        et = bt + 20.0 + 0.3 * u; // ET follows the heater
    }
    let f = s
        .fault_at_s()
        .expect("A1: frozen BT under RoR-follow must latch");
    assert!(
        f - tc <= 300.0,
        "A1: latched {:.0} s after the charge (want ≤ 300)",
        f - tc
    );
}

// ── A2 (M-2): RoR-follow must advance with a long PID cycle time ──

#[test]
fn a2_ror_follow_advances_with_pid_ct_2000() {
    let _g = lock();
    let mut s = Sim::new();
    let mut bt = 150.0f32;
    assert!(s.cmd(ArtisanCommand::SetPidCycleTime(2000)));
    let _tc = ror_roast(&mut s, &mut bt);
    plant_run(&mut s, 400.0, &mut bt, |_, _| {});
    assert!(s.fault_at_s().is_none(), "fault at {:?}", s.fault_at_s());
    assert!(
        s.c.get_status().bean_temp > 140.0,
        "A2: RoR-follow stalled with PID;CT;2000, BT {:.1} 400 s after the charge",
        s.c.get_status().bean_temp
    );
}

// ── A3 (P-17): PID;SV before the turning point must not end RoR-follow ──

#[test]
fn a3_sv_before_turning_point_keeps_ror_follow() {
    let _g = lock();
    let mut s = Sim::new();
    let mut bt = 150.0f32;
    let _tc = ror_roast(&mut s, &mut bt);
    plant_run(&mut s, 0.7, &mut bt, |_, _| {}); // marker applied, follower armed
                                                // Artisan's first PID ON of a session sends PID;SV right after PID;ON.
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(200.0)));
    plant_run(&mut s, 120.0, &mut bt, |_, _| {});
    assert!(
        s.c.ror_follow_active(),
        "A3: SV before the turning point ended RoR-follow"
    );
    assert!(
        s.c.ror_target_c_per_min() > 0.0,
        "A3: RoR-follow never ramped"
    );
}

#[test]
fn a3b_sv_after_turning_point_still_ends_ror_follow() {
    let _g = lock();
    let mut s = Sim::new();
    let mut bt = 150.0f32;
    let _tc = ror_roast(&mut s, &mut bt);
    plant_run(&mut s, 120.0, &mut bt, |_, _| {});
    assert!(s.c.ror_target_c_per_min() > 0.0, "precondition: ramping");
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(210.0)));
    plant_run(&mut s, 5.0, &mut bt, |_, _| {});
    assert!(
        !s.c.ror_follow_active(),
        "operator override after the turning point"
    );
}

// ── A4 (M-1): a door-dip false #CHARGE must not ramp an empty drum ──

#[test]
fn a4_false_auto_charge_in_empty_drum_never_ramps() {
    let _g = lock();
    let mut s = Sim::new();
    let mut bt = 150.0f32;
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    assert!(s.cmd(ArtisanCommand::PidOn));
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(200.0)));
    assert!(wire(&mut s, "RORPROFILE;0,15;300,10;600,6"));
    plant_run(&mut s, 600.0, &mut bt, |_, _| {});
    // Door opened ~2 s with the probe in air: BT reads 12 °C low, then
    // recovers over 30 s (plant keeps running).
    let mut dip = 12.0f32;
    for i in 0..((32.0 / DT) as u64) {
        if s.n.is_multiple_of(6) {
            s.cmd(ArtisanCommand::ReadStatus);
        }
        if i >= 7 {
            dip = (dip - 12.0 * DT / 30.0).max(0.0);
        }
        s.tick(bt - dip, bt + 20.0);
        let u = s.c.get_status().ssr_output;
        bt += (0.004 * u - 0.001 * (bt - 25.0)) * DT;
    }
    assert!(
        s.c.get_status().charge_detected,
        "precondition: the dip trips #CHARGE"
    );
    plant_run(&mut s, 900.0, &mut bt, |_, _| {});
    assert!(
        s.c.ror_target_c_per_min() == 0.0 && s.c.get_status().target_temp <= 205.0,
        "A4: empty drum ramped to SV {:.1} (RoR {:.1})",
        s.c.get_status().target_temp,
        s.c.ror_target_c_per_min()
    );
}

// ── A5 (M-7): a charge during an OT1 takeover must still give RoR-follow ──

#[test]
fn a5_charge_during_ot1_takeover_rearms_on_pid_on() {
    let _g = lock();
    let mut s = Sim::new();
    let mut bt = 150.0f32;
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    assert!(s.cmd(ArtisanCommand::PidOn));
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(200.0)));
    assert!(wire(&mut s, "RORPROFILE;0,15;300,10;600,6"));
    plant_run(&mut s, 600.0, &mut bt, |_, _| {});
    assert!(s.cmd(ArtisanCommand::SetHeater(0))); // operator cuts heat for the charge
    plant_run(&mut s, 5.0, &mut bt, |_, _| {});
    assert!(s.cmd(ArtisanCommand::Charge(None)));
    bt = 95.0;
    plant_run(&mut s, 5.0, &mut bt, |_, _| {});
    assert!(s.cmd(ArtisanCommand::PidOn));
    plant_run(&mut s, 300.0, &mut bt, |_, _| {});
    assert!(
        s.c.ror_follow_active() && s.c.ror_target_c_per_min() > 0.0,
        "A5: PID;ON after a charge in manual must re-arm RoR-follow"
    );
}

// ── A6 (M-4): PID;SV from Idle must not inherit the manual detector clock ──

#[test]
fn a6_sv_from_idle_after_flat_manual_starts_fresh_window() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetHeater(30)));
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    s.run(15.0, |_| 150.0, |_| 160.0);
    let t0 = s.secs();
    s.run(150.0, |_| 150.0, |t| 160.0 + 10.0 * ((t - t0) / 150.0));
    assert!(s.fault_at_s().is_none());
    let t_sv = s.secs();
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(200.0)));
    s.run(30.0, |_| 150.0, |t| 170.0 + 0.3 * (t - t_sv));
    assert!(
        s.fault_at_s().is_none(),
        "A6: PID;SV from Idle latched at {:?}",
        s.fault_at_s()
    );
}

// ── A7 (M-3): PREHEAT drops a CHARGE marker that predates it ──

#[test]
fn a7_preheat_drops_a_stale_charge_marker() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    assert!(s.cmd(ArtisanCommand::PidOn));
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(200.0)));
    s.run(60.0, |_| 200.0, |_| 220.0);
    assert!(s.cmd(ArtisanCommand::EmergencyStop));
    assert!(s.cmd(ArtisanCommand::Charge(None))); // stray press while latched
    s.run(1.0, |_| 199.0, |_| 220.0);
    assert!(s.cmd(ArtisanCommand::Preheat(180.0)));
    s.run(600.0, |_| 180.0, |_| 200.0);
    assert!(s.cmd(ArtisanCommand::PidOn));
    s.run(2.0, |_| 180.0, |_| 200.0);
    assert!(
        !s.c.get_status().charge_detected,
        "A7: a marker sent before PREHEAT anchored the roast started 10 min later"
    );
}

// ── A8 (P-2): CHARGE weight sent as a decimal is accepted ──

#[test]
fn a8_charge_weight_decimal_is_accepted() {
    assert_eq!(
        parse_artisan_command("CHARGE;250.0"),
        Ok(ArtisanCommand::Charge(Some(250)))
    );
    assert_eq!(
        parse_artisan_command("CHARGE;249.6"),
        Ok(ArtisanCommand::Charge(Some(250)))
    );
    assert_eq!(
        parse_artisan_command("CHARGE;0.0"),
        Ok(ArtisanCommand::Charge(None))
    );
    assert!(parse_artisan_command("CHARGE;-1").is_err());
    assert!(parse_artisan_command("CHARGE;70000").is_err());
    assert!(parse_artisan_command("CHARGE;abc").is_err());
}

// ── A9 (P-9): TUNE;STATUS and RORPROFILE;OFF work while latched ──

#[test]
fn a9_tune_status_and_ror_off_accepted_while_latched() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::EmergencyStop));
    assert!(
        wire(&mut s, "TUNE;STATUS"),
        "A9: TUNE;STATUS refused while latched"
    );
    assert!(
        wire(&mut s, "RORPROFILE;OFF"),
        "A9: RORPROFILE;OFF refused while latched"
    );
    assert!(
        !wire(&mut s, "TUNE;20"),
        "TUNE start must stay refused while latched"
    );
}

// ── A10 (M-6): TUNE is refused when the PID regulates ET ──

#[test]
fn a10_tune_refused_on_et_channel() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetPidChannel(1)));
    assert!(s.cmd(ArtisanCommand::SetHeater(40)));
    s.run(5.0, |_| 150.0, |_| 170.0);
    assert!(
        !wire(&mut s, "TUNE;20"),
        "A10: TUNE identifies BT; refuse it on PID;CHAN;1"
    );
    assert!(!s.c.tune_running());
}
