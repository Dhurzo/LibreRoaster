use crate::control::traits::{Fan, Heater, Thermometer};
use crate::control::RoasterError;
use crate::hardware::{fan::FanError, max31856::Max31856Error, ssr::SsrError};

/// Mock thermometer that can return a configurable temperature or inject errors.
#[derive(Debug, Clone)]
pub struct MockThermometer {
    inject_error: Option<Max31856Error>,
    default_temp: f32,
}

impl MockThermometer {
    pub fn new() -> Self {
        Self {
            inject_error: None,
            default_temp: 25.0,
        }
    }

    pub fn inject_error(&mut self, error: Max31856Error) {
        self.inject_error = Some(error);
    }

    pub fn clear_error(&mut self) {
        self.inject_error = None;
    }

    pub fn set_default_temp(&mut self, temp: f32) {
        self.default_temp = temp;
    }
}

impl Thermometer for MockThermometer {
    fn read_temperature(&mut self) -> Result<f32, RoasterError> {
        if let Some(error) = self.inject_error {
            return Err(RoasterError::from(error));
        }

        Ok(self.default_temp)
    }
}

/// Mock SSR/heater that exposes error injection for safe shutdown validation.
#[derive(Debug, Clone)]
pub struct MockSsr {
    inject_error: Option<SsrError>,
    current_power: f32,
}

impl MockSsr {
    pub fn new() -> Self {
        Self {
            inject_error: None,
            current_power: 0.0,
        }
    }

    pub fn inject_error(&mut self, error: SsrError) {
        self.inject_error = Some(error);
    }

    pub fn clear_error(&mut self) {
        self.inject_error = None;
    }
}

impl Heater for MockSsr {
    fn set_power(&mut self, duty: f32) -> Result<(), RoasterError> {
        if let Some(error) = self.inject_error.clone() {
            return Err(RoasterError::from(error));
        }

        self.current_power = duty;
        Ok(())
    }

    fn get_status(&self) -> crate::config::constants::SsrHardwareStatus {
        crate::config::constants::SsrHardwareStatus::Available
    }

    fn last_duty_delta_ticks(&self) -> i16 {
        0
    }

    fn last_retry_count(&self) -> u8 {
        0
    }
}

/// Mock fan controller that either reports a speed or injects hardware errors.
#[derive(Debug, Clone)]
pub struct MockFan {
    inject_error: Option<FanError>,
    current_speed: f32,
}

impl MockFan {
    pub fn new() -> Self {
        Self {
            inject_error: None,
            current_speed: 0.0,
        }
    }

    pub fn inject_error(&mut self, error: FanError) {
        self.inject_error = Some(error);
    }

    pub fn clear_error(&mut self) {
        self.inject_error = None;
    }
}

impl Fan for MockFan {
    fn set_speed(&mut self, duty: f32) -> Result<(), RoasterError> {
        if let Some(error) = self.inject_error.clone() {
            return Err(RoasterError::from(error));
        }

        self.current_speed = duty;
        Ok(())
    }

    fn get_speed(&self) -> f32 {
        self.current_speed
    }
}
