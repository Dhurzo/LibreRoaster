#![cfg(all(test, feature = "test", not(target_arch = "riscv32")))]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

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
            if self.n % 6 == 0 {
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

// ── Plan 2026-10-05 ──

use libreroaster::config::constants::{
    ET_OVERTEMP_THRESHOLD, MAX_MANUAL_HEAT_SESSION_SECS, OVERTEMP_THRESHOLD,
};

fn charge_curve(t: f32, t_charge: f32, from: f32, to: f32) -> f32 {
    to + (from - to) * libm::expf(-(t - t_charge) / 12.0)
}

#[test]
fn t1_hot_et_below_et_cutoff_does_not_trip() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetHeater(60)));
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    s.run(60.0, |t| 180.0 + 0.1 * t, |_| 270.0);
    assert!(
        s.fault_at_s().is_none(),
        "T1: ET 270 must not trip, fault at {:?}",
        s.fault_at_s()
    );
}
#[test]
fn t1_et_above_et_cutoff_trips() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetHeater(60)));
    s.run(10.0, |_| 180.0, |_| ET_OVERTEMP_THRESHOLD + 10.0);
    assert!(s.fault_at_s().is_some());
}
#[test]
fn t1_bt_cutoff_unchanged() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetHeater(60)));
    s.run(5.0, |_| OVERTEMP_THRESHOLD + 1.0, |_| 200.0);
    assert!(s.fault_at_s().is_some());
}
#[test]
fn t2_sv_above_bt_cutoff_is_capped() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(280.0)));
    assert_eq!(s.c.get_status().target_temp, OVERTEMP_THRESHOLD - 10.0);
}
#[test]
fn t2_et_channel_allows_higher_sv_and_recaps_on_switch_to_bt() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetPidChannel(1)));
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(280.0)));
    assert_eq!(s.c.get_status().target_temp, 280.0);
    assert!(s.cmd(ArtisanCommand::SetPidChannel(2)));
    s.run(2.0, |_| 150.0, |_| 200.0);
    assert_eq!(s.c.get_status().target_temp, OVERTEMP_THRESHOLD - 10.0);
}
#[test]
fn t2_normal_sv_untouched() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(225.0)));
    assert_eq!(s.c.get_status().target_temp, 225.0);
}
fn pid_on_preheat_charge_roast(s: &mut Sim, roast_secs: f32) {
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    assert!(s.cmd(ArtisanCommand::PidOn));
    let now = s.now();
    s.c.set_profile_start_for_test(now);
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(200.0)));
    s.run(
        1500.0,
        |t| 200.0 - 175.0 * libm::expf(-t / 300.0),
        |t| 220.0 - 190.0 * libm::expf(-t / 300.0),
    );
    let tc = s.secs();
    s.run(90.0, |t| charge_curve(t, tc, 200.0, 95.0), |_| 215.0);
    let tr = s.secs();
    s.run(roast_secs, |t| 95.0 + 0.18 * (t - tr), |_| 225.0);
}
#[test]
fn t3_pid_on_long_preheat_does_not_eat_roast_budget() {
    let _g = lock();
    let mut s = Sim::new();
    pid_on_preheat_charge_roast(&mut s, 720.0);
    assert!(
        s.fault_at_s().is_none(),
        "T3: fault at {:?}",
        s.fault_at_s()
    );
}
#[test]
fn t3_pid_on_cap_still_fires_30_min_after_charge() {
    let _g = lock();
    let mut s = Sim::new();
    pid_on_preheat_charge_roast(&mut s, 578.0);
    s.run(1900.0, |_| 199.0, |_| 225.0);
    let f = s
        .fault_at_s()
        .expect("T3: 30-min cap after charge must latch");
    assert!((3290.0..=3330.0).contains(&f), "T3: latch at {:.1}", f);
}
#[test]
fn t4_pid_unreachable_sv_plateau_does_not_latch() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    assert!(s.cmd(ArtisanCommand::PidOn));
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(240.0)));
    s.run(420.0, |_| 220.0, |_| 250.0);
    assert!(
        s.fault_at_s().is_none(),
        "T4: fault at {:?}",
        s.fault_at_s()
    );
}
#[test]
fn t4_pid_frozen_bt_with_moving_et_still_latches() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    assert!(s.cmd(ArtisanCommand::PidOn));
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(240.0)));
    s.run(300.0, |_| 180.0, |t| 180.0 + 0.2 * t);
    assert!(s.fault_at_s().is_some());
}
fn pid_then_manual_takeover(s: &mut Sim) {
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    assert!(s.cmd(ArtisanCommand::PidOn));
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(160.0)));
    s.run(30.0, |t| 150.0 + 0.1 * t, |_| 200.0);
    assert!(s.cmd(ArtisanCommand::SetHeater(60)));
    s.run(30.0, |t| 155.0 + 0.05 * (t - 30.0), |_| 200.0);
    assert!(
        !s.c.get_status().pid_enabled,
        "precondition: manual took over"
    );
    assert!(
        (s.c.get_status().ssr_output - 60.0).abs() < 0.5,
        "precondition: heater at 60 %, got {}",
        s.c.get_status().ssr_output
    );
}
#[test]
fn t6_pid_on_after_ot1_resumes_pid_without_bump() {
    let _g = lock();
    let mut s = Sim::new();
    pid_then_manual_takeover(&mut s);
    assert!(s.cmd(ArtisanCommand::PidOn));
    let st = s.c.get_status();
    assert!(
        st.pid_enabled && !st.artisan_control,
        "T6: PID;ON must resume the PID"
    );
    s.run(0.4, |_| 157.0, |_| 200.0);
    let out = s.c.last_desired_heater_output();
    assert!((50.0..=70.0).contains(&out), "T6: got {out}");
}
#[test]
fn t6_sv_after_ot1_mid_roast_is_bumpless() {
    let _g = lock();
    let mut s = Sim::new();
    pid_then_manual_takeover(&mut s);
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(162.0)));
    s.run(0.4, |_| 157.0, |_| 200.0);
    let out = s.c.last_desired_heater_output();
    assert!((50.0..=70.0).contains(&out), "T6: bumpless SV, got {out}");
}
#[test]
fn t6_no_preload_when_above_setpoint() {
    let _g = lock();
    let mut s = Sim::new();
    pid_then_manual_takeover(&mut s);
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(150.0)));
    s.run(0.4, |_| 157.0, |_| 200.0);
    assert!(
        s.c.last_desired_heater_output() < 1.0,
        "got {}",
        s.c.last_desired_heater_output()
    );
}
#[test]
fn t7_manual_three_batches_after_long_preheat_do_not_trip() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetHeater(80)));
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    s.run(
        2400.0,
        |t| {
            if t < 900.0 {
                25.0 + 175.0 * t / 900.0
            } else {
                200.0
            }
        },
        |t| {
            if t < 900.0 {
                30.0 + 190.0 * t / 900.0
            } else {
                220.0
            }
        },
    );
    for _ in 0..3 {
        let tc = s.secs();
        s.run(90.0, |t| charge_curve(t, tc, 200.0, 95.0), |_| 220.0);
        let tr = s.secs();
        s.run(720.0, |t| 95.0 + 0.18 * (t - tr), |_| 230.0);
        let tb = s.secs();
        s.run(300.0, |t| 225.0 - 25.0 * (t - tb) / 300.0, |_| 220.0);
    }
    assert!(s.secs() > MAX_MANUAL_HEAT_SESSION_SECS as f32);
    assert!(
        s.fault_at_s().is_none(),
        "T7: fault at {:?}",
        s.fault_at_s()
    );
}
