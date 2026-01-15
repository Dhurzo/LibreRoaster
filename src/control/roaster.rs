use crate::config::*;
use crate::hardware::pid::CoffeeRoasterPid;
use embassy_time::{Duration, Instant, Timer};
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
                if self.state != RoasterState::Idle {
                    return Err(RoasterError::InvalidState);
                }

                self.pid_controller
                    .set_target(target_temp)
                    .map_err(|_| RoasterError::PidError)?;
                self.pid_controller.enable();
                self.status.target_temp = target_temp;
                self.state = RoasterState::Heating;
                self.status.pid_enabled = true;

                info!(
                    "Starting roast with target temperature: {:.1}°C",
                    target_temp
                );
            }

            RoasterCommand::StopRoast => {
                if self.state == RoasterState::EmergencyStop {
                    return Err(RoasterError::InvalidState);
                }

                self.pid_controller.disable();
                self.status.ssr_output = 0.0;
                self.status.pid_enabled = false;
                self.state = RoasterState::Cooling;

                info!("Stopping roast, entering cooling phase");
            }

            RoasterCommand::SetTemperature(target_temp) => {
                if !self.pid_controller.is_enabled() {
                    return Err(RoasterError::InvalidState);
                }

                self.pid_controller
                    .set_target(target_temp)
                    .map_err(|_| RoasterError::PidError)?;
                self.status.target_temp = target_temp;

                info!("Target temperature updated to: {:.1}°C", target_temp);
            }

            RoasterCommand::EmergencyStop => {
                self.emergency_shutdown("Manual emergency stop")?;
            }

            RoasterCommand::Reset => {
                self.reset_system(current_time)?;
                info!("System reset completed");
            }
        }

        Ok(())
    }

    pub fn update_control(&mut self, current_time: Instant) -> Result<f32, RoasterError> {
        // Check temperature sensor validity
        if let Some(last_read) = self.last_temp_read {
            if current_time.duration_since(last_read)
                > Duration::from_millis(TEMP_VALIDITY_TIMEOUT_MS)
            {
                warn!("Temperature sensor timeout detected");
                self.emergency_shutdown("Temperature sensor timeout")?;
            }
        }

        // Update PID control based on current state
        let output = match self.state {
            RoasterState::Heating => {
                // PID control with maximum output allowed
                self.update_pid_control(current_time)
            }
            RoasterState::Stable => {
                // Normal PID control
                self.update_pid_control(current_time)
            }
            RoasterState::Idle => {
                // No heating when idle
                self.pid_controller.disable();
                self.status.pid_enabled = false;
                0.0
            }
            RoasterState::Cooling => {
                // Gradual reduction to zero
                self.status.pid_enabled = false;
                self.pid_controller.disable();

                let current_output = self.status.ssr_output;
                let reduction = current_output * 0.1; // Reduce by 10% per cycle

                if current_output <= 1.0 {
                    self.state = RoasterState::Idle;
                    0.0
                } else {
                    current_output - reduction
                }
            }
            RoasterState::Fault | RoasterState::EmergencyStop => {
                // Always off in fault states
                0.0
            }
        };

        self.status.ssr_output = output.clamp(0.0, 100.0);
        Ok(self.status.ssr_output)
    }

    fn update_pid_control(&mut self, current_time: Instant) -> f32 {
        // Check if it's time for PID update
        let should_update = if let Some(last_update) = self.last_pid_update {
            current_time.duration_since(last_update) >= Duration::from_millis(PID_SAMPLE_TIME_MS)
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

        Ok(())
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
