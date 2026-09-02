//! S1–S8 reproduction tests for the critical-safety bug hunt.
//!
//! Each test documents a candidate finding. Tests that document current
//! (possibly undesirable) behaviour PASS and serve as evidence for the report;
//! S5 (the NaN heater-output invariant) now asserts the FIXED behaviour
//! (`Err(RoasterError::InvalidState("non_finite_heater_output"))`) as a
//! plain `#[test]` — the historical `#[ignore]` marker was removed when the
//! fix landed (Audit A-TC4, 2026-08-12: the stale header above used to claim
//! the `#[ignore]` still existed).

#![cfg(all(test, feature = "test", not(target_arch = "riscv32")))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

extern crate alloc;
extern crate std;

use std::boxed::Box;
use std::cell::RefCell;

use embassy_time::{Duration, Instant};

use libreroaster::common::{StubFan, StubHeater};
use libreroaster::config::constants::{
    RoasterState, FAN_MIN_SAFETY_PCT, MAX_ROAST_TIME_SECS, OVERTEMP_THRESHOLD,
};
use libreroaster::config::{ArtisanCommand, SystemStatus};
use libreroaster::control::controllers::ActuatorController;
use libreroaster::control::roaster_control::RoasterControl;
use libreroaster::control::traits::Fan;
use libreroaster::control::RoasterError;
use libreroaster::hardware::sensors::SensorConversionHub;

/// Build a stub `RoasterControl` for the S1–S8 safety repro tests.
fn make_control() -> RoasterControl {
    RoasterControl::new(
        Box::new(StubHeater::new()),
        Box::new(StubFan::new()),
        SensorConversionHub::new(),
    )
    .expect("test control should build")
}

/// Build a stub `RoasterControl` using the supplied fan implementation.
fn make_control_with_fan(fan: Box<dyn Fan + Send>) -> RoasterControl {
    RoasterControl::new(Box::new(StubHeater::new()), fan, SensorConversionHub::new())
        .expect("test control should build")
}

/// One control tick with fresh sensor data at the same instant.
fn tick(ctrl: &mut RoasterControl, bt: f32, et: f32, now: Instant) {
    ctrl.update_temperatures(bt, et, now).expect("temps");
    let _ = ctrl.update_control(now);
}

/// Fan whose emergency 100% path ALWAYS fails (every retry).
struct DeadFan {
    speed: RefCell<f32>,
}

impl DeadFan {
    fn new() -> Self {
        Self {
            speed: RefCell::new(0.0),
        }
    }
}

impl Fan for DeadFan {
    fn set_speed(&mut self, duty: f32) -> Result<(), RoasterError> {
        *self.speed.borrow_mut() = duty;
        Ok(())
    }
    fn emergency_set_speed(&mut self, _percentage: f32) -> Result<(), RoasterError> {
        Err(RoasterError::HardwareError {
            source: Some("dead_fan"),
        })
    }
    fn get_speed(&self) -> f32 {
        *self.speed.borrow()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// S1 — (candidate CRÍTICO-SEGURIDAD) manual mode with a dead probe has NO
// sensor-based supervision
// ═══════════════════════════════════════════════════════════════════════════
//
// A shorted thermocouple reads ~0 °C without setting the MAX31856 fault bit,
// and `is_temperature_valid` accepts it (sensor.rs:278-280), so:
//   - overtemp trap: never fires (0 °C < 260 °C)
//   - NaN trap:      never fires (0.0 is finite)
//   - RoR guard:     not armed in manual mode (pid_enabled == false)
//   - probe-stuck:   armed at any duty > 0 (Bug S1 fix); two-stage in manual
//                    mode since Audit A-TC4-C — warning at 120 s, latch at
//                    300 s. This repro's loop spans only milliseconds of REAL
//                    time, so neither window elapses inside it.
//   - staleness:     reads keep arriving, so never stale
//   - comms-idle:    Artisan polls READ every ~1 s, refreshing
//                    `last_command_received_at_ms` on EVERY command
//                    (roaster_control.rs:1079) — the 15 s idle backstop
//                    never elapses in a live session
// The only backstop left for the blind loop is MAX_ROAST_TIME (30 min).
#[test]
fn s1_dead_probe_manual_roast_runs_without_sensor_supervision() {
    let mut ctrl = make_control();
    let t0 = Instant::now();
    tick(&mut ctrl, 25.0, 30.0, t0);

    // Manual session: OT1 30. Post-fix S1 the probe-stuck detector arms at
    // any duty > 0, but this loop advances only REAL milliseconds of clock,
    // so neither the 120 s warning nor the 300 s manual latch (A-TC4-C) can
    // elapse inside it.
    ctrl.process_artisan_command(ArtisanCommand::SetHeater(30))
        .expect("OT1 30");
    assert!(ctrl.get_status().artisan_control);

    // ~2 minutes of simulated roasting with the dead probe (valid 0.0 °C)
    // and Artisan-style READ polling on every tick.
    for _ in 0..360 {
        let now = Instant::now();
        ctrl.update_temperatures(0.0, 0.0, now)
            .expect("dead probe read");
        ctrl.process_artisan_command(ArtisanCommand::ReadStatus)
            .expect("READ poll");
        let _ = ctrl.update_control(now);
    }

    // No trap fired: the heater kept running blind.
    assert!(
        !ctrl.safety().is_emergency_active(),
        "S1: no supervision fires during a dead-probe manual roast at 30%"
    );
    assert_eq!(ctrl.get_state(), RoasterState::Idle);
    assert!(
        ctrl.get_status().ssr_output > 0.0,
        "S1: the heater keeps running blind"
    );
    assert_eq!(
        ctrl.get_status().fan_output,
        FAN_MIN_SAFETY_PCT,
        "the fan interlock floor is the only protection still active"
    );

    // The LAST resort is temporal: MAX_ROAST_TIME (30 min) eventually fires.
    let far = Instant::now() + Duration::from_millis((MAX_ROAST_TIME_SECS as u64 + 60) * 1000);
    ctrl.update_temperatures(0.0, 0.0, far).expect("read");
    ctrl.process_artisan_command(ArtisanCommand::ReadStatus)
        .expect("READ poll");
    let _ = ctrl.update_control(far);
    assert!(
        ctrl.safety().is_emergency_active(),
        "S1: only the 30-minute temporal backstop eventually fires"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// S2 — (design HIGH) the serial whitelist clears a latched emergency and
// allows immediate re-energization
// ═══════════════════════════════════════════════════════════════════════════
//
// START / PREHEAT / OFF are whitelisted during `fault_condition`
// (roaster_control.rs:1004-1019) and START calls
// `clear_emergency_explicit` (roaster_control.rs:1125-1127), so any serial
// client that can emit START + OT1 defeats the emergency latch. This is a
// deliberate trust-the-operator design (Bug P3), recorded here as evidence.
#[test]
fn s2_serial_start_clears_latched_emergency_and_reenergizes() {
    let mut ctrl = make_control();
    let t0 = Instant::now();

    // Arm a real latched emergency via the overtemp trap.
    let _ = ctrl.update_temperatures(OVERTEMP_THRESHOLD + 10.0, 25.0, t0);
    assert!(ctrl.safety().is_emergency_active());
    assert_eq!(ctrl.get_state(), RoasterState::Error);
    assert!(ctrl.get_status().fault_condition);

    // Positive control: a NON-whitelisted heater command is rejected while
    // the fault is latched.
    let rejected = ctrl.process_artisan_command(ArtisanCommand::SetHeater(100));
    assert!(
        rejected.is_err(),
        "OT1 must be rejected while fault_condition is active"
    );

    // The whitelist: START is accepted during the fault and un-latches it...
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("START allowed during fault");
    assert!(
        !ctrl.safety().is_emergency_active(),
        "S2: START un-latches a real latched emergency"
    );
    assert!(
        !ctrl.get_status().fault_condition,
        "S2: START clears fault_condition"
    );

    // ...and the heater can be re-energized immediately afterwards.
    ctrl.process_artisan_command(ArtisanCommand::SetHeater(100))
        .expect("re-energization accepted after START");
    let t1 = Instant::now();
    tick(&mut ctrl, 150.0, 25.0, t1);
    assert!(
        ctrl.get_status().ssr_output > 0.0,
        "S2: heater re-energized one tick after a latched overtemp"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// S3 — (OPERATIVO) PID;CT accepts an unbounded u32 → regulation freezes
// ═══════════════════════════════════════════════════════════════════════════
//
// `parse_pid_subcommand` accepts any u32 (parser.rs:390-402, only a 10 ms
// floor). With `PID;CT 4294967295`, `update_pid_control`'s throttle
// (`cycle_ms = pid_cycle_time_ms.max(10)`, roaster_control.rs:1673) never
// expires after the first cycle: the PID stops computing and the heater holds
// its last output with zero regulation. The temporal backstops still run.
#[test]
fn s3_pid_cycle_time_huge_freezes_regulation() {
    // Frozen-throttle control: PID;CT = u32::MAX.
    let mut frozen = make_control();
    let t0 = Instant::now();
    tick(&mut frozen, 25.0, 30.0, t0);
    frozen
        .process_artisan_command(ArtisanCommand::SetPidGain(1.0, 0.05, 0.0))
        .expect("PIDGAIN");
    frozen
        .process_artisan_command(ArtisanCommand::SetPidCycleTime(u32::MAX))
        .expect("PID;CT max");
    frozen
        .process_artisan_command(ArtisanCommand::Preheat(50.0))
        .expect("preheat");
    assert_eq!(frozen.get_status().pid_cycle_time_ms, u32::MAX);

    let t1 = t0 + Duration::from_millis(330);
    tick(&mut frozen, 25.0, 30.0, t1);
    let first_output = frozen.get_status().ssr_output;
    assert!(
        first_output > 0.0 && first_output < 100.0,
        "first PID cycle lands mid-range (kp=1, error 25 °C)"
    );

    // 40 ticks with a persistent 25 °C error: a live PID integrates; the
    // frozen throttle must not move.
    for i in 0..40 {
        let t = t0 + Duration::from_millis(330 * (i + 2));
        tick(&mut frozen, 25.0, 30.0, t);
    }
    assert_eq!(
        frozen.get_status().ssr_output,
        first_output,
        "S3: PID;CT u32::MAX freezes regulation — output never recomputed"
    );

    // Live control with the default cycle time (~100 ms): the same persistent
    // error makes the I-term integrate and the output climb — proving the
    // freeze is caused by the unbounded cycle time, not by the scenario.
    let mut live = make_control();
    let u0 = Instant::now();
    tick(&mut live, 25.0, 30.0, u0);
    live.process_artisan_command(ArtisanCommand::SetPidGain(1.0, 0.05, 0.0))
        .expect("PIDGAIN");
    live.process_artisan_command(ArtisanCommand::Preheat(50.0))
        .expect("preheat");
    for i in 0..40 {
        let t = u0 + Duration::from_millis(330 * (i + 1));
        tick(&mut live, 25.0, 30.0, t);
    }
    assert!(
        live.get_status().ssr_output > frozen.get_status().ssr_output,
        "S3 control: a live PID must integrate above the frozen output ({} vs {})",
        live.get_status().ssr_output,
        frozen.get_status().ssr_output
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// S4 — (OPERATIVO, FIX 2026-08-05) internal-trap emergencies must propagate a
// total fan failure
// ═══════════════════════════════════════════════════════════════════════════
//
// Pre-fix, `ActuatorController::emergency_shutdown` ignored the
// `force_fan_100` result for internal traps: the caller received
// `Err(EmergencyShutdown)` whether the fan reached 100 % or never moved,
// absorbing the failure silently. The command-driven paths (STOP, safety
// outcomes) already propagated fan failure — only the trap path absorbed it.
// Post-fix (S4): a trap with an unresponsive fan escalates as
// `Err(HardwareError { source: "emergency_fan_failed" })` so the control loop
// surfaces an ERR to Artisan ("no fan means unsafe to continue").
#[test]
fn s4_internal_trap_absorbs_fan_failure() {
    let mut ctrl = make_control_with_fan(Box::new(DeadFan::new()));
    let now = Instant::now();

    let result = ctrl.update_temperatures(OVERTEMP_THRESHOLD + 10.0, 25.0, now);
    assert!(
        matches!(
            result,
            Err(RoasterError::HardwareError {
                source: Some("emergency_fan_failed")
            })
        ),
        "S4 fix: a total fan failure during an internal trap must escalate as \
         HardwareError(emergency_fan_failed), got {result:?}"
    );
    assert_eq!(ctrl.get_state(), RoasterState::Error);
    assert!(ctrl.safety().is_emergency_active());
    assert_eq!(ctrl.get_status().ssr_output, 0.0, "heater off");

    // The fan retries exhausted and never moved — and now the error channel
    // surfaced it: the trap is no longer indistinguishable from a fan-OK
    // overtemp.
    assert_ne!(
        ctrl.get_status().fan_output,
        100.0,
        "fan never reached 100 % (retries exhausted)"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// S5 — (latente, FIX 2026-08-05) NaN into the actuator is rejected instead of
// poisoning ssr_output and disarming the temporal backstops
// ═══════════════════════════════════════════════════════════════════════════
//
// Pre-fix, `apply_guarded_heater` passed NaN through the clamp and the slew:
// `status.ssr_output` became NaN, and the comms-idle / MAX_ROAST_TIME gates
// (`ssr_output > 0.0`) then evaluated `NaN > 0.0 == false` and disarmed.
// Physically the heater was OFF (NaN → duty 0), but the supervision was blind.
// Post-fix (S5): non-finite `desired` is rejected with
// `Err(InvalidState(non_finite_heater_output))` before any state mutation —
// the control loop escalates it to a latched emergency (fail-safe).
#[test]
fn s5_nan_input_poisons_ssr_output_and_disarms_backstops() {
    let mut status = SystemStatus::default();
    let mut act = ActuatorController::new(Box::new(StubHeater::new()), Box::new(StubFan::new()));
    let now = Instant::from_millis(1_000);

    let result = act.apply_guarded_heater(f32::NAN, now, false, &mut status);

    assert!(
        matches!(
            result,
            Err(RoasterError::InvalidState {
                source: Some("non_finite_heater_output")
            })
        ),
        "S5 fix: NaN must be rejected before any state mutation, got {result:?}"
    );
    assert!(
        !status.ssr_output.is_nan(),
        "S5: ssr_output must stay finite — NaN here makes comms-idle and \
         MAX_ROAST_TIME inert (NaN > 0.0 == false)"
    );
    assert!(
        status.ssr_output <= 0.0,
        "S5: with a rejected NaN the output stays at the previous value"
    );

    // Same for +Inf/-Inf — every non-finite class must be rejected.
    for hostile in [f32::INFINITY, f32::NEG_INFINITY] {
        let result = act.apply_guarded_heater(hostile, now, false, &mut status);
        assert!(
            result.is_err(),
            "S5: {hostile:?} must be rejected too, got {result:?}"
        );
        assert!(status.ssr_output.is_finite());
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// S6 — (OPERATIVO, bajo, FIX 2026-08-05) OT2 below the fan floor is clamped on
// the command path too
// ═══════════════════════════════════════════════════════════════════════════
//
// Pre-fix, the command path (`apply_policy_outcome`) wrote the fan value
// directly with no `FAN_MIN_SAFETY_PCT` floor; the floor re-asserted only on
// the next control tick. In between, a hot heater ran with 0 % airflow (+ up
// to ~1.1 s of hardware fade to recover). Post-fix (S6): the command path
// applies the same interlock whenever the heater is energized, so the fan
// never dips below the floor even for one command.
#[test]
fn s6_ot2_zero_bypasses_fan_floor_until_next_tick() {
    let mut ctrl = make_control();
    // Bug fix (2026-08-10): the host time driver has 1 µs tick resolution —
    // consecutive `Instant::now()` calls can return the SAME tick, which
    // zeroes the slew limiter's dt (`actual_output = slewing + 50·dt = 0`)
    // and leaves `ssr_output == 0.0` — the clamp gate below would then see
    // "heater off" and skip the fan floor, making this test flaky depending
    // on scheduler timing. Advance the clock explicitly so every tick and
    // command sees dt > 0.
    let t0 = Instant::now();
    tick(&mut ctrl, 25.0, 30.0, t0);

    ctrl.process_artisan_command(ArtisanCommand::SetHeater(50))
        .expect("OT1 50");
    // +500 ms: slew applies (dt > 0) and the SSR cycle guard (100 ms) is
    // open again, so this tick physically applies the heater.
    let t1 = t0 + Duration::from_millis(500);
    tick(&mut ctrl, 25.0, 30.0, t1);
    assert_eq!(
        ctrl.get_status().fan_output,
        FAN_MIN_SAFETY_PCT,
        "floor applied on the tick"
    );

    // OT2 0 while the heater is ON: the command path now clamps to the floor
    // immediately — no 0 % airflow window.
    ctrl.process_artisan_command(ArtisanCommand::SetFanSpeed(0, false))
        .expect("OT2 0");
    assert_eq!(
        ctrl.get_status().fan_output,
        FAN_MIN_SAFETY_PCT,
        "S6 fix: OT2 0 with heater on must clamp to FAN_MIN_SAFETY_PCT on the \
         command path (no 0 % window)"
    );
    assert!(
        ctrl.get_status().ssr_output > 0.0,
        "the heater stays on during the fan-zero window"
    );

    // The floor also re-asserts on the next control tick (idempotent).
    let t2 = t1 + Duration::from_millis(500);
    tick(&mut ctrl, 25.0, 30.0, t2);
    assert_eq!(
        ctrl.get_status().fan_output,
        FAN_MIN_SAFETY_PCT,
        "floor restored on the next tick"
    );
}
