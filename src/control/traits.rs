use crate::config::constants::SsrHardwareStatus;
use crate::control::RoasterError;

pub trait Thermometer: Send {
    fn read_temperature(&mut self) -> Result<f32, RoasterError>;
}

/// Async thermometer trait for non-blocking temperature reads.
/// Must be implemented separately from Thermometer because async methods
/// make a trait not dyn-compatible.
#[allow(async_fn_in_trait)]
pub trait AsyncThermometer: Send {
    async fn read_temperature_async(&mut self) -> Result<f32, RoasterError>;
}

pub trait Heater: Send {
    fn set_power(&mut self, duty: f32) -> Result<(), RoasterError>;

    fn get_status(&self) -> SsrHardwareStatus;

    /// Periodic health check — called every ~1s by the control loop.
    /// Implementations should re-detect heat source, verify PWM integrity, etc.
    /// `current_time_ms` is a real monotonic timestamp in milliseconds so the
    /// implementation can apply its own rate-limiting using the actual wall clock.
    /// Default implementation is a no-op.
    fn periodic_health_check(&mut self, _current_time_ms: u32) {}

    fn last_duty_delta_ticks(&self) -> i16 {
        0
    }

    fn last_retry_count(&self) -> u8 {
        0
    }
}

pub trait Fan: Send {
    fn set_speed(&mut self, duty: f32) -> Result<(), RoasterError>;

    fn emergency_set_speed(&mut self, percentage: f32) -> Result<(), RoasterError>;

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

    fn periodic_health_check(&mut self, current_time_ms: u32) {
        (**self).periodic_health_check(current_time_ms);
    }
}

impl<T: Fan + ?Sized> Fan for &mut T {
    fn set_speed(&mut self, duty: f32) -> Result<(), RoasterError> {
        (**self).set_speed(duty)
    }

    fn emergency_set_speed(&mut self, percentage: f32) -> Result<(), RoasterError> {
        (**self).emergency_set_speed(percentage)
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

impl<T: AsyncThermometer + ?Sized> AsyncThermometer for &mut T {
    async fn read_temperature_async(&mut self) -> Result<f32, RoasterError> {
        (**self).read_temperature_async().await
    }
}
