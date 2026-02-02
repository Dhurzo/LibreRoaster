use super::{RoasterCommandHandler, RoasterError};
use crate::config::{RoasterCommand, SsrHardwareStatus, SystemStatus};
use crate::control::pid::CoffeeRoasterPid;
use crate::output::OutputManager;
use embassy_time::Instant;
use log::{info, warn};

/// Maneja comandos relacionados con el control de temperatura (PID y manual)
pub struct TemperatureCommandHandler {
    pid_controller: CoffeeRoasterPid,
    output_manager: OutputManager,
    // El hardware SSR ha sido movido a RoasterControl
}

impl TemperatureCommandHandler {
    pub fn new() -> Result<Self, RoasterError> {
        let pid = CoffeeRoasterPid::new().map_err(|_| RoasterError::PidError)?;

        Ok(Self {
            pid_controller: pid,
            output_manager: OutputManager::new(),
        })
    }

    // Nota: with_ssr ha sido eliminado. El hardware se inyecta en RoasterControl.

    pub fn get_pid_output(&mut self, bean_temp: f32, current_time: Instant) -> f32 {
        // Solo calculamos, no aplicamos. RoasterControl aplica al hardware.
        self.pid_controller
            .compute_output(bean_temp, current_time.as_millis() as u32)
    }

    pub fn set_pid_target(&mut self, target_temp: f32) -> Result<(), RoasterError> {
        self.pid_controller
            .set_target(target_temp)
            .map_err(|_| RoasterError::PidError)?;
        Ok(())
    }

    pub fn enable_pid(&mut self) {
        self.pid_controller.enable();
    }

    pub fn disable_pid(&mut self) {
        self.pid_controller.disable();
    }

    pub fn get_output_manager(&self) -> &OutputManager {
        &self.output_manager
    }

    pub fn get_output_manager_mut(&mut self) -> &mut OutputManager {
        &mut self.output_manager
    }
}

impl RoasterCommandHandler for TemperatureCommandHandler {
    fn handle_command(
        &mut self,
        command: RoasterCommand,
        _current_time: Instant,
        status: &mut SystemStatus,
    ) -> Result<(), RoasterError> {
        match command {
            RoasterCommand::StartRoast(target_temp) => {
                self.set_pid_target(target_temp)?;
                self.enable_pid();
                status.target_temp = target_temp;
                status.pid_enabled = true;
                // status.ssr_hardware_status se actualiza en el bucle principal de RoasterControl

                self.output_manager.reset();

                info!(
                    "Artisan+ control started with target temperature: {:.1}°C",
                    target_temp
                );
                Ok(())
            }

            RoasterCommand::StopRoast => {
                self.disable_pid();

                status.ssr_output = 0.0;
                status.pid_enabled = false;

                info!("Artisan+ control stopped - heating disabled");
                Ok(())
            }

            RoasterCommand::SetTemperature(target_temp) => {
                self.set_pid_target(target_temp)?;
                status.target_temp = target_temp;

                info!("Artisan+ target temperature set to: {:.1}°C", target_temp);
                Ok(())
            }

            RoasterCommand::SetHeaterManual(value) => {
                status.artisan_control = true;
                status.pid_enabled = false;
                self.disable_pid();

                let heater_value = value as f32;
                status.ssr_output = heater_value;

                info!("Artisan+ manual heater set to: {}%", value);
                Ok(())
            }

            _ => Err(RoasterError::InvalidState),
        }
    }

    fn can_handle(&self, command: RoasterCommand) -> bool {
        matches!(
            command,
            RoasterCommand::StartRoast(_)
                | RoasterCommand::StopRoast
                | RoasterCommand::SetTemperature(_)
                | RoasterCommand::SetHeaterManual(_)
        )
    }
}

pub struct SafetyCommandHandler {
    emergency_flag: bool,
}

impl SafetyCommandHandler {
    pub fn new() -> Self {
        Self {
            emergency_flag: false,
        }
    }

    pub fn is_emergency_active(&self) -> bool {
        self.emergency_flag
    }

    pub fn clear_emergency(&mut self) {
        self.emergency_flag = false;
    }

    pub fn trigger_emergency(&mut self, reason: &str) -> Result<(), RoasterError> {
        warn!("EMERGENCY SHUTDOWN: {}", reason);
        self.emergency_flag = true;
        Err(RoasterError::TemperatureOutOfRange)
    }
}

impl RoasterCommandHandler for SafetyCommandHandler {
    fn handle_command(
        &mut self,
        command: RoasterCommand,
        _current_time: Instant,
        status: &mut SystemStatus,
    ) -> Result<(), RoasterError> {
        match command {
            RoasterCommand::EmergencyStop => {
                status.fault_condition = true;
                status.ssr_output = 0.0;
                status.pid_enabled = false;
                status.ssr_hardware_status = SsrHardwareStatus::Error;
                self.trigger_emergency("Manual emergency stop")
            }

            RoasterCommand::ArtisanEmergencyStop => {
                status.fault_condition = true;
                status.ssr_output = 0.0;
                status.pid_enabled = false;
                status.ssr_hardware_status = SsrHardwareStatus::Error;
                self.trigger_emergency("Artisan+ emergency stop")
            }

            _ => Err(RoasterError::InvalidState),
        }
    }

    fn can_handle(&self, command: RoasterCommand) -> bool {
        matches!(
            command,
            RoasterCommand::EmergencyStop | RoasterCommand::ArtisanEmergencyStop
        )
    }
}

pub struct ArtisanCommandHandler {
    manual_heater: f32,
    manual_fan: f32,
}

impl ArtisanCommandHandler {
    pub fn new() -> Self {
        Self {
            manual_heater: 0.0,
            manual_fan: 0.0,
        }
    }

    pub fn get_manual_heater(&self) -> f32 {
        self.manual_heater
    }

    pub fn get_manual_fan(&self) -> f32 {
        self.manual_fan
    }

    pub fn set_manual_values(&mut self, heater: f32, fan: f32) {
        self.manual_heater = heater;
        self.manual_fan = fan;
    }
}

impl RoasterCommandHandler for ArtisanCommandHandler {
    fn handle_command(
        &mut self,
        command: RoasterCommand,
        _current_time: Instant,
        status: &mut SystemStatus,
    ) -> Result<(), RoasterError> {
        match command {
            RoasterCommand::SetHeaterManual(value) => {
                status.artisan_control = true;
                self.manual_heater = value as f32;
                status.pid_enabled = false;

                info!("Artisan+ manual heater set to: {}%", value);
                Ok(())
            }

            RoasterCommand::SetFanManual(value) => {
                self.manual_fan = value as f32;
                status.fan_output = value as f32;

                info!("Artisan+ manual fan set to: {}%", value);
                Ok(())
            }

            _ => Err(RoasterError::InvalidState),
        }
    }

    fn can_handle(&self, command: RoasterCommand) -> bool {
        matches!(
            command,
            RoasterCommand::SetHeaterManual(_) | RoasterCommand::SetFanManual(_)
        )
    }
}

/// Maneja comandos de sistema (reset, etc.)
pub struct SystemCommandHandler;

impl RoasterCommandHandler for SystemCommandHandler {
    fn handle_command(
        &mut self,
        command: RoasterCommand,
        _current_time: Instant,
        status: &mut SystemStatus,
    ) -> Result<(), RoasterError> {
        match command {
            RoasterCommand::Reset => {
                *status = SystemStatus::default();
                info!("System reset completed");
                Ok(())
            }

            _ => Err(RoasterError::InvalidState),
        }
    }

    fn can_handle(&self, command: RoasterCommand) -> bool {
        matches!(command, RoasterCommand::Reset)
    }
}
