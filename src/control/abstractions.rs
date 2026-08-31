//! Shared control-layer abstractions: errors, command handler trait, output flag.
//!
//! Defines `RoasterError` (the unified error type with `From` conversions from
//! hardware/input errors), the `RoasterCommandHandler` trait for dispatch, and
//! `OutputController` (tracks the continuous-output enable flag feeding the
//! streaming state machine).

use crate::config::{RoasterCommand, SystemStatus};
use crate::hardware::{fan::FanError, ssr::SsrError, uart::UartError};
use crate::input::InputError;
use embassy_time::Instant;

/// Unified control-layer error type; wraps hardware/input faults with a source token.
#[derive(Debug, Clone, PartialEq)]
pub enum RoasterError {
    /// A temperature reading fell outside the safe operating range.
    TemperatureOutOfRange { source: Option<&'static str> },
    /// A thermocouple reported a fault (open/short/out-of-range).
    SensorFault { source: Option<&'static str> },
    /// The requested command is illegal in the current roast state.
    InvalidState { source: Option<&'static str> },
    /// The PID controller produced an unrecoverable error.
    PidError { source: Option<&'static str> },
    /// A heater/fan hardware error (SSR, PWM, LEDC, or UART).
    HardwareError { source: Option<&'static str> },
    /// An emergency shutdown latched the actuators off.
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
    /// Returns the stable wire token for this error (e.g. `sensor_fault`).
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

    /// Returns the optional underlying source token attached to the error.
    pub fn source(&self) -> Option<&'static str> {
        match self {
            RoasterError::TemperatureOutOfRange { source }
            | RoasterError::SensorFault { source }
            | RoasterError::InvalidState { source }
            | RoasterError::PidError { source }
            | RoasterError::HardwareError { source }
            | RoasterError::EmergencyShutdown { source } => *source,
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

/// Trait for components that can apply a `RoasterCommand` to their state.
pub trait RoasterCommandHandler {
    /// Applies `command`, updating `status`; returns an error on rejection/fault.
    fn handle_command(
        &mut self,
        command: RoasterCommand,
        current_time: Instant,
        status: &mut SystemStatus,
    ) -> Result<(), RoasterError>;

    /// Returns true if this handler accepts `command`.
    fn can_handle(&self, command: RoasterCommand) -> bool;
}

/// Tracks the continuous-output enable flag consumed by the streaming state machine.
#[derive(Debug, Default)]
pub struct OutputController {
    continuous_enabled: bool,
}

impl OutputController {
    /// Returns a controller with continuous output disabled.
    pub fn new() -> Self {
        OutputController {
            continuous_enabled: false,
        }
    }

    // Audit M-A5 (2026-08-11): the `process_status` no-op stub was removed —
    // it returned `Ok(())` unconditionally (fake `OutputController` API). The
    // real continuous-output state machine lives in `MutableArtisanFormatter`
    // (driven by `emit_telemetry_stage` in tasks.rs); this type now only
    // tracks the enable flag feeding `CommandDispatcher::is_streaming`.
    /// Enables continuous (`#`-line) telemetry output.
    pub fn enable_continuous_output(&mut self) {
        self.continuous_enabled = true;
    }

    /// Disables continuous telemetry output.
    pub fn disable_continuous_output(&mut self) {
        self.continuous_enabled = false;
    }

    /// Returns the current continuous-output enable flag.
    pub fn is_continuous_enabled(&self) -> bool {
        self.continuous_enabled
    }
}
