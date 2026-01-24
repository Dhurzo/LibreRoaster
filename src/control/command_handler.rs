use super::RoasterError;
use crate::config::{RoasterCommand, SystemStatus};
use embassy_time::Instant;

/// Trait para el patrón Command - maneja comandos específicos del tostador
pub trait RoasterCommandHandler {
    /// Procesa un comando específico y retorna si fue exitoso
    fn handle_command(
        &mut self,
        command: RoasterCommand,
        current_time: Instant,
        status: &mut SystemStatus,
    ) -> Result<(), RoasterError>;

    /// Determina si este handler puede procesar el comando dado
    fn can_handle(&self, command: RoasterCommand) -> bool;
}

/// Contexto compartido que todos los handlers necesitan acceder
pub struct RoasterContext {
    pub pid_enabled: bool,
    pub artisan_control: bool,
    pub emergency_flag: bool,
}

impl RoasterContext {
    pub fn new() -> Self {
        Self {
            pid_enabled: false,
            artisan_control: false,
            emergency_flag: false,
        }
    }

    pub fn from_status(status: &SystemStatus) -> Self {
        Self {
            pid_enabled: status.pid_enabled,
            artisan_control: status.artisan_control,
            emergency_flag: status.fault_condition,
        }
    }
}
