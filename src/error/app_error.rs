use crate::input::InputError;
use crate::memory::ERROR_MSG_MAX_LEN;
use core::fmt;
#[cfg(feature = "std")]
extern crate std;
use alloc::string::String;

#[derive(Debug, Clone, PartialEq)]
pub enum AppError {
    Temperature {
        message: heapless::String<ERROR_MSG_MAX_LEN>,
        source: TemperatureError,
    },

    Control {
        source: ControlError,
    },

    Hardware {
        source: HardwareError,
    },

    Communication {
        source: CommunicationError,
    },

    Initialization {
        source: InitError,
    },

    Safety {
        severity: SafetyLevel,
    },

    Configuration {
        source: ConfigError,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum TemperatureError {
    OutOfRange,
    SensorFault,
    ReadingTimeout,
    InvalidValue,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ControlError {
    PidError,
    InvalidState,
    CommandFailed,
    OutputError,
    EmergencyShutdown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HardwareError {
    UartError,
    FanError,
    SsrError,
    GpioError,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommunicationError {
    UartError,
    ProtocolError,
    SerializationError,
    TimeoutError,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InitError {
    ServiceContainer { what: &'static str, reason: String },
    HardwareInit { what: &'static str, reason: String },
    TaskSpawn { what: &'static str, reason: String },
    MemoryAllocation { what: &'static str, reason: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfigError {
    InvalidValue,
    MissingConfig,
    CorruptedData,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SafetyLevel {
    Warning,
    Critical,
    Emergency,
}

impl AppError {
    pub fn is_recoverable(&self) -> bool {
        match self {
            AppError::Temperature { source, .. } => matches!(
                source,
                TemperatureError::ReadingTimeout | TemperatureError::InvalidValue
            ),
            AppError::Communication { source } => {
                matches!(source, CommunicationError::TimeoutError)
            }
            AppError::Hardware { .. } | AppError::Control { .. } => false,
            AppError::Safety { severity } => matches!(severity, SafetyLevel::Warning),
            AppError::Initialization { .. } | AppError::Configuration { .. } => false,
        }
    }

    pub fn requires_emergency_shutdown(&self) -> bool {
        match self {
            AppError::Temperature { source, .. } => matches!(source, TemperatureError::OutOfRange),
            AppError::Hardware { source } => matches!(source, HardwareError::SsrError),
            _ => false,
        }
    }

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

    pub fn user_message(&self) -> &'static str {
        match self {
            AppError::Temperature { source, .. } => match source {
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
                ControlError::EmergencyShutdown => "Emergency shutdown",
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
                InitError::ServiceContainer { what: _, .. } => "System initialization failed",
                InitError::HardwareInit { what: _, .. } => "Hardware initialization failed",
                InitError::TaskSpawn { what: _, .. } => "Task startup failed",
                InitError::MemoryAllocation { what: _, .. } => "Memory allocation failed",
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

    pub fn source(&self) -> Option<&str> {
        match self {
            AppError::Temperature { source, .. } => Some(match source {
                TemperatureError::OutOfRange => "temperature_out_of_range",
                TemperatureError::SensorFault => "sensor_fault",
                TemperatureError::ReadingTimeout => "sensor_timeout",
                TemperatureError::InvalidValue => "sensor_invalid",
            }),
            AppError::Control { source } => Some(match source {
                ControlError::PidError => "pid_error",
                ControlError::InvalidState => "invalid_state",
                ControlError::CommandFailed => "command_failed",
                ControlError::OutputError => "output_error",
                ControlError::EmergencyShutdown => "emergency_shutdown",
            }),
            AppError::Hardware { source } => Some(match source {
                HardwareError::UartError => "uart_error",
                HardwareError::FanError => "fan_error",
                HardwareError::SsrError => "ssr_error",
                HardwareError::GpioError => "gpio_error",
            }),
            AppError::Communication { source } => Some(match source {
                CommunicationError::UartError => "comm_uart_error",
                CommunicationError::ProtocolError => "protocol_error",
                CommunicationError::SerializationError => "serialization_error",
                CommunicationError::TimeoutError => "timeout_error",
            }),
            AppError::Initialization { source } => Some(match source {
                InitError::ServiceContainer { .. } => "service_container_init_failed",
                InitError::HardwareInit { .. } => "hardware_init_failed",
                InitError::TaskSpawn { .. } => "task_spawn_failed",
                InitError::MemoryAllocation { .. } => "memory_alloc_failed",
            }),
            AppError::Safety { severity } => Some(match severity {
                SafetyLevel::Warning => "safety_warning",
                SafetyLevel::Critical => "safety_critical",
                SafetyLevel::Emergency => "safety_emergency",
            }),
            AppError::Configuration { source } => Some(match source {
                ConfigError::InvalidValue => "config_invalid",
                ConfigError::MissingConfig => "config_missing",
                ConfigError::CorruptedData => "config_corrupted",
            }),
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} (source: {})",
            self.category(),
            self.user_message(),
            self.source().unwrap_or("unknown")
        )
    }
}

pub trait ErrorRecovery {
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

impl From<crate::control::RoasterError> for AppError {
    fn from(err: crate::control::RoasterError) -> Self {
        match err {
            crate::control::RoasterError::TemperatureOutOfRange { .. } => AppError::Temperature {
                message: heapless::String::<ERROR_MSG_MAX_LEN>::try_from(
                    "Temperature out of range",
                )
                .unwrap_or_default(),
                source: TemperatureError::OutOfRange,
            },
            crate::control::RoasterError::SensorFault { .. } => AppError::Temperature {
                message: heapless::String::<ERROR_MSG_MAX_LEN>::try_from(
                    "Temperature sensor fault",
                )
                .unwrap_or_default(),
                source: TemperatureError::SensorFault,
            },
            crate::control::RoasterError::InvalidState { .. } => AppError::Control {
                source: ControlError::InvalidState,
            },
            crate::control::RoasterError::PidError { .. } => AppError::Control {
                source: ControlError::PidError,
            },
            crate::control::RoasterError::HardwareError { .. } => AppError::Hardware {
                source: HardwareError::SsrError,
            },
            crate::control::RoasterError::EmergencyShutdown { .. } => AppError::Control {
                source: ControlError::EmergencyShutdown,
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

impl From<crate::hardware::ssr::SsrError> for AppError {
    fn from(_err: crate::hardware::ssr::SsrError) -> Self {
        AppError::Hardware {
            source: HardwareError::SsrError,
        }
    }
}

impl From<InputError> for AppError {
    fn from(err: InputError) -> Self {
        match err {
            InputError::UartError => AppError::Communication {
                source: CommunicationError::UartError,
            },
            InputError::ParseError => AppError::Communication {
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
    use crate::control::RoasterError;
    use crate::hardware::fan::FanError;
    use crate::hardware::ssr::SsrError;
    use crate::input::InputError;

    #[test]
    fn test_error_categorization() {
        let temp_err = AppError::Temperature {
            message: heapless::String::<ERROR_MSG_MAX_LEN>::try_from("Test").unwrap_or_default(),
            source: TemperatureError::OutOfRange,
        };
        assert_eq!(temp_err.category(), "temperature");
        assert!(!temp_err.is_recoverable());
        assert!(temp_err.requires_emergency_shutdown());
    }

    #[test]
    fn test_source_propagation_from_roaster_error() {
        let roaster_err = crate::control::RoasterError::TemperatureOutOfRange {
            source: Some("sensor_read"),
        };
        let app_err = AppError::from(roaster_err);

        assert_eq!(app_err.source(), Some("temperature_out_of_range"));
        assert!(format!("{}", app_err).contains("source:"));
    }

    #[test]
    fn test_user_messages() {
        let err = AppError::Temperature {
            message: heapless::String::<ERROR_MSG_MAX_LEN>::try_from("Test").unwrap_or_default(),
            source: TemperatureError::SensorFault,
        };
        assert_eq!(err.user_message(), "Temperature sensor malfunction");
    }

    #[test]
    fn test_source_from_hardware_errors() {
        let fan_err = crate::hardware::fan::FanError::PwmError {
            source: "set_duty_failed",
        };
        let app_err = AppError::from(fan_err);

        assert_eq!(app_err.source(), Some("fan_error"));
    }

    #[test]
    fn test_boundary_contract_hardware_to_control() {
        let ssr_err = SsrError::PwmError { source: "test" };
        let ctrl_err = RoasterError::from(ssr_err);
        assert!(matches!(ctrl_err, RoasterError::HardwareError { .. }));
    }

    #[test]
    fn test_boundary_contract_control_to_app() {
        let ctrl_err = RoasterError::TemperatureOutOfRange {
            source: Some("sensor"),
        };
        let app_err = AppError::from(ctrl_err);
        assert!(matches!(app_err, AppError::Temperature { .. }));
        assert_eq!(app_err.source(), Some("temperature_out_of_range"));
    }

    #[test]
    fn test_boundary_contract_hardware_direct_to_app() {
        let fan_err = FanError::PwmError { source: "test" };
        let app_err = AppError::from(fan_err);
        assert!(matches!(app_err, AppError::Hardware { .. }));
        assert_eq!(app_err.source(), Some("fan_error"));
    }

    #[test]
    fn test_boundary_contract_input_to_app() {
        let input_err = InputError::ParseError;
        let app_err = AppError::from(input_err);
        assert!(matches!(app_err, AppError::Communication { .. }));
    }

    #[test]
    fn test_display_outputs_expected_tokens() {
        let err = AppError::Temperature {
            message: heapless::String::<ERROR_MSG_MAX_LEN>::try_from("Test").unwrap_or_default(),
            source: TemperatureError::OutOfRange,
        };
        let repr = format!("{}", err);
        assert!(repr.contains("temperature:"));
        assert!(repr.contains("source:"));
    }

    #[test]
    fn test_debug_contains_variant_name() {
        let err = AppError::Control {
            source: ControlError::PidError,
        };
        let repr = format!("{:?}", err);
        assert!(repr.contains("Control"));
        assert!(repr.contains("PidError"));
    }
}
