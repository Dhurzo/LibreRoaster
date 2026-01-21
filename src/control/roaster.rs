use crate::config::*;
use crate::control::pid::CoffeeRoasterPid;
use crate::output::OutputManager;
use embassy_time::{Duration, Instant};
use log::{info, warn};

#[derive(Debug)]
pub enum RoasterError {
    TemperatureOutOfRange,
    SensorFault,
    InvalidState,
    PidError,
}

pub struct RoasterControl {
    state: RoasterState,
    pid_controller: CoffeeRoasterPid,
    status: SystemStatus,
    last_temp_read: Option<Instant>,
    last_pid_update: Option<Instant>,
    emergency_flag: bool,
    output_manager: OutputManager,
    manual_heater: f32, // Manual heater output when Artisan+ control active
    manual_fan: f32,    // Manual fan output
}

impl RoasterControl {
    pub fn new() -> Result<Self, RoasterError> {
        let pid = CoffeeRoasterPid::new().map_err(|_| RoasterError::PidError)?;

        Ok(RoasterControl {
            state: RoasterState::Idle,
            pid_controller: pid,
            status: SystemStatus::default(),
            last_temp_read: None,
            last_pid_update: None,
            emergency_flag: false,
            output_manager: OutputManager::new(),
            manual_heater: 0.0,
            manual_fan: 0.0,
        })
    }

    pub fn get_status(&self) -> SystemStatus {
        self.status
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
        // Validate temperature readings
        if !Self::is_temperature_valid(bean_temp) || !Self::is_temperature_valid(env_temp) {
            return Err(RoasterError::TemperatureOutOfRange);
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

    pub fn process_command(
        &mut self,
        command: RoasterCommand,
        current_time: Instant,
    ) -> Result<(), RoasterError> {
        match command {
            RoasterCommand::StartRoast(target_temp) => {
                // Simplified for Artisan+ - no state restrictions
                self.pid_controller
                    .set_target(target_temp)
                    .map_err(|_| RoasterError::PidError)?;
                self.pid_controller.enable();
                self.status.target_temp = target_temp;
                self.status.pid_enabled = true;

                // Reset output manager for new roast
                self.output_manager.reset();

                info!(
                    "Artisan+ control started with target temperature: {:.1}°C",
                    target_temp
                );
            }

            RoasterCommand::StopRoast => {
                // Simplified for Artisan+ - immediate stop
                self.pid_controller.disable();
                self.status.ssr_output = 0.0;
                self.status.pid_enabled = false;

                info!("Artisan+ control stopped - heating disabled");
            }

            RoasterCommand::SetTemperature(target_temp) => {
                // Simplified for Artisan+ - allow temperature setting even if not enabled
                self.pid_controller
                    .set_target(target_temp)
                    .map_err(|_| RoasterError::PidError)?;
                self.status.target_temp = target_temp;

                info!("Artisan+ target temperature set to: {:.1}°C", target_temp);
            }

            RoasterCommand::EmergencyStop => {
                self.emergency_shutdown("Manual emergency stop")?;
            }

            RoasterCommand::Reset => {
                self.reset_system(current_time)?;
                info!("System reset completed");
            }

            RoasterCommand::SetHeaterManual(value) => {
                // Artisan+ takes control, disable PID
                self.status.artisan_control = true;
                self.manual_heater = value as f32;
                self.pid_controller.disable();
                self.status.pid_enabled = false;

                info!("Artisan+ manual heater set to: {}%", value);
            }

            RoasterCommand::SetFanManual(value) => {
                // Fan control works independently
                self.manual_fan = value as f32;
                self.status.fan_output = value as f32;

                info!("Artisan+ manual fan set to: {}%", value);
            }

            RoasterCommand::ArtisanEmergencyStop => {
                self.emergency_shutdown("Artisan+ emergency stop")?;
            }
        }

        Ok(())
    }

    pub fn update_control(&mut self, current_time: Instant) -> Result<f32, RoasterError> {
        // Check temperature sensor validity
        if let Some(last_read) = self.last_temp_read {
            if current_time.duration_since(last_read)
                > Duration::from_millis(TEMP_VALIDITY_TIMEOUT_MS as u64)
            {
                warn!("Temperature sensor timeout detected");
                self.emergency_shutdown("Temperature sensor timeout")?;
            }
        }

        // Control logic with Artisan+ override
        let output = match self.state {
            RoasterState::Fault | RoasterState::EmergencyStop => {
                // Always off in fault states - safety override
                0.0
            }
            _ => {
                // Check if Artisan+ has control
                if self.status.artisan_control {
                    // Use manual heater setting from Artisan+
                    self.manual_heater
                } else if self.status.pid_enabled {
                    // Use PID control
                    self.update_pid_control(current_time)
                } else {
                    // Neither PID nor Artisan+ control - no heating
                    0.0
                }
            }
        };

        self.status.ssr_output = output.clamp(0.0, 100.0);
        self.status.fan_output = self.manual_fan; // Always use manual fan setting

        // Update output status
        self.status.state = self.state;

        Ok(self.status.ssr_output)
    }

    /// Process output (serial printing) - call this from main loop
    ///
    /// Modified for Artisan+ control - always processes output when enabled
    pub async fn process_output(&mut self) -> Result<(), RoasterError> {
        // Always process output for Artisan+ compatibility
        if let Err(e) = self.output_manager.process_status(&self.status).await {
            warn!("Output error: {:?}", e);
        }
        Ok(())
    }

    /// Get reference to output manager for configuration
    pub fn get_output_manager(&self) -> &OutputManager {
        &self.output_manager
    }

    /// Get mutable reference to output manager for configuration
    pub fn get_output_manager_mut(&mut self) -> &mut OutputManager {
        &mut self.output_manager
    }

    fn update_pid_control(&mut self, current_time: Instant) -> f32 {
        // Check if it's time for PID update
        let should_update = if let Some(last_update) = self.last_pid_update {
            current_time.duration_since(last_update)
                >= Duration::from_millis(PID_SAMPLE_TIME_MS as u64)
        } else {
            true
        };

        if should_update {
            let output = self
                .pid_controller
                .compute_output(self.status.bean_temp, current_time.as_millis() as u32);

            self.last_pid_update = Some(current_time);

            // Check if we've reached stable temperature
            if self.state == RoasterState::Heating {
                let temp_error = (self.status.bean_temp - self.status.target_temp).abs();
                if temp_error < 2.0 {
                    self.state = RoasterState::Stable;
                    info!("Target temperature reached, entering stable state");
                }
            }

            output
        } else {
            self.status.ssr_output
        }
    }

    fn emergency_shutdown(&mut self, reason: &str) -> Result<(), RoasterError> {
        warn!("EMERGENCY SHUTDOWN: {}", reason);

        self.emergency_flag = true;
        self.state = RoasterState::EmergencyStop;
        self.pid_controller.disable();
        self.status.ssr_output = 0.0;
        self.status.pid_enabled = false;
        self.status.fault_condition = true;

        Err(RoasterError::TemperatureOutOfRange)
    }

    fn reset_system(&mut self, current_time: Instant) -> Result<(), RoasterError> {
        self.state = RoasterState::Idle;
        self.pid_controller = CoffeeRoasterPid::new().map_err(|_| RoasterError::PidError)?;
        self.status = SystemStatus::default();
        self.last_temp_read = Some(current_time);
        self.last_pid_update = Some(current_time);
        self.emergency_flag = false;
        self.manual_heater = 0.0;
        self.manual_fan = 0.0;

        Ok(())
    }

    /// Process Artisan+ commands
    pub fn process_artisan_command(&mut self, command: ArtisanCommand) -> Result<(), RoasterError> {
        let _current_time = Instant::now();

        match command {
            ArtisanCommand::StartRoast => {
                // Start roasting for Artisan+ with default target
                self.status.artisan_control = true;
                self.enable_pid_control(DEFAULT_TARGET_TEMP)?;
                self.output_manager.enable_continuous_output();

                info!("Artisan+ roast started with target {:.1}°C", DEFAULT_TARGET_TEMP);
            }

            ArtisanCommand::SetHeater(value) => {
                // Artisan+ takes control, disable PID
                self.status.artisan_control = true;
                self.manual_heater = value as f32;
                self.pid_controller.disable();
                self.status.pid_enabled = false;

                info!("Artisan+ heater set to: {}%", value);
            }

            ArtisanCommand::SetFan(value) => {
                // Fan control works independently
                self.manual_fan = value as f32;
                self.status.fan_output = value as f32;

                info!("Artisan+ fan set to: {}%", value);
            }

            ArtisanCommand::EmergencyStop => {
                self.output_manager.disable_continuous_output();
                self.emergency_shutdown("Artisan+ emergency stop")?;
            }

            ArtisanCommand::ReadStatus => {
                // This will be handled by the command processor
                // No action needed here
            }
        }

        Ok(())
    }

    /// Re-enable PID control (disables Artisan+ manual control)
    pub fn enable_pid_control(&mut self, target_temp: f32) -> Result<(), RoasterError> {
        self.status.artisan_control = false;
        self.pid_controller
            .set_target(target_temp)
            .map_err(|_| RoasterError::PidError)?;
        self.pid_controller.enable();
        self.status.pid_enabled = true;
        self.status.target_temp = target_temp;

        info!("PID control re-enabled with target: {:.1}°C", target_temp);

        Ok(())
    }

    /// Get fan speed for READ command
    pub fn get_fan_speed(&self) -> f32 {
        self.status.fan_output
    }

    fn is_temperature_valid(temp: f32) -> bool {
        temp >= MIN_TEMP && temp <= MAX_TEMP && !temp.is_nan() && temp.is_finite()
    }

    pub fn is_emergency_condition(&self) -> bool {
        self.emergency_flag || self.status.fault_condition
    }
}

impl Default for RoasterControl {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roaster_creation() {
        let roaster = RoasterControl::new().unwrap();
        assert_eq!(roaster.get_state(), RoasterState::Idle);
        assert!(!roaster.is_emergency_condition());
    }

    #[test]
    fn test_temperature_validation() {
        assert!(RoasterControl::is_temperature_valid(25.0));
        assert!(!RoasterControl::is_temperature_valid(-10.0));
        assert!(!RoasterControl::is_temperature_valid(400.0));
        assert!(!RoasterControl::is_temperature_valid(f32::NAN));
        assert!(!RoasterControl::is_temperature_valid(f32::INFINITY));
    }

    #[test]
    fn test_start_roast() {
        let mut roaster = RoasterControl::new().unwrap();
        let current_time = Instant::now();

        roaster
            .process_command(RoasterCommand::StartRoast(200.0), current_time)
            .unwrap();
        assert_eq!(roaster.get_state(), RoasterState::Heating);
        assert_eq!(roaster.get_status().target_temp, 200.0);
    }

    #[test]
    fn test_emergency_stop() {
        let mut roaster = RoasterControl::new().unwrap();
        let current_time = Instant::now();

        roaster
            .process_command(RoasterCommand::EmergencyStop, current_time)
            .unwrap_err();
        assert_eq!(roaster.get_state(), RoasterState::EmergencyStop);
        assert!(roaster.is_emergency_condition());
    }
}
