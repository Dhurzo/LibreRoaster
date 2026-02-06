use super::{RoasterCommandHandler, RoasterError};
use crate::config::{RoasterCommand, SsrHardwareStatus, SystemStatus};
use crate::control::pid::CoffeeRoasterPid;
use crate::control::OutputController;
use embassy_time::Instant;
use log::{info, warn};

pub struct TemperatureCommandHandler {
    pid_controller: CoffeeRoasterPid,
    output_manager: OutputController,
}

impl TemperatureCommandHandler {
    pub fn new() -> Result<Self, RoasterError> {
        let pid = CoffeeRoasterPid::new().map_err(|_| RoasterError::PidError)?;

        Ok(Self {
            pid_controller: pid,
            output_manager: OutputController::new(),
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

    pub fn get_output_manager(&self) -> &OutputController {
        &self.output_manager
    }

    pub fn get_output_manager_mut(&mut self) -> &mut OutputController {
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
                // Idempotent START: if PID/streaming already active, leave session intact
                if self.output_manager.is_continuous_enabled() || status.pid_enabled {
                    info!("Artisan+ start requested but already active; keeping current session");
                    return Ok(());
                }

                self.set_pid_target(target_temp)?;
                self.enable_pid();
                status.target_temp = target_temp;
                status.pid_enabled = true;
                status.artisan_control = false;
                // status.ssr_hardware_status se actualiza en el bucle principal de RoasterControl

                self.output_manager.enable_continuous_output();

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
                status.artisan_control = false;

                self.output_manager.disable_continuous_output();

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
                if value > 100 {
                    warn!("Ignoring manual heater value above 100%: {}", value);
                    return Err(RoasterError::InvalidState);
                }

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

    pub fn set_manual_heater(&mut self, heater: f32) {
        self.manual_heater = heater;
    }

    pub fn set_manual_fan(&mut self, fan: f32) {
        self.manual_fan = fan;
    }

    pub fn clear_manual(&mut self) {
        self.manual_heater = 0.0;
        self.manual_fan = 0.0;
    }

    const HEATER_DELTA: i8 = 5;

    fn apply_heater_delta(current_value: f32, direction: i8) -> f32 {
        let delta = direction * Self::HEATER_DELTA;
        let new_value = (current_value as i16 + delta as i16).clamp(0, 100);
        new_value as f32
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
                if value > 100 {
                    warn!("Ignoring manual heater value above 100%: {}", value);
                    return Err(RoasterError::InvalidState);
                }

                status.artisan_control = true;
                self.manual_heater = value as f32;
                status.pid_enabled = false;
                status.ssr_output = self.manual_heater;

                info!("Artisan+ manual heater set to: {}%", value);
                Ok(())
            }

            RoasterCommand::SetFanManual(value) => {
                if value > 100 {
                    warn!("Ignoring manual fan value above 100%: {}", value);
                    return Err(RoasterError::InvalidState);
                }

                status.artisan_control = true;
                status.pid_enabled = false;
                self.manual_fan = value as f32;
                status.fan_output = value as f32;

                info!("Artisan+ manual fan set to: {}%", value);
                Ok(())
            }

            RoasterCommand::IncreaseHeater => {
                status.artisan_control = true;
                status.pid_enabled = false;

                let current = status.ssr_output;
                let new_value = Self::apply_heater_delta(current, 1);
                status.ssr_output = new_value;
                self.manual_heater = new_value;

                info!("Artisan+ UP: heater increased to {:.0}%", new_value);
                Ok(())
            }

            RoasterCommand::DecreaseHeater => {
                status.artisan_control = true;
                status.pid_enabled = false;

                let current = status.ssr_output;
                let new_value = Self::apply_heater_delta(current, -1);
                status.ssr_output = new_value;
                self.manual_heater = new_value;

                info!("Artisan+ DOWN: heater decreased to {:.0}%", new_value);
                Ok(())
            }

            _ => Err(RoasterError::InvalidState),
        }
    }

    fn can_handle(&self, command: RoasterCommand) -> bool {
        matches!(
            command,
            RoasterCommand::SetHeaterManual(_)
                | RoasterCommand::SetFanManual(_)
                | RoasterCommand::IncreaseHeater
                | RoasterCommand::DecreaseHeater
        )
    }
}

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

#[cfg(test)]
mod artisan_command_handler_tests {
    use super::*;
    use crate::config::{RoasterState, SsrHardwareStatus, SystemStatus};

    fn create_test_status() -> SystemStatus {
        SystemStatus {
            state: RoasterState::Stable,
            bean_temp: 150.5,
            env_temp: 120.3,
            target_temp: 200.0,
            ssr_output: 50.0,
            fan_output: 50.0,
            pid_enabled: false,
            artisan_control: false,
            fault_condition: false,
            ssr_hardware_status: SsrHardwareStatus::Available,
        }
    }

    #[test]
    fn test_heater_delta_constant() {
        assert_eq!(ArtisanCommandHandler::HEATER_DELTA, 5);
    }

    #[test]
    fn test_up_increases_heater() {
        let current = 50.0;
        let result = ArtisanCommandHandler::apply_heater_delta(current, 1);
        assert_eq!(result, 55.0);
    }

    #[test]
    fn test_up_at_max_clamped() {
        let current = 100.0;
        let result = ArtisanCommandHandler::apply_heater_delta(current, 1);
        assert_eq!(result, 100.0);
    }

    #[test]
    fn test_up_near_max_clamped() {
        let current = 98.0;
        let result = ArtisanCommandHandler::apply_heater_delta(current, 1);
        assert_eq!(result, 100.0);
    }

    #[test]
    fn test_down_decreases_heater() {
        let current = 50.0;
        let result = ArtisanCommandHandler::apply_heater_delta(current, -1);
        assert_eq!(result, 45.0);
    }

    #[test]
    fn test_down_at_min_clamped() {
        let current = 0.0;
        let result = ArtisanCommandHandler::apply_heater_delta(current, -1);
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_down_near_min_clamped() {
        let current = 3.0;
        let result = ArtisanCommandHandler::apply_heater_delta(current, -1);
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_up_down_boundary_values() {
        // Test various boundary conditions
        assert_eq!(ArtisanCommandHandler::apply_heater_delta(0.0, 1), 5.0); // 0 -> 5
        assert_eq!(ArtisanCommandHandler::apply_heater_delta(5.0, 1), 10.0); // 5 -> 10
        assert_eq!(ArtisanCommandHandler::apply_heater_delta(10.0, -1), 5.0); // 10 -> 5
        assert_eq!(ArtisanCommandHandler::apply_heater_delta(5.0, -1), 0.0); // 5 -> 0
    }

    #[test]
    fn test_handler_initialization() {
        let handler = ArtisanCommandHandler::new();
        assert_eq!(handler.get_manual_heater(), 0.0);
        assert_eq!(handler.get_manual_fan(), 0.0);
    }

    #[test]
    fn test_can_handle_increase_heater() {
        let handler = ArtisanCommandHandler::new();
        assert!(handler.can_handle(RoasterCommand::IncreaseHeater));
    }

    #[test]
    fn test_can_handle_decrease_heater() {
        let handler = ArtisanCommandHandler::new();
        assert!(handler.can_handle(RoasterCommand::DecreaseHeater));
    }
}
