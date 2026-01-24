use crate::config::*;
use super::{RoasterError, RoasterCommandHandler};
use crate::control::handlers::{
    TemperatureCommandHandler, SafetyCommandHandler, ArtisanCommandHandler, SystemCommandHandler
};
use embassy_time::{Duration, Instant};
use log::{info, warn};

/// Controlador principal del tostador refactorizado con Command Pattern
/// 
/// Ahora cumple con:
/// - Single Responsibility: Cada handler maneja un tipo específico de comando
/// - Open/Closed: Nuevos comandos不需要修改现有代码
/// - Dependency Inversion: Depende de abstracciones (handlers) no de implementaciones específicas
pub struct RoasterControl {
    state: RoasterState,
    status: SystemStatus,
    last_temp_read: Option<Instant>,
    last_pid_update: Option<Instant>,
    
    // Command handlers - inyección de dependencias
    temp_handler: TemperatureCommandHandler,
    safety_handler: SafetyCommandHandler,
    artisan_handler: ArtisanCommandHandler,
    system_handler: SystemCommandHandler,
}

impl RoasterControl {
    pub fn new() -> Result<Self, RoasterError> {
        let temp_handler = TemperatureCommandHandler::new()?;
        
        Ok(RoasterControl {
            state: RoasterState::Idle,
            status: SystemStatus::default(),
            last_temp_read: None,
            last_pid_update: None,
            temp_handler,
            safety_handler: SafetyCommandHandler::new(),
            artisan_handler: ArtisanCommandHandler::new(),
            system_handler: SystemCommandHandler,
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

    /// Procesa comandos usando el patrón Command en lugar del match gigante
    /// 
    /// Ahora cumple con Open/Closed Principle - nuevos comandos不需要 modificar este método
    pub fn process_command(
        &mut self,
        command: RoasterCommand,
        current_time: Instant,
    ) -> Result<(), RoasterError> {
        // Intentar procesar con cada handler en orden de prioridad
        let mut handlers: [&mut dyn RoasterCommandHandler; 4] = [
            &mut self.safety_handler,  // Prioridad más alta: seguridad
            &mut self.temp_handler,    // Control de temperatura
            &mut self.artisan_handler, // Control Artisan+
            &mut self.system_handler,  // Comandos de sistema
        ];

        for handler in &mut handlers {
            if handler.can_handle(command) {
                let result = handler.handle_command(command, current_time, &mut self.status);
                
                // Actualizar estado del safety handler si es necesario
                self.status.fault_condition = self.safety_handler.is_emergency_active();
                
                return result;
            }
        }

        warn!("No handler found for command: {:?}", command);
        Err(RoasterError::InvalidState)
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

        // Control logic con prioridad de seguridad
        let output = if self.safety_handler.is_emergency_active() {
            // Safety override
            0.0
        } else {
            // Check if Artisan+ has control
            if self.status.artisan_control {
                // Use manual heater setting from Artisan+
                self.artisan_handler.get_manual_heater()
            } else if self.status.pid_enabled {
                // Use PID control
                self.update_pid_control(current_time)
            } else {
                // Neither PID nor Artisan+ control - no heating
                0.0
            }
        };

        self.status.ssr_output = output.clamp(0.0, 100.0);
        self.status.fan_output = self.artisan_handler.get_manual_fan();

        // Update output status
        self.status.state = self.state;

        Ok(self.status.ssr_output)
    }

    /// Process output (serial printing) - call this from main loop
    pub async fn process_output(&mut self) -> Result<(), RoasterError> {
        // Always process output for Artisan+ compatibility
        if let Err(e) = self.temp_handler.get_output_manager_mut().process_status(&self.status).await {
            warn!("Output error: {:?}", e);
        }
        Ok(())
    }

    /// Get reference to output manager for configuration
    pub fn get_output_manager(&self) -> &crate::output::OutputManager {
        self.temp_handler.get_output_manager()
    }

    /// Get mutable reference to output manager for configuration
    pub fn get_output_manager_mut(&mut self) -> &mut crate::output::OutputManager {
        self.temp_handler.get_output_manager_mut()
    }

    /// Process Artisan+ commands usando el command pattern
    pub fn process_artisan_command(&mut self, command: ArtisanCommand) -> Result<(), RoasterError> {
        let _current_time = Instant::now();

        match command {
            ArtisanCommand::StartRoast => {
                // Start roasting for Artisan+ with default target
                self.status.artisan_control = true;
                self.enable_pid_control(DEFAULT_TARGET_TEMP)?;
                self.temp_handler.get_output_manager_mut().enable_continuous_output();

                info!("Artisan+ roast started with target {:.1}°C", DEFAULT_TARGET_TEMP);
            }

            ArtisanCommand::SetHeater(value) => {
                self.artisan_handler.set_manual_values(value as f32, self.artisan_handler.get_manual_fan());
                self.status.artisan_control = true;
                self.status.pid_enabled = false;
                self.temp_handler.disable_pid();

                info!("Artisan+ heater set to: {}%", value);
            }

            ArtisanCommand::SetFan(value) => {
                self.artisan_handler.set_manual_values(self.artisan_handler.get_manual_heater(), value as f32);
                self.status.fan_output = value as f32;

                info!("Artisan+ fan set to: {}%", value);
            }

            ArtisanCommand::EmergencyStop => {
                self.temp_handler.get_output_manager_mut().disable_continuous_output();
                self.safety_handler.trigger_emergency("Artisan+ emergency stop")?;
                self.status.fault_condition = true;
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
        self.temp_handler.set_pid_target(target_temp)?;
        self.temp_handler.enable_pid();
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
        self.safety_handler.is_emergency_active() || self.status.fault_condition
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
            let output = self.temp_handler.get_pid_output(self.status.bean_temp, current_time);

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
        self.safety_handler.trigger_emergency(reason)?;
        self.state = RoasterState::EmergencyStop;
        self.temp_handler.disable_pid();
        self.status.ssr_output = 0.0;
        self.status.pid_enabled = false;
        self.status.fault_condition = true;

        Err(RoasterError::TemperatureOutOfRange)
    }

    #[allow(dead_code)]
    fn reset_system(&mut self, current_time: Instant) -> Result<(), RoasterError> {
        self.state = RoasterState::Idle;
        self.temp_handler = TemperatureCommandHandler::new()?; // Recreate for fresh state
        self.status = SystemStatus::default();
        self.last_temp_read = Some(current_time);
        self.last_pid_update = Some(current_time);
        self.safety_handler.clear_emergency();
        self.artisan_handler = ArtisanCommandHandler::new();

        Ok(())
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
        assert_eq!(roaster.get_state(), RoasterState::Idle); // State management simplified
        assert_eq!(roaster.get_status().target_temp, 200.0);
    }

    #[test]
    fn test_emergency_stop() {
        let mut roaster = RoasterControl::new().unwrap();
        let current_time = Instant::now();

        roaster
            .process_command(RoasterCommand::EmergencyStop, current_time)
            .unwrap_err();
        assert!(roaster.is_emergency_condition());
    }

    #[test]
    fn test_command_handlers_priority() {
        let mut roaster = RoasterControl::new().unwrap();
        let current_time = Instant::now();

        // Test that safety handler takes priority
        roaster.process_command(RoasterCommand::StartRoast(200.0), current_time).unwrap();
        assert!(roaster.get_status().pid_enabled);

        // Emergency stop should override
        roaster.process_command(RoasterCommand::EmergencyStop, current_time).unwrap_err();
        assert!(roaster.safety_handler.is_emergency_active());
        assert!(!roaster.get_status().pid_enabled);
    }
}