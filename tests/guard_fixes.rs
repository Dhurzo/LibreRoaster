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
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f32 / (1u64 << 31) as f32) * 2.0 - 1.0
    }
}

// ── R1 ──

#[test]
fn r1_ot1_5_is_delivered() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetHeater(5)));
    s.run(2.0, |_| 25.0, |_| 25.0);
    let st = s.c.get_status();
    assert_eq!(
        st.ssr_output, 5.0,
        "R1: OT1 5 must be delivered, got {}",
        st.ssr_output
    );
    assert_eq!(
        st.fan_output, 20.0,
        "R1: fan floor must apply with real heat, got {}",
        st.fan_output
    );
}

#[test]
fn r1_ot1_4_is_not_delivered_and_no_fan_floor() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetHeater(4)));
    s.run(2.0, |_| 25.0, |_| 25.0);
    let st = s.c.get_status();
    assert_eq!(
        st.ssr_output, 0.0,
        "R1: OT1 4 must snap to 0, got {}",
        st.ssr_output
    );
    assert_eq!(
        st.fan_output, 0.0,
        "R1: no fan floor without real heat, got {}",
        st.fan_output
    );
}

// ── R3: H1 ──

#[test]
fn h1_latch_recovery_starts_pid_clean() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::StartRoast));
    // 120 s regulating around 225 ±1.
    let mut lcg = Lcg(1);
    let noise: Vec<f32> = (0..500).map(|_| lcg.next()).collect();
    let mut idx = 0usize;
    s.run(
        120.0,
        |t| {
            let n = noise[idx % noise.len()];
            idx += 1;
            225.0 + libm::sinf(t / 5.0) + 0.2 * n
        },
        |_| 245.0,
    );
    let i_before = s.c.dispatch().pid_integrator_value();
    // Internal latch 60 s with BT sagging to 215.
    s.c.emergency_shutdown("repro").ok();
    s.run(60.0, |_| 215.0, |_| 235.0);
    // Recovery with START.
    assert!(s.cmd(ArtisanCommand::StartRoast));
    s.run(0.33, |_| 215.0, |_| 235.0);
    let i_after = s.c.dispatch().pid_integrator_value();
    assert!(
        (i_after - i_before).abs() < 50.0 && i_after.abs() < 20.0,
        "H1: integrator must restart clean, before={:.1} after={:.1}",
        i_before,
        i_after
    );
    // With BT above SV the PID must back off quickly.
    let mut backed_off = false;
    for _ in 0..3 {
        s.tick(230.0, 250.0);
        if s.c.last_desired_heater_output() < 99.9 {
            backed_off = true;
            break;
        }
    }
    assert!(
        backed_off,
        "H1: with BT 230 > SV the PID must back off <99.9% within 3 ticks, got {:.1}%",
        s.c.last_desired_heater_output()
    );
}

// ── R3: H2 ──

#[test]
fn h2_manual_preheat_does_not_eat_roast_budget() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetHeater(70)));
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    // 25 min: ramp 25→180 in 10 min, then 180 ±2 sin.
    s.run(
        1500.0,
        |t| {
            if t < 600.0 {
                25.0 + 155.0 * t / 600.0
            } else {
                180.0 + 2.0 * libm::sinf(t / 30.0)
            }
        },
        |t| {
            if t < 600.0 {
                30.0 + 170.0 * t / 600.0
            } else {
                200.0
            }
        },
    );
    // OT1 0 for 30 s (below the 60 s debounce — session stays open).
    assert!(s.cmd(ArtisanCommand::SetHeater(0)));
    s.run(30.0, |_| 178.0, |_| 200.0);
    // New heat + charge dip 178→95 in 60 s.
    assert!(s.cmd(ArtisanCommand::SetHeater(80)));
    let t_charge = s.secs();
    s.run(
        60.0,
        |t| {
            let dt = t - t_charge;
            178.0 - 83.0 * (dt / 60.0)
        },
        |_| 210.0,
    );
    // 12 min roast at +0.18 °C/s from ~95.
    let t_roast = s.secs();
    s.run(720.0, |t| 95.0 + 0.18 * (t - t_roast), |_| 220.0);
    assert!(
        s.fault_at_s().is_none(),
        "H2: 25 min preheat + roast must not trip, fault at {:?}",
        s.fault_at_s()
    );
}

#[test]
fn h2_manual_session_cap_fires() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetHeater(60)));
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    // BT 150 ±2 (30 s sin), ET 200, 95 min.
    s.run(
        5700.0,
        |t| 150.0 + 2.0 * libm::sinf(t * core::f32::consts::TAU / 30.0),
        |_| 200.0,
    );
    let f = s
        .fault_at_s()
        .expect("H2-cap: manual 90 min cap must latch");
    assert!(
        (5400.0..=5460.0).contains(&f),
        "H2-cap: latch must land at 5400 s, got {:.1}",
        f
    );
}

// ── R3: H3 ──

#[test]
fn h3_ot1_after_pid_off_releases_fan() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetHeater(60)));
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    s.run(60.0, |_| 190.0, |_| 210.0);
    assert!(s.cmd(ArtisanCommand::Stop)); // PID;OFF
    s.run(30.0, |_| 190.0, |_| 210.0);
    assert!(s.cmd(ArtisanCommand::SetHeater(60)));
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    s.run(120.0, |_| 190.0, |_| 210.0);
    assert_eq!(
        s.c.get_status().fan_output,
        40.0,
        "H3: operator fan must win after PID;OFF + sliders"
    );
}

#[test]
fn h3_stop_then_pid_off_then_sliders() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetHeater(60)));
    s.run(30.0, |_| 190.0, |_| 210.0);
    assert!(s.cmd(ArtisanCommand::EmergencyStop)); // STOP
                                                   // OT1 while latched must be rejected.
    assert!(
        !s.cmd(ArtisanCommand::SetHeater(60)),
        "OT1 while latched must be rejected"
    );
    assert!(s.cmd(ArtisanCommand::Stop)); // PID;OFF recovery
    assert!(s.cmd(ArtisanCommand::SetHeater(60)));
    assert!(s.cmd(ArtisanCommand::SetFan(30)));
    s.run(60.0, |_| 190.0, |_| 210.0);
    assert_eq!(
        s.c.get_status().fan_output,
        30.0,
        "H3b: fan must follow sliders after STOP → PID;OFF"
    );
}

// ── R3: H6 ──

#[test]
fn h6_sliders_after_start_disarm_ror() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::StartRoast));
    assert!(s.cmd(ArtisanCommand::SetHeater(80)));
    // Hard-band ramp 1.4 °C/s for 20 s, ET 220.
    let t0 = s.secs();
    s.run(20.0, |t| 180.0 + 1.4 * (t - t0), |_| 220.0);
    assert!(
        s.fault_at_s().is_none(),
        "H6: START+OT1 (manual takeover) must disarm RoR, fault at {:?}",
        s.fault_at_s()
    );
}

#[test]
fn h6_fw_pid_ror_still_armed() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::StartRoast));
    // Same hard ramp but firmware PID stays in control.
    let t0 = s.secs();
    s.run(20.0, |t| 180.0 + 1.4 * (t - t0), |_| 220.0);
    let f = s.fault_at_s().expect("H6: FW-PID hard ramp must latch");
    assert!(
        f < 5.0,
        "H6: FW-PID hard ramp must latch in <5 s, got {:.2}",
        f
    );
}

// ── R3: H8 ──

#[test]
fn h8_manual_equilibrium_hold() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetHeater(35)));
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    s.run(420.0, |t| 180.0 + 0.3 * libm::sinf(t / 10.0), |_| 210.0);
    assert!(
        s.fault_at_s().is_none(),
        "H8: hot manual equilibrium must not latch, fault at {:?}",
        s.fault_at_s()
    );
}

#[test]
fn h8_shorted_probe_latches() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetHeater(30)));
    s.run(420.0, |_| 0.0, |_| 150.0);
    let f = s.fault_at_s().expect("H8: shorted probe must latch");
    assert!(
        (295.0..=320.0).contains(&f),
        "H8: short must latch ~300 s, got {:.1}",
        f
    );
}

// ── R3: H11 / H12 ──

#[test]
fn h11_pid_on_while_latched_is_rejected() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetHeater(60)));
    s.run(10.0, |_| 150.0, |_| 180.0);
    s.c.emergency_shutdown("repro").ok();
    // PidOn must be rejected while latched.
    assert!(
        !s.cmd(ArtisanCommand::PidOn),
        "H11: PidOn while latched must be rejected"
    );
    assert!(s.c.safety().is_emergency_active());
    assert_eq!(s.c.get_status().ssr_output, 0.0);
    // START is the sanctioned recovery.
    assert!(s.cmd(ArtisanCommand::StartRoast));
    assert!(!s.c.safety().is_emergency_active());
}

#[test]
fn h12_start_keeps_preheat_target() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::Preheat(200.0)));
    s.run(5.0, |_| 100.0, |_| 120.0);
    assert!(s.cmd(ArtisanCommand::StartRoast));
    assert_eq!(
        s.c.get_status().target_temp,
        200.0,
        "H12: START must inherit PREHEAT target"
    );
}

// ── R3: day tests ──

#[test]
fn day_manual_two_batches() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetHeater(90)));
    assert!(s.cmd(ArtisanCommand::SetFan(30)));
    // 20 min: 12 min ramp to 200, then 200 ±1.5.
    s.run(
        1200.0,
        |t| {
            if t < 720.0 {
                25.0 + 175.0 * t / 720.0
            } else {
                200.0 + 1.5 * libm::sinf(t / 20.0)
            }
        },
        |t| {
            if t < 720.0 {
                30.0 + 190.0 * t / 720.0
            } else {
                220.0
            }
        },
    );
    // Charge 200→95 in 70 s.
    let t_charge = s.secs();
    assert!(s.cmd(ArtisanCommand::SetHeater(80)));
    s.run(70.0, |t| 200.0 - 105.0 * (t - t_charge) / 70.0, |_| 220.0);
    // 11 min at +0.19 °C/s.
    let t_roast = s.secs();
    s.run(660.0, |t| 95.0 + 0.19 * (t - t_roast), |_| 230.0);
    assert!(
        s.fault_at_s().is_none(),
        "DAY-manual batch 1 must not latch, fault at {:?}",
        s.fault_at_s()
    );
    assert!(s.cmd(ArtisanCommand::Stop)); // PID;OFF
                                          // 4 min cool towards 150.
    let t_cool = s.secs();
    let bt_cool_start = 95.0 + 0.19 * 660.0;
    s.run(
        240.0,
        |t| bt_cool_start - (bt_cool_start - 150.0) * (t - t_cool) / 240.0,
        |_| 180.0,
    );
    assert!(s.cmd(ArtisanCommand::SetHeater(70)));
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    s.run(30.0, |_| 150.0, |_| 180.0);
    assert!(
        s.fault_at_s().is_none(),
        "DAY-manual batch 2 must not latch, fault at {:?}",
        s.fault_at_s()
    );
    let st = s.c.get_status();
    assert_eq!(st.ssr_output, 70.0);
    assert_eq!(st.fan_output, 40.0);
}

#[test]
fn day_fw_pid_preheat_start() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::Preheat(200.0)));
    // 20 min preheat ramp.
    s.run(
        1200.0,
        |t| 25.0 + 165.0 * t / 1200.0,
        |t| 30.0 + 180.0 * t / 1200.0,
    );
    assert!(s.cmd(ArtisanCommand::StartRoast));
    // Re-anchor the roast budget to synthetic time (real-clock artefact).
    let now = s.now();
    s.c.set_profile_start_for_test(now);
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(215.0))); // PID;SV;215
                                                          // Charge + 11 min at +0.19 °C/s from ~95.
    let t_charge = s.secs();
    s.run(70.0, |t| 190.0 - 95.0 * (t - t_charge) / 70.0, |_| 220.0);
    let t_roast = s.secs();
    s.run(660.0, |t| 95.0 + 0.19 * (t - t_roast), |_| 230.0);
    assert!(
        s.fault_at_s().is_none(),
        "DAY-fw batch 1 must not latch, fault at {:?}",
        s.fault_at_s()
    );
    assert!(s.cmd(ArtisanCommand::Stop)); // PID;OFF
    assert!(s.cmd(ArtisanCommand::SetHeater(70)));
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    s.run(30.0, |_| 150.0, |_| 180.0);
    assert!(
        s.fault_at_s().is_none(),
        "DAY-fw batch 2 must not latch, fault at {:?}",
        s.fault_at_s()
    );
    assert_eq!(s.c.get_status().fan_output, 40.0);
}

// ── R2 ──

#[test]
fn r2_frozen_hot_bt_with_moving_et_latches() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetHeater(70)));
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    // BT frozen at 180, ET rises 0.1 °C/s (leaves the 3 °C band at ~30 s).
    s.run(420.0, |_| 180.0, |t| 180.0 + 0.1 * t);
    let f = s
        .fault_at_s()
        .expect("R2: frozen BT with moving ET must latch");
    assert!(
        (330.0..=360.0).contains(&f),
        "R2: latch must land 300 s after leaving the ET band (~330 s), got {:.1}",
        f
    );
}

#[test]
fn r2_slow_et_drift_with_flat_bt_stays_healthy_for_10min() {
    // Decision case from the plan: ET drifts +4 °C in 10 min, BT ±0.5 °C.
    // With a 3 °C band the drift leaves the band at ~7.5 min and the 300 s
    // window has not elapsed by minute 10 — no latch yet. If this ever
    // latches within 10 min the band is too tight.
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetHeater(35)));
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    let t0 = s.secs();
    s.run(
        600.0,
        |t| 180.0 + 0.5 * libm::sinf((t - t0) / 20.0),
        |t| 200.0 + 4.0 * (t - t0) / 600.0,
    );
    assert!(
        s.fault_at_s().is_none(),
        "R2-decision: slow ET drift must not latch within 10 min, fault at {:?}",
        s.fault_at_s()
    );
}
