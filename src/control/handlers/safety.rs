// Safety command handler for roaster control
//
// This module handles safety-related commands:
// - EmergencyStop - Emergency shutdown with immediate heater cutoff
// - ArtisanEmergencyStop - Artisan-initiated emergency stop
//
// These commands manage emergency conditions and ensure safe system state.

use crate::config::{RoasterCommand, SsrHardwareStatus, SystemStatus};
use crate::control::policies::{SafetyPolicy, SafetyPolicyOutcome};
use crate::control::{RoasterCommandHandler, RoasterError};
use log::warn;

/// Safety command handler
///
/// Manages emergency conditions and safety-related commands
pub struct SafetyCommandHandler {
    emergency_flag: bool,
}

impl SafetyCommandHandler {
    /// Create a new safety command handler
    pub fn new() -> Self {
        Self {
            emergency_flag: false,
        }
    }

    /// Check if emergency is active
    pub fn is_emergency_active(&self) -> bool {
        self.emergency_flag
    }

    /// Clear emergency flag
    pub fn clear_emergency(&mut self) {
        self.emergency_flag = false;
    }

    /// Trigger emergency shutdown
    ///
    /// # Arguments
    ///
    /// * `reason` - Reason for emergency shutdown
    ///
    /// # Returns
    ///
    /// Error indicating emergency condition
    pub fn trigger_emergency(&mut self, reason: &str) -> Result<(), RoasterError> {
        warn!("EMERGENCY SHUTDOWN: {}", reason);
        self.emergency_flag = true;
        Err(RoasterError::TemperatureOutOfRange {
            source: Some("emergency_shutdown"),
        })
    }
}

impl Default for SafetyCommandHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl RoasterCommandHandler for SafetyCommandHandler {
    /// Handle safety-related roaster commands
    ///
    /// # Commands Handled
    ///
    /// - `EmergencyStop` - Emergency shutdown
    /// - `ArtisanEmergencyStop` - Artisan-initiated emergency stop
    ///
    /// Both commands:
    /// 1. Set fault_condition = true
    /// 2. Cut heater output (ssr_output = 0)
    /// 3. Disable PID control (pid_enabled = false)
    /// 4. Mark SSR hardware status as Error
    ///
    /// # Arguments
    ///
    /// * `command` - Roaster command to handle
    /// * `_current_time` - Current timestamp (unused for safety commands)
    /// * `status` - Mutable system status
    ///
    /// # Returns
    ///
    /// Ok(()) if command handled, Err if invalid command
    fn handle_command(
        &mut self,
        command: RoasterCommand,
        _current_time: embassy_time::Instant,
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
                self.trigger_emergency("Artisan emergency stop")
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
            RoasterCommand::EmergencyStop | RoasterCommand::ArtisanEmergencyStop
        )
    }
}

impl SafetyPolicy for SafetyCommandHandler {
    /// Evaluate safety command policy
    ///
    /// # Arguments
    ///
    /// * `command` - Roaster command to evaluate
    /// * `status` - Mutable system status
    ///
    /// # Returns
    ///
    /// Safety policy outcome
    fn evaluate(
        &mut self,
        command: RoasterCommand,
        status: &mut SystemStatus,
    ) -> SafetyPolicyOutcome {
        match command {
            RoasterCommand::EmergencyStop => {
                let outcome = SafetyPolicyOutcome::emergency("Manual emergency stop");
                outcome.apply_to_status(status);
                self.emergency_flag = true;
                warn!("EMERGENCY SHUTDOWN: Manual emergency stop");
                outcome
            }

            RoasterCommand::ArtisanEmergencyStop => {
                let outcome = SafetyPolicyOutcome::emergency("Artisan+ emergency stop");
                outcome.apply_to_status(status);
                self.emergency_flag = true;
                warn!("EMERGENCY SHUTDOWN: Artisan+ emergency stop");
                outcome
            }

            _ => SafetyPolicyOutcome::normal(),
        }
    }

    /// Check if this handler can evaluate the given command
    fn can_handle(&self, command: RoasterCommand) -> bool {
        matches!(
            command,
            RoasterCommand::EmergencyStop | RoasterCommand::ArtisanEmergencyStop
        )
    }

    /// Check if emergency is active
    fn is_emergency_active(&self) -> bool {
        self.emergency_flag
    }

    /// Clear emergency flag
    fn clear_emergency(&mut self) {
        self.emergency_flag = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let handler = SafetyCommandHandler::new();
        assert!(!handler.is_emergency_active());
    }

    #[test]
    fn test_trigger_emergency() {
        let mut handler = SafetyCommandHandler::new();
        let result = handler.trigger_emergency("test reason");
        assert!(result.is_err());
        assert!(handler.is_emergency_active());
    }

    #[test]
    fn test_clear_emergency() {
        let mut handler = SafetyCommandHandler::new();
        let _ = handler.trigger_emergency("test");
        handler.clear_emergency();
        assert!(!handler.is_emergency_active());
    }

    #[test]
    fn test_default() {
        let handler = SafetyCommandHandler::default();
        assert!(!handler.is_emergency_active());
    }
}
