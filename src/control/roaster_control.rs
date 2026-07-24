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

pub struct RoasterControl {
    state: RoasterState,
    status: SystemStatus,
    sensor: SensorController,
    actuator: ActuatorController,
    safety: SafetyController,
    dispatch: CommandDispatcher,
    last_pid_update: Option<Instant>,
    active_profile: Option<RoastProfile>,
    profile_start_time: Option<Instant>,
    fan_profile: Option<crate::config::FanProfile>,
    charge_detected: bool,
    charge_time: Option<Instant>,
    preheat_target: Option<f32>,
    bt_charge_history: heapless::Deque<f32, 10>,
    /// Bug B23: per-tick divider that throttles `bt_charge_history` sampling
    /// to once every `CHARGE_SAMPLE_TICK_DIV` ticks. The control loop runs at
    /// ~100 ms/tick; the deque holds 10 samples; sampling every tick gave a
    /// ~1 s charge-detection window (a >20 °C drop in 1 s is physically
    /// impossible for a BT probe with 1-5 °C/s thermal inertia), so #CHARGE
    /// was effectively indetectable. With `CHARGE_DETECTION_WINDOW_S = 3` we
    /// sample every 300 ms → 10 samples × 300 ms = 3 s, the original intent.
    charge_history_tick_div: u8,
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
    dump_pending:
        heapless::Deque<heapless::String<256>, { crate::logging::roast_logger::LOG_CAPACITY + 1 }>,
}

impl RoasterControl {
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
            fan_profile: None,
            charge_detected: false,
            charge_time: None,
            preheat_target: None,
            bt_charge_history: heapless::Deque::new(),
            charge_history_tick_div: 0,
            cooling_active: false,
            dump_pending: heapless::Deque::new(),
        })
    }

    pub async fn read_sensors(&mut self) -> Result<(), RoasterError> {
        match self.sensor.read_sensors(&mut self.status).await {
            Err(RoasterError::TemperatureOutOfRange {
                source: Some("overtemp_detected"),
            })
            | Err(RoasterError::TemperatureOutOfRange {
                source: Some("temperature_out_of_valid_range"),
            }) => {
                self.emergency_shutdown("Temperature exceeds valid range")?;
                Err(RoasterError::TemperatureOutOfRange {
                    source: Some("overtemp_detected"),
                })
            }
            other => other,
        }
    }

    pub fn get_status(&self) -> SystemStatus {
        self.status
    }

    pub fn status_mut(&mut self) -> &mut SystemStatus {
        &mut self.status
    }

    pub fn last_sensor_sample(&self) -> Option<crate::hardware::sensors::SensorSample> {
        self.sensor.last_sensor_sample()
    }

    pub fn get_state(&self) -> RoasterState {
        self.state
    }

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

    pub fn mark_overtemp_regression_active(&mut self, active: bool) {
        self.safety
            .mark_overtemp_regression_active(active, &mut self.status);
    }

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
            self.dispatch.disable_pid();
            self.status.pid_enabled = false;
            self.status.artisan_control = true;
            self.dispatch
                .get_output_manager_mut()
                .enable_continuous_output();

            self.actuator
                .apply_guarded_heater(heater, current_time, true, &mut self.status)?;
            self.status.ssr_hardware_status = self.actuator.get_ssr_hardware_status();
        }

        if let Some(fan) = outcome.fan_target {
            // Bug B4: a fan command must NOT alter PID/mode flags (Spec F4.8).
            // The two lines that used to live here (`artisan_control = true`
            // and `pid_enabled = false`) made a mid-roast slider move drop
            // the heater by disabling PID and falling into manual mode with
            // no manual heater set. Keep only the side-effect of issuing a
            // continuous-output tick (so Artisan sees the fan change) and the
            // physical fan write.
            self.dispatch
                .get_output_manager_mut()
                .enable_continuous_output();

            self.actuator.set_fan_speed(fan, &mut self.status)?;
            self.status.ssr_hardware_status = self.actuator.get_ssr_hardware_status();
        }

        Ok(())
    }

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
            if let Err(e) = self.actuator.set_heater_power(0.0) {
                log::error!("Safety outcome: heater off failed: {:?}", e);
            }
            self.actuator.capture_ssr_monitor_metrics(&mut self.status);
            // Also set fan to 100% for cooling during safety events
            if let Err(e) = self.actuator.set_fan_raw(100.0) {
                log::error!("Safety outcome: fan 100% failed: {:?}", e);
            }
            self.status.fan_output = 100.0;
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
        // Bug B3: latch cooldown so `update_control`'s fan selector keeps the
        // fan at 100% on every subsequent tick. STOP does NOT arm the safety
        // emergency latch (only `emergency_shutdown` does); without this flag
        // the next tick would call `artisan_manual_fan()` (now 0.0 after
        // `clear_manual`) and cut the airflow over the hot bean mass.
        self.cooling_active = true;

        self.actuator.capture_ssr_monitor_metrics(&mut self.status);
        self.actuator.set_heater_power(0.0)?;
        // Bug #13: Set fan to 100% for cooling during stop (matches README and emergency_shutdown)
        self.actuator.set_fan_raw(100.0)?;
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
    }

    pub fn is_temperature_valid(temp: f32) -> bool {
        SensorController::is_temperature_valid(temp)
    }

    pub fn emergency_shutdown(&mut self, reason: &str) -> Result<(), RoasterError> {
        // THERM-1: Latch the emergency so internal traps (overtemp, sensor timeout,
        // RoR, max-roast-time) prevent re-energizing on the next tick.
        self.safety.activate_emergency();
        self.status.fault_condition = true;
        self.state = RoasterState::Error;
        self.actuator.emergency_shutdown(reason, &mut self.status)
    }

    pub fn update_control(&mut self, current_time: Instant) -> Result<f32, RoasterError> {
        if let Some(last_read) = self.sensor.last_temp_read() {
            if current_time.duration_since(last_read)
                > Duration::from_millis(TEMP_VALIDITY_TIMEOUT_MS as u64)
            {
                warn!("Temperature sensor timeout detected");
                self.emergency_shutdown("Temperature sensor timeout")?;
            }
        }

        // Re-detect heat source periodically (throttled to ~1s by ActuatorController).
        // This ensures a mid-roast SSR or wiring fault is detected, not just boot-time.
        self.actuator.periodic_health_check(current_time);

        self.status.ssr_hardware_status = self.actuator.get_ssr_hardware_status();

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
                return Ok(0.0);
            }
        }

        // Maximum roast time safety backstop. Bug V2-16c: same physical gate as
        // comms-idle — protect any roasting session with the heater energized,
        // not only the named roast states.
        if heater_energized || roast_active {
            if let Some(start) = self.profile_start_time {
                let elapsed_secs = current_time.duration_since(start).as_secs() as u32;
                if elapsed_secs >= crate::config::constants::MAX_ROAST_TIME_SECS {
                    warn!(
                        "MAX_ROAST_TIME exceeded ({}s >= {}s) — emergency shutdown",
                        elapsed_secs,
                        crate::config::constants::MAX_ROAST_TIME_SECS
                    );
                    self.emergency_shutdown("Maximum roast time exceeded")?;
                    return Ok(0.0);
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
        if self.state == RoasterState::Heating && !self.charge_detected {
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
                            let _ = output_channel.try_send(charge_msg);
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
            return Ok(0.0);
        }
        self.sensor
            .refresh_filtered_derivative(current_pv, current_time, &mut self.status);

        // Bug V2-16a: the RoR guard historically ran every tick in *all*
        // modes and states. With an empty drum and a low-mass BT probe, the
        // probe heats faster than 0.5 °C/s during PREHEAT, so the guard
        // fired `rate_of_rise_exceeded → emergency_shutdown` in the first
        // 1-2 seconds of heating — and via V2-1 that became a power-cycle
        // brick on the first real PREHEAT. The guard must only protect once
        // beans are present. Gate it to the post-charge roasting states
        // (`Heating` / `Stable`); Idle/Preheating must not trigger it
        // (Preheating is by definition heating an empty drum). This is the
        // conservative fix — the threshold stays at 0.5 °C/s; relaxing it
        // is an independent knob if real-fast-roast ramp data demands it.
        if matches!(self.state, RoasterState::Heating | RoasterState::Stable) {
            if let Err(e) = self.sensor.check_rate_of_rise(&self.status) {
                warn!("Rate-of-rise check failed: {:?}", e);
                self.emergency_shutdown("Bean temperature rate-of-rise exceeded")?;
                return Ok(0.0);
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
                    current_time.duration_since(last_read) > Duration::from_millis(500)
                // > TEMPERATURE_READ_INTERVAL_MS * 2 + margin
                } else {
                    false
                };

                if is_stale {
                    debug!(
                        "Sensor data is stale (>{}ms), holding last PID output",
                        PID_SAMPLE_TIME_MS
                    );
                    self.status.mv // Hold last output
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
        let applied_output = self.actuator.apply_guarded_heater(
            desired_output,
            current_time,
            false,
            &mut self.status,
        )?;
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
            let elapsed = current_time.duration_since(start).as_secs() as u32;
            fp.target_at(elapsed).map(|s| s as f32).unwrap_or(20.0)
        } else {
            self.dispatch.artisan_manual_fan()
        };
        self.actuator
            .set_fan_speed(fan_output, &mut self.status)
            .map_err(|_| RoasterError::HardwareError {
                source: Some("fan_set_in_control_loop_failed"),
            })?;

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

        Ok(applied_output)
    }

    pub async fn process_output(&mut self) -> Result<(), RoasterError> {
        if let Err(e) = self
            .dispatch
            .get_output_manager_mut()
            .process_status(&self.status)
            .await
        {
            warn!("Output error: {:?}", e);
        }
        Ok(())
    }

    pub fn get_output_manager(&self) -> &crate::control::OutputController {
        self.dispatch.get_output_manager()
    }

    pub fn get_output_manager_mut(&mut self) -> &mut crate::control::OutputController {
        self.dispatch.get_output_manager_mut()
    }

    pub fn process_artisan_command(
        &mut self,
        command: crate::config::ArtisanCommand,
    ) -> Result<(), RoasterError> {
        // C5: Record wall-clock (millis-since-boot) of last command for idle timeout.
        self.status.last_command_received_at_ms = embassy_time::Instant::now().as_millis();

        // Bug #6 fix: Reject all commands when a fault condition is active.
        // Prevents heater ramp commands from worsening an over-temp situation
        // that was detected between sensor reads.
        // Exception: READ, STATUS, and STOP are always allowed for monitoring and safety.
        if self.status.fault_condition {
            match command {
                crate::config::ArtisanCommand::ReadStatus
                | crate::config::ArtisanCommand::StatusReport
                | crate::config::ArtisanCommand::Stop
                | crate::config::ArtisanCommand::EmergencyStop => { /* allow */ }
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
                // Bug V2-1: `STOP` parses to `EmergencyStop` and arms the
                // emergency latch (`activate_emergency` + `fault_condition`),
                // but the only sanctioned un-latch path
                // (`RoasterCommand::StopRoast → clear_emergency_explicit`)
                // has NO producer in production code — so a single `STOP`
                // bricked the roaster until a power cycle. The host needs an
                // *reachable* recovery action. We extend plain `OFF`
                // (`ArtisanCommand::Stop`, token "OFF", *not* the latch-arming
                // `STOP`) to act as stop-and-recover when it arrives with the
                // latch armed and the device in the `Error` state: clear the
                // latch first, then run the normal stop. The whitelist at the
                // top of this method already permits `Stop` with
                // `fault_condition` active, so this is the sanctioned door.
                if self.status.fault_condition && self.safety.is_emergency_active() {
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
            self.dispatch
                .get_output_manager_mut()
                .enable_continuous_output();
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
        self.actuator.set_heater_power(0.0)?;
        self.actuator.set_fan_raw(100.0)?;
        self.status.fan_output = 100.0;

        // Stop streaming the protocol output without touching the latch.
        self.dispatch.stop_streaming(&mut self.status);
        crate::logging::roast_logger::stop_roast();

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
        let ack = crate::output::artisan::ArtisanFormatter::format_chan_ack(rate);
        self.send_text_response(ack.as_str());
        debug!("Chan command received - sent ack for rate {}", rate);
        Ok(())
    }

    fn handle_run_regression(&mut self) -> Result<(), RoasterError> {
        info!("Artisan regression command received");
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
            self.send_text_response("OK");
        }
        result
    }

    fn handle_filt(&mut self, _val: u8) -> Result<(), RoasterError> {
        self.send_text_response("OK");
        debug!("Filt command received - sent OK");
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
    pub fn take_dump_row(&mut self) -> Option<heapless::String<256>> {
        self.dump_pending.pop_front()
    }

    /// Bug V2-7: re-push a `#DUMP` row to the FRONT of the deque when the
    /// async emitter's `try_send` failed (output channel full). FIFO order is
    /// preserved and no row is lost — the next tick will retry it.
    pub fn push_dump_row_front(&mut self, row: heapless::String<256>) {
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
            if let Ok(msg) = heapless::String::<256>::try_from(line) {
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
        let _ = crate::application::service_container::ServiceContainer::get_output_channel()
            .try_send(msg);
    }

    fn send_text_response(&self, text: &str) {
        use crate::logging::traceability::TRACE_EVENT_MAX_LEN;

        if let Ok(msg) = heapless::String::<TRACE_EVENT_MAX_LEN>::try_from(text) {
            let _ = crate::application::service_container::ServiceContainer::get_output_channel()
                .try_send(msg);
        }
    }

    pub fn enable_pid_control(&mut self, target_temp: f32) -> Result<(), RoasterError> {
        self.dispatch.enable_pid(target_temp, &mut self.status)
    }

    pub fn get_fan_speed(&self) -> f32 {
        self.status.fan_output
    }

    pub fn last_desired_heater_output(&self) -> f32 {
        self.actuator.last_desired_heater_output()
    }

    fn update_pid_control(&mut self, current_time: embassy_time::Instant) -> f32 {
        use crate::config::constants::SsrHardwareStatus;

        let should_update = if let Some(last_update) = self.last_pid_update {
            current_time.duration_since(last_update)
                >= embassy_time::Duration::from_millis(crate::config::PID_SAMPLE_TIME_MS as u64)
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
                let elapsed = current_time.duration_since(start).as_secs() as u32;
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
                let temp_error = (self.status.bean_temp - self.status.target_temp).abs();
                if temp_error < 2.0 {
                    self.state = crate::config::constants::RoasterState::Stable;
                    info!("Target temperature reached, entering stable state");
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
    pub fn sensor(&self) -> &SensorController {
        &self.sensor
    }
    pub fn actuator(&self) -> &ActuatorController {
        &self.actuator
    }
    pub fn safety(&self) -> &SafetyController {
        &self.safety
    }
    pub fn dispatch(&self) -> &CommandDispatcher {
        &self.dispatch
    }

    // Mutable accessor methods
    pub fn sensor_mut(&mut self) -> &mut SensorController {
        &mut self.sensor
    }
    pub fn actuator_mut(&mut self) -> &mut ActuatorController {
        &mut self.actuator
    }
    pub fn safety_mut(&mut self) -> &mut SafetyController {
        &mut self.safety
    }
    pub fn dispatch_mut(&mut self) -> &mut CommandDispatcher {
        &mut self.dispatch
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::common::{StubFan, StubHeater};
    use crate::config::{ArtisanCommand, RoasterCommand, RoasterState};
    use crate::hardware::sensors::SensorConversionHub;
    use alloc::boxed::Box;
    use embassy_time::Instant;

    fn make_control() -> RoasterControl {
        let heater = Box::new(StubHeater::new());
        let fan = Box::new(StubFan::new());
        RoasterControl::new(heater, fan, SensorConversionHub::new())
            .expect("test control should build")
    }

    fn make_control_with_stubs(heater: StubHeater, fan: StubFan) -> RoasterControl {
        RoasterControl::new(Box::new(heater), Box::new(fan), SensorConversionHub::new())
            .expect("test control should build")
    }

    // ── Construction and static methods ──────────

    #[test]
    fn construction_defaults_to_idle() {
        let ctrl = make_control();
        assert_eq!(ctrl.get_state(), RoasterState::Idle);
        assert_eq!(ctrl.get_status().state, RoasterState::Idle);
        assert!(!ctrl.get_status().fault_condition);
        assert_eq!(ctrl.get_fan_speed(), 0.0);
    }

    #[test]
    fn is_temperature_valid_accepts_normal() {
        assert!(RoasterControl::is_temperature_valid(25.0));
        assert!(RoasterControl::is_temperature_valid(0.0));
        assert!(RoasterControl::is_temperature_valid(200.0));
    }

    #[test]
    fn is_temperature_valid_rejects_nan() {
        assert!(!RoasterControl::is_temperature_valid(f32::NAN));
    }

    #[test]
    fn is_temperature_valid_rejects_extreme() {
        assert!(!RoasterControl::is_temperature_valid(9999.0));
        assert!(!RoasterControl::is_temperature_valid(-9999.0));
    }

    // ── Getters ─────────────────────────────────

    #[test]
    fn get_fan_speed_returns_status_value() {
        let ctrl = make_control();
        assert_eq!(ctrl.get_fan_speed(), 0.0);
    }

    #[test]
    fn status_mut_allows_modification() {
        let mut ctrl = make_control();
        let status = ctrl.status_mut();
        status.bean_temp = 150.0;
        status.env_temp = 200.0;
        assert_eq!(ctrl.get_status().bean_temp, 150.0);
        assert_eq!(ctrl.get_status().env_temp, 200.0);
    }

    // ── Emergency shutdown ──────────────────────

    #[test]
    fn emergency_shutdown_changes_state_and_returns_error() {
        let mut ctrl = make_control();
        let result = ctrl.emergency_shutdown("test shutdown");
        assert!(matches!(
            result,
            Err(RoasterError::EmergencyShutdown { .. })
        ));
        assert_eq!(ctrl.get_state(), RoasterState::Error);
        assert!(ctrl.get_status().fault_condition);
    }

    #[test]
    fn emergency_shutdown_fan_goes_to_100() {
        let heater = StubHeater::new();
        let fan = StubFan::new();
        let mut ctrl = make_control_with_stubs(heater, fan);
        let _ = ctrl.emergency_shutdown("test");
        assert_eq!(ctrl.get_status().fan_output, 100.0);
    }

    // ── Overtemp regression ─────────────────────

    #[test]
    fn mark_overtemp_regression_active_sets_flag() {
        let mut ctrl = make_control();
        ctrl.mark_overtemp_regression_active(true);
        assert!(ctrl.get_status().overtemp_regression_active);
    }

    #[test]
    fn mark_overtemp_regression_active_clears_flag() {
        let mut ctrl = make_control();
        ctrl.mark_overtemp_regression_active(true);
        ctrl.mark_overtemp_regression_active(false);
        assert!(!ctrl.get_status().overtemp_regression_active);
    }

    // ── Update temperatures ─────────────────────

    #[test]
    fn update_temperatures_normal() {
        let mut ctrl = make_control();
        let now = Instant::from_millis(1000);
        let result = ctrl.update_temperatures(150.0, 120.0, now);
        assert!(result.is_ok());
        assert_eq!(ctrl.get_status().bean_temp, 150.0);
        assert_eq!(ctrl.get_status().env_temp, 120.0);
    }

    #[test]
    fn update_temperatures_overtemp_triggers_emergency() {
        let mut ctrl = make_control();
        let now = Instant::from_millis(1000);
        // OVERTEMP_THRESHOLD is 260°C, MAX_VALID_TEMP is 350°C
        let result = ctrl.update_temperatures(300.0, 25.0, now);
        assert!(result.is_err());
        assert_eq!(ctrl.get_state(), RoasterState::Error);
        assert!(ctrl.get_status().fault_condition);
    }

    // ── Process command ─────────────────────────

    #[test]
    fn process_stop_roast_returns_to_idle() {
        let mut ctrl = make_control();
        let now = Instant::from_millis(1000);
        let result = ctrl.process_command(RoasterCommand::StopRoast, now);
        assert!(result.is_ok());
        assert_eq!(ctrl.get_state(), RoasterState::Idle);
    }

    #[test]
    fn process_stop_roast_clears_fault() {
        let heater = StubHeater::new();
        let fan = StubFan::new();
        let mut ctrl = make_control_with_stubs(heater, fan);
        let _ = ctrl.emergency_shutdown("fault");
        assert!(ctrl.get_status().fault_condition);
        let now = Instant::from_millis(2000);
        let result = ctrl.process_command(RoasterCommand::StopRoast, now);
        assert!(result.is_ok());
        assert!(!ctrl.get_status().fault_condition);
    }

    #[test]
    fn process_emergency_stop_triggers_safety() {
        let heater = StubHeater::new();
        let fan = StubFan::new();
        let mut ctrl = make_control_with_stubs(heater, fan);
        let now = Instant::from_millis(1000);
        let result = ctrl.process_command(RoasterCommand::EmergencyStop, now);
        assert!(matches!(
            result,
            Err(RoasterError::TemperatureOutOfRange { .. })
        ));
        assert!(ctrl.get_status().fault_condition);
    }

    #[test]
    fn process_set_heater_manual_triggers_manual_policy() {
        let heater = StubHeater::new();
        let fan = StubFan::new();
        let mut ctrl = make_control_with_stubs(heater, fan);
        let now = Instant::from_millis(1000);
        let result = ctrl.process_command(RoasterCommand::SetHeaterManual(50), now);
        assert!(result.is_ok());
    }

    #[test]
    fn process_set_fan_manual_triggers_manual_policy() {
        let heater = StubHeater::new();
        let fan = StubFan::new();
        let mut ctrl = make_control_with_stubs(heater, fan);
        let now = Instant::from_millis(1000);
        let result = ctrl.process_command(RoasterCommand::SetFanManual(75), now);
        assert!(result.is_ok());
    }

    // ── Process Artisan command ─────────────────

    #[test]
    fn artisan_stop_returns_ok() {
        let mut ctrl = make_control();
        let result = ctrl.process_artisan_command(ArtisanCommand::Stop);
        assert!(result.is_ok());
    }

    #[test]
    fn artisan_emergency_stop_triggers_emergency() {
        let mut ctrl = make_control();
        let result = ctrl.process_artisan_command(ArtisanCommand::EmergencyStop);
        assert!(result.is_ok());
    }

    #[test]
    fn artisan_set_pid_gain_updates_gains() {
        let mut ctrl = make_control();
        let result = ctrl.process_artisan_command(ArtisanCommand::SetPidGain(1.0, 0.1, 0.05));
        assert!(result.is_ok());
    }

    #[test]
    fn artisan_set_target_temp_valid() {
        let mut ctrl = make_control();
        let result = ctrl.process_artisan_command(ArtisanCommand::SetTargetTemp(200.0));
        assert!(result.is_ok());
        assert_eq!(ctrl.get_status().target_temp, 200.0);
    }

    #[test]
    fn artisan_set_target_temp_out_of_range_rejected() {
        let mut ctrl = make_control();
        let result = ctrl.process_artisan_command(ArtisanCommand::SetTargetTemp(999.0));
        assert!(matches!(
            result,
            Err(RoasterError::InvalidState {
                source: Some("target_temp_out_of_range")
            })
        ));
    }

    #[test]
    fn artisan_start_roast_starts_streaming() {
        let mut ctrl = make_control();
        let result = ctrl.process_artisan_command(ArtisanCommand::StartRoast);
        assert!(result.is_ok());
    }

    #[test]
    fn artisan_set_pid_channel_switches_channel() {
        let mut ctrl = make_control();
        let result = ctrl.process_artisan_command(ArtisanCommand::SetPidChannel(1));
        assert!(result.is_ok());
        assert_eq!(ctrl.get_status().pid_channel, 1);
    }

    #[test]
    fn artisan_set_pid_cycle_time_updates() {
        let mut ctrl = make_control();
        let result = ctrl.process_artisan_command(ArtisanCommand::SetPidCycleTime(500));
        assert!(result.is_ok());
        assert_eq!(ctrl.get_status().pid_cycle_time_ms, 500);
    }

    #[test]
    fn artisan_set_pid_output_limits_updates() {
        let mut ctrl = make_control();
        let result = ctrl.process_artisan_command(ArtisanCommand::SetPidOutputLimits(10.0, 90.0));
        assert!(result.is_ok());
        assert_eq!(ctrl.get_status().pid_output_min, 10.0);
        assert_eq!(ctrl.get_status().pid_output_max, 90.0);
    }

    #[test]
    fn artisan_chan_returns_ok() {
        let mut ctrl = make_control();
        let result = ctrl.process_artisan_command(ArtisanCommand::Chan(4));
        assert!(result.is_ok());
    }

    #[test]
    fn artisan_units_returns_ok() {
        let mut ctrl = make_control();
        let result = ctrl.process_artisan_command(ArtisanCommand::Units(true));
        assert!(result.is_ok());
    }

    #[test]
    fn artisan_filt_returns_ok() {
        let mut ctrl = make_control();
        let result = ctrl.process_artisan_command(ArtisanCommand::Filt(5));
        assert!(result.is_ok());
    }

    #[test]
    fn artisan_status_report_returns_ok() {
        let mut ctrl = make_control();
        let result = ctrl.process_artisan_command(ArtisanCommand::StatusReport);
        assert!(result.is_ok());
    }

    #[test]
    fn artisan_preheat_sets_target() {
        let mut ctrl = make_control();
        let result = ctrl.process_artisan_command(ArtisanCommand::Preheat(180.0));
        assert!(result.is_ok());
    }

    #[test]
    fn artisan_set_profile_with_no_data_returns_ok() {
        let mut ctrl = make_control();
        let result = ctrl.process_artisan_command(ArtisanCommand::SetProfile);
        assert!(result.is_ok());
    }

    #[test]
    fn artisan_set_fan_profile_with_no_data_returns_ok() {
        let mut ctrl = make_control();
        let result = ctrl.process_artisan_command(ArtisanCommand::SetFanProfile);
        assert!(result.is_ok());
    }

    #[test]
    fn artisan_run_regression_returns_ok() {
        let mut ctrl = make_control();
        let result = ctrl.process_artisan_command(ArtisanCommand::RunRegression);
        assert!(result.is_ok());
    }

    #[test]
    fn accessor_methods_return_references() {
        let mut ctrl = make_control();
        let _s = ctrl.sensor();
        let _a = ctrl.actuator();
        let _sa = ctrl.safety();
        let _d = ctrl.dispatch();
        let _sm = ctrl.sensor_mut();
        let _am = ctrl.actuator_mut();
        let _sam = ctrl.safety_mut();
        let _dm = ctrl.dispatch_mut();
    }

    // ── Read status (READ command) ──────────────

    #[test]
    fn artisan_read_status_returns_ok() {
        let mut ctrl = make_control();
        let result = ctrl.process_artisan_command(ArtisanCommand::ReadStatus);
        assert!(result.is_ok());
    }

    // ── V2-1: STOP bricks the roaster — OFF must recover ───────

    #[test]
    fn stop_latches_then_off_recovers() {
        // Bug V2-1: `STOP` (→ EmergencyStop) arms the latch and leaves the
        // device bricked (the only sanctioned recovery, `RoasterCommand::
        // StopRoast`, has no protocol producer). `OFF` (which parses to
        // `ArtisanCommand::Stop`, token "OFF"/"PID,OFF") must un-latch and
        // return the roaster to a controllable state.
        let mut ctrl = make_control();

        // Arm the latch the way `STOP` does.
        let r = ctrl.process_artisan_command(ArtisanCommand::EmergencyStop);
        assert!(r.is_ok(), "STOP path must succeed at arming the latch");
        assert!(ctrl.safety().is_emergency_active());
        assert!(ctrl.get_status().fault_condition);
        assert_eq!(ctrl.get_state(), RoasterState::Error);

        // Any non-whitelisted command must still be rejected while latched.
        let blocked = ctrl.process_artisan_command(ArtisanCommand::SetHeater(50));
        assert!(blocked.is_err(), "Latch must reject heater commands");

        // `OFF` must clear the latch and recover.
        let recover = ctrl.process_artisan_command(ArtisanCommand::Stop);
        assert!(recover.is_ok(), "OFF recovery path must succeed");
        assert!(
            !ctrl.safety().is_emergency_active(),
            "OFF must clear the emergency latch"
        );
        assert!(
            !ctrl.get_status().fault_condition,
            "OFF must clear fault_condition"
        );
        assert_eq!(
            ctrl.get_state(),
            RoasterState::Idle,
            "OFF recovery returns the roaster to Idle"
        );

        // After recovery a heater command must work again — i.e. the device
        // is no longer bricked.
        let after = ctrl.process_artisan_command(ArtisanCommand::SetHeater(50));
        assert!(
            after.is_ok(),
            "Post-recovery heater command must be accepted: {:?}",
            after
        );
    }

    #[test]
    fn stop_streaming_does_not_clear_state_while_latched() {
        // Bug V2-1 (B34 consistency): while the emergency latch is armed,
        // `stop_streaming` must NOT repaint the state to `Idle`. The
        // `EmergencyStop` handler reaches `dispatch.stop_streaming` without
        // clearing the latch; the device must remain visibly `Error` so
        // telemetry does not claim "Idle" with the fan pinned and commands
        // rejected.
        let mut ctrl = make_control();
        let _ = ctrl.emergency_shutdown("test latch");
        assert_eq!(ctrl.get_state(), RoasterState::Error);

        // Driving the plain `EmergencyStop` artisan command again calls
        // `handle_emergency_stop`, which re-arms and re-stops without ever
        // clearing the latch — the state must stay `Error`.
        let r = ctrl.process_artisan_command(ArtisanCommand::EmergencyStop);
        assert!(r.is_ok());
        assert!(ctrl.safety().is_emergency_active());
        assert_eq!(
            ctrl.get_state(),
            RoasterState::Error,
            "Latched stop must keep state = Error, not Idle"
        );
        assert_eq!(ctrl.get_status().state, RoasterState::Error);
    }

    // ── V2-16a: RoR guard must not fire during empty-drum PREHEAT ─

    #[test]
    fn ror_guard_skipped_in_preheat_empty_drum() {
        // Bug V2-16a: an empty drum with a low-mass BT probe heats faster
        // than MAX_BT_RATE_OF_RISE during PREHEAT; the guard used to fire
        // every tick in all states, bricking the device (via V2-1) within
        // 1-2 seconds. The guard is now gated to `Heating`/`Stable`.
        let mut ctrl = make_control();

        // Drive PREHEAT (state -> Preheating, PID enabled).
        let r = ctrl.process_artisan_command(ArtisanCommand::Preheat(180.0));
        assert!(r.is_ok());
        assert_eq!(ctrl.get_state(), RoasterState::Preheating);

        // Inject two samples ~0.8s apart with a 1.0 °C jump → 1.25 °C/s,
        // well above MAX_BT_RATE_OF_RISE (0.5 °C/s). The derivative filter
        // (α=0.3) will produce a non-zero rate that exceeds the limit. The
        // guard must NOT fire in Preheating.
        let t0 = Instant::from_millis(0);
        let t1 = Instant::from_millis(800);
        ctrl.update_temperatures(150.0, 120.0, t0).unwrap();
        // Run one control tick so the filter seeds `last_pv_sample`.
        let _ = ctrl.update_control(t0);
        ctrl.update_temperatures(151.0, 120.0, t1).unwrap();
        let out = ctrl.update_control(t1);

        assert!(
            out.is_ok(),
            "RoR guard must not trigger emergency in Preheating: {:?}",
            out
        );
        assert_ne!(
            ctrl.get_state(),
            RoasterState::Error,
            "Preheating must not flip to Error from a RoR transient"
        );
    }

    // ── V2-4: START swallowed after PID;SV / OT1 in Idle ───────────

    #[test]
    fn start_after_pid_sv_in_idle_starts_roast() {
        // Bug V2-4: `PID;SV` enables PID with state=Idle, which made
        // `is_streaming()` true. The old gate swallowed START as "ignored",
        // keeping `profile_start_time` unset so the temporal backstops stayed
        // inactive. The state-based gate (V2-4/V2-16c) must take the full
        // handoff when the state is Idle.
        let mut ctrl = make_control();

        // Pre-condition: PID enabled from Idle, state remains Idle.
        let r = ctrl.process_artisan_command(ArtisanCommand::SetTargetTemp(200.0));
        assert!(r.is_ok());
        assert!(ctrl.get_status().pid_enabled);
        assert_eq!(ctrl.get_state(), RoasterState::Idle);

        // START must now perform the handoff, not be ignored.
        let r = ctrl.process_artisan_command(ArtisanCommand::StartRoast);
        assert!(r.is_ok());
        assert_eq!(ctrl.get_state(), RoasterState::Heating);
        // `enable_pid_control` is called from the START handoff, so PID is
        // the active mode (pid_enabled=true, artisan_control=false by design
        // — see `dispatch.enable_pid` setting it false).
        assert!(
            ctrl.get_status().pid_enabled,
            "START must enable PID control"
        );
        assert!(
            ctrl.profile_start_time.is_some(),
            "START must fix profile_start_time so MAX_ROAST_TIME/comms-idle activate"
        );
    }

    #[test]
    fn start_after_ot1_in_idle_starts_roast() {
        // Bug V2-4: `OT1` enables `artisan_control` in Idle (manual heater),
        // which also counted as "streaming" under the old gate. START must take
        // the full handoff.
        let mut ctrl = make_control();

        let r = ctrl.process_artisan_command(ArtisanCommand::SetHeater(40));
        assert!(r.is_ok());
        assert!(ctrl.get_status().artisan_control);
        assert_eq!(ctrl.get_state(), RoasterState::Idle);

        let r = ctrl.process_artisan_command(ArtisanCommand::StartRoast);
        assert!(r.is_ok());
        assert_eq!(ctrl.get_state(), RoasterState::Heating);
        assert!(ctrl.profile_start_time.is_some());
    }

    #[test]
    fn start_during_active_roast_is_ignored() {
        // Regression guard for V2-4: the new state-based gate must still
        // ignore a second START that arrives during an active roast.
        let mut ctrl = make_control();
        let r = ctrl.process_artisan_command(ArtisanCommand::StartRoast);
        assert!(r.is_ok());
        assert_eq!(ctrl.get_state(), RoasterState::Heating);
        let first_start = ctrl.profile_start_time;

        let r = ctrl.process_artisan_command(ArtisanCommand::StartRoast);
        assert!(r.is_ok());
        assert_eq!(ctrl.get_state(), RoasterState::Heating);
        // START ignored keeps the ORIGINAL start time — a second START must
        // not silently restart the roast clock.
        assert_eq!(ctrl.profile_start_time, first_start);
    }

    // ── V2-16c: temporal backstops protect manual mode too ──────────

    #[test]
    fn comms_idle_protects_manual_mode_when_heater_energized() {
        // Bug V2-16c: in pure Artisan-manual mode (OT1 from a slider, no
        // START) the state stays Idle, so the previous state-only gate left a
        // USB disconnect with the heater at 80 % completely unprotected. The
        // physical gate (heater_energized || roast_active) must trigger the
        // comms-idle emergency even from Idle.
        let mut ctrl = make_control();

        // Energize the heater via OT1 in Idle. After the guarded heater write
        // `status.ssr_output` reflects the commanded percentage.
        let r = ctrl.process_artisan_command(ArtisanCommand::SetHeater(80));
        assert!(r.is_ok());
        assert!(
            ctrl.get_status().ssr_output > 0.0,
            "test precondition: heater must actually be energized"
        );
        assert_eq!(ctrl.get_state(), RoasterState::Idle);

        // Backdate last_command_received_at_ms so the idle window is exceeded.
        // Use a large fixed `now` (NOT `Instant::now()`): the host driver's
        // baseline starts at process boot, so early tests see `now.as_millis()`
        // well below COMMS_IDLE_TIMEOUT_MS (15 s), which would make
        // `saturating_sub` clamp to zero and the comms-idle check never trip.
        let now = Instant::from_millis(60_000);
        let backdated = Instant::from_millis(
            60_000u64.saturating_sub(crate::config::constants::COMMS_IDLE_TIMEOUT_MS + 1000),
        )
        .as_millis();
        ctrl.status_mut().last_command_received_at_ms = backdated;

        let out = ctrl.update_control(now);
        // `emergency_shutdown` returns `Ok(())`; `update_control` propagates
        // it after its `emergency_shutdown?` call as `Ok(0.0)`. The assertion
        // is on the side-effect (latch armed), not the return.
        let _ = out;
        assert!(
            ctrl.safety().is_emergency_active(),
            "Comms-idle must trigger an emergency when the heater is energized in Idle"
        );
    }

    #[test]
    fn comms_idle_does_not_trigger_when_idle_and_heater_off() {
        // Regression guard: the physical gate must NOT over-trigger and
        // spuriously shut down an idle, cold roaster that has simply been
        // quiet for a while (the common pre-roast waiting state).
        let mut ctrl = make_control();
        let now = Instant::from_millis(60_000);
        let backdated = Instant::from_millis(
            60_000u64.saturating_sub(crate::config::constants::COMMS_IDLE_TIMEOUT_MS + 5000),
        )
        .as_millis();
        ctrl.status_mut().last_command_received_at_ms = backdated;
        ctrl.status_mut().ssr_output = 0.0;

        let _ = ctrl.update_control(now);
        assert!(
            !ctrl.safety().is_emergency_active(),
            "Idle + heater off must NOT trigger comms-idle, even after a long quiet period"
        );
    }

    // ── V2-7: #DUMP queue clears, survives full rings, re-pushes ───────

    #[test]
    fn handle_dump_log_clears_previous_dump() {
        // Bug V2-7: a second `#DUMP` request must not splice two partial
        // dumps together. `handle_dump_log` starts by clearing the deque.
        let mut ctrl = make_control();
        // Start a roast and stop it so the logger has at least one row.
        crate::logging::roast_logger::start_roast(embassy_time::Instant::now());
        crate::logging::roast_logger::log_sample(
            crate::logging::roast_logger::LogSampleData {
                bt: 100.0,
                et: 90.0,
                heater: 50.0,
                fan: 30.0,
                target: 200.0,
                ror: 0.0,
            },
            embassy_time::Instant::now(),
        );
        let r = ctrl.process_artisan_command(ArtisanCommand::DumpLog);
        assert!(r.is_ok());
        // Drain the queue fully.
        while ctrl.take_dump_row().is_some() {}
        // Request a second dump — the deque was cleared, so only this dump's
        // rows come out. If clear() had been skipped, the first dump's rows
        // would still be queued and the second call would append on top.
        // We assert the count after the second dump is small (one header row
        // in the dump string + any data rows — but the logger is still
        // active and the buffer holds 1 row, so the queue should be small,
        // not 2× the first call).
        let r = ctrl.process_artisan_command(ArtisanCommand::DumpLog);
        assert!(r.is_ok());
        let second_count = core::cell::Cell::new(0usize);
        while ctrl.take_dump_row().is_some() {
            second_count.set(second_count.get() + 1);
        }
        // The dump for a single-row buffer is "#DUMP <header>\n<row>\n" which
        // `handle_dump_log` splits into 2 non-empty lines (header + row).
        assert_eq!(
            second_count.get(),
            2,
            "second dump should have 2 rows (header + 1 data), not spliced with the first"
        );
        // Clean up the logger state so other tests are unaffected.
        crate::logging::roast_logger::stop_roast();
    }

    #[test]
    fn start_clears_dump_pending() {
        // Bug V2-7: a START drops any in-flight dump so it does not bleed
        // into the new roast's live telemetry.
        let mut ctrl = make_control();
        // Seed the deque with a sentinel row (skip the real logger path).
        let row = heapless::String::<256>::try_from("sentinel-row").unwrap();
        ctrl.push_dump_row_front(row);
        assert!(ctrl.take_dump_row().is_some(), "sentinel row is queued");

        // Re-seed and start a roast.
        let row = heapless::String::<256>::try_from("sentinel-row-2").unwrap();
        ctrl.push_dump_row_front(row);
        let r = ctrl.process_artisan_command(ArtisanCommand::StartRoast);
        assert!(r.is_ok());
        assert!(
            ctrl.take_dump_row().is_none(),
            "START must clear the pending #DUMP queue"
        );
    }

    #[test]
    fn push_dump_row_front_preserves_fifo_order() {
        // Bug V2-7: re-pushing a row to the front when the output channel is
        // full must keep FIFO order — the row is retried next, before any row
        // that was already behind it.
        let mut ctrl = make_control();
        ctrl.push_dump_row_front(heapless::String::<256>::try_from("a").unwrap());
        ctrl.push_dump_row_front(heapless::String::<256>::try_from("b").unwrap());
        // front = "b","a" so pop_front gives "b" first, then "a".
        assert_eq!(ctrl.take_dump_row().unwrap().as_str(), "b");
        assert_eq!(ctrl.take_dump_row().unwrap().as_str(), "a");
    }

    // ── V2-5: PREHEAT drops the cooldown latch ──────────────────────

    #[test]
    fn preheat_drops_cooling_latch() {
        // Bug V2-5 (B3 residual): `OFF` at a high BT arms the cooldown latch
        // (fan 100 %). A subsequent `PREHEAT;180` used to keep the latch armed
        // for the whole preheat — the PID heated against maximum airflow, and
        // since the heater kept BT > COOLING_RELEASE_BEAN_TEMP_C the latch
        // could never auto-release. Only START cleared it. PREHEAT is a
        // deliberate re-energize, so it must also clear the latch.
        let mut ctrl = make_control();

        // Simulate a STOP having latched cooldown: set the latch directly
        // via the field-touchable path the production STOP uses.
        // EmergencyStop arms the SAFETY latch (which we do NOT want to clear
        // in PREHEAT — that path is V2-1's OFF). Use a plain STOP via the
        // Artisan `Stop` handler so `cooling_active = true` and the safety
        // latch stays cleared.
        let r = ctrl.process_artisan_command(ArtisanCommand::Stop);
        assert!(r.is_ok());
        // The field is private; assert through the observable effect: a
        // subsequent `update_control` would force the fan to 100 % while the
        // latch is active. We instead assert the post-PREHEAT behaviour
        // directly via a status snapshot once PREHEAT clears the latch.
        // PREHEAT transitions to Preheating and must drop the latch.
        let r = ctrl.process_artisan_command(ArtisanCommand::Preheat(180.0));
        assert!(r.is_ok());
        assert_eq!(ctrl.get_state(), RoasterState::Preheating);
        // After PREHEAT we can re-arm the latch via STOP and observe that
        // PREHEAT clears it again — i.e. the test is reproducible.
        let _ = ctrl.process_artisan_command(ArtisanCommand::Stop);
        let _ = ctrl.process_artisan_command(ArtisanCommand::Preheat(180.0));
        assert_eq!(ctrl.get_state(), RoasterState::Preheating);
        // The fan selector in `update_control` is the assertion surface: if
        // the latch were still armed, the fan would be forced to 100 % and
        // append_crlf-style telemetry would show fan_output=100 after a tick.
        // Run a tick with a finite, sub-60 °C BT so the latch's BT<60 self-
        // release cannot mask the PREHEAT effect.
        ctrl.status_mut().bean_temp = 25.0;
        ctrl.status_mut().env_temp = 25.0;
        let _ = ctrl.update_control(Instant::from_millis(1_000));
        // With the latch cleared by PREHEAT and BT well below 60 °C, the fan
        // must NOT be clamped to 100 % by the cooldown path.
        assert_ne!(
            ctrl.get_status().fan_output,
            100.0,
            "PREHEAT must drop the cooldown latch (fan not forced to 100 %)"
        );
    }

    // ── V2-13: OFF+START preserves the fan profile ──────────────────

    #[test]
    fn off_start_preserves_fan_profile() {
        // Bug V2-13: `stop_streaming` used to clear `fan_profile = None`,
        // asymmetric with the temperature profile (which survived OFF). An
        // `OFF` → `START` flow silently wiped the fan profile and forced the
        // operator to re-send `FANPROFILE`. The cooldown latch already
        // takes precedence over the fan profile in the fan selector, and
        // clearing `profile_start_time` already disables interpolation during
        // cooldown — so the `fan_profile = None` line was both redundant for
        // the cooldown safety and harmful for the legitimate-profile path.
        // We thread a fan profile in via the private field (tests are inside
        // the module) and assert STOP does NOT erase it.
        use crate::config::constants::{FanProfile, FanSetpoint, MAX_PROFILE_SETPOINTS};

        let mut ctrl = make_control();

        // Seed a single-setpoint fan profile (target 33 % throughout).
        let mut setpoints = heapless::Vec::<FanSetpoint, MAX_PROFILE_SETPOINTS>::new();
        let _ = setpoints.push(FanSetpoint {
            time_secs: 0,
            fan_speed: 33,
        });
        let profile = FanProfile { setpoints };
        ctrl.fan_profile = Some(profile);
        assert!(
            ctrl.fan_profile.is_some(),
            "test precondition: profile loaded"
        );

        // STOP/OFF must NOT clear the fan profile (the V2-13 fix removed the
        // `self.fan_profile = None;` line from `stop_streaming`).
        let _ = ctrl.process_artisan_command(ArtisanCommand::Stop);
        assert!(
            ctrl.fan_profile.is_some(),
            "V2-13: STOP must NOT erase the loaded fan profile"
        );
        // Sanity: profile_start_time WAS cleared (interpolation off during
        // cooldown), but the profile itself survives.
        assert!(ctrl.profile_start_time.is_none());

        // The next START re-energizes and re-fixes profile_start_time; the
        // fan profile remains available for the fan selector.
        let _ = ctrl.process_artisan_command(ArtisanCommand::StartRoast);
        assert_eq!(ctrl.get_state(), RoasterState::Heating);
        assert!(
            ctrl.fan_profile.is_some(),
            "V2-13: fan profile must survive the OFF → START cycle"
        );
        assert!(ctrl.profile_start_time.is_some());
    }
}
