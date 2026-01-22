//! Unified error handling system for LibreRoaster
//! Provides consistent error types and handling strategies across all modules

use core::fmt;

/// Unified application error type
/// Consolidates errors from all modules with proper context
#[derive(Debug, Clone, PartialEq)]
pub enum AppError {
    /// Temperature-related errors
    Temperature { source: TemperatureError },

    /// Control system errors
    Control { source: ControlError },

    /// Hardware communication errors
    Hardware { source: HardwareError },

    /// Input/communication errors
    Communication { source: CommunicationError },

    /// System initialization errors
    Initialization { source: InitError },

    /// Safety system errors
    Safety { severity: SafetyLevel },

    /// Configuration errors
    Configuration { source: ConfigError },
}

/// Specific temperature-related errors
#[derive(Debug, Clone, PartialEq)]
pub enum TemperatureError {
    OutOfRange,
    SensorFault,
    ReadingTimeout,
    InvalidValue,
}

/// Control system errors
#[derive(Debug, Clone, PartialEq)]
pub enum ControlError {
    PidError,
    InvalidState,
    CommandFailed,
    OutputError,
}

/// Hardware-related errors
#[derive(Debug, Clone, PartialEq)]
pub enum HardwareError {
    UartError,
    FanError,
    SsrError,
    GpioError,
}

/// Communication errors
#[derive(Debug, Clone, PartialEq)]
pub enum CommunicationError {
    UartError,
    ProtocolError,
    SerializationError,
    TimeoutError,
}

/// Initialization errors
#[derive(Debug, Clone, PartialEq)]
pub enum InitError {
    ServiceContainer,
    HardwareInit,
    TaskSpawn,
    MemoryAllocation,
}

/// Configuration errors
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigError {
    InvalidValue,
    MissingConfig,
    CorruptedData,
}

/// Safety error severity levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SafetyLevel {
    Warning,
    Critical,
    Emergency,
}

impl AppError {
    /// Check if the error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            AppError::Temperature { source } => match source {
                TemperatureError::ReadingTimeout | TemperatureError::InvalidValue => true,
                _ => false,
            },
            AppError::Communication { source } => match source {
                CommunicationError::TimeoutError => true,
                _ => false,
            },
            AppError::Hardware { .. } | AppError::Control { .. } => false,
            AppError::Safety { severity } => match severity {
                SafetyLevel::Warning => true,
                _ => false,
            },
            AppError::Initialization { .. } | AppError::Configuration { .. } => false,
        }
    }

    /// Check if the error requires immediate system shutdown
    pub fn requires_emergency_shutdown(&self) -> bool {
        match self {
            AppError::Safety { severity } => severity == &SafetyLevel::Emergency,
            AppError::Temperature { source } => matches!(source, TemperatureError::OutOfRange),
            AppError::Hardware { source } => matches!(source, HardwareError::SsrError),
            _ => false,
        }
    }

    /// Get error category for logging/metrics
    pub fn category(&self) -> &'static str {
        match self {
            AppError::Temperature { .. } => "temperature",
            AppError::Control { .. } => "control",
            AppError::Hardware { .. } => "hardware",
            AppError::Communication { .. } => "communication",
            AppError::Initialization { .. } => "initialization",
            AppError::Safety { .. } => "safety",
            AppError::Configuration { .. } => "configuration",
        }
    }

    /// Get user-friendly error message
    pub fn user_message(&self) -> &'static str {
        match self {
            AppError::Temperature { source } => match source {
                TemperatureError::OutOfRange => "Temperature out of safe range",
                TemperatureError::SensorFault => "Temperature sensor malfunction",
                TemperatureError::ReadingTimeout => "Temperature reading timeout",
                TemperatureError::InvalidValue => "Invalid temperature reading",
            },
            AppError::Control { source } => match source {
                ControlError::PidError => "Control system error",
                ControlError::InvalidState => "Invalid system state",
                ControlError::CommandFailed => "Command execution failed",
                ControlError::OutputError => "Output control error",
            },
            AppError::Hardware { source } => match source {
                HardwareError::UartError => "Communication hardware error",
                HardwareError::FanError => "Fan controller error",
                HardwareError::SsrError => "Heating element error",
                HardwareError::GpioError => "GPIO hardware error",
            },
            AppError::Communication { source } => match source {
                CommunicationError::UartError => "Communication error",
                CommunicationError::ProtocolError => "Protocol error",
                CommunicationError::SerializationError => "Data formatting error",
                CommunicationError::TimeoutError => "Communication timeout",
            },
            AppError::Initialization { source } => match source {
                InitError::ServiceContainer => "System initialization failed",
                InitError::HardwareInit => "Hardware initialization failed",
                InitError::TaskSpawn => "Task startup failed",
                InitError::MemoryAllocation => "Memory allocation failed",
            },
            AppError::Safety { severity } => match severity {
                SafetyLevel::Warning => "Safety warning",
                SafetyLevel::Critical => "Safety critical error",
                SafetyLevel::Emergency => "Emergency shutdown required",
            },
            AppError::Configuration { source } => match source {
                ConfigError::InvalidValue => "Invalid configuration",
                ConfigError::MissingConfig => "Missing configuration",
                ConfigError::CorruptedData => "Configuration data corrupted",
            },
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.category(), self.user_message())
    }
}

/// Error recovery strategies
pub trait ErrorRecovery {
    /// Attempt to recover from the error
    fn recover(&mut self, error: &AppError) -> Result<RecoveryResult, RecoveryError>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecoveryResult {
    Recovered,
    PartialRecovery,
    Failed,
    RequiresManualIntervention,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecoveryError {
    NotRecoverable,
    RecoveryFailed,
    SystemInconsistent,
}

/// Convert module-specific errors to AppError
impl From<crate::control::RoasterError> for AppError {
    fn from(err: crate::control::RoasterError) -> Self {
        match err {
            crate::control::RoasterError::TemperatureOutOfRange => AppError::Temperature {
                source: TemperatureError::OutOfRange,
            },
            crate::control::RoasterError::SensorFault => AppError::Temperature {
                source: TemperatureError::SensorFault,
            },
            crate::control::RoasterError::InvalidState => AppError::Control {
                source: ControlError::InvalidState,
            },
            crate::control::RoasterError::PidError => AppError::Control {
                source: ControlError::PidError,
            },
        }
    }
}

impl From<crate::hardware::uart::UartError> for AppError {
    fn from(_err: crate::hardware::uart::UartError) -> Self {
        AppError::Communication {
            source: CommunicationError::UartError,
        }
    }
}

impl From<crate::hardware::fan::FanError> for AppError {
    fn from(_err: crate::hardware::fan::FanError) -> Self {
        AppError::Hardware {
            source: HardwareError::FanError,
        }
    }
}

impl From<crate::input::InputError> for AppError {
    fn from(err: crate::input::InputError) -> Self {
        match err {
            crate::input::InputError::UartError => AppError::Communication {
                source: CommunicationError::UartError,
            },
            crate::input::InputError::ParseError => AppError::Communication {
                source: CommunicationError::ProtocolError,
            },
            _ => AppError::Communication {
                source: CommunicationError::UartError,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_categorization() {
        let temp_err = AppError::Temperature {
            message: "Test".to_string(),
            source: TemperatureError::OutOfRange,
        };
        assert_eq!(temp_err.category(), "temperature");
        assert!(!temp_err.is_recoverable());
        assert!(temp_err.requires_emergency_shutdown());
    }

    #[test]
    fn test_error_conversion() {
        let roaster_err = crate::control::RoasterError::TemperatureOutOfRange;
        let app_err = AppError::from(roaster_err);

        assert!(matches!(app_err, AppError::Temperature { .. }));
        assert!(app_err.requires_emergency_shutdown());
    }

    #[test]
    fn test_user_messages() {
        let err = AppError::Temperature {
            message: "Test".to_string(),
            source: TemperatureError::SensorFault,
        };
        assert_eq!(err.user_message(), "Temperature sensor malfunction");
    }
}
