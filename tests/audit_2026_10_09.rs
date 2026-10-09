//! Regression tests for the 2026-10-09 audit (B1..B9). Harness copied from
//! tests/audit_2026_10_06.rs (production-shaped 320 ms ticks, EMA α 0.2,
//! READ polling) plus the TUNE plant from tests/diff_features.rs.
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


/// Integrating plant with 8 s dead time (copied from tests/diff_features.rs).
fn tune_plant_run(
    s: &mut Sim,
    secs: f32,
    bt: &mut f32,
    hist: &mut std::collections::VecDeque<f32>,
) -> f32 {
    let delay = (8.0 / DT) as usize;
    let mut max_u = 0.0f32;
    for _ in 0..(secs / DT) as u64 {
        if s.n.is_multiple_of(6) {
            s.cmd(ArtisanCommand::ReadStatus);
        }
        s.tick(*bt, *bt + 20.0);
        let u = s.c.get_status().ssr_output;
        max_u = max_u.max(u);
        hist.push_back(u);
        let delayed = if hist.len() > delay {
            hist[hist.len() - 1 - delay]
        } else {
            40.0
        };
        *bt += (0.005 + 0.003 * (delayed - 40.0)) * DT;
    }
    max_u
}

/// Door opened ~2 s with the probe in air: BT reads 12 °C low, then recovers
/// over 30 s while the plant keeps running (same shape as audit test A4).
fn door_dip(s: &mut Sim, bt: &mut f32) {
    let mut dip = 12.0f32;
    for i in 0..((32.0 / DT) as u64) {
        if s.n.is_multiple_of(6) {
            s.cmd(ArtisanCommand::ReadStatus);
        }
        if i >= 7 {
            dip = (dip - 12.0 * DT / 30.0).max(0.0);
        }
        s.tick(*bt - dip, *bt + 20.0);
        let u = s.c.get_status().ssr_output;
        *bt += (0.004 * u - 0.001 * (*bt - 25.0)) * DT;
    }
}

/// PID roast with a RoR profile; the operator takes over with `OT1;0` just
/// before the charge and presses CHARGE while in manual (audit test A5 shape).
fn charge_during_takeover(s: &mut Sim, bt: &mut f32) {
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    assert!(s.cmd(ArtisanCommand::PidOn));
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(200.0)));
    assert!(wire(s, "RORPROFILE;0,15;300,10;600,6"));
    plant_run(s, 600.0, bt, |_, _| {});
    assert!(s.cmd(ArtisanCommand::SetHeater(0)));
    plant_run(s, 5.0, bt, |_, _| {});
    assert!(s.cmd(ArtisanCommand::Charge(None)));
    *bt = 95.0;
}

// ── B1 (R-1): DROP → OT1 → PID;ON must not re-arm RoR-follow ──

#[test]
fn b1_drop_then_ot1_then_pid_on_does_not_rearm_ror_follow() {
    let _g = lock();
    let mut s = Sim::new();
    let mut bt = 150.0f32;
    let _tc = ror_roast(&mut s, &mut bt);
    plant_run(&mut s, 600.0, &mut bt, |_, _| {});
    assert!(s.cmd(ArtisanCommand::Drop));
    // The operator cuts the heat after the drop, then hands back to the PID
    // to hold the empty drum for the next batch.
    assert!(s.cmd(ArtisanCommand::SetHeater(0)));
    plant_run(&mut s, 120.0, &mut bt, |_, _| {});
    assert!(s.cmd(ArtisanCommand::PidOn));
    plant_run(&mut s, 600.0, &mut bt, |_, _| {});
    assert!(
        !s.c.ror_follow_active() && s.c.get_status().target_temp <= 215.0,
        "B1: empty drum ramped after DROP (SV {:.1}, RoR {:.1})",
        s.c.get_status().target_temp,
        s.c.ror_target_c_per_min()
    );
}

// ── B2 (R-2): the BT fall of the drop itself must not arm RoR-follow ──

#[test]
fn b2_bt_fall_after_drop_does_not_arm_ror_follow() {
    let _g = lock();
    let mut s = Sim::new();
    let mut bt = 150.0f32;
    let _tc = ror_roast(&mut s, &mut bt);
    plant_run(&mut s, 600.0, &mut bt, |_, _| {});
    assert!(s.cmd(ArtisanCommand::Drop));
    // Beans out: the probe is in air and BT falls 3 °C/s for 25 s while the
    // PID stays on (no PID;OFF after the DROP).
    for _ in 0..((25.0 / DT) as u64) {
        if s.n.is_multiple_of(6) {
            s.cmd(ArtisanCommand::ReadStatus);
        }
        bt -= 3.0 * DT;
        s.tick(bt, bt + 20.0);
    }
    assert!(
        s.c.get_status().charge_detected,
        "precondition: the drop fall trips #CHARGE"
    );
    plant_run(&mut s, 900.0, &mut bt, |_, _| {});
    assert!(
        !s.c.ror_follow_active() && s.c.get_status().target_temp <= 215.0,
        "B2: empty drum ramped after DROP (SV {:.1}, RoR {:.1})",
        s.c.get_status().target_temp,
        s.c.ror_target_c_per_min()
    );
}

// ── B3 (guard, green throughout): a CHARGE marker after a DROP still arms ──

#[test]
fn b3_charge_marker_after_drop_arms_ror_follow_for_next_batch() {
    let _g = lock();
    let mut s = Sim::new();
    let mut bt = 150.0f32;
    let _tc = ror_roast(&mut s, &mut bt);
    plant_run(&mut s, 600.0, &mut bt, |_, _| {});
    assert!(s.cmd(ArtisanCommand::Drop));
    // Back-to-back batch without PID;OFF: the PID holds the drum, then CHARGE.
    plant_run(&mut s, 300.0, &mut bt, |_, _| {});
    assert!(s.cmd(ArtisanCommand::Charge(None)));
    bt = 95.0;
    plant_run(&mut s, 300.0, &mut bt, |_, _| {});
    assert!(
        s.c.ror_follow_active() && s.c.ror_target_c_per_min() > 0.0,
        "B3: the next batch's CHARGE marker must arm RoR-follow"
    );
}

// ── B4 (R-3): CHARGE in Preheating + START after the drop must still ramp ──

#[test]
fn b4_charge_in_preheat_then_late_start_still_ramps() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    assert!(s.cmd(ArtisanCommand::Preheat(200.0)));
    assert!(wire(&mut s, "RORPROFILE;0,15;300,10;600,6"));
    s.run(300.0, |_| 200.0, |_| 220.0);
    assert!(s.cmd(ArtisanCommand::Charge(None)));
    // The beans go in; BT falls 200 → 105 °C in 45 s before START is pressed.
    let t0 = s.secs();
    s.run(
        45.0,
        move |t| (200.0 - (t - t0) * (95.0 / 45.0)).max(105.0),
        |_| 200.0,
    );
    assert!(s.cmd(ArtisanCommand::StartRoast));
    let mut bt = 105.0f32;
    plant_run(&mut s, 300.0, &mut bt, |_, _| {});
    assert!(
        s.c.ror_follow_active() && s.c.ror_target_c_per_min() > 0.0,
        "B4: RoR-follow never ramped (SV {:.1}, BT {:.1})",
        s.c.get_status().target_temp,
        bt
    );
}

// ── B5 (R-1): a door-dip #CHARGE during an OT1 takeover must not ramp ──

#[test]
fn b5_false_auto_charge_during_takeover_does_not_ramp_after_pid_on() {
    let _g = lock();
    let mut s = Sim::new();
    let mut bt = 150.0f32;
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    assert!(s.cmd(ArtisanCommand::PidOn));
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(200.0)));
    assert!(wire(&mut s, "RORPROFILE;0,15;300,10;600,6"));
    plant_run(&mut s, 600.0, &mut bt, |_, _| {});
    assert!(s.cmd(ArtisanCommand::SetHeater(40)));
    door_dip(&mut s, &mut bt);
    assert!(
        s.c.get_status().charge_detected,
        "precondition: the dip trips #CHARGE"
    );
    plant_run(&mut s, 200.0, &mut bt, |_, _| {});
    assert!(s.cmd(ArtisanCommand::PidOn));
    plant_run(&mut s, 600.0, &mut bt, |_, _| {});
    assert!(
        s.c.ror_target_c_per_min() == 0.0 && s.c.get_status().target_temp <= 205.0,
        "B5: empty drum ramped to SV {:.1} (RoR {:.1})",
        s.c.get_status().target_temp,
        s.c.ror_target_c_per_min()
    );
}

// ── B6 (R-4): PID;SV one tick after a resume must not end RoR-follow ──

#[test]
fn b6_sv_one_tick_after_resume_keeps_ror_follow() {
    let _g = lock();
    let mut s = Sim::new();
    let mut bt = 150.0f32;
    charge_during_takeover(&mut s, &mut bt);
    assert!(s.cmd(ArtisanCommand::SetHeater(70)));
    plant_run(&mut s, 200.0, &mut bt, |_, _| {});
    assert!(s.cmd(ArtisanCommand::PidOn));
    plant_run(&mut s, DT, &mut bt, |_, _| {});
    // Artisan's PID ON may send PID;SV in the next serial burst.
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(200.0)));
    plant_run(&mut s, 60.0, &mut bt, |_, _| {});
    assert!(
        s.c.ror_follow_active() && s.c.ror_target_c_per_min() > 0.0,
        "B6: PID;SV right after PID;ON ended the resumed RoR-follow"
    );
}

// ── B7 (R-5): PREHEAT re-sent during Preheating keeps the CHARGE marker ──

#[test]
fn b7_preheat_resent_during_preheat_keeps_charge_marker() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    assert!(s.cmd(ArtisanCommand::Preheat(200.0)));
    s.run(60.0, |_| 200.0, |_| 220.0);
    assert!(s.cmd(ArtisanCommand::Charge(None)));
    s.run(1.0, |_| 199.0, |_| 220.0);
    assert!(s.cmd(ArtisanCommand::Preheat(205.0)));
    s.run(1.0, |_| 198.0, |_| 220.0);
    assert!(s.cmd(ArtisanCommand::StartRoast));
    s.run(2.0, |_| 190.0, |_| 220.0);
    assert!(
        s.c.get_status().charge_detected,
        "B7: a PREHEAT re-sent during Preheating dropped the CHARGE marker"
    );
}

// ── B8 (R-6): switching the PID channel unlocks the BT-tuned gains ──

#[test]
fn b8_channel_switch_unlocks_tuned_gains() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetHeater(40)));
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    let mut bt = 150.0f32;
    let mut hist = std::collections::VecDeque::new();
    tune_plant_run(&mut s, 20.0, &mut bt, &mut hist);
    assert!(wire(&mut s, "TUNE;20"));
    tune_plant_run(&mut s, 420.0, &mut bt, &mut hist);
    assert!(s.c.pid_gains_locked(), "precondition: TUNE locked the gains");
    // Re-sending the SAME channel (Artisan does it on connect) keeps the lock.
    assert!(s.cmd(ArtisanCommand::SetPidChannel(2)));
    assert!(s.c.pid_gains_locked(), "B8: PID;CHAN;2 re-sent must keep the lock");
    assert!(s.cmd(ArtisanCommand::SetPidChannel(1)));
    assert!(
        !s.c.pid_gains_locked(),
        "B8: BT-tuned gains stayed locked on the ET loop"
    );
}

// ── B9 (R-1): PID;SV after the turning point ends RoR-follow for good ──

#[test]
fn b9_sv_override_then_ot1_then_pid_on_does_not_rearm() {
    let _g = lock();
    let mut s = Sim::new();
    let mut bt = 150.0f32;
    let _tc = ror_roast(&mut s, &mut bt);
    plant_run(&mut s, 120.0, &mut bt, |_, _| {});
    assert!(s.c.ror_target_c_per_min() > 0.0, "precondition: ramping");
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(210.0)));
    plant_run(&mut s, 5.0, &mut bt, |_, _| {});
    assert!(s.cmd(ArtisanCommand::SetHeater(50)));
    plant_run(&mut s, 30.0, &mut bt, |_, _| {});
    assert!(s.cmd(ArtisanCommand::PidOn));
    plant_run(&mut s, 60.0, &mut bt, |_, _| {});
    assert!(
        !s.c.ror_follow_active(),
        "B9: OT1 + PID;ON re-armed RoR-follow after the operator's SV override"
    );
}
