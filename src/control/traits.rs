//! Hardware actuator traits: `Thermometer`, `Heater`, `Fan`.
//!
//! These object-safe traits let `RoasterControl` drive heater, fan, and sensor
//! hardware behind `Box<dyn>` handles supplied by `AppBuilder`, with blanket
//! `&mut T` impls so owned and borrowed actuators are interchangeable.

use crate::config::constants::SsrHardwareStatus;
use crate::control::RoasterError;

/// Read-only temperature source (thermocouple conversion hub).
pub trait Thermometer: Send {
    /// Reads the current temperature in degrees Celsius.
    fn read_temperature(&mut self) -> Result<f32, RoasterError>;
}

/// Heater actuator (SSR) with health monitoring and explicit re-arm support.
pub trait Heater: Send {
    /// Sets heater duty cycle (0..=100 %).
    fn set_power(&mut self, duty: f32) -> Result<(), RoasterError>;

    /// Returns the SSR hardware-availability status.
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

    /// Returns the most recent duty-change magnitude in ticks (diagnostics).
    fn last_duty_delta_ticks(&self) -> i16 {
        0
    }

    /// Returns the number of automatic recovery retries since last drive.
    fn last_retry_count(&self) -> u8 {
        0
    }
}

/// Fan actuator with normal and emergency speed control.
pub trait Fan: Send {
    /// Sets fan speed (0..=100 %).
    fn set_speed(&mut self, duty: f32) -> Result<(), RoasterError>;

    /// Forces a fan speed outside the normal control path (safety override).
    fn emergency_set_speed(&mut self, percentage: f32) -> Result<(), RoasterError>;

    /// Returns the current fan speed (0..=100 %).
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
