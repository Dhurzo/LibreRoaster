use crate::config::{RoasterCommand, SystemStatus};
use crate::hardware::{fan::FanError, ssr::SsrError, uart::UartError};
use crate::input::InputError;
use embassy_time::Instant;

#[derive(Debug, Clone, PartialEq)]
pub enum RoasterError {
    TemperatureOutOfRange { source: Option<&'static str> },
    SensorFault { source: Option<&'static str> },
    InvalidState { source: Option<&'static str> },
    PidError { source: Option<&'static str> },
    HardwareError { source: Option<&'static str> },
    EmergencyShutdown { source: Option<&'static str> },
}

impl core::fmt::Display for RoasterError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RoasterError::TemperatureOutOfRange { source } => {
                write!(f, "Temperature out of range")?;
                write_source(f, source)
            }
            RoasterError::SensorFault { source } => {
                write!(f, "Sensor fault")?;
                write_source(f, source)
            }
            RoasterError::InvalidState { source } => {
                write!(f, "Invalid state")?;
                write_source(f, source)
            }
            RoasterError::PidError { source } => {
                write!(f, "PID error")?;
                write_source(f, source)
            }
            RoasterError::HardwareError { source } => {
                write!(f, "Hardware error")?;
                write_source(f, source)
            }
            RoasterError::EmergencyShutdown { source } => {
                write!(f, "Emergency shutdown")?;
                write_source(f, source)
            }
        }
    }
}

fn write_source(
    f: &mut core::fmt::Formatter<'_>,
    source: &Option<&'static str>,
) -> core::fmt::Result {
    match source {
        Some(s) => write!(f, " (source: {})", s),
        None => Ok(()),
    }
}

impl RoasterError {
    pub fn message_token(&self) -> &'static str {
        match self {
            RoasterError::TemperatureOutOfRange { .. } => "temperature_out_of_range",
            RoasterError::SensorFault { .. } => "sensor_fault",
            RoasterError::InvalidState { .. } => "invalid_state",
            RoasterError::PidError { .. } => "pid_error",
            RoasterError::HardwareError { .. } => "hardware_error",
            RoasterError::EmergencyShutdown { .. } => "emergency_shutdown",
        }
    }
}

impl From<SsrError> for RoasterError {
    fn from(err: SsrError) -> Self {
        RoasterError::HardwareError {
            source: Some(match err {
                SsrError::OutputError { source }
                | SsrError::InputError { source }
                | SsrError::HeatSourceNotDetected { source }
                | SsrError::PwmError { source } => source,
            }),
        }
    }
}

impl From<FanError> for RoasterError {
    fn from(err: FanError) -> Self {
        RoasterError::HardwareError {
            source: Some(match err {
                FanError::InitializationError { source }
                | FanError::InvalidSpeed { source }
                | FanError::PwmError { source }
                | FanError::LedcError { source } => source,
            }),
        }
    }
}

impl From<UartError> for RoasterError {
    fn from(_err: UartError) -> Self {
        RoasterError::HardwareError {
            source: Some("uart_error"),
        }
    }
}

impl From<InputError> for RoasterError {
    fn from(err: InputError) -> Self {
        let token = match err {
            InputError::UartError => "uart_error",
            InputError::ParseError => "parse_error",
            InputError::BufferFull => "input_buffer_full",
        };

        RoasterError::InvalidState {
            source: Some(token),
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
pub struct OutputController {
    continuous_enabled: bool,
}

impl OutputController {
    pub fn new() -> Self {
        OutputController {
            continuous_enabled: false,
        }
    }

    pub async fn process_status(&mut self, _status: &SystemStatus) -> Result<(), RoasterError> {
        Ok(())
    }

    pub fn reset(&mut self) {
        self.continuous_enabled = false;
    }

    pub fn enable_continuous_output(&mut self) {
        self.continuous_enabled = true;
    }

    pub fn disable_continuous_output(&mut self) {
        self.continuous_enabled = false;
    }

    pub fn is_continuous_enabled(&self) -> bool {
        self.continuous_enabled
    }
}
