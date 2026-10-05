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

// @@ NEXT TESTS GO HERE (keep this line) @@
