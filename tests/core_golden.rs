//! CORE-1 golden trace (plan CORE-2026-10-09).
//!
//! Every scenario drives `RoasterControl` on SYNTHETIC time through a small
//! closed-loop plant, records one quantized line per control tick (outputs,
//! mode flags, the private core state via `core_snapshot()`, every command
//! result) and hashes the whole trace (FNV-1a 64). The control-core refactor
//! (tasks C2..C6) must not change a single hash: a different hash means the
//! refactor changed behaviour.
//!
//! The invariant checks run on every tick of every scenario.
//!
//! NEVER edit `GOLDEN` to make this test pass. `CORE_GOLDEN_PRINT=1` prints
//! the observed table (used once, by the plan author, to create it).
#![cfg(all(test, feature = "test", not(target_arch = "riscv32")))]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::type_complexity
)]

extern crate std;

use std::fmt::Write as _;
use std::sync::Mutex;

use embassy_time::{Duration, Instant};
use libreroaster::common::{StubFan, StubHeater};
use libreroaster::config::ArtisanCommand;
use libreroaster::control::roaster_control::RoasterControl;
use libreroaster::hardware::sensors::SensorConversionHub;
use libreroaster::input::parser::parse_artisan_command;

static LOCK: Mutex<()> = Mutex::new(());

/// Real embedded cadence: sample stamped before the 210 ms wait, control after it.
const PERIOD_MS: u64 = 320;
const CONV_MS: u64 = 215;
const DT: f32 = PERIOD_MS as f32 / 1000.0;

/// One scripted plant or operator event.
#[derive(Clone, Copy, Debug)]
enum Ev {
    /// An Artisan wire line, parsed exactly like the transport does.
    Wire(&'static str),
    /// Beans in: the true BT jumps down to this value.
    BeansIn(f32),
    /// Beans out: the probe is in air and BT falls at this rate (°C/s) for 25 s.
    BeansOut(f32),
    /// Door opened ~2 s: BT reads 12 °C low, recovering over 30 s.
    DoorDip,
    /// The BT probe freezes at its current reading.
    FreezeBt,
    /// The ET probe freezes at its current reading.
    FreezeEt,
    /// The BT channel reads NaN from now on.
    NanBt,
    /// Artisan stops polling READ (comms loss).
    Silence,
}

struct Rig {
    c: RoasterControl,
    t0: Instant,
    n: u64,
    bt: f32,
    ema_bt: Option<f32>,
    ema_et: Option<f32>,
    frozen: Option<f32>,
    et_frozen: Option<f32>,
    nan: bool,
    dip: f32,
    dip_ticks: u32,
    fall_rate: f32,
    fall_ticks: u32,
    poll: bool,
    hash: u64,
    violations: Vec<String>,
}

fn fnv(hash: &mut u64, bytes: &[u8]) {
    for b in bytes {
        *hash ^= u64::from(*b);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

impl Rig {
    fn new(bt: f32) -> Self {
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
            bt,
            ema_bt: None,
            ema_et: None,
            frozen: None,
            et_frozen: None,
            nan: false,
            dip: 0.0,
            dip_ticks: 0,
            fall_rate: 0.0,
            fall_ticks: 0,
            poll: true,
            hash: 0xcbf2_9ce4_8422_2325,
            violations: Vec::new(),
        }
    }

    fn now(&self) -> Instant {
        self.t0 + Duration::from_millis(self.n * PERIOD_MS)
    }

    fn record(&mut self, line: &str) {
        fnv(&mut self.hash, line.as_bytes());
        fnv(&mut self.hash, b"\n");
    }

    fn cmd(&mut self, c: ArtisanCommand, label: &str) {
        let now = self.now();
        let r = self.c.process_artisan_command_at(c, now);
        let line = match r {
            Ok(()) => format!("{} cmd {} ok", self.n, label),
            Err(e) => format!("{} cmd {} err {:?}", self.n, label, e),
        };
        self.record(&line);
    }

    fn event(&mut self, ev: Ev) {
        match ev {
            Ev::Wire(w) => match parse_artisan_command(w) {
                Ok(c) => self.cmd(c, w),
                Err(e) => {
                    let line = format!("{} parse {} err {:?}", self.n, w, e);
                    self.record(&line);
                }
            },
            Ev::BeansIn(t) => self.bt = t,
            Ev::BeansOut(rate) => {
                self.fall_rate = rate;
                self.fall_ticks = (25.0 / DT) as u32;
            }
            Ev::DoorDip => {
                self.dip = 12.0;
                self.dip_ticks = 0;
            }
            Ev::FreezeBt => {
                if self.frozen.is_none() {
                    self.frozen = Some(self.ema_bt.unwrap_or(self.bt));
                }
            }
            Ev::FreezeEt => {
                if self.et_frozen.is_none() {
                    self.et_frozen = Some(self.ema_et.unwrap_or(self.bt + 20.0));
                }
            }
            Ev::NanBt => self.nan = true,
            Ev::Silence => self.poll = false,
        }
    }

    fn tick(&mut self) {
        if self.poll && self.n.is_multiple_of(6) {
            self.cmd(ArtisanCommand::ReadStatus, "READ");
        }
        // Raw readings (door dip recovers over 30 s after 7 ticks).
        if self.dip > 0.0 {
            self.dip_ticks += 1;
            if self.dip_ticks > 7 {
                self.dip = (self.dip - 12.0 * DT / 30.0).max(0.0);
            }
        }
        let bt_raw = if self.nan {
            f32::NAN
        } else {
            self.frozen.unwrap_or(self.bt) - self.dip
        };
        let et_raw = self.et_frozen.unwrap_or(self.bt + 20.0);
        let a = 0.2;
        let bt = self.ema_bt.map_or(bt_raw, |p| a * bt_raw + (1.0 - a) * p);
        let et = self.ema_et.map_or(et_raw, |p| a * et_raw + (1.0 - a) * p);
        self.ema_bt = Some(bt);
        self.ema_et = Some(et);
        let ts = self.now();
        let r1 = self.c.update_temperatures(bt, et, ts);
        let r2 = self.c.update_control(ts + Duration::from_millis(CONV_MS));
        // Plant: dBT/dt = 0.004·u − 0.001·(BT − 25), plus a forced fall after DROP.
        let st = self.c.get_status();
        let u = st.ssr_output;
        self.bt += (0.004 * u - 0.001 * (self.bt - 25.0)) * DT;
        if self.fall_ticks > 0 {
            self.fall_ticks -= 1;
            self.bt -= self.fall_rate * DT;
        }
        let snap = self.c.core_snapshot();
        let x = self.c.read_extra_channels().map(|e| (q(e.ch3), q(e.ch4)));
        let mut line = String::new();
        let _ = write!(
            line,
            "{} {:?} {:?} {:?} ssr={} fan={} sv={} pv={} mv={} pid={} man={} fault={} cd={} rf={} x={:?} {:?}",
            self.n,
            r1.is_ok(),
            r2.as_ref().map(|v| q(*v)).map_err(|e| format!("{e:?}")),
            st.state,
            q(st.ssr_output),
            q(st.fan_output),
            q(st.target_temp),
            q(st.pv),
            q(st.mv),
            st.pid_enabled,
            st.artisan_control,
            st.fault_condition,
            st.charge_detected,
            self.c.ror_follow_active(),
            x,
            snap
        );
        self.record(&line);
        self.check_invariants();
        self.n += 1;
    }

    /// Invariants that hold on develop @ 9b98804 + the 2026-10-09 fixes, at
    /// the end of every tick. A refactor must keep all of them.
    fn check_invariants(&mut self) {
        let st = self.c.get_status();
        let s = self.c.core_snapshot();
        let ror_active = self.c.ror_follow_active();
        let n = self.n;
        let violations = &mut self.violations;
        let mut bad = |name: &str| violations.push(format!("tick {n}: {name}"));
        if st.charge_detected != s.charge_detected {
            bad("I1 status.charge_detected mirrors the core flag");
        }
        if st.pid_enabled && st.artisan_control {
            bad("I2 never PID and manual at the same time");
        }
        if s.follower.is_some() && !s.ror_profile_loaded {
            bad("I3 a follower needs a loaded RoR profile");
        }
        if s.ror_resume_pending && s.follower.is_some() {
            bad("I4 a pending re-arm implies no follower");
        }
        if s.tune_running && !(st.artisan_control && !st.pid_enabled) {
            bad("I5 TUNE runs only in manual mode");
        }
        if s.emergency && st.ssr_output != 0.0 {
            bad("I6 latched ⇒ heater 0");
        }
        if s.emergency && !matches!(s.state, libreroaster::config::RoasterState::Error) {
            bad("I7 latched ⇒ state Error");
        }
        if ror_active && st.pid_channel == 1 {
            bad("I8 RoR-follow never on the ET channel");
        }
        if s.ror_target_c_per_min != 0.0 && s.follower.is_none() {
            bad("I9 a RoR target needs a follower");
        }
        if s.batch_dropped && s.explicit_charge_seen {
            bad("I10 a dropped batch has no explicit charge");
        }
        if s.charge_detected && !s.charge_anchored {
            bad("I11 a detected charge has a time anchor");
        }
    }
}

/// Quantize to 0.01 so the trace is stable text.
fn q(v: f32) -> String {
    if v.is_finite() {
        format!("{v:.2}")
    } else {
        format!("{v}")
    }
}

struct Scenario {
    name: &'static str,
    secs: f32,
    start_bt: f32,
    events: &'static [(f32, Ev)],
}

fn run(sc: &Scenario) -> (u64, Vec<String>) {
    let mut r = Rig::new(sc.start_bt);
    let ticks = (sc.secs / DT) as u64;
    let mut next = 0usize;
    for _ in 0..ticks {
        let t = r.n as f32 * DT;
        while next < sc.events.len() && sc.events[next].0 <= t {
            r.event(sc.events[next].1);
            next += 1;
        }
        r.tick();
    }
    (r.hash, r.violations)
}

use Ev::*;

const ROR: Ev = Wire("RORPROFILE;0,15;300,10;600,6");

static SCENARIOS: &[Scenario] = &[
    Scenario {
        name: "s01_manual_session",
        secs: 1500.0,
        start_bt: 25.0,
        events: &[
            (1.0, Wire("OT2;40")),
            (2.0, Wire("OT1;70")),
            (600.0, BeansIn(95.0)),
            (650.0, Wire("OT1;60")),
            (900.0, Wire("OT1;45")),
            (1200.0, Wire("DROP")),
            (1210.0, BeansOut(2.0)),
            (1260.0, Wire("OT1;0")),
            (1300.0, Wire("PID;OFF")),
        ],
    },
    Scenario {
        name: "s02_pid_on_markers",
        secs: 1700.0,
        start_bt: 25.0,
        events: &[
            (1.0, Wire("OT2;40")),
            (2.0, Wire("PID;ON")),
            (3.0, Wire("PID;SV;200")),
            (600.0, Wire("CHARGE;250.0")),
            (600.5, BeansIn(95.0)),
            (1300.0, Wire("DROP")),
            (1300.5, Wire("PID;OFF")),
        ],
    },
    Scenario {
        name: "s03_preheat_late_start",
        secs: 1200.0,
        start_bt: 25.0,
        events: &[
            (1.0, Wire("OT2;40")),
            (2.0, Wire("PREHEAT;200")),
            (3.0, ROR),
            (500.0, Wire("CHARGE")),
            (500.5, BeansIn(105.0)),
            (545.0, Wire("START")),
            (1100.0, Wire("PID;OFF")),
        ],
    },
    Scenario {
        name: "s04_ror_follow_full",
        secs: 1700.0,
        start_bt: 150.0,
        events: &[
            (1.0, Wire("OT2;40")),
            (2.0, Wire("PID;ON")),
            (3.0, Wire("PID;SV;200")),
            (4.0, ROR),
            (600.0, Wire("CHARGE")),
            (600.1, BeansIn(95.0)),
            (1500.0, Wire("DROP")),
            (1501.0, Wire("PID;OFF")),
        ],
    },
    Scenario {
        name: "s05_charge_in_takeover_resume",
        secs: 1300.0,
        start_bt: 150.0,
        events: &[
            (1.0, Wire("OT2;40")),
            (2.0, Wire("PID;ON")),
            (3.0, Wire("PID;SV;200")),
            (4.0, ROR),
            (600.0, Wire("OT1;0")),
            (605.0, Wire("CHARGE")),
            (605.1, BeansIn(95.0)),
            (606.0, Wire("OT1;70")),
            (810.0, Wire("PID;ON")),
            (810.4, Wire("PID;SV;200")),
        ],
    },
    Scenario {
        name: "s06_sv_override_then_resume",
        secs: 1100.0,
        start_bt: 150.0,
        events: &[
            (1.0, Wire("OT2;40")),
            (2.0, Wire("PID;ON")),
            (3.0, Wire("PID;SV;200")),
            (4.0, ROR),
            (600.0, Wire("CHARGE")),
            (600.1, BeansIn(95.0)),
            (720.0, Wire("PID;SV;210")),
            (730.0, Wire("OT1;50")),
            (760.0, Wire("PID;ON")),
        ],
    },
    Scenario {
        name: "s07_drop_ot1_pid_on",
        secs: 1700.0,
        start_bt: 150.0,
        events: &[
            (1.0, Wire("OT2;40")),
            (2.0, Wire("PID;ON")),
            (3.0, Wire("PID;SV;200")),
            (4.0, ROR),
            (600.0, Wire("CHARGE")),
            (600.1, BeansIn(95.0)),
            (1200.0, Wire("DROP")),
            (1201.0, Wire("OT1;0")),
            (1320.0, Wire("PID;ON")),
        ],
    },
    Scenario {
        name: "s08_drop_fast_fall_pid_on",
        secs: 2000.0,
        start_bt: 150.0,
        events: &[
            (1.0, Wire("OT2;40")),
            (2.0, Wire("PID;ON")),
            (3.0, Wire("PID;SV;200")),
            (4.0, ROR),
            (600.0, Wire("CHARGE")),
            (600.1, BeansIn(95.0)),
            (1200.0, Wire("DROP")),
            (1200.1, BeansOut(3.0)),
            (1600.0, Wire("CHARGE")),
            (1600.1, BeansIn(95.0)),
        ],
    },
    Scenario {
        name: "s09_door_dip_empty_drum",
        secs: 1600.0,
        start_bt: 150.0,
        events: &[
            (1.0, Wire("OT2;40")),
            (2.0, Wire("PID;ON")),
            (3.0, Wire("PID;SV;200")),
            (4.0, ROR),
            (600.0, DoorDip),
            (800.0, Wire("OT1;40")),
            (830.0, DoorDip),
            (1000.0, Wire("PID;ON")),
        ],
    },
    Scenario {
        name: "s10_frozen_bt_under_ror",
        secs: 1500.0,
        start_bt: 150.0,
        events: &[
            (1.0, Wire("OT2;40")),
            (2.0, Wire("PID;ON")),
            (3.0, Wire("PID;SV;200")),
            (4.0, ROR),
            (600.0, Wire("CHARGE")),
            (600.1, BeansIn(95.0)),
            (720.0, FreezeBt),
            (1300.0, Wire("PID;OFF")),
            (1310.0, Wire("START")),
        ],
    },
    Scenario {
        name: "s11_stop_latch_recoveries",
        secs: 900.0,
        start_bt: 25.0,
        events: &[
            (1.0, Wire("OT2;40")),
            (2.0, Wire("START")),
            (120.0, Wire("STOP")),
            (125.0, Wire("OT1;50")),
            (126.0, Wire("PID;ON")),
            (130.0, Wire("START")),
            (300.0, Wire("STOP")),
            (305.0, Wire("PREHEAT;190")),
            (500.0, Wire("STOP")),
            (505.0, Wire("PID;OFF")),
            (510.0, Wire("OT1;40")),
            (700.0, Wire("CHARGE")),
            (700.5, Wire("STOP")),
            (701.0, Wire("START")),
        ],
    },
    Scenario {
        name: "s12_overtemp_and_nan",
        secs: 900.0,
        start_bt: 150.0,
        events: &[
            (1.0, Wire("OT2;30")),
            (2.0, Wire("OT1;100")),
            (450.0, Wire("PID;OFF")),
            (460.0, Wire("OT1;50")),
            (600.0, NanBt),
            (620.0, Wire("PID;OFF")),
        ],
    },
    Scenario {
        name: "s13_comms_loss",
        secs: 400.0,
        start_bt: 100.0,
        events: &[
            (1.0, Wire("OT2;40")),
            (2.0, Wire("PID;ON")),
            (3.0, Wire("PID;SV;180")),
            (200.0, Silence),
        ],
    },
    Scenario {
        name: "s14_time_budget_start",
        secs: 2000.0,
        start_bt: 150.0,
        events: &[
            (1.0, Wire("OT2;40")),
            (2.0, Wire("START")),
            (3.0, Wire("PID;SV;205")),
        ],
    },
    Scenario {
        name: "s15_tune_paths",
        secs: 1100.0,
        start_bt: 150.0,
        events: &[
            (1.0, Wire("OT2;40")),
            (2.0, Wire("OT1;40")),
            (30.0, Wire("TUNE;20")),
            (31.0, Wire("TUNE;STATUS")),
            (500.0, Wire("TUNE;STATUS")),
            (510.0, Wire("PID;T;2.0;0.5;1.0")),
            (520.0, Wire("PID;CHAN;2")),
            (530.0, Wire("PID;CHAN;1")),
            (540.0, Wire("TUNE;20")),
            (550.0, Wire("PID;CHAN;2")),
            (560.0, Wire("TUNE;20")),
            (600.0, Wire("OT1;45")),
            (700.0, Wire("TUNE;ABORT")),
            (710.0, Wire("TUNE;UNLOCK")),
            (720.0, Wire("PID;ON")),
            (721.0, Wire("TUNE;20")),
        ],
    },
    Scenario {
        name: "s18_pid_plateau_both_frozen",
        secs: 1300.0,
        start_bt: 150.0,
        events: &[
            (1.0, Wire("OT2;40")),
            (2.0, Wire("PID;ON")),
            (3.0, Wire("PID;SV;200")),
            (400.0, FreezeBt),
            (400.0, FreezeEt),
            (401.0, Wire("PID;SV;230")),
        ],
    },
    Scenario {
        name: "s19_manual_flat_bt",
        secs: 800.0,
        start_bt: 150.0,
        events: &[
            (1.0, Wire("OT2;40")),
            (2.0, Wire("OT1;60")),
            (200.0, FreezeBt),
        ],
    },
    Scenario {
        name: "s20_manual_equilibrium_then_resume",
        secs: 1200.0,
        start_bt: 150.0,
        events: &[
            (1.0, Wire("OT2;40")),
            (2.0, Wire("START")),
            (300.0, Wire("OT1;40")),
            (500.0, FreezeBt),
            (500.0, FreezeEt),
            (900.0, Wire("PID;ON")),
            (1000.0, Wire("PID;SV;215")),
        ],
    },
    Scenario {
        name: "s16_et_channel_and_ct",
        secs: 1500.0,
        start_bt: 150.0,
        events: &[
            (1.0, Wire("OT2;40")),
            (2.0, Wire("PID;CT;2000")),
            (3.0, Wire("PID;ON")),
            (4.0, Wire("PID;SV;200")),
            (5.0, ROR),
            (600.0, Wire("CHARGE")),
            (600.1, BeansIn(95.0)),
            (900.0, Wire("PID;CHAN;1")),
            (1000.0, Wire("PID;SV;230")),
            (1100.0, Wire("PID;CHAN;2")),
            (1200.0, Wire("RORPROFILE;OFF")),
        ],
    },
    Scenario {
        name: "s17_profile_units_updown",
        secs: 1300.0,
        start_bt: 25.0,
        events: &[
            (1.0, Wire("UNITS;F")),
            (2.0, Wire("OT2;40")),
            (3.0, Wire("PROFILE;0,356;300,392;600,410")),
            (4.0, Wire("START")),
            (300.0, Wire("PID;SV;400")),
            (400.0, Wire("UNITS;C")),
            (500.0, Wire("UP")),
            (510.0, Wire("UP")),
            (520.0, Wire("DOWN")),
            (600.0, Wire("PID;LIMIT;20;80")),
            (700.0, Wire("PREHEAT;180")),
            (800.0, Wire("PID;OFF")),
            (810.0, Wire("PREHEAT;180")),
            (900.0, Wire("PREHEAT;195")),
            (950.0, Wire("CHARGE")),
            (960.0, Wire("PREHEAT;200")),
            (1000.0, Wire("START")),
            (1001.0, BeansIn(110.0)),
        ],
    },
];

/// Seeded random operator: one command every 3–40 s from a pool, plus plant events.
fn fuzz(seed: u64, secs: f32) -> (u64, Vec<String>) {
    const POOL: &[&str] = &[
        "OT1;0",
        "OT1;30",
        "OT1;60",
        "OT1;100",
        "OT2;40",
        "OT2;0",
        "PID;ON",
        "PID;OFF",
        "PID;SV;180",
        "PID;SV;210",
        "PID;SV;240",
        "PREHEAT;190",
        "START",
        "STOP",
        "CHARGE",
        "CHARGE;250",
        "DROP",
        "RORPROFILE;0,15;300,10;600,6",
        "RORPROFILE;OFF",
        "TUNE;20",
        "TUNE;ABORT",
        "TUNE;UNLOCK",
        "TUNE;STATUS",
        "PID;CHAN;1",
        "PID;CHAN;2",
        "PID;CT;2000",
        "PID;CT;1000",
        "UP",
        "DOWN",
        "UNITS;F",
        "UNITS;C",
        "PROFILE;0,180;120,200",
        "PID;T;2.0;0.5;1.0",
    ];
    let mut s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut next_u = move || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    };
    let mut r = Rig::new(25.0 + (next_u() % 150) as f32);
    let ticks = (secs / DT) as u64;
    let mut wait = 0u64;
    for _ in 0..ticks {
        if wait == 0 {
            let k = next_u() % 100;
            if k < 80 {
                let w = POOL[(next_u() as usize) % POOL.len()];
                r.event(Ev::Wire(w));
            } else if k < 88 {
                r.event(Ev::BeansIn(90.0 + (next_u() % 30) as f32));
            } else if k < 93 {
                r.event(Ev::BeansOut(3.0));
            } else if k < 98 {
                r.event(Ev::DoorDip);
            } else {
                r.event(Ev::FreezeBt);
            }
            wait = 10 + next_u() % 115;
        }
        wait -= 1;
        r.tick();
    }
    (r.hash, r.violations)
}

const FUZZ_SEEDS: u64 = 24;
const FUZZ_SECS: f32 = 1200.0;

/// Observed on develop @ 9b98804 + Fixes-Plan-Audit-2026-10-09 + C0.
const GOLDEN: &[(&str, u64)] = &[
    ("s01_manual_session", 0xe6c27a22fcac1fc3),
    ("s02_pid_on_markers", 0xdf385dfe2cd306dd),
    ("s03_preheat_late_start", 0xec6e7947ceb19b11),
    ("s04_ror_follow_full", 0x8ebb48368debf366),
    ("s05_charge_in_takeover_resume", 0x9afd645591d3ebc0),
    ("s06_sv_override_then_resume", 0xdbe91facc27fb8ce),
    ("s07_drop_ot1_pid_on", 0x3ab7912da3f23692),
    ("s08_drop_fast_fall_pid_on", 0xd6f3e555f8aef89c),
    ("s09_door_dip_empty_drum", 0xd49e3d4d5585467b),
    ("s10_frozen_bt_under_ror", 0x02ae478125b78f75),
    ("s11_stop_latch_recoveries", 0xf5049b462f074aa0),
    ("s12_overtemp_and_nan", 0x293d774936a67b8e),
    ("s13_comms_loss", 0xb972ad085071d951),
    ("s14_time_budget_start", 0xa88b89d359ef1a0b),
    ("s15_tune_paths", 0x207e778d24f33820),
    ("s18_pid_plateau_both_frozen", 0x07d0350c87a52b40),
    ("s19_manual_flat_bt", 0xac831379d38faa22),
    ("s20_manual_equilibrium_then_resume", 0xd2be87e606ee0a5e),
    ("s16_et_channel_and_ct", 0x418582ff2905f76e),
    ("s17_profile_units_updown", 0x5bef108288dc6864),
    ("f00", 0x4510cfaacb90e57c),
    ("f01", 0x5ce0b316187c1232),
    ("f02", 0x6d44a9afd64802f7),
    ("f03", 0x90a5bb61986771af),
    ("f04", 0x0308397c8ab941bc),
    ("f05", 0x9e8aff9ec57fd461),
    ("f06", 0x60f6a2349818b37f),
    ("f07", 0x7afb398b482e8e65),
    ("f08", 0xa97b6eadce6361ce),
    ("f09", 0x5607569e5f4a2783),
    ("f10", 0x4c08a0055bfdd34d),
    ("f11", 0x8f6ec258895724a4),
    ("f12", 0xdc7aa999e874763b),
    ("f13", 0x931fa3667a907681),
    ("f14", 0x8f6b37923fb4222b),
    ("f15", 0x6cc609620d3157ae),
    ("f16", 0xc754d61f815c2839),
    ("f17", 0x832a5e27be31ad7b),
    ("f18", 0x5021c876dcea0897),
    ("f19", 0x218a7182bd34b4fa),
    ("f20", 0x46ae9a25fbc73a11),
    ("f21", 0xe1946d417377b243),
    ("f22", 0x618d495efe81696a),
    ("f23", 0xaddd482e3e42760c),
];

fn observed() -> (Vec<(String, u64)>, Vec<String>) {
    let mut out = Vec::new();
    let mut viol = Vec::new();
    for sc in SCENARIOS {
        let (h, v) = run(sc);
        out.push((sc.name.to_string(), h));
        viol.extend(v.into_iter().map(|x| format!("{}: {}", sc.name, x)));
    }
    for seed in 0..FUZZ_SEEDS {
        let (h, v) = fuzz(seed, FUZZ_SECS);
        let name = format!("f{seed:02}");
        viol.extend(v.into_iter().map(|x| format!("{name}: {x}")));
        out.push((name, h));
    }
    (out, viol)
}

#[test]
fn core_golden_traces_are_unchanged() {
    let _g = LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let (obs, _) = observed();
    if std::env::var("CORE_GOLDEN_PRINT").is_ok() {
        for (n, h) in &obs {
            println!("    (\"{n}\", 0x{h:016x}),");
        }
    }
    let mut mismatches = Vec::new();
    for (name, h) in &obs {
        match GOLDEN.iter().find(|(n, _)| n == name) {
            Some((_, g)) if g == h => {}
            Some((_, g)) => {
                mismatches.push(format!("{name}: golden 0x{g:016x} observed 0x{h:016x}"))
            }
            None => mismatches.push(format!("{name}: missing from GOLDEN")),
        }
    }
    assert!(
        mismatches.is_empty(),
        "behaviour changed in {} scenario(s):\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}

#[test]
fn core_invariants_hold_on_every_tick() {
    let _g = LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let (_, viol) = observed();
    assert!(
        viol.is_empty(),
        "{} invariant violation(s), first ones:\n{}",
        viol.len(),
        viol.iter().take(20).cloned().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn core_golden_is_deterministic() {
    let _g = LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let a = run(&SCENARIOS[3]).0;
    let b = run(&SCENARIOS[3]).0;
    assert_eq!(a, b, "same scenario, two runs, different traces");
    assert_eq!(fuzz(7, 600.0).0, fuzz(7, 600.0).0);
}

/// Post-refactor bug hunt (plan CORE-2026-10-09, §B): the invariants on many
/// FRESH seeds. Not part of the normal run:
/// `CORE_FUZZ_SEEDS=500 cargo test --target x86_64-unknown-linux-gnu --features test --test core_golden -- --ignored`
#[test]
#[ignore]
fn core_invariants_long_fuzz() {
    let _g = LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let env = |k: &str, d: u64| {
        std::env::var(k)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(d)
    };
    let n = env("CORE_FUZZ_SEEDS", 200);
    let base = env("CORE_FUZZ_BASE", 1000);
    let mut viol = Vec::new();
    for seed in base..base + n {
        let (_, v) = fuzz(seed, 1800.0);
        viol.extend(v.into_iter().map(|x| format!("seed {seed}: {x}")));
    }
    assert!(
        viol.is_empty(),
        "{} invariant violation(s) in {} seeds, first ones:\n{}",
        viol.len(),
        n,
        viol.iter().take(20).cloned().collect::<Vec<_>>().join("\n")
    );
}
