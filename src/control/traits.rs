use crate::config::constants::SsrHardwareStatus;
use crate::control::RoasterError;

pub trait Thermometer: Send {
    fn read_temperature(&mut self) -> Result<f32, RoasterError>;
}

pub trait Heater: Send {
    fn set_power(&mut self, duty: f32) -> Result<(), RoasterError>;

    fn get_status(&self) -> SsrHardwareStatus;

    fn last_duty_delta_ticks(&self) -> i16 {
        0
    }

    fn last_retry_count(&self) -> u8 {
        0
    }
}

pub trait Fan: Send {
    fn set_speed(&mut self, duty: f32) -> Result<(), RoasterError>;

    fn get_speed(&self) -> f32 {
        0.0
    }
}

impl<T: Heater + ?Sized> Heater for &mut T {
    fn set_power(&mut self, duty: f32) -> Result<(), RoasterError> {
        (**self).set_power(duty)
    }

    fn get_status(&self) -> SsrHardwareStatus {
        (**self).get_status()
    }
}

impl<T: Fan + ?Sized> Fan for &mut T {
    fn set_speed(&mut self, duty: f32) -> Result<(), RoasterError> {
        (**self).set_speed(duty)
    }

    fn get_speed(&self) -> f32 {
        (**self).get_speed()
    }
}

impl<T: Thermometer + ?Sized> Thermometer for &mut T {
    fn read_temperature(&mut self) -> Result<f32, RoasterError> {
        (**self).read_temperature()
    }
}
