use crate::control::traits::Fan;
use crate::control::RoasterError;

#[derive(Debug, Clone, PartialEq)]
pub enum FanError {
    InitializationError { source: &'static str },
    InvalidSpeed { source: &'static str },
    PwmError { source: &'static str },
    LedcError { source: &'static str },
}

impl embedded_hal::digital::Error for FanError {
    fn kind(&self) -> embedded_hal::digital::ErrorKind {
        embedded_hal::digital::ErrorKind::Other
    }
}

pub struct FanController {
    current_speed: f32,
}

impl FanController {
    pub fn new() -> Result<Self, FanError> {
        Ok(Self { current_speed: 0.0 })
    }

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
