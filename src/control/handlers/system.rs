//! System command handler for roaster control.
//!
//! Handles `Reset`: zeroes telemetry/control state while preserving the
//! safety latch (`fault_condition`). `Reset` is currently unreachable on the
//! wire — no parser produces `RoasterCommand::Reset` — but the handler is
//! retained as a latent recovery primitive; see Bug R5 below.

// System command handler for roaster control
//
// This module handles system-level commands:
// - Reset - Reset system status to default values
//
// This command is used to reset the roaster control state.

use crate::config::{RoasterCommand, SystemStatus};
use crate::control::{RoasterCommandHandler, RoasterError};
use log::info;

/// Handles the `Reset` command, clearing telemetry/control state while
/// preserving the safety latch.
pub struct SystemCommandHandler;

impl RoasterCommandHandler for SystemCommandHandler {
    /// Handle system roaster commands
    ///
    /// # Commands Handled
    ///
    /// - `Reset` - Reset telemetry/control state (safety latch preserved)
    ///
    /// # Arguments
    ///
    /// * `command` - Roaster command to handle
    /// * `_current_time` - Current timestamp (unused for system commands)
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
            RoasterCommand::Reset => {
                // Bug R5 (2026-07-26): the previous `*status = SystemStatus::default()`
                // wiped `fault_condition` and the safety latch — a `Reset`
                // (dead on the wire today — no parser produces it — but
                // latent) would have cleared an armed emergency as a side
                // effect. Reset only telemetry/control data; the safety latch
                // is released exclusively via the explicit recovery path
                // (`clear_emergency_explicit`).
                status.bean_temp = 0.0;
                status.env_temp = 0.0;
                status.target_temp = 0.0;
                status.ssr_output = 0.0;
                status.fan_output = 0.0;
                status.derivative_rate = 0.0;
                status.derivative_available = false;
                status.integrator_value = 0.0;
                status.charge_detected = false;
                // status.fault_condition intentionally untouched — it is
                // controlled by the safety latch.
                info!("System telemetry reset (safety latch preserved)");
                Ok(())
            }

            _ => Err(RoasterError::InvalidState {
                source: Some("command_not_supported"),
            }),
        }
    }

    /// Check if this handler can process the given command
    fn can_handle(&self, command: RoasterCommand) -> bool {
        matches!(command, RoasterCommand::Reset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_handle_reset() {
        let handler = SystemCommandHandler;
        assert!(SystemCommandHandler::can_handle(
            &handler,
            RoasterCommand::Reset
        ));
    }

    #[test]
    fn test_cannot_handle_start() {
        let handler = SystemCommandHandler;
        assert!(!SystemCommandHandler::can_handle(
            &handler,
            RoasterCommand::StartRoast(200.0)
        ));
    }

    #[test]
    fn test_cannot_handle_emergency() {
        let handler = SystemCommandHandler;
        assert!(!SystemCommandHandler::can_handle(
            &handler,
            RoasterCommand::EmergencyStop
        ));
    }
}
