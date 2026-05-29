// Safety command handler for roaster control
//
// This module handles safety-related commands:
// - EmergencyStop - Emergency shutdown with immediate heater cutoff
// - ArtisanEmergencyStop - Artisan-initiated emergency stop
//
// These commands manage emergency conditions and ensure safe system state.

use crate::config::{RoasterCommand, SystemStatus};
use crate::control::policies::{SafetyPolicy, SafetyPolicyOutcome};
use crate::control::RoasterError;
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

    pub fn activate_emergency(&mut self) {
        self.emergency_flag = true;
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
    use crate::config::SsrHardwareStatus;

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

    #[test]
    fn test_evaluate_emergency_stop() {
        let mut handler = SafetyCommandHandler::new();
        let mut status = SystemStatus::default();

        let outcome = handler.evaluate(RoasterCommand::EmergencyStop, &mut status);

        assert!(outcome.emergency_active);
        assert!(outcome.fault_condition);
        assert!(outcome.zero_ssr);
        assert!(outcome.disable_pid);
        assert!(status.fault_condition);
        assert_eq!(status.ssr_output, 0.0);
        assert!(!status.pid_enabled);
        assert_eq!(status.ssr_hardware_status, SsrHardwareStatus::Error);
    }

    #[test]
    fn test_evaluate_artisan_emergency() {
        let mut handler = SafetyCommandHandler::new();
        let mut status = SystemStatus::default();

        let outcome = handler.evaluate(RoasterCommand::ArtisanEmergencyStop, &mut status);

        assert!(outcome.emergency_active);
        assert!(outcome.fault_condition);
        assert!(outcome.zero_ssr);
        assert!(outcome.disable_pid);
        assert!(status.fault_condition);
        assert_eq!(status.ssr_output, 0.0);
        assert!(!status.pid_enabled);
        assert_eq!(status.ssr_hardware_status, SsrHardwareStatus::Error);
    }

    #[test]
    fn test_evaluate_unknown_command() {
        let mut handler = SafetyCommandHandler::new();
        let mut status = SystemStatus::default();

        let outcome = handler.evaluate(RoasterCommand::Reset, &mut status);

        assert!(!outcome.emergency_active);
        assert!(!outcome.fault_condition);
        assert!(!outcome.zero_ssr);
        assert!(!outcome.disable_pid);
    }

    #[test]
    fn test_clear_emergency_after_evaluate() {
        let mut handler = SafetyCommandHandler::new();
        let mut status = SystemStatus::default();

        let _ = handler.evaluate(RoasterCommand::EmergencyStop, &mut status);
        assert!(handler.is_emergency_active());

        handler.clear_emergency();
        assert!(!handler.is_emergency_active());
    }

    #[test]
    fn test_can_handle_emergency() {
        let handler = SafetyCommandHandler::new();

        assert!(<SafetyCommandHandler as SafetyPolicy>::can_handle(
            &handler,
            RoasterCommand::EmergencyStop
        ));
        assert!(<SafetyCommandHandler as SafetyPolicy>::can_handle(
            &handler,
            RoasterCommand::ArtisanEmergencyStop
        ));
        assert!(!<SafetyCommandHandler as SafetyPolicy>::can_handle(
            &handler,
            RoasterCommand::Reset
        ));
    }
}
