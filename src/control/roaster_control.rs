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
        })
    }

    #[cfg(target_arch = "riscv32")]
    pub async fn read_sensors(&mut self) -> Result<(), RoasterError> {
        match self.sensor.read_sensors(&mut self.status).await {
            Err(RoasterError::TemperatureOutOfRange {
                source: Some("overtemp_detected"),
            }) => {
                self.actuator
                    .emergency_shutdown("Over-temperature detected", &mut self.status)?;
                Err(RoasterError::TemperatureOutOfRange {
                    source: Some("overtemp_detected"),
                })
            }
            other => other,
        }
    }

    #[cfg(not(target_arch = "riscv32"))]
    pub async fn read_sensors(&mut self) -> Result<(), RoasterError> {
        match self.sensor.read_sensors(&mut self.status).await {
            Err(RoasterError::TemperatureOutOfRange {
                source: Some("overtemp_detected"),
            }) => {
                self.actuator
                    .emergency_shutdown("Over-temperature detected", &mut self.status)?;
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
        match self.sensor.update_temperatures(
            bean_temp,
            env_temp,
            no_fault,
            no_fault,
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
            return self.stop_streaming();
        }

        if self.safety.can_handle(command) {
            let outcome = self.safety.evaluate(command, &mut self.status);
            self.status.fault_condition = outcome.emergency_active;

            if outcome.emergency_active {
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
            self.status.artisan_control = true;
            self.status.pid_enabled = false;
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

    fn is_streaming(&self) -> bool {
        self.dispatch.is_streaming(&self.status)
    }

    fn stop_streaming(&mut self) -> Result<(), RoasterError> {
        self.dispatch.stop_streaming(&mut self.status);
        self.state = crate::config::constants::RoasterState::Idle;
        self.status.state = self.state;

        if !self.safety.is_emergency_active() {
            self.status.fault_condition = false;
        }

        self.actuator.capture_ssr_monitor_metrics(&mut self.status);
        self.actuator.set_heater_power(0.0)?;
        // Bug #13: Set fan to 100% for cooling during stop (matches README and emergency_shutdown)
        self.actuator.set_fan_raw(100.0)?;
        self.status.fan_output = 100.0;

        self.status.ssr_hardware_status = self.actuator.get_ssr_hardware_status();

        Ok(())
    }

    pub fn is_temperature_valid(temp: f32) -> bool {
        SensorController::is_temperature_valid(temp)
    }

    pub fn emergency_shutdown(&mut self, reason: &str) -> Result<(), RoasterError> {
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

        // Maximum roast time safety backstop
        if matches!(self.state, RoasterState::Heating | RoasterState::Stable) {
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

        // Charge detection: detect bean drop via sharp BT decline
        if self.state == RoasterState::Heating && !self.charge_detected {
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
                        let output_channel = crate::application::service_container::ServiceContainer::get_output_channel();
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

        let current_pv = if self.status.pid_channel == 1 {
            self.status.env_temp
        } else {
            self.status.bean_temp
        };
        self.status.pv = current_pv;

        // Reject NaN / infinite PV (faulted sensor) — force heater off
        if !current_pv.is_finite() && self.status.pid_enabled {
            warn!("PID input NaN/infinite (sensor fault) — forcing heater to 0%");
            self.actuator.set_heater_power(0.0)?;
            self.status.ssr_output = 0.0;
            self.status.pid_enabled = false;
            return Ok(0.0);
        }
        self.sensor
            .refresh_filtered_derivative(current_pv, current_time, &mut self.status);

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
        self.status.mv = applied_output;
        self.status.saturation_active = self.dispatch.pid_saturation_active();
        self.status.integrator_clamped = self.dispatch.pid_integrator_clamped();

        let fan_output =
            if let (Some(ref fp), Some(start)) = (&self.fan_profile, self.profile_start_time) {
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
            crate::config::ArtisanCommand::Stop => self.handle_stop(),
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
        if self.is_streaming() {
            info!("Artisan+ START ignored - streaming already active");
            self.status.ssr_hardware_status = self.actuator.get_ssr_hardware_status();
        } else {
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
        self.forward_artisan_manual_command(
            crate::config::RoasterCommand::SetFanManual(value),
            current_time,
        )?;

        if was_clamped {
            let _ = self.actuator.set_heater_power(0.0);
            self.actuator.capture_ssr_monitor_metrics(&mut self.status);
            // Bug #2: Send notification to Artisan via output channel
            self.send_ot2_clamped_notification(value);
            info!(
                "Artisan+ OT2 out of range - heater stopped, fan set to {}%",
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
        self.safety.activate_emergency();
        self.status.fault_condition = true;
        self.stop_streaming()?;
        crate::logging::roast_logger::stop_roast();
        info!("Artisan+ stop requested - streaming disabled and outputs cleared");
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

        let parts: heapless::Vec<&str, 8> = response.split(',').collect();
        if response.trim().is_empty() || parts.len() != 5 {
            error!(
                "Malformed READ response from ArtisanFormatter: expected 5 values, got {}",
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
        if !crate::config::constants::is_valid_target_temp(target) {
            warn!(
                "SetTargetTemp rejected: {:.1}°C outside valid range ({:.0}–{:.0}°C)",
                target,
                crate::config::constants::MIN_TEMP,
                crate::config::constants::MAX_TEMP
            );
            return Err(RoasterError::InvalidState {
                source: Some("target_temp_out_of_range"),
            });
        }
        self.status.target_temp = target;
        self.enable_pid_control(target)?;
        info!("Target temperature set to {:.1}°C", target);
        Ok(())
    }

    fn handle_set_profile(&mut self) -> Result<(), RoasterError> {
        let taken = crate::input::parser::take_profile();
        if let Some(profile) = taken {
            for sp in &profile.setpoints {
                if !crate::config::constants::is_valid_target_temp(sp.temperature) {
                    warn!(
                        "Profile rejected: setpoint {:.1}°C at {}s outside valid range ({:.0}–{:.0}°C)",
                        sp.temperature, sp.time_secs,
                        crate::config::constants::MIN_TEMP, crate::config::constants::MAX_TEMP
                    );
                    return Err(RoasterError::InvalidState {
                        source: Some("profile_temp_out_of_range"),
                    });
                }
            }
            let count = profile.setpoints.len();
            self.active_profile = Some(profile);
            info!("Profile loaded: {} setpoints", count);
        } else {
            warn!("SetProfile received but no profile data in parser buffer");
        }
        Ok(())
    }

    fn handle_dump_log(&mut self) -> Result<(), RoasterError> {
        use crate::logging::traceability::TRACE_EVENT_MAX_LEN;
        let dump = crate::logging::roast_logger::dump();
        let output_channel =
            crate::application::service_container::ServiceContainer::get_output_channel();
        for line in dump.split('\n') {
            if !line.is_empty() {
                if let Ok(msg) = heapless::String::<TRACE_EVENT_MAX_LEN>::try_from(line) {
                    let _ = output_channel.try_send(msg);
                }
            }
        }
        info!("Roast log dump requested");
        Ok(())
    }

    fn handle_preheat(&mut self, target: f32) -> Result<(), RoasterError> {
        self.preheat_target = Some(target);
        self.state = RoasterState::Preheating;
        self.status.state = RoasterState::Preheating;
        self.enable_pid_control(target)?;
        self.dispatch
            .get_output_manager_mut()
            .disable_continuous_output();
        info!("Preheat started — target {:.1}°C", target);
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
        self.status.pid_output_min = min;
        self.status.pid_output_max = max;
        info!("PID output limits set to {:.1}% – {:.1}%", min, max);
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
    /// is aware the heater was cut due to out-of-range fan values.
    fn send_ot2_clamped_notification(&self, fan_value: u8) {
        use crate::logging::traceability::TRACE_EVENT_MAX_LEN;
        let mut msg = heapless::String::<{ TRACE_EVENT_MAX_LEN }>::new();
        let _ = core::fmt::Write::write_fmt(
            &mut msg,
            core::format_args!("ERR OT2_CLAMPED heater_cut fan={}", fan_value),
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
