#![cfg(all(test, feature = "test", not(target_arch = "riscv32")))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use libreroaster::control::traits::{Fan, Heater, Thermometer};
use libreroaster::control::RoasterError;
use libreroaster::error::app_error::{AppError, CommunicationError, InitError};
use libreroaster::hardware::max31856::Max31856Error;
use libreroaster::hardware::ssr::SsrError;
use libreroaster::hardware::test_mocks::{MockFan, MockSsr, MockThermometer};

#[test]
fn test_error_propagation_from_hardware_to_app() {
    let mut mock_sensor = MockThermometer::new();
    mock_sensor.inject_error(Max31856Error::CommunicationError { source: "spi" });

    let result = mock_sensor.read_temperature();
    assert!(result.is_err());

    let app_err = AppError::from(result.unwrap_err());
    assert!(matches!(app_err, AppError::Temperature { .. }));
    assert_eq!(app_err.source(), Some("sensor_fault"));
}

#[test]
fn test_error_propagation_with_source_chaining() {
    let mut mock_ssr = MockSsr::new();
    mock_ssr.inject_error(SsrError::PwmError { source: "duty" });

    let result = mock_ssr.set_power(50.0);
    assert!(result.is_err());

    let app_err = AppError::from(result.unwrap_err());
    assert!(matches!(app_err, AppError::Hardware { .. }));
    assert_eq!(app_err.source(), Some("ssr_error"));
}

#[test]
fn test_error_recovery_strategies() {
    let timeout_err = AppError::Communication {
        source: CommunicationError::TimeoutError,
    };
    assert!(timeout_err.is_recoverable());

    let init_err = AppError::Initialization {
        source: InitError::HardwareInit {
            what: "spi",
            reason: "timeout".to_string(),
        },
    };
    assert!(!init_err.is_recoverable());
}

#[test]
fn test_boundary_contract_conversions() {
    let sensor_err = Max31856Error::FaultDetected { source: "fault" };
    let roaster_err = RoasterError::from(sensor_err);
    let app_err = AppError::from(roaster_err);

    assert!(matches!(app_err, AppError::Temperature { .. }));
    assert_eq!(app_err.source(), Some("sensor_fault"));
}

#[test]
fn test_fan_error_injection() {
    let mut mock_fan = MockFan::new();
    mock_fan.inject_error(libreroaster::hardware::fan::FanError::PwmError { source: "test" });

    let result = mock_fan.set_speed(30.0);
    assert!(result.is_err());

    let app_err = AppError::from(result.unwrap_err());
    assert!(matches!(app_err, AppError::Hardware { .. }));
}
