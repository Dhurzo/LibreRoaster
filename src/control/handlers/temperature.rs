// Temperature command handler for roaster control
//
// This module handles temperature-related commands:
// - StartRoast - Begin PID control with target temperature
// - StopRoast - Disable PID control
// - SetTemperature - Update PID target temperature
//
// These commands manage the PID controller and output manager for temperature control.

use crate::config::{RoasterCommand, SystemStatus};
use crate::control::pid::{CoffeeRoasterPid, PidFeedback};
use crate::control::OutputController;
use crate::control::{RoasterCommandHandler, RoasterError};
use embassy_time::Instant;
use log::info;

/// Temperature command handler
///
/// Manages PID control and output for temperature-related roaster commands
pub struct TemperatureCommandHandler {
    pid_controller: CoffeeRoasterPid,
    output_manager: OutputController,
}

impl TemperatureCommandHandler {
    /// Create a new temperature command handler
    ///
    /// Initializes PID controller and output manager
    pub fn new() -> Result<Self, RoasterError> {
        let pid = CoffeeRoasterPid::new().map_err(|_| RoasterError::PidError {
            source: Some("pid_init_failed"),
        })?;

        Ok(Self {
            pid_controller: pid,
            output_manager: OutputController::new(),
        })
    }

    /// Get PID output based on current bean temperature
    ///
    /// # Arguments
    ///
    /// * `bean_temp` - Current bean temperature
    /// * `current_time` - Current timestamp for PID calculation
    ///
    /// # Returns
    ///
    /// PID controller output value
    pub fn get_pid_output(&mut self, bean_temp: f32, current_time: Instant) -> f32 {
        self.pid_controller
            .compute_output(bean_temp, current_time.as_millis() as u32)
    }

    /// Push actuator/LEDC guard feedback into PID
    ///
    /// Allows integrator to clamp while guard is busy
    pub fn set_pid_feedback(&mut self, feedback: PidFeedback) {
        self.pid_controller.update_feedback(feedback);
    }

    /// Set PID target temperature
    ///
    /// # Arguments
    ///
    /// * `target_temp` - Target temperature in °C
    pub fn set_pid_target(&mut self, target_temp: f32) -> Result<(), RoasterError> {
        self.pid_controller
            .set_target(target_temp)
            .map_err(|_| RoasterError::PidError {
                source: Some("set_target_failed"),
            })?;
        Ok(())
    }

    /// Enable PID control
    pub fn enable_pid(&mut self) {
        self.pid_controller.enable();
    }

    /// Bug A3 (2026-07-25): expose the PID-enabled state so `enable_pid` in
    /// `CommandDispatcher` can decide whether to (re-)enable the controller
    /// or just update the target. Artisan's ramp/soak driver fires `PID;SV`
    /// on every setpoint change; each call MUST NOT poke `enable()` because
    /// that resets the integrator and the previous-derivative history,
    /// defeating the I-term and causing visible droop on every update.
    pub fn pid_is_enabled(&self) -> bool {
        self.pid_controller.is_enabled()
    }

    /// Disable PID control
    pub fn disable_pid(&mut self) {
        self.pid_controller.disable();
    }

    /// Get output manager reference
    pub fn get_output_manager(&self) -> &OutputController {
        &self.output_manager
    }

    /// Get mutable output manager reference
    pub fn get_output_manager_mut(&mut self) -> &mut OutputController {
        &mut self.output_manager
    }

    /// Get current PID integrator value
    pub fn pid_integrator_value(&self) -> f32 {
        self.pid_controller.integrator_value()
    }

    /// Get current PID derivative value
    pub fn pid_derivative_value(&self) -> f32 {
        self.pid_controller.derivative_value()
    }

    /// Check if PID integrator is clamped
    pub fn pid_integrator_clamped(&self) -> bool {
        self.pid_controller.is_integrator_clamped()
    }

    /// Check if PID saturation is active
    pub fn pid_saturation_active(&self) -> bool {
        self.pid_controller.is_saturation_active()
    }

    /// Set PID gains (Kp, Ki, Kd)
    ///
    /// Allows runtime tuning of PID controller for different roast profiles
    pub fn set_pid_gains(&mut self, kp: f32, ki: f32, kd: f32) -> Result<(), RoasterError> {
        if kp < 0.0 || ki < 0.0 || kd < 0.0 {
            return Err(RoasterError::PidError {
                source: Some("negative_pid_gain"),
            });
        }
        // Bug B5: mutate in place via `set_gains` instead of rebuilding the
        // whole controller with `with_gains` (which produced `enabled: false`
        // and `target: 0.0` — silently disabling the PID and dropping the
        // heater while `status.pid_enabled` still reported true).
        self.pid_controller.set_gains(kp, ki, kd);
        Ok(())
    }

    pub fn set_pid_cycle_time(&mut self, ms: u32) {
        self.pid_controller.set_cycle_time(ms);
    }

    pub fn set_pid_output_limits(&mut self, min: f32, max: f32) {
        self.pid_controller.set_output_limits(min, max);
    }

    /// Returns the PID output limits currently in effect (clamped to [0,100]
    /// and swapped to ensure min <= max), not the raw inputs.
    pub fn pid_output_limits(&self) -> (f32, f32) {
        self.pid_controller.output_limits()
    }
}

impl RoasterCommandHandler for TemperatureCommandHandler {
    /// Handle temperature-related roaster commands
    ///
    /// # Commands Handled
    ///
    /// - `StartRoast(target_temp)` - Begin PID control
    /// - `StopRoast` - Disable PID control
    /// - `SetTemperature(target_temp)` - Update PID target
    ///
    /// # Arguments
    ///
    /// * `command` - Roaster command to handle
    /// * `_current_time` - Current timestamp (unused for temperature commands)
    /// * `status` - Mutable system status
    ///
    /// # Returns
    ///
    /// Ok(()) if command handled, Err if invalid command
    fn handle_command(
        &mut self,
        command: RoasterCommand,
        _current_time: Instant,
        status: &mut SystemStatus,
    ) -> Result<(), RoasterError> {
        match command {
            RoasterCommand::StartRoast(target_temp) => {
                if self.output_manager.is_continuous_enabled() || status.pid_enabled {
                    info!("Artisan+ start requested but already active; keeping current session");
                    return Ok(());
                }

                self.set_pid_target(target_temp)?;
                self.enable_pid();
                status.target_temp = target_temp;
                status.pid_enabled = true;
                status.artisan_control = false;

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

            _ => Err(RoasterError::InvalidState {
                source: Some("invalid_command_for_mode"),
            }),
        }
    }

    /// Check if this handler can process the given command
    fn can_handle(&self, command: RoasterCommand) -> bool {
        matches!(
            command,
            RoasterCommand::StartRoast(_)
                | RoasterCommand::StopRoast
                | RoasterCommand::SetTemperature(_)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let handler = TemperatureCommandHandler::new();
        assert!(handler.is_ok());
    }

    #[test]
    fn test_can_handle_start() {
        let handler = TemperatureCommandHandler::new().unwrap();
        let can_handle = handler.can_handle(RoasterCommand::StartRoast(200.0));
        assert!(can_handle);
    }

    #[test]
    fn test_can_handle_stop() {
        let handler = TemperatureCommandHandler::new().unwrap();
        let can_handle = handler.can_handle(RoasterCommand::StopRoast);
        assert!(can_handle);
    }

    #[test]
    fn test_can_handle_set_temp() {
        let handler = TemperatureCommandHandler::new().unwrap();
        let can_handle = handler.can_handle(RoasterCommand::SetTemperature(200.0));
        assert!(can_handle);
    }

    #[test]
    fn test_cannot_handle_emergency() {
        let handler = TemperatureCommandHandler::new().unwrap();
        let can_handle = handler.can_handle(RoasterCommand::EmergencyStop);
        assert!(!can_handle);
    }
}
