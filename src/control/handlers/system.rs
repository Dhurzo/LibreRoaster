// System command handler for roaster control
//
// This module handles system-level commands:
// - Reset - Reset system status to default values
//
// This command is used to reset the roaster control state.

use crate::config::{RoasterCommand, SystemStatus};
use crate::control::{RoasterCommandHandler, RoasterError};
use log::info;

/// System command handler
///
/// Manages system-level commands for roaster control
pub struct SystemCommandHandler;

impl RoasterCommandHandler for SystemCommandHandler {
    /// Handle system roaster commands
    ///
    /// # Commands Handled
    ///
    /// - `Reset` - Reset system status to default values
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
                *status = SystemStatus::default();
                info!("System reset completed");
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
