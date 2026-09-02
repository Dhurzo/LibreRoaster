//! Top-level control facade for the roaster: owns the sensor, actuator,
//! safety and dispatch controllers plus roast state (`RoasterState`,
//! profiles, charge detection) and is the single writer to hardware.
//!
//! The application layer drives it from two entry points: `update_control`
//! (one control-loop tick: safety backstops, PID/manual selection, guarded
//! heater/fan writes) and `process_artisan_command` / `process_command`
//! (TC4/Artisan commands). All safety latches live here.
use super::policies::{ManualPolicyOutcome, SafetyPolicyOutcome};
use super::RoasterError;
use crate::config::*;
use crate::control::controllers::{
    ActuatorController, CommandDispatchResult, CommandDispatcher, SafetyController,
    SensorController,
};
use crate::control::pid::PidFeedback;
use crate::control::traits::{Fan, Heater};
use alloc::boxed::Box;
use embassy_time::{Duration, Instant};

use log::{debug, error, info, warn};

use crate::hardware::sensors::SensorConversionHub;
use crate::logging::edge_log_gate::EdgeLogGate;

/// Bug R7 (2026-07-26): a manual heater session (`heat_session_start`) only
/// closes after the heater has been OFF for this long. Prevents a momentary
/// `OT1 0` from resetting the MAX_ROAST_TIME budget.
const HEAT_SESSION_OFF_DEBOUNCE_SECS: u64 = 60;

/// Central control object: roast state machine, safety latches and the
/// single writer that applies sensor/actuator/safety/dispatch decisions.
pub struct RoasterControl {
    state: RoasterState,
    status: SystemStatus,
    sensor: SensorController,
    actuator: ActuatorController,
    safety: SafetyController,
    dispatch: CommandDispatcher,
    /// Last time the PID actually computed output (throttles to `pid_cycle_time_ms`).
    last_pid_update: Option<Instant>,
    active_profile: Option<RoastProfile>,
    profile_start_time: Option<Instant>,
    /// Bug M3 (2026-07-25): the maximum-roast-time backstop used to key on
    /// `profile_start_time` (the timestamp `START` captures). In TC4 manual
    /// mode (the most common Artisan flow) the heater is energized at OT1
    /// without a `START` ever arriving, so the time cap was inert and a
    /// forgotten heater could keep cooking indefinitely. Track when the
    /// heater first crosses 0 with the time cap active; both
    /// `profile_start_time`-set roasts and heater-energized manual sessions
    /// get the same time budget.
    heat_session_start: Option<Instant>,
    /// Bug R7 (2026-07-26): tracks when the heater last went OFF mid-session.
    /// `heat_session_start` used to be dropped on the FIRST heater==0 tick,
    /// so a single `OT1 0` between commands reset the MAX_ROAST_TIME budget
    /// (a 30-min cap bypassable by toggling the heater off for one tick).
    /// With this debounce, the session only closes after the heater has been
    /// OFF for `HEAT_SESSION_OFF_DEBOUNCE_SECS`.
    heat_session_off_since: Option<Instant>,
    fan_profile: Option<crate::config::FanProfile>,
    charge_detected: bool,
    charge_time: Option<Instant>,
    preheat_target: Option<f32>,
    /// Rolling bean-temperature samples feeding charge (bean-drop) detection.
    bt_charge_history: heapless::Deque<f32, 10>,
    /// Bug B23: per-tick divider that throttles `bt_charge_history` sampling
    /// to once every `CHARGE_SAMPLE_TICK_DIV` ticks. With the real tick
    /// cadence (`CONTROL_LOOP_TICK_MS` ≈ 330 ms, see constants.rs) the
    /// divisor resolves to 1 — the deque of 10 samples covers the intended
    /// ≈ 3 s charge window. (The earlier derivation assumed the 100 ms timer
    /// alone, so the deque actually spanned ~9.9 s and `#CHARGE` was
    /// effectively indetectable — bug audit 2026-08-02.)
    charge_history_tick_div: u8,
    /// Bug P5 (2026-08-03): probe-stuck detector state. A hard thermocouple
    /// short reads a flat ~0 °C, which is a VALID temperature — no MAX31856
    /// fault bit, so the fault/NaN paths never fire and the PID would drive
    /// the heater blind. While the heater runs, BT must move by more than
    /// `PROBE_STUCK_VARIATION_C` within the timeout window; otherwise the
    /// probe is shorted/broken. Firmware-PID mode latches via
    /// `emergency_shutdown("Probe stuck")` at `PROBE_STUCK_TIMEOUT_SECS`;
    /// manual / Artisan software-PID mode is two-stage (Audit A-TC4-C,
    /// 2026-08-12) — see `update_control`.
    probe_stuck_last_bt: Option<f32>,
    probe_stuck_last_change: Option<Instant>,
    /// Audit A-TC4-C (2026-08-12): manual-mode two-stage probe-stuck. Set once
    /// the `ERR probe_stuck_warning` wire line has been emitted for the
    /// current stuck episode; cleared on BT movement > `PROBE_STUCK_VARIATION_C`,
    /// on disarm (heater 0 / non-finite BT), and when a new episode begins.
    /// Guarantees exactly one warning line per episode.
    probe_stuck_warning_sent: bool,
    // Bug B3: latched cooling fan after a plain STOP. `stop_streaming` sets the
    // fan to 100% but does NOT arm the safety emergency latch (only
    // `emergency_shutdown` does). Without this flag, the next `update_control`
    // tick would fall through to `artisan_manual_fan()` (cleared to 0.0 by
    // `dispatch.stop_streaming → clear_manual`) and annihilate the cooldown a
    // single tick (~100 ms) after STOP. Set on STOP, dropped when a new roast
    // starts (handle_start_roast), on explicit recovery (clear_emergency_explicit),
    // or once the bean mass cools below the safe-to-handle threshold.
    cooling_active: bool,
    /// Bug B13 / V2-7: queue of `#DUMP` rows waiting to be sent. The async
    /// emitter in `src/application/tasks.rs::emit_telemetry_stage` drains up
    /// to `MAX_DUMP_ROWS_PER_TICK` rows per 100 ms tick (outside the 1 Hz
    /// `should_emit` gate) and re-pushes a row to the front if the output
    /// channel is full — so no row is lost. The queue is sized to hold a
    /// full-ring dump (`LOG_CAPACITY + 1` rows) so a complete roast can be
    /// requested via `#DUMP` without losing any row to queue overflow.
    // Audit H-5 (2026-08-11): rows were `String<256>` (257 × ~264 B ≈ 68 KB
    // static) for 33-40 B rows. `DUMP_ROW_CAPACITY` (128, the ring-entries'
    // true upper bound — see roast_logger.rs) halves the static cost to
    // ≈ 34 KB with zero truncation risk.
    dump_pending: heapless::Deque<
        heapless::String<{ crate::logging::roast_logger::DUMP_ROW_CAPACITY }>,
        { crate::logging::roast_logger::LOG_CAPACITY + 1 },
    >,
    fan_floor_gate: EdgeLogGate,
}

impl RoasterControl {
    /// Build the control facade from boxed heater/fan drivers and the sensor hub.
    pub fn new(
        heater: Box<dyn Heater + Send>,
        fan: Box<dyn Fan + Send>,
        sensor_hub: SensorConversionHub,
    ) -> Result<Self, RoasterError> {
        Ok(RoasterControl {
            state: RoasterState::Idle,
            status: SystemStatus::default(),
            sensor: SensorController::new(sensor_hub),
            actuator: ActuatorController::new(heater, fan),
            safety: SafetyController::new(),
            dispatch: CommandDispatcher::new()?,
            last_pid_update: None,
            active_profile: None,
            profile_start_time: None,
            heat_session_start: None,
            heat_session_off_since: None,
            fan_profile: None,
            charge_detected: false,
            charge_time: None,
            preheat_target: None,
            bt_charge_history: heapless::Deque::new(),
            charge_history_tick_div: 0,
            probe_stuck_last_bt: None,
            probe_stuck_last_change: None,
            probe_stuck_warning_sent: false,
            cooling_active: false,
            dump_pending: heapless::Deque::new(),
            fan_floor_gate: EdgeLogGate::new(),
        })
    }

    /// Read both MAX31856 channels into status; over-range readings latch the emergency.
    pub async fn read_sensors(&mut self) -> Result<(), RoasterError> {
        match self.sensor.read_sensors(&mut self.status).await {
            Err(RoasterError::TemperatureOutOfRange {
                source: Some("overtemp_detected"),
            })
            | Err(RoasterError::TemperatureOutOfRange {
                source: Some("temperature_out_of_valid_range"),
            }) => {
                // Bug L2 (2026-08-10): `emergency_shutdown` ALWAYS returns an
                // Err (`actuator.emergency_shutdown` never returns Ok), so the
                // trailing `Err(TemperatureOutOfRange{...overtemp_detected})`
                // after the `?` was unreachable dead code. Return the actual
                // error — the callers (tasks / ServiceContainer) match on the
                // variant generically, so nothing downstream changes.
                self.emergency_shutdown("Temperature exceeds valid range")
            }
            other => other,
        }
    }

    /// Return a copy of the current system status snapshot.
    pub fn get_status(&self) -> SystemStatus {
        self.status
    }

    /// Mutable handle to the live `SystemStatus`.
    pub fn status_mut(&mut self) -> &mut SystemStatus {
        &mut self.status
    }

    /// Most recent raw sensor sample, if one has been read.
    pub fn last_sensor_sample(&self) -> Option<crate::hardware::sensors::SensorSample> {
        self.sensor.last_sensor_sample()
    }

    /// Current roast state machine state.
    pub fn get_state(&self) -> RoasterState {
        self.state
    }

    /// Apply a fresh BT/ET sample assuming no sensor faults.
    pub fn update_temperatures(
        &mut self,
        bean_temp: f32,
        env_temp: f32,
        current_time: Instant,
    ) -> Result<(), RoasterError> {
        let no_fault = Default::default();
        self.update_temperatures_with_fault(bean_temp, env_temp, no_fault, no_fault, current_time)
    }

    /// Temperature update with explicit sensor-fault flags. Used by the
    /// production read path (which derives faults from the MAX31856 status
    /// register) and by tests that need to inject a faulted sample directly.
    /// Bug B7: a faulted channel does NOT immediately poison its temperature
    /// with NaN — the F4.11 debouncer must confirm the fault is persistent
    /// (≥ `SENSOR_FAULT_DEBOUNCE` consecutive samples) before NaN propagates
    /// and the PID/emergency guard sees a faulted PV.
    pub fn update_temperatures_with_fault(
        &mut self,
        bean_temp: f32,
        env_temp: f32,
        bean_fault: crate::hardware::sensors::conversion::SensorFault,
        env_fault: crate::hardware::sensors::conversion::SensorFault,
        current_time: Instant,
    ) -> Result<(), RoasterError> {
        // Mirror the production read path: drive the F4.11 debouncer with
        // each channel's fault separately BEFORE applying the temperature
        // update, so `consecutive_{bean,env}_faults` is current when the
        // NaN-vs-hold decision is taken inside
        // `SensorController::update_temperatures`. V2-3: per-channel counters
        // so a chronically disconnected ET cannot push BT's debounce to the
        // NaN threshold.
        self.sensor.apply_fault_debounce(
            bean_fault.has_fault(),
            env_fault.has_fault(),
            &mut self.status,
        );
        match self.sensor.update_temperatures(
            bean_temp,
            env_temp,
            bean_fault,
            env_fault,
            current_time,
            &mut self.status,
        ) {
            Err(RoasterError::TemperatureOutOfRange {
                source: Some("overtemp_detected"),
            }) => self.emergency_shutdown("Over-temperature detected"),
            other => other,
        }
    }

    /// Flag whether an over-temperature regression run is active.
    pub fn mark_overtemp_regression_active(&mut self, active: bool) {
        self.safety
            .mark_overtemp_regression_active(active, &mut self.status);
    }

    /// Route an internal `RoasterCommand`: safety policy first, then manual
    /// policy, then dispatch; `StopRoast` is the explicit recovery path.
    pub fn process_command(
        &mut self,
        command: RoasterCommand,
        current_time: Instant,
    ) -> Result<(), RoasterError> {
        if matches!(command, RoasterCommand::StopRoast) {
            // Bug #3: `StopRoast` is the *single* explicit recovery path
            // (operator presses stop-and-recover). It stops streaming AND
            // un-latches a held emergency / fault, so the roaster can resume.
            // No other command path is permitted to clear the latch.
            self.stop_streaming()?;
            self.clear_emergency_explicit();
            return Ok(());
        }

        if self.safety.can_handle(command) {
            let outcome = self.safety.evaluate(command, &mut self.status);
            self.status.fault_condition = outcome.emergency_active;

            if outcome.emergency_active {
                // Bug B34: `RoasterCommand::EmergencyStop` (the safety-policy
                // branch) used to set `fault_condition = true` but leave
                // `self.state` untouched — so the artisan protocol showed
                // Heating/Stable while the safety latch was armed. Mirror the
                // state transition that `emergency_shutdown()` performs
                // (RoasterState::Error + `apply_guarded_heater` zeroing the
                // SSR) so observers see a consistent `state = Error`.
                self.state = crate::config::constants::RoasterState::Error;
                self.status.state = self.state;
                self.apply_safety_outcome(&outcome, current_time)?;
                return Err(RoasterError::TemperatureOutOfRange {
                    source: Some("emergency_shutdown"),
                });
            }
            return Ok(());
        }

        if self.dispatch.can_handle_manual(command) {
            let outcome = self
                .dispatch
                .evaluate_manual_policy(command, &mut self.status);

            if outcome.success {
                self.apply_policy_outcome(&outcome, current_time)?;
                return Ok(());
            } else {
                return Err(RoasterError::InvalidState {
                    source: Some("manual_command_failed"),
                });
            }
        }

        match self
            .dispatch
            .process_command(command, current_time, &mut self.status)
        {
            CommandDispatchResult::StopStreaming => self.stop_streaming(),
            CommandDispatchResult::Handled(result) => result,
        }
    }

    /// Apply a manual policy outcome: guarded heater/fan writes, then PID/manual state commits.
    fn apply_policy_outcome(
        &mut self,
        outcome: &ManualPolicyOutcome,
        current_time: Instant,
    ) -> Result<(), RoasterError> {
        debug!(
            "Policy outcome: heater={:?}, fan={:?}, pid={:?}, artisan={:?}",
            outcome.heater_target, outcome.fan_target, outcome.pid_enabled, outcome.artisan_control
        );

        if let Some(heater) = outcome.heater_target {
            // M10: hardware-first discipline. The software state mutations
            // (`pid_enabled = false`, `artisan_control = true`,
            // `manual_heater = heater`) commit ONLY if `apply_guarded_heater`
            // accepts the write. `ArtisanCommandHandler::evaluate` already
            // wrote `self.manual_heater` (handler-local state); it no longer
            // calls `apply_to_status(status)` for heater commands. The
            // pid/artisan_control/continuous-output commits live here,
            // post-write.
            match self
                .actuator
                .apply_guarded_heater(heater, current_time, true, &mut self.status)
            {
                Ok(_) => {}
                // Bug H2 (2026-08-10): a `ssr_cycle_busy` rejection used to
                // propagate an Err — the command had already been consumed
                // from the channel, so it was LOST (the previous comment
                // claimed "the next tick retries the same value", but nothing
                // re-issued it; 6 control ticks later the heater still ran at
                // the old power, worst case a lost REDUCTION). Adopt the
                // value as the manual setpoint instead: the next control tick
                // applies it via the non-rejecting path (`reject_on_busy =
                // false`), so the operator's command always lands within one
                // guard window.
                Err(RoasterError::InvalidState {
                    source: Some("ssr_cycle_busy"),
                }) => {
                    warn!(
                        "SSR guard busy — adopting heater {:.1}% as manual setpoint (applied next window)",
                        heater
                    );
                    self.dispatch.commit_manual_heater(heater);
                    self.dispatch.disable_pid();
                    self.status.pid_enabled = false;
                    self.status.artisan_control = true;
                    return Ok(());
                }
                Err(e) => return Err(e),
            }

            // Hardware accepted the write — commit the rest of the policy.
            // Bug C (2026-08-03): `manual_heater` is committed here too, ONLY
            // after the write was accepted. Pre-fix, `ArtisanCommandHandler::
            // evaluate` wrote it before the write, so a `ssr_cycle_busy`
            // rejection left manual state ahead of the mode flags and PID
            // kept control, silently ignoring the operator's value (worst
            // case: an `OT1 0` cut that never lands).
            self.dispatch.commit_manual_heater(heater);
            self.dispatch.disable_pid();
            self.status.pid_enabled = false;
            self.status.artisan_control = true;
            self.status.ssr_hardware_status = self.actuator.get_ssr_hardware_status();
        }

        if let Some(fan) = outcome.fan_target {
            // Bug B4: a fan command must NOT alter PID/mode flags (Spec F4.8).
            // The two lines that used to live here (`artisan_control = true`
            // and `pid_enabled = false`) made a mid-roast slider move drop
            // the heater by disabling PID and falling into manual mode with
            // no manual heater set. The physical fan write remains the only
            // side effect.

            // Bug S6 (2026-08-05): the `FAN_MIN_SAFETY_PCT` interlock floor
            // used to live only in the control tick, so an `OT2 0` (or any
            // slider move below 20 %) while the heater was on wrote 0 %
            // airflow immediately and the floor re-asserted one tick later
            // (~330 ms + up to ~1.1 s hardware fade). Enforce the same floor
            // here, on the command path, whenever the heater is energized.
            // The tick-level floor remains as a second line of defense.
            let mut fan = fan;
            if self.status.ssr_output > 0.0 && fan < FAN_MIN_SAFETY_PCT {
                if self.fan_floor_gate.rising(true) {
                    warn!(
                        "SAFETY FAN-FLOOR: heater at {:.1}% with fan command {:.1}% — raising fan to minimum {:.0}%",
                        self.status.ssr_output, fan, FAN_MIN_SAFETY_PCT
                    );
                } else {
                    debug!(
                        "SAFETY FAN-FLOOR active: heater {:.1}%, fan command raised to {:.0}%",
                        self.status.ssr_output, FAN_MIN_SAFETY_PCT
                    );
                }
                fan = FAN_MIN_SAFETY_PCT;
            } else {
                self.fan_floor_gate.rising(false);
            }

            self.actuator.set_fan_speed(fan, &mut self.status)?;
            // Bug C (2026-08-03): commit `manual_fan` only after the write
            // succeeded — a failed fan write must not leave the handler state
            // ahead of the hardware.
            self.dispatch.commit_manual_fan(fan);
            self.status.ssr_hardware_status = self.actuator.get_ssr_hardware_status();
        }

        Ok(())
    }

    /// Apply a safety outcome: zero SSR, disable PID and latch the emergency state.
    fn apply_safety_outcome(
        &mut self,
        outcome: &SafetyPolicyOutcome,
        _current_time: Instant,
    ) -> Result<(), RoasterError> {
        warn!(
            "Safety outcome: emergency={}, fault={}, reason={:?}",
            outcome.emergency_active, outcome.fault_condition, outcome.reason
        );

        if outcome.zero_ssr {
            // Bug B-E (2026-08-04): the previous single-shot log-only writes
            // (`set_heater_power(0.0)` / `set_fan_raw(100.0)` with
            // `log::error!` on failure and an unconditional
            // `status.fan_output = 100.0`) silently left the heater possibly
            // energised and let the telemetry claim full cooling. Use the
            // shared retried force methods; `status.fan_output` is only
            // published after a successful write, and a fan that cannot
            // reach 100 % is "unsafe to continue" (same rule as
            // `stop_streaming`), so it escalates to the caller.
            //
            // Note: this path is currently reachable only via
            // `process_command` with a `RoasterCommand::EmergencyStop` /
            // `ArtisanEmergencyStop` (the Artisan STOP token routes through
            // `handle_emergency_stop`), but it must not silently fail if
            // future code routes an emergency here.
            self.actuator.force_heater_off(&mut self.status);
            let fan_ok = self.actuator.force_fan_100(&mut self.status);
            if !fan_ok {
                return Err(RoasterError::HardwareError {
                    source: Some("safety_outcome_fan_failed"),
                });
            }
        }

        if outcome.disable_pid {
            self.dispatch.disable_pid();
            self.status.pid_enabled = false;
        }

        Ok(())
    }

    fn stop_streaming(&mut self) -> Result<(), RoasterError> {
        self.dispatch.stop_streaming(&mut self.status);
        // Bug V2-1 (B34 consistency): do NOT repaint the roaster state to
        // `Idle` while the emergency latch is still armed. `OFF` with the
        // latch armed now runs `clear_emergency_explicit` first (see
        // `process_artisan_command`), so by the time `stop_streaming` runs
        // through that path the latch is already cleared and we happily set
        // `Idle`. But `handle_emergency_stop` and other internal callers
        // reach `stop_streaming` (or `dispatch.stop_streaming`) WITHOUT
        // clearing the latch — overwriting `state` to `Idle` here made the
        // telemetry claim "Idle" while the SSR was force-zeroed, the fan was
        // pinned at 100 %, and every command was rejected with
        // `fault_condition_active`. Keep the armed `Error` state in that
        // case so observers see a consistent picture.
        if !self.safety.is_emergency_active() {
            self.state = crate::config::constants::RoasterState::Idle;
            self.status.state = self.state;
        }

        // Bug #3: this function used to call `self.safety.clear_emergency()`
        // and reset `fault_condition = false` here, which had the effect of
        // un-latching an armed emergency *as a side effect of stopping the
        // stream*. That meant `handle_emergency_stop()` → `stop_streaming()`
        // cleared the very emergency it had just armed, and Artisan `STOP`
        // also un-latched it — nothing prevented immediate re-energizing. We
        // now split those concerns: `stop_streaming` only stops the stream
        // and resets charge detection; the safety latch is cleared *only* by
        // the explicit recovery path `clear_emergency_explicit()` (called
        // from `RoasterCommand::StopRoast`).

        // Reset charge detection state so the next roast can detect bean drop.
        self.charge_detected = false;
        self.charge_time = None;
        self.status.charge_detected = false;
        self.bt_charge_history.clear();
        self.charge_history_tick_div = 0;
        // Bug V2-15: drop any in-flight `#DUMP` rows on stop so a dump
        // requested mid-roast does not bleed into the next roast's telemetry.
        self.dump_pending.clear();
        // Bug B3 / V2-13: the cooldown latch (set below) takes precedence over
        // the fan profile in `update_control`'s fan selector, so clearing
        // `fan_profile` here is redundant for the cooldown itself. But it also
        // silently wiped a legitimate profile on `OFF` → `START`: the temp
        // profile survived (cleared via `active_profile` only on a new
        // `PROFILE`), yet the fan profile vanished, forcing the operator to
        // re-send `FANPROFILE`. `profile_start_time = None` already disables
        // interpolation during cooldown (the latch), so we keep the profile.
        self.profile_start_time = None;
        // Bug M3 (2026-07-25): a STOP closes the heat session too — drop
        // `heat_session_start` so the next tick does not consider a manual
        // session still in progress against the time budget.
        self.heat_session_start = None;
        // Bug B3: latch cooldown so `update_control`'s fan selector keeps the
        // fan at 100% on every subsequent tick. STOP does NOT arm the safety
        // emergency latch (only `emergency_shutdown` does); without this flag
        // the next tick would call `artisan_manual_fan()` (now 0.0 after
        // `clear_manual`) and cut the airflow over the hot bean mass.
        self.cooling_active = true;

        self.actuator.capture_ssr_monitor_metrics(&mut self.status);
        // Bug NEW-2 (2026-07-26): the previous `?` on the heater-off write
        // skipped the fan-100% write when the heater failed — leaving the hot
        // bean mass without cooling exactly when things are going wrong. Cut
        // the heater AND force the fan independently; only a fan failure
        // propagates (no fan means unsafe to continue).
        if let Err(e) = self.actuator.set_heater_power(0.0) {
            log::error!(
                "stop_streaming: heater off failed: {:?} — continuing to fan 100%",
                e
            );
        }
        // Bug #13: Set fan to 100% for cooling during stop (matches README and emergency_shutdown)
        if let Err(e) = self.actuator.set_fan_raw(100.0) {
            log::error!("stop_streaming: fan 100% failed: {:?}", e);
            return Err(e);
        }
        self.status.fan_output = 100.0;

        self.status.ssr_hardware_status = self.actuator.get_ssr_hardware_status();

        Ok(())
    }

    /// Explicitly un-latch a previously armed emergency and clear the fault
    /// flag, returning the roaster to a recoverable `Idle` state.
    ///
    /// This is the **only** sanctioned recovery path. It is invoked from
    /// `RoasterCommand::StopRoast` (the operator's explicit stop-and-recover
    /// action). Artisan's plain `STOP` and `EmergencyStop`, plus any internal
    /// `stop_streaming()` call site, must NOT clear the latch — otherwise the
    /// emergency we just armed is un-latched a single tick later, which is
    /// bug #3, and nothing prevents immediate re-energizing.
    pub fn clear_emergency_explicit(&mut self) {
        self.safety.clear_emergency();
        self.status.fault_condition = false;
        self.state = crate::config::constants::RoasterState::Idle;
        self.status.state = self.state;
        // Bug B3: explicit recovery also drops the cooldown latch — the
        // operator is taking over, so airflow returns to operator control.
        self.cooling_active = false;
        self.actuator.rearm_heater_hardware_status(&mut self.status);
    }

    /// Validate a raw temperature reading (finite and within sensor range).
    pub fn is_temperature_valid(temp: f32) -> bool {
        SensorController::is_temperature_valid(temp)
    }

    /// Latch an emergency shutdown with `reason`, zeroing all actuator outputs.
    pub fn emergency_shutdown(&mut self, reason: &str) -> Result<(), RoasterError> {
        // THERM-1: Latch the emergency so internal traps (overtemp, sensor timeout,
        // RoR, max-roast-time) prevent re-energizing on the next tick.
        //
        // Audit A-TC4 (2026-08-12): emit a wire notification so the host
        // learns the latch was armed. Previously every internal trap only
        // logged `warn!` — Artisan discovered the fault through the next
        // rejected command or a STATUS poll. Guard on
        // `!is_emergency_active()` so the line is emitted once per latch
        // event even if a trap re-fires on consecutive ticks.
        if !self.safety.is_emergency_active() {
            self.send_safety_fault_notification(reason);
        }
        self.safety.activate_emergency();
        self.status.fault_condition = true;
        self.state = RoasterState::Error;
        self.actuator.emergency_shutdown(reason, &mut self.status)
    }

    /// Run one control-loop tick: staleness guard, safety backstops, output selection and actuator writes.
    pub fn update_control(&mut self, current_time: Instant) -> Result<f32, RoasterError> {
        if let Some(last_read) = self.sensor.last_temp_read() {
            if current_time.saturating_duration_since(last_read)
                > Duration::from_millis(TEMP_VALIDITY_TIMEOUT_MS as u64)
            {
                warn!("Temperature sensor timeout detected");
                self.emergency_shutdown("Temperature sensor timeout")?;
            }
        }

        // Re-detect heat source periodically (per-tick, bug H7 — the
        // ~1 s throttle was removed from ActuatorController so the debounce
        // phase-separation argument in heat_presence.rs holds).
        // This ensures a mid-roast SSR or wiring fault is detected, not just boot-time.
        self.actuator.periodic_health_check(current_time);

        // Bug M9 (2026-08-10): `force_heater_off` writes the honest signal
        // "heater did NOT turn off" into `status.ssr_hardware_status = Error`
        // when every retry failed — but this line unconditionally overwrote
        // it from the driver on the next tick, and the driver never reports
        // Error for that cause, so the marker lived less than one tick.
        // Preserve the marker while the emergency is armed; once it clears,
        // the driver value (re)applies.
        let hw = self.actuator.get_ssr_hardware_status();
        if self.status.ssr_hardware_status != SsrHardwareStatus::Error
            || !self.safety.is_emergency_active()
        {
            self.status.ssr_hardware_status = hw;
        }

        // C5: Comms idle timeout — no command from Artisan for > COMMS_IDLE_TIMEOUT_MS.
        // Bug V2-16c: gate by *physical condition* (heater energized OR an active
        // roast/preheat state), not by roast state alone. In pure Artisan-manual
        // mode (OT1/OT2 from sliders, no START) the state stays Idle, so the
        // previous state-only gate left a USB disconnect with the heater at 80 %
        // completely unprotected. We add `ssr_output > 0` so manual heater
        // commands also cover this backstop. Preheating/Heating/Stable stay
        // covered (the OR state arm already matched them).
        let heater_energized = self.status.ssr_output > 0.0;
        let roast_active = matches!(
            self.state,
            RoasterState::Preheating | RoasterState::Heating | RoasterState::Stable
        );
        if heater_energized || roast_active {
            let idle_ms = current_time
                .as_millis()
                .saturating_sub(self.status.last_command_received_at_ms);
            if idle_ms > crate::config::constants::COMMS_IDLE_TIMEOUT_MS {
                warn!(
                    "SAFETY COMMS-IDLE: no command for {}ms (>{COMMS_IDLE_TIMEOUT_MS}ms) — emergency shutdown",
                    idle_ms
                );
                self.emergency_shutdown("Comms idle timeout")?;
            }
        }

        // Bug M3 (2026-07-25): track when the heater first crosses 0 → positive
        // and clear it on 0 again, so manual-mode (no `START`) heater sessions
        // also get a time budget. The instrumentation is colocated here (next
        // to the `roast_active` gate that already inspects the same state)
        // because both decisions conceptually belong together: "what is the
        // physical session for this tick?".
        if heater_energized {
            if self.heat_session_start.is_none() {
                self.heat_session_start = Some(current_time);
            }
            self.heat_session_off_since = None;
        } else if let Some(off_since) = self.heat_session_off_since {
            // Bug R7 (2026-07-26): the session end is debounced — a momentary
            // `OT1 0` (or a one-tick heater dropout) must NOT close the heat
            // session and reset the 30-min MAX_ROAST_TIME budget. Only after
            // the heater has been OFF for HEAT_SESSION_OFF_DEBOUNCE_SECS does
            // the session really end and the timestamp drop.
            if current_time.saturating_duration_since(off_since).as_secs()
                >= HEAT_SESSION_OFF_DEBOUNCE_SECS
            {
                self.heat_session_start = None;
                self.heat_session_off_since = None;
            }
        } else if self.heat_session_start.is_some() {
            // Heater off mid-session — start the debounce window.
            self.heat_session_off_since = Some(current_time);
        }

        // Maximum roast time safety backstop. Bug V2-16c: same physical gate as
        // comms-idle — protect any roasting session with the heater energized,
        // not only the named roast states.
        // Bug M3 (2026-07-25): use `profile_start_time.or(heat_session_start)`
        // so the cap covers BOTH named-roast (START with profile) AND manual
        // (OT1/OT2 without START) heater sessions; the previous design keyed
        // exclusively on `profile_start_time`, inert to the most common
        // Artisan flow (OT1/OT2).
        // Bug P6 (2026-08-03): exclude the Preheating state from the cap. A
        // big drum can legitimately preheat for well over 30 minutes; counting
        // that time against the roast budget caused a false
        // `emergency_shutdown("Maximum roast time exceeded")` before the beans
        // were ever loaded. The START handoff anchors the clock to
        // `profile_start_time`, and comms-idle (above) still covers a
        // forgotten preheat.
        let max_roast_time_armed = (heater_energized
            && !matches!(self.state, RoasterState::Preheating))
            || matches!(self.state, RoasterState::Heating | RoasterState::Stable);
        if max_roast_time_armed {
            if let Some(start) = self.profile_start_time.or(self.heat_session_start) {
                let elapsed_secs = current_time.saturating_duration_since(start).as_secs() as u32;
                if elapsed_secs >= crate::config::constants::MAX_ROAST_TIME_SECS {
                    warn!(
                        "MAX_ROAST_TIME exceeded ({}s >= {}s) — emergency shutdown",
                        elapsed_secs,
                        crate::config::constants::MAX_ROAST_TIME_SECS
                    );
                    self.emergency_shutdown("Maximum roast time exceeded")?;
                }
            }
        }

        // Bug B3: drop the cooldown latch once the bean mass has cooled below
        // a safe-to-handle threshold. The cooldown fan is meant to protect the
        // beans after a hot STOP; once the bean temp is safely low the latch
        // releases so the operator (or a new roast) can take manual control
        // of the fan again. Use a real BT reading; if BT is NaN/non-finite
        // (faulted sensor) do NOT drop the latch — keep cooling by default.
        if self.cooling_active
            && self.status.bean_temp.is_finite()
            // Bug R8 (2026-07-26): BT=0.0 — the status default, or the
            // MAX31856 POR 0x000000 fault value — must NOT release the
            // cooldown latch: the probe may simply not have been read yet.
            // Only a real reading above 0 °C and below the threshold releases.
            && self.status.bean_temp > 0.0
            && self.status.bean_temp < COOLING_RELEASE_BEAN_TEMP_C
        {
            info!(
                "Cooldown latch released — BT {:.1}°C < {:.1}°C",
                self.status.bean_temp, COOLING_RELEASE_BEAN_TEMP_C
            );
            self.cooling_active = false;
        }

        // Charge detection: detect bean drop via sharp BT decline.
        // Bug B23: throttle the history sampling to once every
        // `CHARGE_SAMPLE_TICK_DIV` ticks so the 10-sample deque covers the
        // full 3 s `CHARGE_DETECTION_WINDOW_S` (was ~1 s when sampled every
        // tick, making #CHARGE effectively indetectable for any realistic BT
        // thermal inertia).
        // Bug M2 (2026-07-25): the previous gate stopped sampling after the
        // Heating → Stable transition (which fires immediately after PREHEAT
        // START because the target is already met). Combined with the 20 °C
        // threshold (verified by simulation to be unattainable with a real
        // probe cooling at ≈ 2–3 °C/s), `#CHARGE` was dead. Activate sampling
        // in BOTH roast-active states until charge is detected.
        if matches!(self.state, RoasterState::Heating | RoasterState::Stable)
            && !self.charge_detected
        {
            self.charge_history_tick_div = self.charge_history_tick_div.saturating_add(1);
            if self.charge_history_tick_div >= CHARGE_SAMPLE_TICK_DIV {
                self.charge_history_tick_div = 0;
                let bt = self.status.bean_temp;
                if bt > 50.0 {
                    if self.bt_charge_history.len() >= 10 {
                        let _ = self.bt_charge_history.pop_front();
                    }
                    let _ = self.bt_charge_history.push_back(bt);
                    if self.bt_charge_history.len() >= 5 {
                        let (front, _back) = self.bt_charge_history.as_slices();
                        let first = front.first().copied().unwrap_or(bt);
                        let drop = first - bt;
                        if drop > CHARGE_DROP_THRESHOLD_C {
                            self.charge_detected = true;
                            self.charge_time = Some(current_time);
                            self.status.charge_detected = true;
                            info!("#CHARGE detected — BT dropped {:.1}°C", drop);
                            let output_channel =
                                crate::application::service_container::ServiceContainer::get_output_channel();
                            let mut charge_msg = heapless::String::<
                                { crate::logging::traceability::TRACE_EVENT_MAX_LEN },
                            >::new();
                            let _ = core::fmt::Write::write_fmt(
                                &mut charge_msg,
                                core::format_args!("#CHARGE dt={:.1}", drop),
                            );
                            crate::hardware::error_counters::try_send_output(
                                output_channel,
                                charge_msg,
                            );
                        }
                    }
                }
            }
        }

        let current_pv = if self.status.pid_channel == 1 {
            self.status.env_temp
        } else {
            self.status.bean_temp
        };
        self.status.pv = current_pv;

        // Reject NaN / infinite PV (faulted sensor) — force heater off.
        // Must cover ALL modes (manual, PID, profile), not just PID.
        // THERM-2: Without this, a broken thermocouple in manual mode leaves
        // the heater stuck at the last commanded power with zero supervision.
        if !current_pv.is_finite() {
            warn!("Sensor input NaN/infinite (fault) — emergency shutdown");
            self.emergency_shutdown("Sensor fault (NaN/infinite temperature)")?;
        }
        self.sensor
            .refresh_filtered_derivative(current_pv, current_time, &mut self.status);

        // Bug V2-16a (2026-07-25): the RoR guard historically ran every tick in
        // *all* modes and states. With an empty drum and a low-mass BT probe,
        // the probe heats faster than 0.5 °C/s during PREHEAT, so the guard
        // fired `rate_of_rise_exceeded → emergency_shutdown` in the first
        // 1-2 seconds of heating — and via V2-1 that became a power-cycle
        // brick on the first real PREHEAT. The guard must only protect once
        // beans are present. Gate it to the post-charge roasting states
        // (`Heating` / `Stable`); Idle/Preheating must not trigger it
        // (Preheating is by definition heating an empty drum).
        //
        // Bug M4 (2026-07-25): additionally, FEED the guard from the BT-only
        // `refresh_bt_guard_derivative`, not from `status.derivative_rate`
        // (which is the active PV — either BT or ET). With `PID;CHAN;1`
        // (ET-as-PV, supported by `pid_channel_1_uses_env_temp_as_pv`), the
        // 0.5 °C/s threshold calibrated for the sluggish BT was being applied
        // to ET (which climbs much faster) → spurious emergency on a healthy
        // roast while a genuine BT runaway remained unguarded.
        //
        // Bug P4 (2026-08-03): extend the gate — `PID;SV`/`SETTARGET` from
        // `Idle` enables the PID (state stays `Idle`) and the heater heats
        // toward the setpoint with NO RoR supervision (the previous gate only
        // covered Heating/Stable). Arm the guard in `Idle` whenever the PID
        // is enabled AND the heater is actually energized; Preheating stays
        // exempt (empty drum, V2-16a) and pure-manual `OT1` sessions
        // (`pid_enabled = false`) are not covered (their runaway backstop is
        // the comms-idle / MAX_ROAST_TIME gate).
        let ror_guard_active = matches!(self.state, RoasterState::Heating | RoasterState::Stable)
            || (matches!(self.state, RoasterState::Idle)
                && self.status.pid_enabled
                && heater_energized);
        if ror_guard_active {
            if let Some(bt_rate) = self
                .sensor
                .refresh_bt_guard_derivative(self.status.bean_temp, current_time)
            {
                if let Err(e) = self.sensor.check_bt_rate(bt_rate) {
                    warn!("BT rate-of-rise guard failed: {:?}", e);
                    self.emergency_shutdown("Bean temperature rate-of-rise exceeded")?;
                }
            }
            // Backwards-compat: also feed the legacy `status.derivative_rate`
            // path so telemetry still sees the PV-derivative (relevant when
            // ET is the chosen PV). Bug R1 (2026-07-26): this call is fully
            // INDEPENDENT of the BT guard — `check_rate_of_rise` uses its own
            // `pv_ror_exceeded_count`, while `check_bt_rate` above uses
            // `bt_ror_exceeded_count`. (Previously a single shared counter
            // meant every healthy ET tick reset the BT runaway debounce,
            // defeating the guard in PID;CHAN;1 mode.)
            //
            // Bug P1 (2026-08-03): this legacy check consumes
            // `status.derivative_rate`, which is refreshed from the ACTIVE PV
            // (`env_temp` under `PID;CHAN;1`). The 0.5 °C/s threshold is
            // calibrated for the sluggish BT; with ET-as-PV a healthy heat-up
            // (ET climbing >0.5 °C/s) aborted every roast ~1 s after entering
            // the guard. Gate the legacy check to the BT channel only — the
            // genuine runaway protection for CHAN;1 is `check_bt_rate` above
            // (BT-only, per M4). Telemetry still sees the PV derivative (the
            // `refresh_filtered_derivative` call above is untouched).
            if self.status.pid_channel != 1 {
                if let Err(e) = self.sensor.check_rate_of_rise(&self.status) {
                    warn!("Rate-of-rise check failed: {:?}", e);
                    self.emergency_shutdown("Bean temperature rate-of-rise exceeded")?;
                }
            }
        }

        let desired_output = if self.safety.is_emergency_active() {
            debug!("Emergency active - forcing SSR output to 0%");
            0.0
        } else if self.status.artisan_control {
            if self.status.ssr_hardware_status
                != crate::config::constants::SsrHardwareStatus::Available
            {
                warn!("Artisan+ manual control: SSR not available - output: 0%");
                0.0
            } else {
                let manual_output = self.dispatch.artisan_manual_heater();
                debug!(
                    "Artisan+ control - manual heater output: {:.1}%",
                    manual_output
                );
                manual_output
            }
        } else if self.status.pid_enabled {
            if self.status.ssr_hardware_status
                == crate::config::constants::SsrHardwareStatus::Available
            {
                // Sensor reads take ~160ms (TEMPERATURE_READ_INTERVAL_MS), PID runs at 100ms
                // (PID_SAMPLE_TIME_MS). Skip PID if data is stale to avoid computing on old readings.
                let is_stale = if let Some(last_read) = self.sensor.last_temp_read() {
                    current_time.saturating_duration_since(last_read) > Duration::from_millis(500)
                // > TEMPERATURE_READ_INTERVAL_MS * 2 + margin
                } else {
                    // Bug NEW-5 (2026-07-26): "never read" was treated as
                    // "fresh" — if the first sensor read failed and the
                    // operator sent START, the PID computed against PV=0.0 and
                    // drove the heater to 100% with no temperature feedback
                    // (up to the 15 s comms-idle backstop). Treat a never-read
                    // sensor as stale so the PID holds instead of ramping.
                    warn!("PID enabled but no sensor read yet — treating as stale");
                    true
                };

                if is_stale {
                    debug!(
                        "Sensor data is stale (>{}ms), holding last APPLIED output",
                        PID_SAMPLE_TIME_MS
                    );
                    // Bug R2 (2026-07-26): the stale hold used to return
                    // `status.mv` — the PID's *intent* (can be 100%) — so the
                    // actuator's slew limiter kept ramping toward it during
                    // the staleness window instead of holding the last value
                    // the SSR is physically applying. Return `ssr_output`.
                    self.status.ssr_output // Hold last applied output
                } else {
                    self.update_pid_control(current_time)
                }
            } else {
                warn!("PID enabled but SSR not available - output: 0%");
                0.0
            }
        } else {
            0.0
        };

        self.actuator.set_last_desired_output(desired_output);
        let pid_integrator_value = self.dispatch.pid_integrator_value();
        // CTRL-1: Check guard state BEFORE apply_guarded_heater so guard_busy
        // reflects whether this write will be accepted or rejected, not the
        // post-mark_cycle() result (which is always busy by construction).
        let guard_busy = self
            .actuator
            .ssr_guard_next_cycle_allowed(current_time)
            .is_err();
        // Bug B (2026-08-03): a heater-write failure here previously short-
        // circuited into the tasks-level warn-only path. But on a failed write
        // the SSR keeps whatever duty it last latched (possibly 100 %) with no
        // command to change it — an unknown-heater-state condition. Treat it
        // like the fan failure below and escalate to a full emergency shutdown.
        let applied_output = match self.actuator.apply_guarded_heater(
            desired_output,
            current_time,
            false,
            &mut self.status,
        ) {
            Ok(output) => output,
            Err(e) => {
                warn!(
                    "SAFETY HEATER-FAIL: heater write failed in control loop: {:?}",
                    e
                );
                self.emergency_shutdown("Heater control failure")?;
                0.0 // unreachable; emergency_shutdown always returns Err
            }
        };
        let feedback = PidFeedback::new(desired_output, applied_output, guard_busy);
        self.dispatch.set_pid_feedback(feedback);

        self.status.integrator_value = pid_integrator_value;
        // Bug fix: STATUS field "MV" (INSTRUMENTATION.md #11) is documented as
        // the PID's manipulated variable *before* the actuator clamps/slew-limits
        // it. `applied_output` is the slew-rate-limited, guard-busy-aware value
        // the SSR physically received (already exposed via `status.ssr_output`).
        // Storing `applied_output` here made MV a duplicate of the applied
        // heater %, hiding the raw PID intent from tuning/audit telemetry.
        // Use `desired_output` (the value the control logic selected before the
        // actuator's slew/guard stage) so MV reflects the controller's intent.
        self.status.mv = desired_output;
        self.status.saturation_active = self.dispatch.pid_saturation_active();
        self.status.integrator_clamped = self.dispatch.pid_integrator_clamped();

        // Emergency cooldown priority: while the safety latch is active, OR
        // the B3 cooling latch is active (plain STOP without an emergency),
        // force the fan to 100% to cool the hot bean mass. This must NOT be
        // overridden by the fan profile, the manual Artisan setting, or any
        // other source — the previous re-wrote the fan speed every ~100ms,
        // annihilating the cooldown a single tick after the STOP/emergency set
        // it. The emergency latch clears via `clear_emergency_explicit`
        // (StopRoast); the cooling latch clears on new roast start, explicit
        // recovery, or when BT drops below COOLING_RELEASE_BEAN_TEMP_C.
        let fan_output = if self.safety.is_emergency_active() || self.cooling_active {
            100.0
        } else if let (Some(ref fp), Some(start)) = (&self.fan_profile, self.profile_start_time) {
            let elapsed = current_time.saturating_duration_since(start).as_secs() as u32;
            fp.target_at(elapsed).map(|s| s as f32).unwrap_or(20.0)
        } else {
            self.dispatch.artisan_manual_fan()
        };
        // Bug A (2026-08-03): heater↔fan interlock. The selector above falls
        // through to `artisan_manual_fan()` (0.0 by default) whenever the
        // operator never sent `OT2` / `FANPROFILE` — so a PID roast or PREHEAT
        // ran the heater at up to 100 % with ZERO airflow every tick. The
        // firmware's own standard says no-fan is unsafe (see `stop_streaming`);
        // enforce the same on the energizing path: whenever the heater is being
        // driven above 0 %, the fan never drops below `FAN_MIN_SAFETY_PCT`.
        // Explicit operator values at or above the floor pass through untouched.
        let mut fan_output = fan_output;
        if desired_output > 0.0 && fan_output < FAN_MIN_SAFETY_PCT {
            if self.fan_floor_gate.rising(true) {
                warn!(
                    "SAFETY FAN-FLOOR: heater at {:.1}% with fan at {:.1}% — raising fan to minimum {:.0}%",
                    desired_output, fan_output, FAN_MIN_SAFETY_PCT
                );
            } else {
                debug!(
                    "SAFETY FAN-FLOOR active: heater {:.1}%, fan raised to {:.0}%",
                    desired_output, FAN_MIN_SAFETY_PCT
                );
            }
            fan_output = FAN_MIN_SAFETY_PCT;
        } else {
            self.fan_floor_gate.rising(false);
        }
        // Bug B (2026-08-03): a fan-write failure in the control loop used to
        // propagate via `?` up to `update_control_stage` (tasks.rs), which only
        // logged a warning — while the heater duty written moments earlier
        // stayed on the SSR and the watchdog kept certifying the loop healthy.
        // A failed fan write with heat on is exactly the 'no fan = unsafe to
        // continue' condition the STOP path treats as fatal; escalate to a
        // full `emergency_shutdown` (latch + heater off + fan 100 % retries)
        // instead of returning a logged error.
        if let Err(e) = self.actuator.set_fan_speed(fan_output, &mut self.status) {
            warn!("SAFETY FAN-FAIL: fan write failed in control loop: {:?}", e);
            self.emergency_shutdown("Fan control failure")?;
        }

        // Bug P5 (2026-08-03): probe-stuck detector. A hard thermocouple
        // short reads a flat ~0 °C — a VALID temperature (no MAX31856 fault
        // bit), so the fault/NaN guards below never fire and the PID drives
        // the heater blind until MAX_ROAST_TIME. If the heater has been at or
        // above `PROBE_STUCK_HEATER_MIN_PCT` and BT has not moved by more than
        // `PROBE_STUCK_VARIATION_C` for `PROBE_STUCK_TIMEOUT_SECS`, the probe
        // is shorted or broken → emergency. Any real probe moves well over
        // 1 °C within 2 minutes at ≥50 % power; a non-finite BT (faulted
        // channel) keeps the detector disarmed — the NaN emergency covers it.
        // The detector disarms while the PID regulates within
        // `PROBE_STUCK_TARGET_MARGIN_C` of the setpoint: a stable roast holds
        // BT nearly flat BY DESIGN, and at a cold ambient / big drum the
        // equilibrium duty can exceed 50 % — a healthy steady state must not
        // trip. Manual mode (no PID target) stays fully armed.
        //
        // Bug S1 (2026-08-05): the arm gate used to require
        // `ssr_output >= PROBE_STUCK_HEATER_MIN_PCT` (50 %). A dead probe in
        // manual mode at lower duty (e.g. `OT1 30`) therefore ran the heater
        // blind with NO supervision until MAX_ROAST_TIME (30 min): overtemp,
        // RoR, staleness and NaN all ignore a valid-but-frozen 0 °C reading,
        // and Artisan's READ polling keeps comms-idle from ever firing. Arm
        // the detector at ANY positive duty. Trade-off (accepted, fail-safe):
        // a manual session holding BT flat < 1 °C for 2 min at low power can
        // now trip a recoverable emergency (STOP → OFF) — physically unlikely
        // while heat is being applied, and strictly safer than an unguarded
        // heater.
        //
        // Audit A-TC4-C (2026-08-12): manual / Artisan software-PID mode is
        // now TWO-STAGE. At `PROBE_STUCK_TIMEOUT_SECS` (120 s) the firmware
        // emits `ERR probe_stuck_warning` on the wire WITHOUT latching — a
        // legitimately slow finish (RoR below ~0.5 °C/min) can hold BT flat
        // for 2 min at low duty, and the operator/Artisan deserves a warning
        // before the roast is torn down. Only after
        // `PROBE_STUCK_MANUAL_LATCH_SECS` (300 s) of continuous flat BT does
        // the detector escalate to the emergency latch (which emits
        // `ERR safety_fault Probe stuck` via `emergency_shutdown`). The
        // dead-probe backstop (Bug S1) stays closed: worst-case manual
        // exposure is now 5 min instead of 2, still far under
        // `MAX_ROAST_TIME_SECS`. Firmware-PID mode keeps the original
        // single-stage 120 s latch: the `regulating` disarm below already
        // protects healthy PID holds, so a flat PV far from the setpoint
        // remains a genuine control hazard there.
        let probe_bt = self.status.bean_temp;
        // Bug H1 (2026-08-10): the arm gate used to compare BT against the
        // setpoint directly. With `PID;CHAN;1` the PID regulates ET
        // (`status.pv = env_temp`) and BT legitimately lives tens of degrees
        // below the target, flat — `near_target` was always false and a
        // HEALTHY BT (varying < PROBE_STUCK_VARIATION_C while ET converged)
        // tripped a spurious latched "Probe stuck" emergency at 120 s. The
        // rest of the loop was already migrated to `status.pv` for this exact
        // reason (Bug L11 / Bug F arms below). Gate the detector on the
        // REGULATED variable: the stuck-probe signature is a flat PV far
        // from the target the loop is chasing. When PID controls ET
        // (pid_channel == 1), the detector disarms entirely — a BT flat
        // while ET is regulated is a telemetry concern, not a control
        // hazard, and BT may legitimately sit far below the setpoint.
        let regulating = self.status.pid_enabled
            && ((self.status.target_temp - self.status.pv).abs() <= PROBE_STUCK_TARGET_MARGIN_C
                || self.status.pid_channel == 1);
        if self.status.ssr_output > 0.0 && probe_bt.is_finite() && !regulating {
            match self.probe_stuck_last_bt {
                None => {
                    self.probe_stuck_last_bt = Some(probe_bt);
                    self.probe_stuck_last_change = Some(current_time);
                    self.probe_stuck_warning_sent = false;
                }
                Some(prev) => {
                    if (probe_bt - prev).abs() > PROBE_STUCK_VARIATION_C {
                        self.probe_stuck_last_bt = Some(probe_bt);
                        self.probe_stuck_last_change = Some(current_time);
                        self.probe_stuck_warning_sent = false;
                    } else if let Some(last_change) = self.probe_stuck_last_change {
                        let flat_secs = current_time
                            .saturating_duration_since(last_change)
                            .as_secs();
                        if flat_secs >= PROBE_STUCK_TIMEOUT_SECS {
                            if self.status.pid_enabled {
                                // Firmware-PID mode: single-stage latch (the
                                // original Bug P5 contract — a flat PV far
                                // from the setpoint is a control hazard).
                                warn!(
                                    "SAFETY PROBE-STUCK: BT flat ({:.1}°C) for ≥{}s with heater on — emergency",
                                    probe_bt, PROBE_STUCK_TIMEOUT_SECS
                                );
                                self.emergency_shutdown("Probe stuck")?;
                            } else {
                                // Manual / Artisan software-PID mode:
                                // two-stage (Audit A-TC4-C). Stage 1 at
                                // PROBE_STUCK_TIMEOUT_SECS: one wire warning
                                // per stuck episode, no latch. Stage 2 at
                                // PROBE_STUCK_MANUAL_LATCH_SECS: real latch.
                                if !self.probe_stuck_warning_sent {
                                    self.probe_stuck_warning_sent = true;
                                    self.send_text_response("ERR probe_stuck_warning");
                                    warn!(
                                        "SAFETY PROBE-STUCK: BT flat ({:.1}°C) for ≥{}s with heater on — manual-mode warning",
                                        probe_bt, PROBE_STUCK_TIMEOUT_SECS
                                    );
                                }
                                if flat_secs >= PROBE_STUCK_MANUAL_LATCH_SECS {
                                    warn!(
                                        "SAFETY PROBE-STUCK: BT flat ({:.1}°C) for ≥{}s in manual mode — emergency",
                                        probe_bt, PROBE_STUCK_MANUAL_LATCH_SECS
                                    );
                                    self.emergency_shutdown("Probe stuck")?;
                                }
                            }
                        }
                    }
                }
            }
        } else {
            // Heater below the threshold (or BT faulted) — disarm.
            self.probe_stuck_last_bt = None;
            self.probe_stuck_last_change = None;
            self.probe_stuck_warning_sent = false;
        }

        self.status.state = self.state;

        if applied_output > 0.0
            && self.status.ssr_hardware_status
                != crate::config::constants::SsrHardwareStatus::Available
        {
            debug!(
                "SSR output {:.1}% applied but no heat source detected",
                applied_output
            );
        }

        #[cfg(feature = "simulated-sensors")]
        {
            // Closed-loop plant: next sensor sample reflects this tick's heater/fan.
            self.sensor
                .set_simulated_actuators(self.status.ssr_output, self.status.fan_output);
        }

        Ok(applied_output)
    }

    // Audit M-A5 (2026-08-11): `process_output` removed — it only awaited the
    // deleted no-op `OutputController::process_status`, so it had zero effect
    // and zero callers. Continuous output is emitted by `emit_telemetry_stage`
    // in tasks.rs, not through this type.
    /// Shared access to the continuous-output `OutputController`.
    pub fn get_output_manager(&self) -> &crate::control::OutputController {
        self.dispatch.get_output_manager()
    }

    /// Mutable shared access to the continuous-output `OutputController`.
    pub fn get_output_manager_mut(&mut self) -> &mut crate::control::OutputController {
        self.dispatch.get_output_manager_mut()
    }

    #[cfg(feature = "simulated-sensors")]
    /// Inject a bean charge dip in plant mode (test/HIL helper).
    pub fn inject_plant_charge(&mut self, bean_drop_c: f32) {
        self.sensor.inject_simulated_charge(bean_drop_c);
    }

    /// Process one parsed Artisan command; rejects re-energizing commands while latched.
    pub fn process_artisan_command(
        &mut self,
        command: crate::config::ArtisanCommand,
    ) -> Result<(), RoasterError> {
        // C5: Record wall-clock (millis-since-boot) of last command for idle timeout.
        self.status.last_command_received_at_ms = embassy_time::Instant::now().as_millis();

        // Bug #6 fix: Reject all commands when a fault condition is active.
        // Prevents heater ramp commands from worsening an over-temp situation
        // that was detected between sensor reads.
        // Exception: READ, STATUS, STOP, START, PREHEAT and the handshake
        // commands CHAN/UNITS/FILT are always allowed.
        // READ/STATUS are monitoring; STOP/EmergencyStop are safety; and
        // START/PREHEAT (Bug P3, 2026-08-03) are the operator's deliberate
        // re-energize actions — the handlers clear the held latch, so the
        // manual flow after a STOP is no longer bricked until `OFF`.
        // Audit A-TC4 (2026-08-12): CHAN/UNITS/FILT were previously rejected
        // here too, but they are pure handshake/display-state commands with
        // no actuator or latch side effects (handle_chan records the poll
        // rate + acks; handle_filt records the requested filter + acks;
        // handle_units only switches the display scale). Rejecting them
        // breaks Artisan reconnects: the ArduinoTC4 driver hard-fails its
        // initialisation ("Arduino could not set channels/units/filters")
        // on any non-'#' line and re-initialises forever, so a latched
        // device could never be re-attached until someone typed PID;OFF by
        // hand. Allowing them keeps the handshake working while the latch
        // still rejects every re-energizing command below.
        if self.status.fault_condition {
            match command {
                crate::config::ArtisanCommand::ReadStatus
                | crate::config::ArtisanCommand::StatusReport
                | crate::config::ArtisanCommand::Stop
                | crate::config::ArtisanCommand::EmergencyStop
                | crate::config::ArtisanCommand::StartRoast
                | crate::config::ArtisanCommand::Preheat(_)
                | crate::config::ArtisanCommand::Chan(_)
                | crate::config::ArtisanCommand::Units(_)
                | crate::config::ArtisanCommand::Filt(_)
                | crate::config::ArtisanCommand::SetStreaming(_) => { /* allow */ }
                _ => {
                    warn!("Command rejected: fault condition active");
                    return Err(RoasterError::InvalidState {
                        source: Some("fault_condition_active"),
                    });
                }
            }
        }

        let current_time = embassy_time::Instant::now();

        match command {
            crate::config::ArtisanCommand::StartRoast => self.handle_start_roast(),
            crate::config::ArtisanCommand::SetHeater(value) => {
                self.handle_set_heater(value, current_time)
            }
            crate::config::ArtisanCommand::SetFan(value) => {
                self.handle_set_fan(value, current_time)
            }
            crate::config::ArtisanCommand::SetFanSpeed(value, was_clamped) => {
                self.handle_set_fan_speed(value, was_clamped, current_time)
            }
            crate::config::ArtisanCommand::Stop => {
                // Reported bug C3 / V2-1: `STOP` (token "STOP" via
                // `EmergencyStop`) arms the emergency latch
                // (`activate_emergency` + `fault_condition`), but the only
                // sanctioned un-latch path
                // (`RoasterCommand::StopRoast → clear_emergency_explicit`) has
                // no producer in production code, so a single sensor-fault
                // latched the roaster until a power cycle. Decision
                // (2026-07-25): make plain `OFF` (`ArtisanCommand::Stop`,
                // token "OFF") the *unconditional* recovery: if any fault or
                // emergency latch is active, clear it BEFORE running the
                // normal stop, so the host always has a reachable door back to
                // `Idle`. The whitelist at the top of this method already
                // permits `Stop` while `fault_condition` is active.
                if self.status.fault_condition || self.safety.is_emergency_active() {
                    self.clear_emergency_explicit();
                }
                self.handle_stop()
            }
            crate::config::ArtisanCommand::EmergencyStop => self.handle_emergency_stop(),
            crate::config::ArtisanCommand::IncreaseHeater => {
                self.handle_increase_heater(current_time)
            }
            crate::config::ArtisanCommand::DecreaseHeater => {
                self.handle_decrease_heater(current_time)
            }
            crate::config::ArtisanCommand::StatusReport => self.handle_status_report(),
            crate::config::ArtisanCommand::ReadStatus => self.handle_read_status(),
            crate::config::ArtisanCommand::Chan(rate) => self.handle_chan(rate),
            crate::config::ArtisanCommand::RunRegression => self.handle_run_regression(),
            crate::config::ArtisanCommand::Units(is_fahrenheit) => {
                self.handle_units(is_fahrenheit, current_time)
            }
            crate::config::ArtisanCommand::Filt(val) => self.handle_filt(val),
            crate::config::ArtisanCommand::SetPidGain(kp, ki, kd) => {
                self.handle_set_pid_gain(kp, ki, kd)
            }
            crate::config::ArtisanCommand::SetTargetTemp(target) => {
                self.handle_set_target_temp(target)
            }
            crate::config::ArtisanCommand::SetProfile => self.handle_set_profile(),
            crate::config::ArtisanCommand::DumpLog => self.handle_dump_log(),
            crate::config::ArtisanCommand::Preheat(target) => self.handle_preheat(target),
            crate::config::ArtisanCommand::SetFanProfile => self.handle_set_fan_profile(),
            crate::config::ArtisanCommand::SetPidChannel(ch) => self.handle_set_pid_channel(ch),
            crate::config::ArtisanCommand::SetPidCycleTime(ms) => {
                self.handle_set_pid_cycle_time(ms)
            }
            crate::config::ArtisanCommand::SetPidOutputLimits(min, max) => {
                self.handle_set_pid_output_limits(min, max)
            }
            crate::config::ArtisanCommand::SetStreaming(enabled) => {
                self.handle_set_streaming(enabled)
            }
        }
    }

    // Artisan command handlers (extracted from process_artisan_command)

    fn handle_start_roast(&mut self) -> Result<(), RoasterError> {
        use crate::config::constants::DEFAULT_TARGET_TEMP;
        // Bug V2-4 (B14 residual): the original `is_streaming() && state !=
        // Preheating` gate swallowed START whenever `PID;SV` or `OT1` had been
        // issued in Idle — both make `is_streaming()` true via
        // `status.pid_enabled` / `status.artisan_control` WITHOUT leaving the
        // Idle state. Then `profile_start_time` stayed unset, so the temporal
        // backstops (comms-idle, MAX_ROAST_TIME_SECS — V2-16c) and charge
        // detection were all inert for that roast. Gate by *state* instead: a
        // START during an actually-active roast (Heating/Stable) is "ignored";
        // every other state (Idle-with-PID, Idle-manual, Preheating, Error
        // recovery via V2-1) takes the full handoff.
        if matches!(self.state, RoasterState::Heating | RoasterState::Stable) {
            info!(
                "Artisan+ START ignored - roast already active (state={:?})",
                self.state
            );
            self.status.ssr_hardware_status = self.actuator.get_ssr_hardware_status();
        } else {
            // PREHEAT → START handoff: this is the bug-B14 path. If we were
            // preheating, the PID was already enabled by `handle_preheat`;
            // here we transition to `Heating`, fix `profile_start_time`
            // (so MAX_ROAST_TIME_SECS / charge detection come alive), load
            // the profile's t=0 target (or zero-profiled), and turn on
            // continuous output.
            //
            // Bug B3: a new roast start drops the cooldown latch — we are
            // re-energizing deliberately, so airflow follows the new roast.
            //
            // Bug P3 (2026-08-03): START is the operator's deliberate act of
            // re-energizing — clear any latched emergency/fault BEFORE the
            // handoff so a STOP (which arms the latch via `EmergencyStop`) no
            // longer bricks the next roast until the undocumented `OFF` token.
            // Same rationale as `clear_emergency_explicit`'s doc: START is the
            // sanctioned recovery in the documented manual flow.
            if self.status.fault_condition || self.safety.is_emergency_active() {
                self.clear_emergency_explicit();
            }
            // Bug P11 (2026-08-03): reset the charge-detection state on START.
            // `stop_streaming` already resets it on every STOP/OFF, but a
            // batch that ends WITHOUT a STOP (PREHEAT → START cadence) kept
            // `charge_detected = true`, so the `!charge_detected` gate never
            // re-fired `#CHARGE` on the next batch. Clearing here makes START
            // idempotent for every path into a new roast.
            // Bug M8 (2026-08-10): the history deque and its sampling divider
            // were NOT cleared (stop_streaming resets all five fields; this
            // path only three). In the PREHEAT → START cadence the deque
            // still held the previous batch's pre-charge BT (~205 °C), so the
            // first samples of batch 2 compared fresh BT against the old
            // batch's values and fired a FALSE `#CHARGE` (no grain dropped),
            // also disabling the real detection for the rest of the batch.
            self.charge_detected = false;
            self.charge_time = None;
            self.status.charge_detected = false;
            self.bt_charge_history.clear();
            self.charge_history_tick_div = 0;
            // Bug P6 (2026-08-03): reset the manual heat-session clock on
            // START. The 30-minute MAX_ROAST_TIME budget then anchors to
            // `profile_start_time` (set below) — preheat time (which can
            // legitimately exceed half an hour on big drums, and which the
            // time-cap gate now excludes) must not carry into the new roast.
            self.heat_session_start = None;
            self.cooling_active = false;
            // Bug V2-7: drop any pending `#DUMP` rows from a previous roast so
            // they do not interleave with the new roast's live telemetry.
            self.dump_pending.clear();
            self.status.artisan_control = true;
            // Use loaded profile if available, otherwise fall back to default target
            if self.active_profile.is_some() {
                self.profile_start_time = Some(embassy_time::Instant::now());
                // Set initial target from profile
                let elapsed = 0u32;
                if let Some(target) = self
                    .active_profile
                    .as_ref()
                    .and_then(|p| p.target_at(elapsed))
                {
                    self.status.target_temp = target;
                    self.enable_pid_control(target)?;
                }
                info!(
                    "Artisan+ roast started with profile ({} setpoints)",
                    self.active_profile
                        .as_ref()
                        .map_or(0, |p| p.setpoints.len())
                );
            } else {
                self.profile_start_time = Some(embassy_time::Instant::now());
                self.enable_pid_control(DEFAULT_TARGET_TEMP)?;
                info!(
                    "Artisan+ roast started with default target {:.1}°C",
                    DEFAULT_TARGET_TEMP
                );
            }
            crate::logging::roast_logger::start_roast(embassy_time::Instant::now());
            self.status.ssr_hardware_status = self.actuator.get_ssr_hardware_status();
            self.state = crate::config::constants::RoasterState::Heating;
            self.status.state = self.state;
            self.status.ssr_hardware_status = self.actuator.get_ssr_hardware_status();
        }
        Ok(())
    }

    fn handle_set_heater(&mut self, value: u8, current_time: Instant) -> Result<(), RoasterError> {
        self.forward_artisan_manual_command(
            crate::config::RoasterCommand::SetHeaterManual(value),
            current_time,
        )?;
        info!("Artisan+ heater command processed: {}%", value);
        Ok(())
    }

    fn handle_set_fan(&mut self, value: u8, current_time: Instant) -> Result<(), RoasterError> {
        self.forward_artisan_manual_command(
            crate::config::RoasterCommand::SetFanManual(value),
            current_time,
        )?;

        info!("Artisan+ fan command processed: {}%", value);
        Ok(())
    }

    fn handle_set_fan_speed(
        &mut self,
        value: u8,
        was_clamped: bool,
        current_time: Instant,
    ) -> Result<(), RoasterError> {
        // Spec F4.8: OT2 is a fan-override command. It must NOT change the heater
        // state or affect PID status. Just forward the fan command and notify.
        self.forward_artisan_manual_command(
            crate::config::RoasterCommand::SetFanManual(value),
            current_time,
        )?;

        if was_clamped {
            // Notify Artisan that fan was clamped but heater/PID remain unaffected.
            self.send_ot2_clamped_notification(value);
            info!(
                "Artisan+ OT2 fan clamped to {}% (heater/PID unchanged)",
                value
            );
        } else {
            info!("Artisan+ OT2 fan command processed: {}%", value);
        }
        Ok(())
    }

    fn handle_stop(&mut self) -> Result<(), RoasterError> {
        info!("Artisan+ STOP - stopping roast");
        self.stop_streaming()?;
        crate::logging::roast_logger::stop_roast();
        self.actuator.rearm_heater_hardware_status(&mut self.status);
        Ok(())
    }

    fn handle_emergency_stop(&mut self) -> Result<(), RoasterError> {
        // Bug #3: latch the emergency and DO NOT un-latch it as a side
        // effect. The previous implementation called `stop_streaming()`,
        // which cleared the latch via `clear_emergency()`, leaving the
        // emergency inert; Artisan (or any client) could immediately
        // re-energize the heater. Recovery is reserved for the explicit
        // `RoasterCommand::StopRoast` path → `clear_emergency_explicit()`.
        self.safety.activate_emergency();
        self.status.fault_condition = true;
        self.state = RoasterState::Error;
        self.status.state = RoasterState::Error;

        // Cut the heater directly and force the fan to 100% to cool the
        // hot bean mass. Fan persistence during the cooldown is enforced
        // in `update_control` (see fix #2), so this 100% is sticky.
        //
        // Bug B-H (2026-08-04): the previous single-attempt log-only
        // writes (`set_heater_power(0.0)` / `set_fan_raw(100.0)` with an
        // unconditional `status.fan_output = 100.0`) silently left the
        // heater possibly energised and let the telemetry claim full
        // cooling. Use the shared retried force methods; the fan failure
        // escalates to an `Err` so the control loop emits an ERR to
        // Artisan ("no fan means unsafe to continue", same rule as
        // `stop_streaming`).
        self.actuator.force_heater_off(&mut self.status);
        let fan_ok = self.actuator.force_fan_100(&mut self.status);

        // Stop streaming the protocol output without touching the latch.
        self.dispatch.stop_streaming(&mut self.status);
        crate::logging::roast_logger::stop_roast();

        if !fan_ok {
            log::error!(
                "Emergency stop: fan FAILED to reach 100% after {} retries — unsafe to continue",
                crate::config::constants::EMERGENCY_FAN_RETRIES
            );
            return Err(RoasterError::HardwareError {
                source: Some("emergency_stop_fan_failed"),
            });
        }

        info!("Artisan+ emergency stop - latched, heater off, fan 100% (recovery via StopRoast)");
        Ok(())
    }

    fn handle_increase_heater(&mut self, current_time: Instant) -> Result<(), RoasterError> {
        self.forward_artisan_manual_command(
            crate::config::RoasterCommand::IncreaseHeater,
            current_time,
        )?;
        info!("Artisan+ UP command processed");
        Ok(())
    }

    fn handle_decrease_heater(&mut self, current_time: Instant) -> Result<(), RoasterError> {
        self.forward_artisan_manual_command(
            crate::config::RoasterCommand::DecreaseHeater,
            current_time,
        )?;
        info!("Artisan+ DOWN command processed");
        Ok(())
    }

    fn handle_status_report(&mut self) -> Result<(), RoasterError> {
        self.status.ssr_hardware_status = self.actuator.get_ssr_hardware_status();

        // STATUS response is emitted by control_loop_task after command returns Ok(()).
        // Sending it here would produce a duplicate (Bug #3 in docs/BUGS.md).

        debug!(
            "STATUS command - SSR status: {:?}",
            self.status.ssr_hardware_status
        );
        Ok(())
    }

    fn handle_read_status(&mut self) -> Result<(), RoasterError> {
        self.status.ssr_hardware_status = self.actuator.get_ssr_hardware_status();

        let response =
            crate::output::artisan::ArtisanFormatter::format_read_response_full(&self.status);

        let parts: heapless::Vec<&str, 8> = response.split(',').take(8).collect();
        let expected = if self.status.pid_enabled { 8 } else { 5 };
        if response.trim().is_empty() || parts.len() != expected {
            error!(
                "Malformed READ response from ArtisanFormatter: expected {} values, got {}",
                expected,
                parts.len()
            );
        }

        debug!(
            "READ command - SSR status: {:?}, response generated",
            self.status.ssr_hardware_status
        );
        Ok(())
    }

    fn handle_chan(&mut self, rate: u16) -> Result<(), RoasterError> {
        // Bug DRA-7 (2026-07-26): the polling-rate request was only echoed in
        // the ack and otherwise discarded. Record it on the status so it is
        // observable (the emitter keeps its own 1 Hz cadence for now).
        self.status.chan_poll_rate_hz = rate;
        let ack = crate::output::artisan::ArtisanFormatter::format_chan_ack(rate);
        self.send_text_response(ack.as_str());
        debug!("Chan command received - sent ack for rate {}", rate);
        Ok(())
    }

    fn handle_run_regression(&mut self) -> Result<(), RoasterError> {
        // Bug M9 (2026-07-26): the old handler was an info-only no-op and the
        // task layer `continue`-ed past dispatch, so REG produced NO output
        // (not even OK) — Artisan had zero feedback about whether the
        // regression ran. Now the handler replies explicitly depending on
        // whether the `regression` feature is compiled into the build.
        #[cfg(all(target_arch = "riscv32", feature = "regression"))]
        {
            self.send_text_response("OK regression_started");
            info!("Artisan regression command received");
        }
        #[cfg(not(all(target_arch = "riscv32", feature = "regression")))]
        {
            self.send_text_response("ERR regression_disabled");
            warn!("REG command received but regression feature is not enabled");
        }
        Ok(())
    }

    fn handle_units(
        &mut self,
        is_fahrenheit: bool,
        current_time: Instant,
    ) -> Result<(), RoasterError> {
        let cmd = crate::config::RoasterCommand::SetUnits(is_fahrenheit);
        let result = self.forward_artisan_manual_command(cmd, current_time);
        if result.is_ok() {
            // Bug P-TC4 (2026-08-04): the previous `OK` response failed
            // Artisan's ArduinoTC4 handshake check, which only accepts an
            // empty or '#'-prefixed line (`Arduino could not set temperature
            // unit`). The reference TC4 firmware answers with
            // `# Changed units to C/F`; reply with a '#'-prefixed ack so the
            // initialisation completes and READ polling starts.
            let ack = crate::output::artisan::ArtisanFormatter::format_handshake_ack();
            self.send_text_response(ack.as_str());
        }
        result
    }

    fn handle_set_streaming(&mut self, enabled: bool) -> Result<(), RoasterError> {
        if enabled {
            self.dispatch
                .get_output_manager_mut()
                .enable_continuous_output();
            info!("Continuous telemetry enabled (STREAM;ON)");
        } else {
            self.dispatch
                .get_output_manager_mut()
                .disable_continuous_output();
            info!("Continuous telemetry disabled (STREAM;OFF)");
        }
        // '#'-prefixed ack: this is an extension command for custom clients,
        // but keeping the handshake-shaped ack lets any line-oriented peer
        // treat it uniformly.
        let ack = crate::output::artisan::ArtisanFormatter::format_handshake_ack();
        self.send_text_response(ack.as_str());
        Ok(())
    }

    fn handle_filt(&mut self, val: u8) -> Result<(), RoasterError> {
        // Bug DRA-7 (2026-07-26): the requested filter value was discarded.
        // Record it on the status — the firmware applies its internal EMA
        // alpha, but the host's request is now observable.
        self.status.requested_filter = val;
        // Bug P-TC4 (2026-08-04): same handshake fix as `handle_units` — the
        // previous `OK` tripped Artisan's `Arduino could not set filters`
        // check. The reference TC4 firmware is silent here; a '#'-prefixed
        // ack satisfies the same `#`/empty contract.
        let ack = crate::output::artisan::ArtisanFormatter::format_handshake_ack();
        self.send_text_response(ack.as_str());
        debug!("Filt command received - sent handshake ack");
        Ok(())
    }

    fn handle_set_pid_gain(&mut self, kp: f32, ki: f32, kd: f32) -> Result<(), RoasterError> {
        self.dispatch.set_pid_gains(kp, ki, kd)?;
        info!("PID gains updated: Kp={}, Ki={}, Kd={}", kp, ki, kd);
        Ok(())
    }

    fn handle_set_target_temp(&mut self, target: f32) -> Result<(), RoasterError> {
        // Bug #5: Artisan reports setpoints in its own display units. When the
        // host app is in °F mode, `PID;SV;250` means 250 °F (~121 °C), not
        // 250 °C. The previous code stored the raw value as °C and the PID
        // chased a dangerously high target. Convert input → °C *before* we
        // validate and store.
        let target_celsius = self
            .status
            .temperature_settings
            .convert_from_display(target);

        if !crate::config::constants::is_valid_target_temp(target_celsius) {
            warn!(
                "SetTargetTemp rejected: {:.1}°C (raw input {:.1}) outside valid range (50–300°C)",
                target_celsius, target,
            );
            return Err(RoasterError::InvalidState {
                source: Some("target_temp_out_of_range"),
            });
        }
        self.status.target_temp = target_celsius;
        self.enable_pid_control(target_celsius)?;
        info!(
            "Target temperature set to {:.1}°C (raw input: {:.1})",
            target_celsius, target
        );
        Ok(())
    }

    fn handle_set_profile(&mut self) -> Result<(), RoasterError> {
        let taken = crate::input::parser::take_profile();
        if let Some(mut profile) = taken {
            // Bug B25: PROFILE setpoints arrive in the host's display units
            // (same convention as SetTargetTemp / Preheat, fixed by "Bug #5").
            // The previous code validated the raw value as °C directly: a
            // °F user sending `60,300` meaning 300 °F (≈149 °C) had it
            // stored as a 300 °C target — the same over-temperature safety
            // bug class that "Bug #5" closed, on the parallel PROFILE path.
            // Convert each setpoint to °C before validating and storing.
            for sp in profile.setpoints.iter_mut() {
                let converted = self
                    .status
                    .temperature_settings
                    .convert_from_display(sp.temperature);
                if !crate::config::constants::is_valid_target_temp(converted) {
                    warn!(
                        "Profile rejected: setpoint {:.1} (raw) → {:.1}°C at {}s outside valid range (50–300°C)",
                        sp.temperature, converted, sp.time_secs,
                    );
                    return Err(RoasterError::InvalidState {
                        source: Some("profile_temp_out_of_range"),
                    });
                }
                sp.temperature = converted;
            }
            let count = profile.setpoints.len();
            self.active_profile = Some(profile);
            info!("Profile loaded: {} setpoints", count);
        } else {
            warn!("SetProfile received but no profile data in parser buffer");
        }
        Ok(())
    }

    /// Bug B13 / V2-7: drain one queued `#DUMP` row for the async emitter
    /// to send. Called from `ServiceContainer::with_roaster_async` (sync
    /// closure); the caller `.await`s `output_channel.try_send(row)` outside
    /// the lock (and re-pushes via `push_dump_row_front` on a full channel).
    pub fn take_dump_row(
        &mut self,
    ) -> Option<heapless::String<{ crate::logging::roast_logger::DUMP_ROW_CAPACITY }>> {
        self.dump_pending.pop_front()
    }

    /// Bug V2-7: re-push a `#DUMP` row to the FRONT of the deque when the
    /// async emitter's `try_send` failed (output channel full). FIFO order is
    /// preserved and no row is lost — the next tick will retry it.
    pub fn push_dump_row_front(
        &mut self,
        row: heapless::String<{ crate::logging::roast_logger::DUMP_ROW_CAPACITY }>,
    ) {
        // If the deque is somehow full (shouldn't happen — it's sized to
        // LOG_CAPACITY+1, larger than any single dump), drop the oldest row
        // to make room for the retry at the front.
        let _ = self.dump_pending.push_front(row);
    }

    fn handle_dump_log(&mut self) -> Result<(), RoasterError> {
        // Bug V2-7: start every dump clean so a second `#DUMP` request mid-
        // drain (or a dump requested right after a roast finished) does not
        // splice two partial dumps together. The deque is sized to hold a
        // full ring (LOG_CAPACITY+1 rows), and `roast_logger::dump()` now
        // preserves the roast's tail when the output buffer would overflow.
        self.dump_pending.clear();
        let dump = crate::logging::roast_logger::dump();
        for line in dump.split('\n') {
            if line.is_empty() {
                continue;
            }
            if let Ok(msg) =
                heapless::String::<{ crate::logging::roast_logger::DUMP_ROW_CAPACITY }>::try_from(
                    line,
                )
            {
                // Deque sized for a full ring; push_back should not fail in
                // practice. If it ever does (defensive), the front rows are
                // the dump header + earliest samples — drop the row rather
                // than truncate the irreplaceable tail.
                let _ = self.dump_pending.push_back(msg);
            }
        }
        info!(
            "Roast log dump requested ({} rows queued)",
            self.dump_pending.len()
        );
        Ok(())
    }

    fn handle_preheat(&mut self, target: f32) -> Result<(), RoasterError> {
        // Bug P3 (2026-08-03): PREHEAT is a deliberate re-energize action —
        // clear any latched emergency/fault so the STOP → PREHEAT flow works
        // (same rationale as the START recovery in `handle_start_roast`).
        if self.status.fault_condition || self.safety.is_emergency_active() {
            self.clear_emergency_explicit();
        }
        // Bug #5 (units) + plan-informe F4.7 (unbounded value): convert the
        // incoming target from the host's display units to °C, then validate
        // against the same range as SetTargetTemp. Artisan in °F mode reports
        // the preheat target in °F; storing 250 °F as 250 °C is a safety bug.
        let target_celsius = self
            .status
            .temperature_settings
            .convert_from_display(target);

        if !crate::config::constants::is_valid_target_temp(target_celsius) {
            warn!(
                "Preheat rejected: {:.1}°C (raw input {:.1}) outside valid range (50–300°C)",
                target_celsius, target,
            );
            return Err(RoasterError::InvalidState {
                source: Some("preheat_target_out_of_range"),
            });
        }

        self.preheat_target = Some(target_celsius);
        // Bug V2-5 (B3 residual): drop the cooldown latch on a deliberate
        // re-energize — same justification as `handle_start_roast`. Without
        // this, a consecutive batch (`OFF` at BT≈205 °C → cooling latch armed
        // → `PREHEAT;180` immediate) keeps the fan forced to 100 % while the
        // PID tries to heat against maximum airflow, and since the heater
        // keeps BT > COOLING_RELEASE_BEAN_TEMP_C the latch can never release
        // for the whole preheat. Only START used to clear it.
        self.cooling_active = false;
        self.state = RoasterState::Preheating;
        self.status.state = RoasterState::Preheating;
        self.enable_pid_control(target_celsius)?;
        self.dispatch
            .get_output_manager_mut()
            .disable_continuous_output();
        info!(
            "Preheat started — target {:.1}°C (raw input: {:.1})",
            target_celsius, target
        );
        Ok(())
    }

    fn handle_set_fan_profile(&mut self) -> Result<(), RoasterError> {
        let taken = crate::input::parser::fan_profile_take();
        if let Some(profile) = taken {
            let count = profile.setpoints.len();
            self.fan_profile = Some(profile);
            info!("Fan profile loaded: {} setpoints", count);
        } else {
            warn!("SetFanProfile received but no fan profile data in buffer");
        }
        Ok(())
    }

    fn handle_set_pid_channel(&mut self, ch: u8) -> Result<(), RoasterError> {
        self.status.pid_channel = ch;
        info!(
            "PID input channel set to {} ({})",
            ch,
            if ch == 1 {
                "ET"
            } else if ch == 2 {
                "BT"
            } else {
                "other"
            }
        );
        Ok(())
    }

    fn handle_set_pid_cycle_time(&mut self, ms: u32) -> Result<(), RoasterError> {
        self.dispatch.set_pid_cycle_time(ms);
        self.status.pid_cycle_time_ms = ms;
        info!("PID cycle time set to {}ms", ms);
        Ok(())
    }

    fn handle_set_pid_output_limits(&mut self, min: f32, max: f32) -> Result<(), RoasterError> {
        self.dispatch.set_pid_output_limits(min, max);
        // Bug fix: echo the PID's post-clamp/swap limits, not the raw inputs.
        // Raw inputs like PID;LIMIT;-50;200 or PID;LIMIT;80;20 would mislead
        // telemetry; the PID uses [0,100] and [20,80] respectively.
        let (actual_min, actual_max) = self.dispatch.pid_output_limits();
        self.status.pid_output_min = actual_min;
        self.status.pid_output_max = actual_max;
        info!(
            "PID output limits set to {:.1}% – {:.1}%",
            actual_min, actual_max
        );
        Ok(())
    }

    fn forward_artisan_manual_command(
        &mut self,
        command: crate::config::RoasterCommand,
        current_time: Instant,
    ) -> Result<(), RoasterError> {
        self.process_command(command, current_time)
    }

    /// Send OT2-clamped notification through the output channel so Artisan
    /// is aware the fan value was clamped. Per Spec F4.8 an out-of-range OT2
    /// clamps the fan value to [0,100] but must NOT change the heater state
    /// or affect PID status — the previous `heater_cut` wording claimed the
    /// heater was cut when it was not, the dangerous direction of the error
    /// (an operator/automation that reads `heater_cut` would assume the
    /// heater is off while it is still energised). Bug B24: report the
    /// actual semantics — fan clamped, heater unchanged.
    fn send_ot2_clamped_notification(&self, fan_value: u8) {
        use crate::logging::traceability::TRACE_EVENT_MAX_LEN;
        let mut msg = heapless::String::<{ TRACE_EVENT_MAX_LEN }>::new();
        let _ = core::fmt::Write::write_fmt(
            &mut msg,
            core::format_args!("ERR OT2_CLAMPED fan={} heater_unchanged", fan_value),
        );
        crate::hardware::error_counters::try_send_output(
            crate::application::service_container::ServiceContainer::get_output_channel(),
            msg,
        );
    }

    /// Send an `ERR safety_fault <reason>` notification through the output
    /// channel when an internal trap arms the emergency latch, so the host
    /// (Artisan or a script) learns about the fault immediately instead of
    /// discovering it through the next rejected command (Audit A-TC4,
    /// 2026-08-12). Same emission pattern as `send_ot2_clamped_notification`.
    /// The reason keeps its human-readable spaces on the wire; it is not a
    /// stable contract (see PROTOCOL.md §10).
    fn send_safety_fault_notification(&self, reason: &str) {
        use crate::logging::traceability::TRACE_EVENT_MAX_LEN;
        let mut msg = heapless::String::<{ TRACE_EVENT_MAX_LEN }>::new();
        let _ = core::fmt::Write::write_fmt(
            &mut msg,
            core::format_args!("ERR safety_fault {}", reason),
        );
        crate::hardware::error_counters::try_send_output(
            crate::application::service_container::ServiceContainer::get_output_channel(),
            msg,
        );
    }

    fn send_text_response(&self, text: &str) {
        use crate::logging::traceability::TRACE_EVENT_MAX_LEN;

        if let Ok(msg) = heapless::String::<TRACE_EVENT_MAX_LEN>::try_from(text) {
            crate::hardware::error_counters::try_send_output(
                crate::application::service_container::ServiceContainer::get_output_channel(),
                msg,
            );
        }
    }

    /// Enable PID control toward `target_temp` via the dispatch handler.
    pub fn enable_pid_control(&mut self, target_temp: f32) -> Result<(), RoasterError> {
        self.dispatch.enable_pid(target_temp, &mut self.status)
    }

    /// Current fan output percentage from status (0-100).
    pub fn get_fan_speed(&self) -> f32 {
        self.status.fan_output
    }

    /// Last desired heater (SSR) output recorded by the actuator controller.
    pub fn last_desired_heater_output(&self) -> f32 {
        self.actuator.last_desired_heater_output()
    }

    /// Run one PID update when due per `pid_cycle_time_ms`; returns the SSR duty to apply.
    fn update_pid_control(&mut self, current_time: embassy_time::Instant) -> f32 {
        use crate::config::constants::SsrHardwareStatus;

        // M7: the throttle must respect the operator-configured
        // `pid_cycle_time_ms` (PID;CT;...), not the compile-time default. A
        // setting like `PID;CT;1000` was previously inert — the I-term would
        // integrate ten times faster than the configured cadence. Defensive
        // floor of 10 ms guards against absurd inputs (PID;CT;0 would freeze
        // the throttle at "due" and burn CPU).
        let cycle_ms = self.status.pid_cycle_time_ms.max(10) as u64;
        let should_update = if let Some(last_update) = self.last_pid_update {
            current_time.saturating_duration_since(last_update)
                >= embassy_time::Duration::from_millis(cycle_ms)
        } else {
            true
        };

        if should_update {
            if self.status.ssr_hardware_status != SsrHardwareStatus::Available {
                warn!("PID update requested but SSR not available - skipping");
                return 0.0;
            }

            // Profile-following: update PID target from profile interpolation
            if let (Some(ref profile), Some(start)) =
                (&self.active_profile, self.profile_start_time)
            {
                let elapsed = current_time.saturating_duration_since(start).as_secs() as u32;
                if let Some(new_target) = profile.target_at(elapsed) {
                    if (new_target - self.status.target_temp).abs() > 0.5 {
                        self.status.target_temp = new_target;
                        let _ = self.dispatch.set_pid_target(new_target);
                        debug!("Profile target: {:.1}°C at t={}s", new_target, elapsed);
                    }
                }
            }

            let output = self.dispatch.get_pid_output(self.status.pv, current_time);

            self.last_pid_update = Some(current_time);

            if self.state == crate::config::constants::RoasterState::Heating {
                // Bug L11 (2026-07-25): pivot on the active PV, not always on
                // `bean_temp`. When `PID;CHAN;1` selects ET as the PID's PV
                // (`status.pv = env_temp`), the PID is driving ET toward the
                // target. The Heating→Stable transition records "the loop has
                // converged on its setpoint" — using `bean_temp` here would
                // ignore the loop the PID actually closed (BT may lag ET by
                // tens of degrees during heat-up). With channel=2 (the
                // default) `status.pv == bean_temp`, so this is behaviour-
                // preserving for the common case.
                let pv_for_transition = self.status.pv;
                let temp_error = (pv_for_transition - self.status.target_temp).abs();
                if temp_error < 2.0 {
                    self.state = crate::config::constants::RoasterState::Stable;
                    info!("Target temperature reached, entering stable state");
                }
            } else if self.state == crate::config::constants::RoasterState::Stable {
                // Bug M2 (2026-07-25): hysteresis — the previous transition
                // Heating → Stable was unidirectional, so once the operator
                // (or a ramp) raised target beyond `+3 °C`, the state stayed
                // `Stable` even though the system was clearly chasing the new
                // setpoint again. Flip back to `Heating` when the gap widens
                // past the upper threshold (3 °C) so the rest of the FSM keeps
                // its expected Heating-typed semantics.
                // Bug F (2026-08-03): pivot on the active PV here too, mirroring the
                // L11 fix on the Heating→Stable arm below. Pre-fix this arm
                // read `bean_temp` unconditionally, so under `PID;CHAN;1`
                // (ET as PV) the FSM flip-flopped Heating↔Stable on every PID
                // cycle during heat-up: ET converged (±2 °C) → Stable, then
                // BT's lag (tens of degrees) re-opened the ≥3 °C gap →
                // Heating. With channel 2 (default) `status.pv == bean_temp`,
                // so behaviour is unchanged there.
                let temp_error = (self.status.pv - self.status.target_temp).abs();
                if temp_error >= 3.0 {
                    self.state = crate::config::constants::RoasterState::Heating;
                    info!("Target moved beyond hysteresis band, re-entering heating state");
                }
            }

            debug!(
                "PID update: bean_temp={:.1}°C, target={:.1}°C, output={:.1}%",
                self.status.bean_temp, self.status.target_temp, output
            );

            output
        } else {
            self.status.ssr_output
        }
    }

    // Immutable accessor methods
    /// Immutable access to the `SensorController`.
    pub fn sensor(&self) -> &SensorController {
        &self.sensor
    }
    /// Immutable access to the `ActuatorController`.
    pub fn actuator(&self) -> &ActuatorController {
        &self.actuator
    }
    /// Immutable access to the `SafetyController`.
    pub fn safety(&self) -> &SafetyController {
        &self.safety
    }
    /// Immutable access to the `CommandDispatcher`.
    pub fn dispatch(&self) -> &CommandDispatcher {
        &self.dispatch
    }

    // Mutable accessor methods
    /// Mutable access to the `SensorController`.
    pub fn sensor_mut(&mut self) -> &mut SensorController {
        &mut self.sensor
    }
    /// Mutable access to the `ActuatorController`.
    pub fn actuator_mut(&mut self) -> &mut ActuatorController {
        &mut self.actuator
    }
    /// Mutable access to the `SafetyController`.
    pub fn safety_mut(&mut self) -> &mut SafetyController {
        &mut self.safety
    }
    /// Mutable access to the `CommandDispatcher`.
    pub fn dispatch_mut(&mut self) -> &mut CommandDispatcher {
        &mut self.dispatch
    }
}

#[cfg(test)]
#[path = "roaster_control_tests.rs"]
mod tests;
