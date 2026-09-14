//! Host-build variant of the fan controller.
//!
//! A no-hardware `FanController` used on host/test targets in place of the
//! LEDC-backed `hardware::fan`. Stores the requested speed and clamps it to
//! 0–100 %; no PWM is driven.

use crate::control::traits::Fan;
use crate::control::RoasterError;

/// Errors returned by host fan control operations.
#[derive(Debug, Clone, PartialEq)]
pub enum FanError {
    /// Controller initialisation failed.
    InitializationError { source: &'static str },
    /// Requested speed was outside the valid range.
    InvalidSpeed { source: &'static str },
    /// PWM write failed.
    PwmError { source: &'static str },
    /// Underlying peripheral error.
    LedcError { source: &'static str },
}

impl embedded_hal::digital::Error for FanError {
    fn kind(&self) -> embedded_hal::digital::ErrorKind {
        embedded_hal::digital::ErrorKind::Other
    }
}

/// Host fan controller: stores speed only, drives no hardware.
pub struct FanController {
    /// Last requested fan speed in percent (0–100).
    current_speed: f32,
}

impl FanController {
    /// Create a host fan controller initialised at 0 %.
    pub fn new() -> Result<Self, FanError> {
        Ok(Self { current_speed: 0.0 })
    }

    /// Store the requested fan speed, clamped to 0–100 %.
    pub fn set_speed(&mut self, speed_percent: f32) -> Result<(), FanError> {
        self.current_speed = speed_percent.clamp(0.0, 100.0);
        Ok(())
    }

    pub fn get_speed(&self) -> f32 {
        self.current_speed
    }

    pub fn is_enabled(&self) -> bool {
        self.current_speed > 0.0
    }
}

impl Default for FanController {
    fn default() -> Self {
        Self { current_speed: 0.0 }
    }
}

impl Fan for FanController {
    fn set_speed(&mut self, duty: f32) -> Result<(), RoasterError> {
        self.set_speed(duty)
            .map_err(|_| RoasterError::HardwareError {
                source: Some("fan_set_failed"),
            })
    }

    fn emergency_set_speed(&mut self, percentage: f32) -> Result<(), RoasterError> {
        self.current_speed = percentage.clamp(0.0, 100.0);
        Ok(())
    }

    fn get_speed(&self) -> f32 {
        self.current_speed
    }
}
