use crate::config::constants::SsrHardwareStatus;
use crate::control::RoasterError;

pub trait Thermometer: Send {
    fn read_temperature(&mut self) -> Result<f32, RoasterError>;
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

    /// BUG-02 (2026-08-21): re-arm the heater's hardware-availability state
    /// machine after an explicit operator recovery (`OFF`/`START`/`PREHEAT`/
    /// `StopRoast`). `NotDetected`/`Error` force the output to 0 % and the
    /// automatic re-detection paths can never run at 0 % duty, so without
    /// this the heater stays dead until a power cycle. A real physical fault
    /// is re-detected within the debounce window once the heater is driven
    /// ≥ 50 % again — the re-arm removes irreversibility, not protection.
    /// Default implementation is a no-op (stubs/mocks need no change).
    fn rearm_hardware_status(&mut self) {}

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

    fn rearm_hardware_status(&mut self) {
        (**self).rearm_hardware_status();
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
