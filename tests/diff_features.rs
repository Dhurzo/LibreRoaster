//! Differentiation features (plan DIFF-2026-10-05): CHARGE/DROP markers,
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


use libreroaster::input::parser::parse_artisan_command;

/// Seconds per Sim tick.
const DT: f32 = PERIOD_MS as f32 / 1000.0;

/// Closed-loop plant helper: BT reacts to the heater the firmware actually
/// applied. dBT/dt = 0.004·u − 0.001·(BT − 25) (°C/s). ET = BT + 20.
/// READ every 6 ticks keeps the comms-idle guard quiet.
fn plant_run(s: &mut Sim, secs: f32, bt: &mut f32, mut each: impl FnMut(&Sim, f32)) {
    let ticks = (secs / DT) as u64;
    for _ in 0..ticks {
        if s.n % 6 == 0 {
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

// ── D2: CHARGE / DROP markers (E1) ──

#[test]
fn e1_parse_charge_and_drop() {
    assert_eq!(parse_artisan_command("CHARGE"), Ok(ArtisanCommand::Charge(None)));
    assert_eq!(parse_artisan_command("CHARGE;250"), Ok(ArtisanCommand::Charge(Some(250))));
    assert_eq!(parse_artisan_command("charge;0"), Ok(ArtisanCommand::Charge(None)));
    assert!(parse_artisan_command("CHARGE;-1").is_err());
    assert!(parse_artisan_command("CHARGE;1;2").is_err());
    assert_eq!(parse_artisan_command("DROP"), Ok(ArtisanCommand::Drop));
    assert!(parse_artisan_command("DROP;1").is_err());
}

#[test]
fn e1_markers_never_touch_actuators() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetHeater(60)));
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    s.run(30.0, |_| 180.0, |_| 200.0);
    assert!(s.cmd(ArtisanCommand::Charge(Some(250))));
    s.run(1.0, |_| 180.0, |_| 200.0);
    assert!(s.cmd(ArtisanCommand::Drop));
    s.run(1.0, |_| 180.0, |_| 200.0);
    let st = s.c.get_status();
    assert_eq!(st.ssr_output, 60.0);
    assert_eq!(st.fan_output, 40.0);
    assert!(s.fault_at_s().is_none());
    assert_eq!(s.c.batch_grams(), Some(250));
}

#[test]
fn e1_markers_accepted_while_latched() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::EmergencyStop));
    assert!(s.cmd(ArtisanCommand::Charge(None)), "CHARGE is a pure marker: no ERR while latched");
    assert!(s.cmd(ArtisanCommand::Drop), "DROP is a pure marker: no ERR while latched");
    s.run(1.0, |_| 180.0, |_| 200.0);
    assert_eq!(s.c.get_status().ssr_output, 0.0);
}

#[test]
fn e1_charge_restarts_manual_session_budget() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetHeater(70)));
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    // Hot equilibrium (BT and ET flat) for 5000 s — no automatic charge.
    s.run(5000.0, |_| 200.0, |_| 220.0);
    assert!(s.cmd(ArtisanCommand::Charge(None)));
    // Without the CHARGE marker the 90-min cap would latch at 5400 s.
    s.run(600.0, |_| 200.0, |_| 220.0);
    assert!(s.fault_at_s().is_none(), "fault at {:?}", s.fault_at_s());
}

#[test]
fn e1_explicit_charge_anchors_pid_on_budget() {
    let _g = lock();
    let mut s = Sim::new();
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
    assert!(s.cmd(ArtisanCommand::Charge(None)));
    let tc = s.secs();
    // SLOW drop (< 6 °C in 3 s): the automatic detector does NOT fire, so only
    // the explicit marker can anchor the 30-min budget.
    s.run(120.0, |t| 199.0 - 49.0 * (t - tc) / 120.0, |_| 215.0);
    let tr = s.secs();
    s.run(2000.0, |t| (150.0 + 0.15 * (t - tr)).min(199.0), |_| 225.0);
    let f = s.fault_at_s().expect("30-min cap after the explicit CHARGE must latch");
    assert!(
        (tc + 1795.0..=tc + 1815.0).contains(&f),
        "latch at {f:.1}, charge at {tc:.1}"
    );
}

#[test]
fn e1_drop_rearms_automatic_charge_detection() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    assert!(s.cmd(ArtisanCommand::PidOn));
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(200.0)));
    s.run(300.0, |t| 200.0 - 50.0 * libm::expf(-t / 60.0), |_| 220.0);
    assert!(s.cmd(ArtisanCommand::Charge(None)));
    s.run(2.0, |_| 199.0, |_| 220.0);
    assert!(s.c.get_status().charge_detected);
    assert!(s.cmd(ArtisanCommand::Drop));
    s.run(30.0, |_| 199.0, |_| 220.0);
    assert!(!s.c.get_status().charge_detected, "DROP re-arms detection");
    let tc = s.secs();
    s.run(30.0, |t| 95.0 + 104.0 * libm::expf(-(t - tc) / 12.0), |_| 215.0);
    assert!(s.c.get_status().charge_detected, "next batch charge auto-detected");
}

#[test]
fn e1_charge_then_pid_on_in_a_later_tick_anchors_the_roast() {
    // Artisan pidOnCHARGE: CHARGE marker, then the PID ON sequence. Here they
    // land in DIFFERENT control ticks (worst case).
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetHeater(70)));
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    s.run(30.0, |_| 200.0, |_| 220.0);
    assert!(s.cmd(ArtisanCommand::Charge(Some(300))));
    s.run(1.0, |_| 200.0, |_| 220.0); // marker applied to the manual session, kept pending
    assert!(!s.c.get_status().charge_detected);
    assert!(s.cmd(ArtisanCommand::PidOn));
    s.run(1.0, |_| 199.0, |_| 220.0);
    assert!(s.c.get_status().charge_detected, "roast anchored to the earlier CHARGE marker");
    assert_eq!(s.c.batch_grams(), Some(300));
}

#[test]
fn e1_charge_during_preheat_survives_until_start() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    assert!(s.cmd(ArtisanCommand::Preheat(200.0)));
    s.run(60.0, |t| 150.0 + 0.5 * t, |_| 200.0);
    assert!(s.cmd(ArtisanCommand::Charge(None)));
    s.run(20.0, |_| 180.0, |_| 200.0); // operator pours the beans, 20 s later clicks START
    assert!(!s.c.get_status().charge_detected);
    assert!(s.cmd(ArtisanCommand::StartRoast));
    s.run(1.0, |_| 178.0, |_| 200.0);
    assert!(s.c.get_status().charge_detected, "marker kept through Preheating");
}

#[test]
fn e1_unused_marker_expires_after_grace() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::Charge(None))); // idle, heater off
    s.run(10.0, |_| 25.0, |_| 25.0);
    assert!(s.cmd(ArtisanCommand::PidOn));
    s.run(1.0, |_| 25.0, |_| 25.0);
    assert!(!s.c.get_status().charge_detected, "a 10 s old marker must not anchor a new roast");
}

// ── D3: RoR-follow (E3) ──

/// PID;ON at t=0, SV 200, RoR profile 15→10→6 °C/min, 600 s preheat on the
/// plant, CHARGE (BT jumps to 95, like beans hitting the probe).
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

#[test]
fn e3_parse_ror_profile() {
    assert_eq!(
        parse_artisan_command("RORPROFILE;0,15;300,10"),
        Ok(ArtisanCommand::SetRorProfile)
    );
    let _ = libreroaster::input::parser::ror_profile_take();
    assert_eq!(parse_artisan_command("RORPROFILE;OFF"), Ok(ArtisanCommand::ClearRorProfile));
    assert!(parse_artisan_command("RORPROFILE;0").is_err());
    assert!(parse_artisan_command("RORPROFILE;x,10").is_err());
}

#[test]
fn e3_ror_follow_tracks_profile_and_stays_near_bt() {
    let _g = lock();
    let mut s = Sim::new();
    let mut bt = 150.0f32;
    let tc = ror_roast(&mut s, &mut bt);
    let mut max_gap = 0.0f32;
    let mut samples: Vec<(f32, f32)> = Vec::new();
    plant_run(&mut s, 600.0, &mut bt, |s, _| {
        if s.c.ror_follow_active() && s.c.ror_target_c_per_min() > 0.0 {
            let st = s.c.get_status();
            max_gap = max_gap.max((st.target_temp - st.bean_temp).abs());
        }
        samples.push((s.secs() - tc, s.c.get_status().bean_temp));
    });
    assert!(s.fault_at_s().is_none(), "fault at {:?}", s.fault_at_s());
    assert!(s.c.ror_follow_active());
    assert!(max_gap <= 3.0 + 0.5, "setpoint left the ±3 °C band: {max_gap}");
    // Measured BT RoR between 200 s and 400 s after the charge vs profile
    // (≈11.7 → ≈8.7 °C/min, average ≈10.2).
    let at = |t: f32| samples.iter().find(|p| p.0 >= t).map(|p| p.1).unwrap();
    let ror = (at(400.0) - at(200.0)) / 200.0 * 60.0;
    assert!((ror - 10.2).abs() <= 2.0, "measured RoR {ror:.2} °C/min");
}

#[test]
fn e3_sv_command_stops_ror_follow() {
    let _g = lock();
    let mut s = Sim::new();
    let mut bt = 150.0f32;
    let _ = ror_roast(&mut s, &mut bt);
    plant_run(&mut s, 120.0, &mut bt, |_, _| {});
    assert!(s.c.ror_follow_active());
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(210.0)));
    plant_run(&mut s, 5.0, &mut bt, |_, _| {});
    assert!(!s.c.ror_follow_active(), "an explicit SV is an operator override");
    assert_eq!(s.c.get_status().target_temp, 210.0);
}

#[test]
fn e3_drop_and_pid_off_stop_ror_follow() {
    let _g = lock();
    let mut s = Sim::new();
    let mut bt = 150.0f32;
    let _ = ror_roast(&mut s, &mut bt);
    plant_run(&mut s, 60.0, &mut bt, |_, _| {});
    assert!(s.c.ror_follow_active());
    assert!(s.cmd(ArtisanCommand::Drop));
    assert!(!s.c.ror_follow_active());
    assert_eq!(s.c.ror_target_c_per_min(), 0.0);
    assert!(s.cmd(ArtisanCommand::Stop)); // PID;OFF
    assert!(!s.c.ror_follow_active());
}

#[test]
fn e3_profile_range_and_units() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(!wire(&mut s, "RORPROFILE;0,31"), "31 °C/min > 30 max");
    assert!(!wire(&mut s, "RORPROFILE;60,10;60,9"), "times must increase");
    assert!(wire(&mut s, "RORPROFILE;0,30"));
    assert!(s.cmd(ArtisanCommand::Units(true)));
    assert!(wire(&mut s, "RORPROFILE;0,54"), "54 °F/min = 30 °C/min");
    assert!(!wire(&mut s, "RORPROFILE;0,55"), "55 °F/min > 30 °C/min");
    assert!(wire(&mut s, "RORPROFILE;OFF"));
}

#[test]
fn e3_latched_ror_profile_is_rejected_and_drained() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::EmergencyStop));
    assert!(!wire(&mut s, "RORPROFILE;0,10"));
    assert!(
        libreroaster::input::parser::ror_profile_take().is_none(),
        "BUG-2c-1 discipline: a refused profile must not stay staged"
    );
}

#[test]
fn e3_manual_mode_never_follows() {
    let _g = lock();
    let mut s = Sim::new();
    let mut bt = 150.0f32;
    assert!(s.cmd(ArtisanCommand::SetHeater(60)));
    assert!(wire(&mut s, "RORPROFILE;0,15"));
    assert!(s.cmd(ArtisanCommand::Charge(None)));
    plant_run(&mut s, 60.0, &mut bt, |_, _| {});
    assert!(!s.c.ror_follow_active());
    assert_eq!(s.c.get_status().ssr_output, 60.0);
}

#[test]
fn e3_ot1_takeover_suspends_follow_and_pid_on_resumes_it() {
    let _g = lock();
    let mut s = Sim::new();
    let mut bt = 150.0f32;
    let _ = ror_roast(&mut s, &mut bt);
    plant_run(&mut s, 120.0, &mut bt, |_, _| {});
    assert!(s.c.ror_follow_active());
    assert!(s.cmd(ArtisanCommand::SetHeater(50)));
    plant_run(&mut s, 30.0, &mut bt, |_, _| {});
    assert!(!s.c.ror_follow_active(), "suspended while OT1 controls the heater");
    assert!((s.c.get_status().ssr_output - 50.0).abs() < 1e-3);
    assert!(s.cmd(ArtisanCommand::PidOn));
    plant_run(&mut s, 10.0, &mut bt, |_, _| {});
    assert!(s.c.ror_follow_active(), "PID;ON resumes RoR-follow");
    let st = s.c.get_status();
    assert!((st.target_temp - st.bean_temp).abs() <= 3.5, "no setpoint jump after the gap");
}

#[test]
fn e3_late_charge_button_keeps_a_running_ramp() {
    let _g = lock();
    let mut s = Sim::new();
    let mut bt = 150.0f32;
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    assert!(s.cmd(ArtisanCommand::PidOn));
    let now = s.now();
    s.c.set_profile_start_for_test(now);
    assert!(s.cmd(ArtisanCommand::SetTargetTemp(200.0)));
    assert!(wire(&mut s, "RORPROFILE;0,12"));
    plant_run(&mut s, 600.0, &mut bt, |_, _| {});
    // Beans in WITHOUT the button: automatic detection arms RoR-follow.
    bt = 95.0;
    plant_run(&mut s, 90.0, &mut bt, |_, _| {});
    assert!(s.c.get_status().charge_detected, "automatic charge detected");
    assert!(s.c.ror_follow_active() && s.c.ror_target_c_per_min() > 0.0, "ramping");
    let sv_before = s.c.get_status().target_temp;
    // The operator presses CHARGE 90 s late.
    assert!(s.cmd(ArtisanCommand::Charge(None)));
    plant_run(&mut s, 2.0, &mut bt, |_, _| {});
    assert!(s.c.ror_target_c_per_min() > 0.0, "the running ramp is kept");
    assert!((s.c.get_status().target_temp - sv_before).abs() < 2.0);
}

#[test]
fn e3_et_channel_never_follows() {
    let _g = lock();
    let mut s = Sim::new();
    let mut bt = 150.0f32;
    assert!(s.cmd(ArtisanCommand::SetPidChannel(1)));
    let _ = ror_roast(&mut s, &mut bt);
    plant_run(&mut s, 60.0, &mut bt, |_, _| {});
    assert!(!s.c.ror_follow_active(), "RoR profile is a BT rate: refused on ET");
    assert_eq!(s.c.ror_target_c_per_min(), 0.0);
}

// ── D4: READ extra channels (E2) ──

#[test]
fn e2_chan_1200_keeps_extras_off() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::Chan(1200)));
    assert_eq!(s.c.read_extra_channels(), None);
    assert!(s.cmd(ArtisanCommand::Chan(1234)));
    assert!(s.c.read_extra_channels().is_some());
    assert!(s.cmd(ArtisanCommand::Chan(1230)));
    assert_eq!(s.c.read_extra_channels(), None, "both slots must be requested");
}

#[test]
fn e2_read_line_is_byte_identical_without_extras() {
    use libreroaster::output::artisan::ArtisanFormatter;
    let _g = lock();
    let mut s = Sim::new();
    s.run(5.0, |_| 180.0, |_| 200.0);
    let st = s.c.get_status();
    assert_eq!(
        ArtisanFormatter::format_read_response_full(&st),
        ArtisanFormatter::format_read_response_with_extras(&st, None)
    );
    assert!(s.cmd(ArtisanCommand::PidOn));
    s.run(5.0, |_| 180.0, |_| 200.0);
    let st = s.c.get_status();
    assert_eq!(
        ArtisanFormatter::format_read_response_full(&st),
        ArtisanFormatter::format_read_response_with_extras(&st, None)
    );
}

#[test]
fn e2_extras_carry_ror_target_and_scale_to_fahrenheit() {
    use libreroaster::output::artisan::ArtisanFormatter;
    let _g = lock();
    let mut s = Sim::new();
    let mut bt = 150.0f32;
    assert!(s.cmd(ArtisanCommand::Chan(1234)));
    let _ = ror_roast(&mut s, &mut bt);
    plant_run(&mut s, 120.0, &mut bt, |_, _| {});
    let target = s.c.ror_target_c_per_min();
    assert!(target > 0.0);
    let x = s.c.read_extra_channels().unwrap();
    assert!((x.ch3 - target).abs() < 1e-4);
    assert!(x.ch4.is_finite());
    let line = ArtisanFormatter::format_read_response_with_extras(&s.c.get_status(), Some(x));
    let fields: Vec<&str> = line.as_str().split(',').collect();
    assert_eq!(fields.len(), 8, "AMB,ET,BT,CH3,CH4,heater,fan,SV: {line}");
    assert_eq!(fields[3], format!("{:.1}", target));
    assert!(s.cmd(ArtisanCommand::Units(true)));
    let xf = s.c.read_extra_channels().unwrap();
    assert!((xf.ch3 - target * 1.8).abs() < 1e-3, "rates scale by 1.8 in °F");
    // OT1 takeover suspends RoR-follow: channel 3 must be honest and read 0.
    assert!(s.cmd(ArtisanCommand::Units(false)));
    assert!(s.cmd(ArtisanCommand::SetHeater(50)));
    plant_run(&mut s, 2.0, &mut bt, |_, _| {});
    let xs = s.c.read_extra_channels().unwrap();
    assert_eq!(xs.ch3, 0.0, "suspended follow reports 0 on channel 3");
    assert!(xs.ch4.is_finite());
}

// ── D5: step-test autotune (E4) ──

/// Integrating plant with 8 s dead time: dBT/dt = 0.005 + 0.003·(u(t−8) − 40).
fn tune_plant_run(s: &mut Sim, secs: f32, bt: &mut f32, hist: &mut std::collections::VecDeque<f32>) -> f32 {
    let delay = (8.0 / DT) as usize;
    let mut max_u = 0.0f32;
    for _ in 0..(secs / DT) as u64 {
        if s.n % 6 == 0 {
            s.cmd(ArtisanCommand::ReadStatus);
        }
        s.tick(*bt, *bt + 20.0);
        let u = s.c.get_status().ssr_output;
        max_u = max_u.max(u);
        hist.push_back(u);
        let delayed = if hist.len() > delay { hist[hist.len() - 1 - delay] } else { 40.0 };
        *bt += (0.005 + 0.003 * (delayed - 40.0)) * DT;
    }
    max_u
}

fn manual_at_40(s: &mut Sim) -> (f32, std::collections::VecDeque<f32>) {
    assert!(s.cmd(ArtisanCommand::SetHeater(40)));
    assert!(s.cmd(ArtisanCommand::SetFan(40)));
    let mut bt = 150.0f32;
    let mut hist = std::collections::VecDeque::new();
    tune_plant_run(s, 20.0, &mut bt, &mut hist);
    (bt, hist)
}

#[test]
fn e4_parse_tune() {
    use libreroaster::config::TuneCommand;
    assert_eq!(parse_artisan_command("TUNE;20"), Ok(ArtisanCommand::Tune(TuneCommand::Start(20))));
    assert_eq!(parse_artisan_command("TUNE;abort"), Ok(ArtisanCommand::Tune(TuneCommand::Abort)));
    assert_eq!(parse_artisan_command("TUNE;UNLOCK"), Ok(ArtisanCommand::Tune(TuneCommand::Unlock)));
    assert_eq!(parse_artisan_command("TUNE;STATUS"), Ok(ArtisanCommand::Tune(TuneCommand::Status)));
    assert!(parse_artisan_command("TUNE").is_err());
    assert!(parse_artisan_command("TUNE;101").is_err());
}

#[test]
fn e4_tune_identifies_plant_and_locks_gains() {
    let _g = lock();
    let mut s = Sim::new();
    let (mut bt, mut hist) = manual_at_40(&mut s);
    assert!(wire(&mut s, "TUNE;20"));
    let max_u = tune_plant_run(&mut s, 420.0, &mut bt, &mut hist);
    assert!(s.fault_at_s().is_none(), "fault at {:?}", s.fault_at_s());
    assert!(!s.c.tune_running());
    let r = s.c.last_tune_result().expect("tune must finish with a result");
    assert!((r.gain - 0.003).abs() < 0.0006, "gain {}", r.gain);
    assert!(r.kp > 3.0 && r.kp < 40.0, "kp {}", r.kp);
    assert!(max_u <= 60.0 + 1e-3, "heater never above base + step: {max_u}");
    assert!((s.c.get_status().ssr_output - 40.0).abs() < 1e-3, "back to the manual duty");
    assert!(s.c.pid_gains_locked());
    // Artisan's PID ON handshake (PID;T) must not overwrite the tuned gains.
    assert!(s.cmd(ArtisanCommand::SetPidGain(1.0, 1.0, 1.0)));
    assert!(s.c.pid_gains_locked());
    assert!(wire(&mut s, "TUNE;UNLOCK"));
    assert!(!s.c.pid_gains_locked());
}

#[test]
fn e4_operator_command_aborts_tune() {
    let _g = lock();
    let mut s = Sim::new();
    let (mut bt, mut hist) = manual_at_40(&mut s);
    assert!(wire(&mut s, "TUNE;20"));
    tune_plant_run(&mut s, 30.0, &mut bt, &mut hist);
    assert!(s.c.tune_running());
    assert!(s.cmd(ArtisanCommand::SetHeater(50)));
    tune_plant_run(&mut s, 5.0, &mut bt, &mut hist);
    assert!(!s.c.tune_running());
    assert!((s.c.get_status().ssr_output - 50.0).abs() < 1e-3);
    assert!(s.c.last_tune_result().is_none());
}

#[test]
fn e4_tune_refused_outside_manual_mode_and_when_latched() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::PidOn));
    assert!(!wire(&mut s, "TUNE;20"), "firmware PID in control: refused");
    assert!(s.cmd(ArtisanCommand::EmergencyStop));
    assert!(!wire(&mut s, "TUNE;20"), "latched: refused");
    assert!(!s.c.tune_running());
}

#[test]
fn e4_latch_aborts_running_tune() {
    let _g = lock();
    let mut s = Sim::new();
    let (mut bt, mut hist) = manual_at_40(&mut s);
    assert!(wire(&mut s, "TUNE;20"));
    tune_plant_run(&mut s, 10.0, &mut bt, &mut hist);
    assert!(s.c.tune_running());
    let _ = s.c.emergency_shutdown("test");
    tune_plant_run(&mut s, 1.0, &mut bt, &mut hist);
    assert!(!s.c.tune_running());
    assert_eq!(s.c.get_status().ssr_output, 0.0);
}

#[test]
fn e4_cold_probe_fails_without_touching_heater() {
    let _g = lock();
    let mut s = Sim::new();
    assert!(s.cmd(ArtisanCommand::SetHeater(40)));
    s.run(5.0, |_| 30.0, |_| 35.0);
    assert!(wire(&mut s, "TUNE;20"));
    s.run(2.0, |_| 30.0, |_| 35.0);
    assert!(!s.c.tune_running());
    assert!((s.c.get_status().ssr_output - 40.0).abs() < 1e-3);
}

// @@ NEXT TESTS GO HERE (keep this line) @@
