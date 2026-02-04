use crate::config::{RoasterCommand, SystemStatus};
use embassy_time::Instant;

#[derive(Debug, Clone, PartialEq)]
pub enum RoasterError {
    TemperatureOutOfRange,
    SensorFault,
    InvalidState,
    PidError,
    HardwareError,
    EmergencyShutdown,
}

impl core::fmt::Display for RoasterError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RoasterError::TemperatureOutOfRange => write!(f, "Temperature out of range"),
            RoasterError::SensorFault => write!(f, "Sensor fault"),
            RoasterError::InvalidState => write!(f, "Invalid state"),
            RoasterError::PidError => write!(f, "PID error"),
            RoasterError::HardwareError => write!(f, "Hardware error"),
            RoasterError::EmergencyShutdown => write!(f, "Emergency shutdown"),
        }
    }
}

pub trait PidController {
    type Error;

    fn set_target(&mut self, target: f32) -> Result<(), Self::Error>;
    fn enable(&mut self);
    fn disable(&mut self);
    fn compute_output(&mut self, current_temp: f32, current_time: u32) -> f32;
    fn is_enabled(&self) -> bool;
    fn get_target(&self) -> f32;
}

pub trait RoasterCommandHandler {
    fn handle_command(
        &mut self,
        command: RoasterCommand,
        current_time: Instant,
        status: &mut SystemStatus,
    ) -> Result<(), RoasterError>;

    fn can_handle(&self, command: RoasterCommand) -> bool;
}

#[derive(Debug, Default)]
pub struct OutputController;

impl OutputController {
    pub fn new() -> Self {
        OutputController
    }

    pub async fn process_status(&mut self, _status: &SystemStatus) -> Result<(), RoasterError> {
        Ok(())
    }

    pub fn reset(&mut self) {
    }

    pub fn enable_continuous_output(&mut self) {
    }

    pub fn disable_continuous_output(&mut self) {
    }

    pub fn is_continuous_enabled(&self) -> bool {
        true
    }
}
