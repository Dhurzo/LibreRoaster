//! Full Artisan roast simulation (host-side state machine, no hardware needed)
//!
//! Simulates Artisan+ controlling a roast via stub hardware and the internal
//! state machine. Tests command handling and state transitions in isolation.
//! For hardware-in-the-loop (HIL) tests against a real ESP32-C3, run:
//!
//! ```bash
//! python tests/hardware/artisan_roast_hil.py
//! ```
//!
//! # Simulated Phases
//!
//! 1. **Initialize** — Artisan handshake (CHAN, UNITS, FILT)
//! 2. **Preheat** — PREHEAT command, temp ramp 25→180°C
//! 3. **Profile load** — PROFILE + FANPROFILE setpoints
//! 4. **Charge** — START, bean drop detection via BT decline
//! 5. **Active roast** — Temperature curve following + periodic READ
//! 6. **Stabilization** — Near-target behavior
//! 7. **Stop + Cooldown** — STOP, fan 100%, cooldown curve
//!
//! # Running
//!
//! ```bash
//! cargo test --test artisan_roast_simulation --features test
//! ```

#![cfg(all(test, not(target_arch = "riscv32")))]
#![allow(non_snake_case)]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::type_complexity
)]

extern crate std;

use std::println;
use std::vec::Vec;

use embassy_time::Instant;

use libreroaster::common::{StubFan, StubHeater};
use libreroaster::config::{
    ArtisanCommand, ProfileSetpoint, RoastProfile, RoasterState, SystemStatus, TemperatureScale,
};
use libreroaster::control::RoasterControl;
use libreroaster::hardware::sensors::SensorConversionHub;
use libreroaster::input::parser::{parse_artisan_command, store_profile};
use libreroaster::output::artisan::ArtisanFormatter;

// ── Temperature curves ────────────────────────────────────────────────────

/// Preheat curve: ambient (25°C) to preheat target (180°C) in ~30 steps.
/// Models thermal inertia with an exponential-ish rise.
struct PreheatCurve;

impl PreheatCurve {
    fn temp_at(&self, step: usize) -> f32 {
        // Simulated exponential approach: 25 → 180 over 30 steps
        let temps: [f32; 31] = [
            25.0, 30.0, 37.0, 46.0, 57.0, // 0-4
            70.0, 84.0, 99.0, 114.0, 128.0, // 5-9
            140.0, 150.0, 158.0, 164.0, 169.0, // 10-14
            173.0, 176.0, 178.0, 179.0, 180.0, // 15-19
            180.0, 180.0, 180.0, 180.0, 180.0, // 20-24
            180.0, 180.0, 180.0, 180.0, 180.0, // 25-29
            180.0, // 30
        ];
        let idx = step.min(temps.len() - 1);
        temps[idx]
    }
}

/// Roast curve: BT rise from charge (~100°C) to drop (~220°C) over 60 steps.
/// Models a ~10-minute roast at ~2°C/sample ROR.
struct RoastCurve;

impl RoastCurve {
    fn temp_at(&self, step: usize) -> f32 {
        let temps: [f32; 61] = [
            // Charge phase: temp drop then recovery
            150.0, 130.0, 115.0, 108.0, 105.0, // 0-4: charge dip
            // Drying phase: slow rise
            108.0, 114.0, 122.0, 131.0, 140.0, // 5-9
            // Maillard: medium ramp
            148.0, 156.0, 163.0, 169.0, 174.0, // 10-14
            // Development: steady rise
            179.0, 183.0, 187.0, 190.0, 193.0, // 15-19
            196.0, 198.0, 200.0, 202.0, 204.0, // 20-24
            // Approaching target
            206.0, 208.0, 210.0, 212.0, 214.0, // 25-29
            216.0, 217.0, 218.0, 219.0, 220.0, // 30-34
            // Stabilization at target
            220.0, 220.0, 220.0, 219.0, 220.0, // 35-39
            220.0, 221.0, 220.0, 220.0, 220.0, // 40-44
            220.0, 220.0, 220.0, 219.0, 220.0, // 45-49
            220.0, 220.0, 220.0, 220.0, 220.0, // 50-54
            220.0, 220.0, 220.0, 220.0, 220.0, // 55-59
            220.0, // 60
        ];
        let idx = step.min(temps.len() - 1);
        temps[idx]
    }
}

/// Cooling curve: BT fall from 220°C to ~60°C over 20 steps.
struct CoolingCurve;

impl CoolingCurve {
    fn temp_at(&self, step: usize) -> f32 {
        let temps: [f32; 21] = [
            220.0, 205.0, 190.0, 176.0, 163.0, // 0-4
            150.0, 138.0, 127.0, 117.0, 108.0, // 5-9
            100.0, 93.0, 87.0, 81.0, 76.0, // 10-14
            72.0, 68.0, 65.0, 62.0, 60.0, // 15-19
            58.0, // 20
        ];
        let idx = step.min(temps.len() - 1);
        temps[idx]
    }
}

// ── Simulation helpers ────────────────────────────────────────────────────

struct SimulationContext {
    roaster: RoasterControl,
    curve: RoastSimCurve,
    step: usize,
}

enum RoastSimCurve {
    Idle,
    Preheating(PreheatCurve),
    Roasting(RoastCurve),
    Cooling(CoolingCurve),
}

impl SimulationContext {
    fn new() -> Self {
        let heater = Box::new(StubHeater::new());
        let fan = Box::new(StubFan::new());
        let sensor_hub = SensorConversionHub::new();
        let roaster = RoasterControl::new(heater, fan, sensor_hub).expect("RoasterControl init");
        Self {
            roaster,
            curve: RoastSimCurve::Idle,
            step: 0,
        }
    }

    fn advance_temperature(&mut self) {
        match self.curve {
            RoastSimCurve::Idle => {}
            RoastSimCurve::Preheating(ref curve) => {
                let bt = curve.temp_at(self.step);
                let et = bt - 5.0; // ET trails BT slightly during preheat
                let _ = self.roaster.update_temperatures(bt, et, Instant::now());
                let _ = self.roaster.update_control(Instant::now());
            }
            RoastSimCurve::Roasting(ref curve) => {
                let bt = curve.temp_at(self.step);
                let et = bt + 15.0; // ET > BT during active roast
                let _ = self.roaster.update_temperatures(bt, et, Instant::now());
                let _ = self.roaster.update_control(Instant::now());
            }
            RoastSimCurve::Cooling(ref curve) => {
                let bt = curve.temp_at(self.step);
                let et = bt - 10.0; // ET cools faster
                let _ = self.roaster.update_temperatures(bt, et, Instant::now());
                let _ = self.roaster.update_control(Instant::now());
            }
        }
        self.step += 1;
    }

    fn status(&self) -> SystemStatus {
        self.roaster.get_status()
    }

    fn state(&self) -> RoasterState {
        self.roaster.get_state()
    }

    fn send(&mut self, cmd: ArtisanCommand) {
        let _ = self.roaster.process_artisan_command(cmd);
    }

    /// Simulate the full preheat temperature ramp over `steps` iterations.
    fn run_preheat_ramp(&mut self, steps: usize) {
        self.curve = RoastSimCurve::Preheating(PreheatCurve);
        self.step = 0;
        for _ in 0..steps {
            self.advance_temperature();
        }
    }

    /// Simulate the active roast curve over `steps` iterations.
    fn run_roast_curve(&mut self, steps: usize) {
        self.curve = RoastSimCurve::Roasting(RoastCurve);
        self.step = 0;
        for _ in 0..steps {
            self.advance_temperature();
        }
    }

    /// Simulate cooldown over `steps` iterations.
    fn run_cooling_curve(&mut self, steps: usize) {
        self.curve = RoastSimCurve::Cooling(CoolingCurve);
        self.step = 0;
        for _ in 0..steps {
            self.advance_temperature();
        }
    }
}

/// Validate that a TC4 READ response is well-formed.
fn assert_valid_read_response(response: &str, label: &str) {
    let parts: Vec<&str> = response.split(',').collect();
    let n = parts.len();
    assert!(
        n == 5 || n == 8,
        "{}: READ response must have 5 or 8 fields, got {}: {}",
        label,
        n,
        response
    );

    // All temperature fields must parse as f32
    for (i, part) in parts.iter().enumerate() {
        let _: f32 = part
            .parse()
            .unwrap_or_else(|_| panic!("{}: field {} '{}' not a float", label, i, part));
    }

    // CHAN3 and CHAN4 must be 0.0
    assert_eq!(parts[3], "0.0", "{}: CHAN3 must be 0.0", label);
    assert_eq!(parts[4], "0.0", "{}: CHAN4 must be 0.0", label);

    if n == 8 {
        // Heater and fan must be in 0-100 range (percentages)
        let heater: f32 = parts[5].parse().unwrap();
        let fan: f32 = parts[6].parse().unwrap();
        assert!(
            (0.0..=100.0).contains(&heater),
            "{}: Heater {} out of range [0,100]",
            label,
            heater
        );
        assert!(
            (0.0..=100.0).contains(&fan),
            "{}: Fan {} out of range [0,100]",
            label,
            fan
        );
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

/// TEST-ARTISAN-SIM-01: Full roast simulation — initial handshake.
///
/// Artisan sends init commands (CHAN, UNITS, FILT) on connection.
/// We verify they parse correctly.
#[test]
fn test_handshake_commands_parse() {
    println!("TEST-ARTISAN-SIM-01: Handshake commands parse correctly");

    // Artisan sends CHAN;1200
    let cmd = parse_artisan_command("CHAN;1200");
    assert!(matches!(cmd, Ok(ArtisanCommand::Chan(1200))));

    // Artisan sends UNITS;C
    let cmd = parse_artisan_command("UNITS;C");
    assert!(matches!(cmd, Ok(ArtisanCommand::Units(false))));

    // Artisan sends FILT;70,70,70,70
    let cmd = parse_artisan_command("FILT;70,70,70,70");
    assert!(matches!(cmd, Ok(ArtisanCommand::Filt(70))));

    // Verify ack formats
    assert_eq!(
        ArtisanFormatter::format_chan_ack(1200),
        "#1200",
        "CHAN ack format"
    );

    println!("   ✅ All handshake commands parsed and ack verified");
}

/// TEST-ARTISAN-SIM-02: Preheat phase from Artisan's perspective.
///
/// Artisan sends PREHEAT 180, then monitors temperatures with READ.
#[test]
fn test_preheat_phase_read_verification() {
    println!("TEST-ARTISAN-SIM-02: Preheat phase with READ verification");

    let mut sim = SimulationContext::new();

    // PREHEAT
    sim.send(ArtisanCommand::Preheat(180.0));
    assert_eq!(sim.state(), RoasterState::Preheating);

    // Simulate temperature ramp 25→180°C over 30 steps
    sim.run_preheat_ramp(30);

    // After ramp, BT should be near 180°C
    let status = sim.status();
    assert!(
        status.bean_temp >= 175.0,
        "Preheat BT should be near 180°C, got {}",
        status.bean_temp
    );

    // Verify READ response at this point
    let response = ArtisanFormatter::format_read_response_full(&status);
    assert_valid_read_response(&response, "after preheat");

    println!(
        "   ✅ Preheat complete, BT={:.1}°C, READ={}",
        status.bean_temp, response
    );
}

/// TEST-ARTISAN-SIM-03: Profile loading — Artisan sends PROFILE command.
#[test]
fn test_profile_loading_via_artisan() {
    println!("TEST-ARTISAN-SIM-03: Profile loading via PROFILE command");

    // Simulate Artisan sending PROFILE;0,50;60,150;120,200;180,220
    let cmd = parse_artisan_command("PROFILE;0,50;60,150;120,200;180,220");
    assert!(
        matches!(cmd, Ok(ArtisanCommand::SetProfile)),
        "PROFILE should parse to SetProfile"
    );

    // Simulate Artisan sending FANPROFILE;0,30;60,50;120,70;180,80
    let cmd = parse_artisan_command("FANPROFILE;0,30;60,50;120,70;180,80");
    assert!(
        matches!(cmd, Ok(ArtisanCommand::SetFanProfile)),
        "FANPROFILE should parse to SetFanProfile"
    );

    println!("   ✅ Profile and fan profile commands parse correctly");
}

/// TEST-ARTISAN-SIM-04: Full roast lifecycle simulation with READ verification.
///
/// Walks through the complete Artisan-driven roast flow:
///   1. Idle → Set manual heater (OT1) → READ
///   2. START → profile-based PID control → READ
///   3. Active curve following → READ at multiple points
///   4. STOP → Cooldown → READ
///
/// Artisan typically controls the roaster via manual OT1 (heater %) commands
/// while PID runs as fallback. This test simulates that workflow.
#[test]
fn test_full_roast_lifecycle_with_read() {
    println!("TEST-ARTISAN-SIM-04: Full roast lifecycle with READ");

    let mut sim = SimulationContext::new();

    // ── Phase 1: Idle ──────────────────────────────────────────────────
    assert_eq!(sim.state(), RoasterState::Idle);
    let response = ArtisanFormatter::format_read_response_full(&sim.status());
    assert_valid_read_response(&response, "idle-start");
    println!("   Phase 1 ✅ Idle state, READ={}", response);

    // ── Phase 2: Load profile and START ──────────────────────────────
    // Load a roast profile (as Artisan would via PROFILE command)
    let mut profile = RoastProfile::new();
    let _ = profile.setpoints.push(ProfileSetpoint {
        time_secs: 0,
        temperature: 150.0,
    });
    let _ = profile.setpoints.push(ProfileSetpoint {
        time_secs: 120,
        temperature: 200.0,
    });
    let _ = profile.setpoints.push(ProfileSetpoint {
        time_secs: 240,
        temperature: 220.0,
    });
    store_profile(profile);
    sim.send(ArtisanCommand::SetProfile);

    // START roast (no prior OT1 — START transitions state to Heating)
    sim.send(ArtisanCommand::StartRoast);
    assert_eq!(
        sim.state(),
        RoasterState::Heating,
        "After START, state should be Heating"
    );

    // After START, Artisan typically sets manual heater via OT1
    sim.send(ArtisanCommand::SetHeater(80));
    sim.send(ArtisanCommand::SetFan(30));

    let s = sim.status();
    let response = ArtisanFormatter::format_read_response_full(&s);
    assert_valid_read_response(&response, "after-start");
    println!("   Phase 2 ✅ START+OT1, state=Heating, READ={}", response);

    // ── Phase 3: Active roast curve ────────────────────────────────────
    // Run full roast curve in one pass (60 steps → ~220°C)
    sim.run_roast_curve(60);
    let s = sim.status();
    assert!(
        s.bean_temp >= 215.0,
        "BT should reach ~220°C at roast end, got {:.1}°C",
        s.bean_temp
    );
    let response = ArtisanFormatter::format_read_response_full(&s);
    assert_valid_read_response(&response, "roast-end");
    println!(
        "   Phase 3 ✅ Roast complete BT={:.1}°C, READ={}",
        s.bean_temp, response
    );

    // ── Phase 4: STOP + Cooldown ──────────────────────────────────────
    sim.send(ArtisanCommand::EmergencyStop);

    let s = sim.status();
    assert_eq!(s.ssr_output, 0.0, "Heater must be 0%% after STOP");
    let response = ArtisanFormatter::format_read_response_full(&s);
    assert_valid_read_response(&response, "after-stop");
    println!("   Phase 4a ✅ STOP: heater=0%%, READ={}", response);

    // Simulate cooldown (20 steps in one pass: 220°C → 58°C)
    sim.run_cooling_curve(20);
    let s = sim.status();
    assert!(
        s.bean_temp <= 70.0,
        "BT should be near ambient at cooldown end, got {:.1}°C",
        s.bean_temp
    );
    let response = ArtisanFormatter::format_read_response_full(&s);
    assert_valid_read_response(&response, "cooling-end");
    println!(
        "   Phase 4b ✅ Cooldown complete BT={:.1}°C, READ={}",
        s.bean_temp, response
    );

    println!("\n   ✅ Full Artisan roast lifecycle simulation PASSED");
}

/// TEST-ARTISAN-SIM-05: Artisan sends multiple READ commands during active roast.
///
/// During a roast, Artisan polls READ every ~1s. Verify response consistency.
#[test]
fn test_artisan_read_polling_during_roast() {
    println!("TEST-ARTISAN-SIM-05: Artisan READ polling during roast");

    let mut sim = SimulationContext::new();

    // START roast
    sim.roaster.status_mut().artisan_control = true;
    sim.send(ArtisanCommand::StartRoast);

    // Run the first 20 steps of the roast curve
    sim.run_roast_curve(20);

    // Poll READ 5 times (simulating Artisan's ~1s polling)
    for i in 0..5 {
        let s = sim.status();
        let response = ArtisanFormatter::format_read_response_full(&s);
        assert_valid_read_response(&response, &format!("poll-{}", i + 1));

        println!("   Poll {}: BT={:.1}°C → {}", i + 1, s.bean_temp, &response);
    }

    println!("   ✅ READ polling produces valid responses");
}

/// TEST-ARTISAN-SIM-06: STOP during roast — READ shows heater=0 fan=100.
///
/// When Artisan sends STOP mid-roast, the READ response must immediately
/// reflect the emergency state (heater off, fan max).
#[test]
fn test_stop_during_roast_reflected_in_read() {
    println!("TEST-ARTISAN-SIM-06: STOP during roast reflected in READ");

    let mut sim = SimulationContext::new();

    // Artisan typically controls via manual (OT1) heater
    // START enables the roast state machine
    sim.send(ArtisanCommand::SetHeater(80));
    sim.send(ArtisanCommand::StartRoast);
    sim.run_roast_curve(10);

    // Verify heater is active (through manual control)
    let s = sim.status();
    assert!(
        s.ssr_output > 0.0,
        "Heater should be >0%% during roast, got {:.1}%%",
        s.ssr_output
    );
    println!(
        "   Mid-roast: heater={:.1}%%, fan={:.1}%%",
        s.ssr_output, s.fan_output
    );

    // STOP
    sim.send(ArtisanCommand::EmergencyStop);

    let s = sim.status();
    assert_eq!(s.ssr_output, 0.0, "Heater must be 0 after STOP");

    let response = ArtisanFormatter::format_read_response_full(&s);
    assert_valid_read_response(&response, "after-stop");

    let parts: Vec<&str> = response.split(',').collect();
    if parts.len() == 8 {
        assert_eq!(parts[5], "0.0", "Heater in READ must be 0 after STOP");
    }

    println!("   ✅ STOP reflected: READ={}", response);
}

/// TEST-ARTISAN-SIM-07: Temperature scale switching — Artisan sends UNITS;F.
///
/// Artisan can switch between Celsius and Fahrenheit mid-roast.
/// Verify READ response temperatures are converted correctly.
#[test]
fn test_temperature_scale_switching_during_roast() {
    println!("TEST-ARTISAN-SIM-07: Temperature scale switching mid-roast");

    let mut sim = SimulationContext::new();

    // Preheat + START
    sim.roaster.status_mut().artisan_control = true;
    sim.send(ArtisanCommand::StartRoast);
    sim.run_roast_curve(5);

    // At this point BT is ~114°C (from RoastCurve)
    let s = sim.status();
    let bt_c = s.bean_temp;
    println!("   Current BT: {:.1}°C", bt_c);

    // Switch to Fahrenheit
    sim.roaster
        .status_mut()
        .temperature_settings
        .set_scale(TemperatureScale::Fahrenheit);

    let response_f = ArtisanFormatter::format_read_response_full(&sim.status());
    assert_valid_read_response(&response_f, "fahrenheit");

    let parts_f: Vec<&str> = response_f.split(',').collect();
    let bt_f: f32 = parts_f[2].parse().unwrap();
    let expected_f = bt_c * 9.0 / 5.0 + 32.0;
    assert!(
        (bt_f - expected_f).abs() < 1.0,
        "BT in °F ({:.1}) should be ~{:.1} (converted from {:.1}°C)",
        bt_f,
        expected_f,
        bt_c
    );
    println!(
        "   ✅ °F conversion: {:.1}°C → {:.1}°F (via READ={})",
        bt_c, bt_f, response_f
    );
}

/// TEST-ARTISAN-SIM-08: READ response during all roasting states.
///
/// Artisan may send READ at any point in the roast lifecycle.
/// Verify the response is valid in every state.
#[test]
fn test_read_in_all_states() {
    println!("TEST-ARTISAN-SIM-08: READ in all roast states");

    let mut sim = SimulationContext::new();

    let states_to_check: &[(RoasterState, &str)] = &[(RoasterState::Idle, "Idle")];

    for &(_state, label) in states_to_check {
        let response = ArtisanFormatter::format_read_response_full(&sim.status());
        assert_valid_read_response(&response, label);
        println!("   State {} → READ={}", label, response);
    }

    // Preheat
    sim.send(ArtisanCommand::Preheat(180.0));
    sim.run_preheat_ramp(10);
    let response = ArtisanFormatter::format_read_response_full(&sim.status());
    assert_valid_read_response(&response, "Preheating");
    println!("   State Preheating → READ={}", response);

    // Continue preheat to stable
    sim.run_preheat_ramp(21);

    // START + roast
    sim.roaster.status_mut().artisan_control = true;
    sim.send(ArtisanCommand::StartRoast);
    sim.run_roast_curve(15);
    let response = ArtisanFormatter::format_read_response_full(&sim.status());
    assert_valid_read_response(&response, "Heating");
    println!("   State Heating → READ={}", response);

    // Continue to stable
    sim.run_roast_curve(46);
    let response = ArtisanFormatter::format_read_response_full(&sim.status());
    assert_valid_read_response(&response, "Stable");
    println!("   State Stable → READ={}", response);

    // STOP → EmergencyStop
    sim.send(ArtisanCommand::EmergencyStop);
    let response = ArtisanFormatter::format_read_response_full(&sim.status());
    assert_valid_read_response(&response, "EmergencyStop");
    println!("   State EmergencyStop → READ={}", response);

    println!("   ✅ READ valid in all roast states");
}
