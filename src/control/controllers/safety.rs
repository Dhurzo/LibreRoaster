//! Safety sub-controller for roaster control.
//!
//! Wraps `SafetyCommandHandler`: emergency-stop policy evaluation, the
//! latched emergency flag, and the overtemp-regression activity marker on
//! `SystemStatus`.

use crate::config::RoasterCommand;
use crate::config::SystemStatus;
use crate::control::handlers::SafetyCommandHandler;
use crate::control::policies::{SafetyPolicy, SafetyPolicyOutcome};

/// Emergency-state facade over the safety command handler.
pub struct SafetyController {
    handler: SafetyCommandHandler,
}

impl SafetyController {
    /// Create a controller with an inactive emergency flag.
    pub fn new() -> Self {
        Self {
            handler: SafetyCommandHandler::new(),
        }
    }

    /// Whether this is a safety (emergency-stop) command.
    pub fn can_handle(&self, command: RoasterCommand) -> bool {
        <SafetyCommandHandler as SafetyPolicy>::can_handle(&self.handler, command)
    }

    /// Set the overtemp-regression activity flag on `status`.
    pub fn mark_overtemp_regression_active(&self, active: bool, status: &mut SystemStatus) {
        status.overtemp_regression_active = active;
    }

    /// Evaluate a safety policy outcome for the given command.
    pub fn evaluate(
        &mut self,
        command: RoasterCommand,
        status: &mut SystemStatus,
    ) -> SafetyPolicyOutcome {
        self.handler.evaluate(command, status)
    }

    /// Whether the emergency latch is currently armed.
    pub fn is_emergency_active(&self) -> bool {
        self.handler.is_emergency_active()
    }

    /// Arm the latched emergency flag.
    pub fn activate_emergency(&mut self) {
        self.handler.activate_emergency();
    }

    /// Clear the latched emergency flag.
    pub fn clear_emergency(&mut self) {
        self.handler.clear_emergency();
    }
}

impl Default for SafetyController {
    fn default() -> Self {
        Self::new()
    }
}
