use crate::config::RoasterCommand;
use crate::config::SystemStatus;
use crate::control::handlers::SafetyCommandHandler;
use crate::control::policies::{SafetyPolicy, SafetyPolicyOutcome};

pub struct SafetyController {
    handler: SafetyCommandHandler,
}

impl SafetyController {
    pub fn new() -> Self {
        Self {
            handler: SafetyCommandHandler::new(),
        }
    }

    pub fn can_handle(&self, command: RoasterCommand) -> bool {
        <SafetyCommandHandler as SafetyPolicy>::can_handle(&self.handler, command)
    }

    pub fn mark_overtemp_regression_active(&self, active: bool, status: &mut SystemStatus) {
        status.overtemp_regression_active = active;
    }

    pub fn evaluate(
        &mut self,
        command: RoasterCommand,
        status: &mut SystemStatus,
    ) -> SafetyPolicyOutcome {
        self.handler.evaluate(command, status)
    }

    pub fn is_emergency_active(&self) -> bool {
        self.handler.is_emergency_active()
    }

    pub fn activate_emergency(&mut self) {
        self.handler.activate_emergency();
    }

    pub fn clear_emergency(&mut self) {
        self.handler.clear_emergency();
    }
}

impl Default for SafetyController {
    fn default() -> Self {
        Self::new()
    }
}
