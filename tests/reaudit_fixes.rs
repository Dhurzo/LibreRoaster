//! Regression tests for the re-audit fixes (BUG_HUNT_2026-10-05-v2: N1, N2, N3,
//! N4, N11). Harness copied from tests/guard_fixes.rs.
#![cfg(all(test, feature = "test", not(target_arch = "riscv32")))]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
// Harness helpers are shared; some are unused in this file.
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

use libreroaster::config::constants::RoasterState;
use libreroaster::config::{ProfileSetpoint, RoastProfile};

// ── N1: probe-stuck clock must not be inherited across a manual→PID resume ──

#[test]
fn n1_resume_after_flat_manual_finish_starts_a_fresh_detector_window() {
    for via_pid_on in [true, false] {
        let _g = lock();
        let mut s = Sim::new();
        assert!(s.cmd(ArtisanCommand::SetFan(40)));
        assert!(s.cmd(ArtisanCommand::PidOn));
        assert!(s.cmd(ArtisanCommand::SetTargetTemp(230.0)));
        s.run(
            300.0,
            |t| 150.0 + 68.0 * (t / 300.0),
            |t| 170.0 + 55.0 * (t / 300.0),
        );
        assert!(s.cmd(ArtisanCommand::SetHeater(30)));
        s.run(15.0, |_| 218.0, |_| 225.0);
        let t0 = s.secs();
        // Manual slow finish: BT flat for 200 s, ET drifting +7 °C (no equilibrium exemption).
        s.run(200.0, |_| 218.0, |t| 225.0 + 7.0 * ((t - t0) / 200.0));
        assert!(
            s.fault_at_s().is_none(),
            "manual two-stage: no latch before 300 s"
        );
        let t_resume = s.secs();
        if via_pid_on {
            assert!(s.cmd(ArtisanCommand::PidOn));
        } else {
            assert!(s.cmd(ArtisanCommand::SetTargetTemp(230.0)));
        }
        // ET keeps moving (+0.3 °C/s) so the equilibrium exemption stays off.
        s.run(10.0, |_| 218.0, |t| 232.0 + 0.3 * (t - t_resume));
        assert!(
            s.fault_at_s().is_none(),
            "N1 (pid_on={via_pid_on}): resume must start a fresh window, fault at {:?}",
            s.fault_at_s()
        );
        // A probe that STAYS frozen must still latch: single-stage 120 s from the fresh anchor.
        s.run(130.0, |_| 218.0, |t| 232.0 + 0.3 * (t - t_resume));
        let f = s
            .fault_at_s()
            .expect("frozen BT under PID must still latch after the fresh window");
        assert!(
            (t_resume + 115.0..=t_resume + 140.0).contains(&f),
            "latch at {f:.1}, resume at {t_resume:.1}"
        );
    }
}

#[test]
fn n1b_pid_on_from_idle_after_flat_manual_starts_a_fresh_detector_window() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetHeater(30)));
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    s.run(15.0, |_| 218.0, |_| 225.0);
    let t0 = s.secs();
    // Manual hold: BT flat for 200 s, ET drifting +7 °C (no equilibrium exemption).
    s.run(200.0, |_| 218.0, |t| 225.0 + 7.0 * ((t - t0) / 200.0));
    assert!(
        s.fault_at_s().is_none(),
        "manual two-stage: no latch before 300 s"
    );
    let t_on = s.secs();
    assert!(s.cmd(ArtisanCommand::PidOn)); // default SV 225 °C: 7 °C away, so not "regulating"
    s.run(10.0, |_| 218.0, |t| 232.0 + 0.3 * (t - t_on));
    assert!(
        s.fault_at_s().is_none(),
        "N1b: PID;ON from Idle must start a fresh window, fault at {:?}",
        s.fault_at_s()
    );
    s.run(130.0, |_| 218.0, |t| 232.0 + 0.3 * (t - t_on));
    let f = s
        .fault_at_s()
        .expect("frozen BT under PID must still latch after the fresh window");
    assert!(
        (t_on + 115.0..=t_on + 140.0).contains(&f),
        "latch at {f:.1}, PID;ON at {t_on:.1}"
    );
}

#[test]
fn n1c_preheat_after_flat_manual_starts_a_fresh_detector_window() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetHeater(30)));
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    s.run(15.0, |_| 218.0, |_| 225.0);
    let t0 = s.secs();
    s.run(200.0, |_| 218.0, |t| 225.0 + 7.0 * ((t - t0) / 200.0));
    assert!(s.fault_at_s().is_none());
    let t_on = s.secs();
    assert!(s.cmd(ArtisanCommand::Preheat(240.0))); // 22 °C away: not "regulating"
    s.run(10.0, |_| 218.0, |t| 232.0 + 0.3 * (t - t_on));
    assert!(
        s.fault_at_s().is_none(),
        "N1c: PREHEAT must start a fresh window, fault at {:?}",
        s.fault_at_s()
    );
    s.run(130.0, |_| 218.0, |t| 232.0 + 0.3 * (t - t_on));
    let f = s
        .fault_at_s()
        .expect("frozen BT under the preheat PID must still latch after the fresh window");
    assert!(
        (t_on + 115.0..=t_on + 140.0).contains(&f),
        "latch at {f:.1}, PREHEAT at {t_on:.1}"
    );
}

// ── N2: roast anchors must not survive STOP + PREHEAT recovery ──

#[test]
fn n2_stop_then_preheat_and_ot1_has_no_stale_roast_anchor() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    assert!(s.cmd(ArtisanCommand::PidOn));
    let now = s.now();
    s.c.set_profile_start_for_test(now);
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(200.0)));
    s.run(
        1200.0,
        |t| 200.0 - 175.0 * libm::expf(-t / 500.0),
        |t| 220.0 - 190.0 * libm::expf(-t / 500.0),
    );
    let bt_end = 200.0 - 175.0 * libm::expf(-1200.0f32 / 500.0);
    let tc = s.secs();
    s.run(
        90.0,
        |t| 95.0 + (bt_end - 95.0) * libm::expf(-(t - tc) / 12.0),
        |_| 215.0,
    );
    assert!(
        s.c.get_status().charge_detected,
        "precondition: charge detected"
    );
    let tr = s.secs();
    s.run(300.0, |t| 95.0 + 0.18 * (t - tr), |_| 225.0);
    assert!(s.cmd(ArtisanCommand::EmergencyStop)); // wire STOP
    s.run(60.0, |_| 140.0, |_| 180.0);
    assert!(s.cmd(ArtisanCommand::Preheat(180.0))); // recovery, empty drum
    assert!(s.cmd(ArtisanCommand::SetHeater(50))); // operator takes over
    s.first_fault = None; // forget the deliberate STOP latch
    s.run(2400.0, |_| 180.0, |_| 200.0);
    assert!(
        s.fault_at_s().is_none(),
        "N2: stale charge anchor cut the preheat at {:?} (charge was at {tc:.0} s)",
        s.fault_at_s()
    );
    assert_ne!(s.c.get_state(), RoasterState::Error);
}

// ── N3: PID;T must not dump the integrator ──

#[test]
fn n3_pid_t_with_unchanged_gains_keeps_the_integrator() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    assert!(s.cmd(ArtisanCommand::PidOn));
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(200.0)));
    s.run(240.0, |_| 198.0, |_| 220.0);
    let before = s.c.last_desired_heater_output();
    assert!(
        before > 50.0,
        "precondition: the integrator carries the load, MV {before}"
    );
    // Artisan re-sends PID;T (its dialog gains) on every PID ON press.
    assert!(s.cmd(ArtisanCommand::SetPidGain(2.0, 0.25, 0.05)));
    s.run(0.4, |_| 198.0, |_| 220.0);
    let after = s.c.last_desired_heater_output();
    assert!(
        after >= 0.9 * before,
        "N3: MV dropped {before:.1} -> {after:.1}"
    );
}

#[test]
fn n3_pid_t_with_a_new_ki_keeps_the_i_contribution() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    assert!(s.cmd(ArtisanCommand::PidOn));
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(200.0)));
    s.run(240.0, |_| 198.0, |_| 220.0);
    let before = s.c.last_desired_heater_output();
    assert!(s.cmd(ArtisanCommand::SetPidGain(2.0, 0.5, 0.05))); // Ki doubled
    s.run(0.4, |_| 198.0, |_| 220.0);
    let after = s.c.last_desired_heater_output();
    assert!(
        (after - before).abs() <= 5.0,
        "N3: Ki change bumped MV {before:.1} -> {after:.1}"
    );
}

// ── N4: bounded equilibrium exemption in firmware-PID mode ──

#[test]
fn n4_both_probes_frozen_hot_under_pid_latch_within_10_min() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    assert!(s.cmd(ArtisanCommand::PidOn));
    let now = s.now();
    s.c.set_profile_start_for_test(now);
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(200.0)));
    // BT 150 / ET 180 frozen: equilibrium exemption applies, PID drives 100 %.
    s.run(900.0, |_| 150.0, |_| 180.0);
    let f = s
        .fault_at_s()
        .expect("N4: frozen hot probes under PID must latch within 10 min");
    assert!((600.0..=640.0).contains(&f), "latch at {f:.1}");
}

#[test]
fn n4_unreachable_sv_plateau_still_runs_for_5_min() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    assert!(s.cmd(ArtisanCommand::PidOn));
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(240.0)));
    s.run(280.0, |_| 220.0, |_| 250.0);
    assert!(
        s.fault_at_s().is_none(),
        "F-C7 behaviour kept for 5 min, fault at {:?}",
        s.fault_at_s()
    );
}

#[test]
fn n4_manual_equilibrium_stays_unbounded() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetHeater(70)));
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    s.run(1500.0, |_| 200.0, |_| 220.0);
    assert!(
        s.fault_at_s().is_none(),
        "R2: manual equilibrium must not latch, fault at {:?}",
        s.fault_at_s()
    );
}

#[test]
fn n4_low_duty_plateau_under_pid_is_not_bounded() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    assert!(s.cmd(ArtisanCommand::SetPidOutputLimits(0.0, 40.0))); // heater capped below 50 %
    assert!(s.cmd(ArtisanCommand::PidOn));
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(200.0)));
    s.run(900.0, |_| 150.0, |_| 180.0);
    assert!(
        s.fault_at_s().is_none(),
        "the bound needs heater >= 50 %, fault at {:?}",
        s.fault_at_s()
    );
    assert!(s.c.get_status().ssr_output <= 40.0);
}

// ── N11: profile setpoint above the cap is capped at START ──

#[test]
fn n11_profile_above_cap_is_capped_at_start() {
    let _g = lock();
    let mut s = Sim::new();
    let mut p = RoastProfile::new();
    p.setpoints
        .push(ProfileSetpoint {
            time_secs: 0,
            temperature: 300.0,
        })
        .unwrap();
    p.setpoints
        .push(ProfileSetpoint {
            time_secs: 600,
            temperature: 300.0,
        })
        .unwrap();
    libreroaster::input::parser::store_profile(p);
    assert!(s.cmd(ArtisanCommand::SetProfile));
    assert!(s.cmd(ArtisanCommand::StartRoast));
    assert_eq!(
        s.c.get_status().target_temp,
        250.0,
        "N11: the cap must apply at START, not one PID cycle later"
    );
}
