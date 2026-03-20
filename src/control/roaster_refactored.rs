use super::policies::{
    ManualCommandPolicy, ManualPolicyOutcome, SafetyPolicy, SafetyPolicyOutcome,
};
use super::{RoasterCommandHandler, RoasterError};
use crate::config::*;
use crate::control::handlers::{
    ArtisanCommandHandler, SafetyCommandHandler, SystemCommandHandler, TemperatureCommandHandler,
};
use crate::control::pid::PidFeedback;
use crate::control::traits::{Fan, Heater};
use crate::control::SsrCycleGuard;
use alloc::boxed::Box;
use embassy_time::{Duration, Instant};

use log::{debug, error, info, warn};

use crate::hardware::sensors::{SensorConversionHub, SensorSample};

const DERIVATIVE_FILTER_ALPHA: f32 = 0.3;

/// RoasterControl - uses concrete Max31856 types for sensor storage
/// This enables calling async temperature methods without blocking the executor
pub struct RoasterControl {
    state: RoasterState,
    status: SystemStatus,
    ssr_guard: SsrCycleGuard,
    last_temp_read: Option<Instant>,
    last_pid_update: Option<Instant>,
    last_desired_output: f32,
    last_pv_sample: Option<f32>,
    last_pv_sample_time: Option<Instant>,
    last_filtered_derivative: f32,

    heater: Box<dyn Heater + Send>,
    fan: Box<dyn Fan + Send>,
    sensor_hub: SensorConversionHub,

    temp_handler: TemperatureCommandHandler,

    safety_handler: SafetyCommandHandler,
    artisan_handler: ArtisanCommandHandler,
    system_handler: SystemCommandHandler,

    /// Temperature scale preference storage
    /// Tracks UNITS command preference (Celsius/Fahrenheit) without conversion
    #[allow(dead_code)]
    temp_settings: TemperatureSettings,
}

impl RoasterControl {
    #[cfg(target_arch = "riscv32")]
    pub fn new(
        heater: Box<dyn Heater + Send>,
        fan: Box<dyn Fan + Send>,
        sensor_hub: SensorConversionHub,
    ) -> Result<Self, RoasterError> {
        let temp_handler = TemperatureCommandHandler::new()?;

        Ok(RoasterControl {
            state: RoasterState::Idle,
            status: SystemStatus::default(),
            ssr_guard: SsrCycleGuard::new(),
            last_temp_read: None,
            last_pid_update: None,
            last_desired_output: 0.0,
            last_pv_sample: None,
            last_pv_sample_time: None,
            last_filtered_derivative: 0.0,
            heater,
            fan,
            sensor_hub,
            temp_handler,
            safety_handler: SafetyCommandHandler::new(),
            artisan_handler: ArtisanCommandHandler::new(),
            system_handler: SystemCommandHandler,
            temp_settings: TemperatureSettings::new(),
        })
    }

    #[cfg(not(target_arch = "riscv32"))]
    pub fn new(
        heater: Box<dyn Heater + Send>,
        fan: Box<dyn Fan + Send>,
        sensor_hub: SensorConversionHub,
    ) -> Result<Self, RoasterError> {
        let temp_handler = TemperatureCommandHandler::new()?;

        Ok(RoasterControl {
            state: RoasterState::Idle,
            status: SystemStatus::default(),
            ssr_guard: SsrCycleGuard::new(),
            last_temp_read: None,
            last_pid_update: None,
            last_desired_output: 0.0,
            last_pv_sample: None,
            last_pv_sample_time: None,
            last_filtered_derivative: 0.0,
            heater,
            fan,
            sensor_hub,
            temp_handler,
            safety_handler: SafetyCommandHandler::new(),
            artisan_handler: ArtisanCommandHandler::new(),
            system_handler: SystemCommandHandler,
            temp_settings: TemperatureSettings::new(),
        })
    }

    /// Async sensor reading - uses the shared conversion helper so every consumer sees the same math
    #[cfg(target_arch = "riscv32")]
    pub async fn read_sensors(&mut self) -> Result<(), RoasterError> {
        let sample = self.sensor_hub.sample().await?;
        let has_fault = sample.bean_fault.has_fault() || sample.env_fault.has_fault();
        if has_fault {
            self.status.fault_condition = true;
        }

        self.update_temperatures(sample.bean_temp, sample.env_temp, sample.timestamp)
    }

    /// Async sensor reading - stub for non-riscv32 targets
    #[cfg(not(target_arch = "riscv32"))]
    pub async fn read_sensors(&mut self) -> Result<(), RoasterError> {
        let sample = self.sensor_hub.sample().await?;
        let has_fault = sample.bean_fault.has_fault() || sample.env_fault.has_fault();
        if has_fault {
            self.status.fault_condition = true;
        }

        self.update_temperatures(sample.bean_temp, sample.env_temp, sample.timestamp)
    }

    pub fn get_status(&self) -> SystemStatus {
        self.status
    }

    pub fn status_mut(&mut self) -> &mut SystemStatus {
        &mut self.status
    }

    pub fn last_sensor_sample(&self) -> Option<SensorSample> {
        self.sensor_hub.last_sample()
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
        if !Self::is_temperature_valid(bean_temp) || !Self::is_temperature_valid(env_temp) {
            return Err(RoasterError::TemperatureOutOfRange {
                source: Some("temperature_out_of_valid_range"),
            });
        }

        self.status.bean_temp = bean_temp + BT_THERMOCOUPLE_OFFSET;
        self.status.env_temp = env_temp + ET_THERMOCOUPLE_OFFSET;
        self.last_temp_read = Some(current_time);

        // Check for emergency conditions
        if self.status.bean_temp >= OVERTEMP_THRESHOLD {
            self.emergency_shutdown("Over-temperature detected")?;
        }

        Ok(())
    }

    fn refresh_filtered_derivative(&mut self, current_pv: f32, current_time: Instant) {
        let mut derivative_rate = 0.0;
        let mut has_valid_rate = false;

        if let (Some(prev_pv), Some(prev_time)) = (self.last_pv_sample, self.last_pv_sample_time) {
            let duration = current_time.duration_since(prev_time);
            let dt_secs = duration.as_micros() as f32 * 1e-6;
            if dt_secs > 0.0 {
                let delta_temp = current_pv - prev_pv;
                if delta_temp.is_finite() {
                    let instantaneous_rate = delta_temp / dt_secs;
                    if instantaneous_rate.is_finite() {
                        derivative_rate = DERIVATIVE_FILTER_ALPHA * instantaneous_rate
                            + (1.0 - DERIVATIVE_FILTER_ALPHA) * self.last_filtered_derivative;
                        if derivative_rate.is_finite() {
                            has_valid_rate = true;
                            self.last_filtered_derivative = derivative_rate;
                        }
                    }
                }
            }
        }

        if has_valid_rate {
            self.status.derivative_rate = derivative_rate;
            self.status.derivative_available = true;
        } else {
            self.status.derivative_rate = 0.0;
            self.status.derivative_available = false;
        }

        self.last_pv_sample = Some(current_pv);
        self.last_pv_sample_time = Some(current_time);
    }

    pub fn mark_overtemp_regression_active(&mut self, active: bool) {
        self.status.overtemp_regression_active = active;
    }

    pub fn process_command(
        &mut self,
        command: RoasterCommand,
        current_time: Instant,
    ) -> Result<(), RoasterError> {
        if matches!(command, RoasterCommand::StopRoast) {
            return self.stop_streaming();
        }

        // Try policy-based handling first (new pattern)
        // Safety policy evaluation
        if SafetyPolicy::can_handle(&self.safety_handler, command) {
            let outcome = self.safety_handler.evaluate(command, &mut self.status);
            self.status.fault_condition = outcome.emergency_active;

            if outcome.emergency_active {
                // Apply hardware writes for emergency
                self.apply_safety_outcome(&outcome, current_time)?;
                return Err(RoasterError::TemperatureOutOfRange {
                    source: Some("emergency_shutdown"),
                });
            }
            return Ok(());
        }

        // Manual command policy evaluation
        if ManualCommandPolicy::can_handle(&self.artisan_handler, command) {
            let outcome = self.artisan_handler.evaluate(command, &mut self.status);

            if outcome.success {
                // RoasterControl is the single writer - apply hardware after policy evaluation
                self.apply_policy_outcome(&outcome, current_time)?;
                return Ok(());
            } else {
                return Err(RoasterError::InvalidState {
                    source: Some("manual_command_failed"),
                });
            }
        }

        // Fall back to legacy handler for non-policy commands (TemperatureHandler, SystemHandler)
        let mut handlers: [&mut dyn RoasterCommandHandler; 2] =
            [&mut self.temp_handler, &mut self.system_handler];

        for handler in &mut handlers {
            if handler.can_handle(command) {
                return handler.handle_command(command, current_time, &mut self.status);
            }
        }

        warn!("No handler found for command: {:?}", command);
        Err(RoasterError::InvalidState {
            source: Some("no_handler_found"),
        })
    }

    /// Apply policy outcome to hardware - RoasterControl is the single writer
    fn apply_policy_outcome(
        &mut self,
        outcome: &ManualPolicyOutcome,
        current_time: Instant,
    ) -> Result<(), RoasterError> {
        // Log policy input for instrumentation
        debug!(
            "Policy outcome: heater={:?}, fan={:?}, pid={:?}, artisan={:?}",
            outcome.heater_target, outcome.fan_target, outcome.pid_enabled, outcome.artisan_control
        );

        // Apply heater if specified
        if let Some(heater) = outcome.heater_target {
            self.temp_handler.disable_pid();
            self.status.pid_enabled = false;
            self.status.artisan_control = true;
            self.temp_handler
                .get_output_manager_mut()
                .enable_continuous_output();

            self.apply_guarded_heater(heater, current_time, true)?;
            self.status.ssr_hardware_status = self.heater.get_status();
        }

        // Apply fan if specified
        if let Some(fan) = outcome.fan_target {
            self.status.artisan_control = true;
            self.status.pid_enabled = false;
            self.temp_handler
                .get_output_manager_mut()
                .enable_continuous_output();

            self.fan.set_speed(fan)?;
            self.status.fan_output = fan;
            self.status.ssr_hardware_status = self.heater.get_status();
        }

        Ok(())
    }

    /// Apply safety outcome to hardware - RoasterControl is the single writer
    fn apply_safety_outcome(
        &mut self,
        outcome: &SafetyPolicyOutcome,
        _current_time: Instant,
    ) -> Result<(), RoasterError> {
        // Log safety policy for instrumentation
        warn!(
            "Safety outcome: emergency={}, fault={}, reason={:?}",
            outcome.emergency_active, outcome.fault_condition, outcome.reason
        );

        if outcome.zero_ssr {
            let _ = self.heater.set_power(0.0);
            self.capture_ssr_monitor_metrics();
        }

        if outcome.disable_pid {
            self.temp_handler.disable_pid();
            self.status.pid_enabled = false;
        }

        Ok(())
    }

    fn is_streaming(&self) -> bool {
        self.temp_handler
            .get_output_manager()
            .is_continuous_enabled()
            || self.status.pid_enabled
            || self.status.artisan_control
    }

    fn stop_streaming(&mut self) -> Result<(), RoasterError> {
        self.temp_handler
            .get_output_manager_mut()
            .disable_continuous_output();
        self.temp_handler.disable_pid();
        self.status.pid_enabled = false;
        self.status.artisan_control = false;
        self.artisan_handler.clear_manual();
        self.status.ssr_output = 0.0;
        self.status.fan_output = 0.0;
        self.state = crate::config::constants::RoasterState::Idle;
        self.status.state = self.state;
        self.status.ssr_cycle_guard_busy_until_ms = 0;

        if !self.safety_handler.is_emergency_active() {
            self.status.fault_condition = false;
        }

        self.capture_ssr_monitor_metrics();
        self.heater.set_power(0.0)?;
        self.fan.set_speed(0.0)?;

        self.status.ssr_hardware_status = self.heater.get_status();

        Ok(())
    }

    pub fn is_temperature_valid(temp: f32) -> bool {
        temp >= MIN_VALID_TEMP && temp <= MAX_VALID_TEMP
    }

    pub fn emergency_shutdown(&mut self, reason: &str) -> Result<(), RoasterError> {
        error!("Emergency shutdown: {}", reason);
        self.status.state = crate::config::constants::RoasterState::Error;
        self.status.ssr_output = 0.0;
        self.status.ssr_cycle_guard_busy_until_ms = 0;

        let _ = self.heater.set_power(0.0);
        self.capture_ssr_monitor_metrics();
        let _ = self.fan.set_speed(100.0);

        Err(RoasterError::EmergencyShutdown {
            source: Some("emergency_shutdown"),
        })
    }

    pub fn update_control(&mut self, current_time: Instant) -> Result<f32, RoasterError> {
        if let Some(last_read) = self.last_temp_read {
            if current_time.duration_since(last_read)
                > Duration::from_millis(TEMP_VALIDITY_TIMEOUT_MS as u64)
            {
                warn!("Temperature sensor timeout detected");
                self.emergency_shutdown("Temperature sensor timeout")?;
            }
        }

        self.status.ssr_hardware_status = self.heater.get_status();

        let current_pv = self.status.bean_temp;
        self.status.pv = current_pv;
        self.refresh_filtered_derivative(current_pv, current_time);

        let desired_output = if self.safety_handler.is_emergency_active() {
            debug!("Emergency active - forcing SSR output to 0%");
            0.0
        } else if self.status.artisan_control {
            let manual_output = self.artisan_handler.get_manual_heater();
            debug!(
                "Artisan+ control - manual heater output: {:.1}%",
                manual_output
            );
            manual_output
        } else if self.status.pid_enabled {
            if self.status.ssr_hardware_status
                == crate::config::constants::SsrHardwareStatus::Available
            {
                self.update_pid_control(current_time)
            } else {
                warn!("PID enabled but SSR not available - output: 0%");
                0.0
            }
        } else {
            0.0
        };

        self.last_desired_output = desired_output;
        let pid_integrator_value = self.temp_handler.pid_integrator_value();
        let guard_busy = self.ssr_guard.next_cycle_allowed(current_time).is_err();
        let applied_output = self.apply_guarded_heater(desired_output, current_time, false)?;
        let feedback = PidFeedback::new(desired_output, applied_output, guard_busy);
        self.temp_handler.set_pid_feedback(feedback);

        self.status.integrator_value = pid_integrator_value;
        self.status.mv = applied_output;
        self.status.saturation_active = self.temp_handler.pid_saturation_active();
        self.status.integrator_clamped = self.temp_handler.pid_integrator_clamped();

        let fan_output = self.artisan_handler.get_manual_fan();
        self.fan
            .set_speed(fan_output)
            .map_err(|_| RoasterError::HardwareError {
                source: Some("fan_set_in_control_loop_failed"),
            })?;

        // Keep status aligned with commanded fan output in control loop.
        self.status.fan_output = fan_output;

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

    fn busy_window_ms(now: Instant, busy_until: Instant) -> u64 {
        if busy_until > now {
            busy_until.duration_since(now).as_millis()
        } else {
            0
        }
    }

    fn update_guard_busy_ms(&mut self, now: Instant) {
        let busy_until = self.ssr_guard.busy_until();
        self.status.ssr_cycle_guard_busy_until_ms = Self::busy_window_ms(now, busy_until);
    }

    fn capture_ssr_monitor_metrics(&mut self) {
        self.status.ssr_last_duty_delta_ticks = self.heater.last_duty_delta_ticks();
        self.status.ssr_retry_count = self.heater.last_retry_count();

        if self.status.ssr_last_duty_delta_ticks != 0 || self.status.ssr_retry_count != 0 {
            info!(
                "SSR monitor delta {} ticks, retries {}",
                self.status.ssr_last_duty_delta_ticks, self.status.ssr_retry_count
            );
        }
    }

    fn apply_guarded_heater(
        &mut self,
        desired: f32,
        now: Instant,
        reject_on_busy: bool,
    ) -> Result<f32, RoasterError> {
        let clamped = desired.clamp(0.0, 100.0);
        self.update_guard_busy_ms(now);

        if clamped <= 0.0 {
            let power_result = self.heater.set_power(0.0);
            self.capture_ssr_monitor_metrics();
            power_result?;
            self.status.ssr_output = 0.0;
            self.status.saturation_active = false;
            self.status.integrator_clamped = false;
            self.update_guard_busy_ms(now);
            return Ok(0.0);
        }

        match self.ssr_guard.next_cycle_allowed(now) {
            Ok(_) => {
                self.ssr_guard.mark_cycle(now);
                let power_result = self.heater.set_power(clamped);
                self.capture_ssr_monitor_metrics();
                power_result?;
                self.status.ssr_output = clamped;
                self.status.saturation_active = false;
                self.status.integrator_clamped = false;
                self.update_guard_busy_ms(now);
                Ok(clamped)
            }
            Err(busy_until) => {
                self.status.saturation_active = true;
                self.status.integrator_clamped = true;
                self.status.ssr_cycle_guard_busy_until_ms = Self::busy_window_ms(now, busy_until);
                warn!("SSR cycle busy until {:?}", busy_until);
                if reject_on_busy {
                    Err(RoasterError::InvalidState {
                        source: Some("ssr_cycle_busy"),
                    })
                } else {
                    Ok(self.status.ssr_output)
                }
            }
        }
    }

    pub async fn process_output(&mut self) -> Result<(), RoasterError> {
        if let Err(e) = self
            .temp_handler
            .get_output_manager_mut()
            .process_status(&self.status)
            .await
        {
            warn!("Output error: {:?}", e);
        }
        Ok(())
    }

    pub fn get_output_manager(&self) -> &crate::control::OutputController {
        self.temp_handler.get_output_manager()
    }

    pub fn get_output_manager_mut(&mut self) -> &mut crate::control::OutputController {
        self.temp_handler.get_output_manager_mut()
    }

    pub fn process_artisan_command(
        &mut self,
        command: crate::config::ArtisanCommand,
    ) -> Result<(), RoasterError> {
        use crate::config::constants::DEFAULT_TARGET_TEMP;
        let current_time = embassy_time::Instant::now();

        match command {
            crate::config::ArtisanCommand::StartRoast => {
                if self.is_streaming() {
                    info!("Artisan+ START ignored - streaming already active");
                    self.status.ssr_hardware_status = self.heater.get_status();
                } else {
                    self.status.artisan_control = true;
                    self.enable_pid_control(DEFAULT_TARGET_TEMP)?;
                    self.temp_handler
                        .get_output_manager_mut()
                        .enable_continuous_output();

                    // Actualizar estado hardware
                    self.status.ssr_hardware_status = self.heater.get_status();
                    self.state = crate::config::constants::RoasterState::Heating;
                    self.status.state = self.state;

                    info!(
                        "Artisan+ roast started with target {:.1}°C - SSR: {:?}",
                        DEFAULT_TARGET_TEMP, self.status.ssr_hardware_status
                    );
                }
            }

            crate::config::ArtisanCommand::SetHeater(value) => {
                self.forward_artisan_manual_command(
                    crate::config::RoasterCommand::SetHeaterManual(value),
                    current_time,
                )?;
                info!("Artisan+ heater command processed: {}%", value);
            }

            crate::config::ArtisanCommand::SetFan(value) => {
                self.forward_artisan_manual_command(
                    crate::config::RoasterCommand::SetFanManual(value),
                    current_time,
                )?;

                info!("Artisan+ fan command processed: {}%", value);
            }

            crate::config::ArtisanCommand::SetFanSpeed(value, was_clamped) => {
                self.forward_artisan_manual_command(
                    crate::config::RoasterCommand::SetFanManual(value),
                    current_time,
                )?;

                if was_clamped {
                    // Out of range value - stop heater as safety measure
                    let _ = self.heater.set_power(0.0);
                    self.capture_ssr_monitor_metrics();
                    info!(
                        "Artisan+ OT2 out of range - heater stopped, fan set to {}%",
                        value
                    );
                } else {
                    info!("Artisan+ OT2 fan command processed: {}%", value);
                }
            }

            crate::config::ArtisanCommand::EmergencyStop => {
                self.stop_streaming()?;
                info!("Artisan+ stop requested - streaming disabled and outputs cleared");
            }

            crate::config::ArtisanCommand::IncreaseHeater => {
                self.forward_artisan_manual_command(
                    crate::config::RoasterCommand::IncreaseHeater,
                    current_time,
                )?;
                info!("Artisan+ UP command processed");
            }

            crate::config::ArtisanCommand::DecreaseHeater => {
                self.forward_artisan_manual_command(
                    crate::config::RoasterCommand::DecreaseHeater,
                    current_time,
                )?;
                info!("Artisan+ DOWN command processed");
            }

            crate::config::ArtisanCommand::StatusReport => {
                self.status.ssr_hardware_status = self.heater.get_status();

                let response =
                    crate::output::artisan::ArtisanFormatter::format_status_response(&self.status);

                debug!(
                    "STATUS command - SSR status: {:?}, response generated",
                    self.status.ssr_hardware_status
                );
                debug!("STATUS payload: {}", response);
            }

            crate::config::ArtisanCommand::ReadStatus => {
                self.status.ssr_hardware_status = self.heater.get_status();

                let response = crate::output::artisan::ArtisanFormatter::format_read_response_full(
                    &self.status,
                );

                // Validate response has 4 comma-separated values
                let parts: heapless::Vec<&str, 8> = response.split(',').collect();
                if response.trim().is_empty() || parts.len() != 4 {
                    error!(
                        "Malformed READ response from ArtisanFormatter: expected 4 values, got {}",
                        parts.len()
                    );
                }

                debug!(
                    "READ command - SSR status: {:?}, response generated",
                    self.status.ssr_hardware_status
                );
            }

            crate::config::ArtisanCommand::Chan(_) => {
                debug!("Chan command received - initialization handled by multiplexer");
            }
            crate::config::ArtisanCommand::RunRegression => {
                info!("Artisan regression command received");
            }
            crate::config::ArtisanCommand::Units(is_fahrenheit) => {
                // Convert to RoasterCommand and forward through policy
                let cmd = crate::config::RoasterCommand::SetUnits(is_fahrenheit);
                return self.forward_artisan_manual_command(cmd, current_time);
            }
            crate::config::ArtisanCommand::Filt(_) => {
                debug!("Filt command received - initialization handled by multiplexer");
            }
        }

        Ok(())
    }

    fn forward_artisan_manual_command(
        &mut self,
        command: crate::config::RoasterCommand,
        current_time: Instant,
    ) -> Result<(), RoasterError> {
        self.process_command(command, current_time)
    }

    pub fn enable_pid_control(&mut self, target_temp: f32) -> Result<(), RoasterError> {
        self.status.artisan_control = false;
        self.temp_handler.set_pid_target(target_temp)?;
        self.temp_handler.enable_pid();
        self.status.pid_enabled = true;
        self.status.target_temp = target_temp;

        info!("PID control re-enabled with target: {:.1}°C", target_temp);

        Ok(())
    }

    pub fn get_fan_speed(&self) -> f32 {
        self.status.fan_output
    }

    pub fn last_desired_heater_output(&self) -> f32 {
        self.last_desired_output
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

            let output = self
                .temp_handler
                .get_pid_output(self.status.bean_temp, current_time);

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
}
