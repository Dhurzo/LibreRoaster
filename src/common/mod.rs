//! Shared test stubs and helper functions for LibreRoaster.
//!
//! This module now lives inside the library so both unit tests and helper crates
//! can reuse the same stub implementations inside the default `#![no_std]` binary.
//! By relying on `core`/`alloc` primitives instead of `std`, the stubs compile on
//! host builds without opting into the `std` feature while remaining available
//! to `tests_common` and integration suites via `libreroaster::common`.
//!
//! # Usage
//!
//! ```rust
//! use libreroaster::common::{StubFan, StubHeater, StubThermometer};
//! use libreroaster::control::traits::{Fan, Heater, Thermometer};
//! ```

use alloc::vec::Vec;
use core::cell::RefCell;

use crate::config::constants::SsrHardwareStatus;

/// Re-export the hardware trait abstractions used by the test stubs.
pub use crate::control::traits::{Fan, Heater, Thermometer};
/// Re-export the shared control error type returned by the stub methods.
pub use crate::control::RoasterError;

// ============================================================================
// Call history tracking enums
// ============================================================================

/// Records of calls made to StubHeater
#[derive(Debug, Clone, PartialEq)]
pub enum HeaterCall {
    /// set_power was called with the given duty cycle
    SetPower(f32),
    /// get_status was called
    GetStatus,
    /// rearm_hardware_status was called (BUG-02 explicit recovery)
    RearmHardwareStatus,
}

/// Records of calls made to StubFan
#[derive(Debug, Clone, PartialEq)]
pub enum FanCall {
    /// set_speed was called with the given duty cycle
    SetSpeed(f32),
    /// get_speed was called
    GetSpeed,
}

/// Records of calls made to StubThermometer
#[derive(Debug, Clone, PartialEq)]
pub enum ThermometerCall {
    /// read_temperature was called
    ReadTemperature,
}

// ============================================================================
// StubHeater - Heater trait implementation with call history
// ============================================================================

/// Test stub for Heater that tracks all method calls and allows
/// configurable hardware status.
///
/// # Example
///
/// ```rust
/// use libreroaster::common::{StubHeater, HeaterCall};
/// use libreroaster::config::constants::SsrHardwareStatus;
/// use libreroaster::control::traits::Heater;
///
/// let mut heater = StubHeater::new();
/// heater.set_status(SsrHardwareStatus::Available);
///
/// // Record a call
/// heater.set_power(50.0).unwrap();
///
/// // Verify the call happened
/// assert!(heater.has_call(&HeaterCall::SetPower(50.0)));
/// ```
#[derive(Debug)]
pub struct StubHeater {
    /// History of method calls made to this stub
    pub calls: RefCell<Vec<HeaterCall>>,
    /// Configurable hardware status returned by get_status()
    pub status: RefCell<SsrHardwareStatus>,
}

impl StubHeater {
    /// Create a new StubHeater with default values.
    ///
    /// - calls: empty
    /// - status: Available
    pub fn new() -> Self {
        Self {
            calls: RefCell::new(Vec::new()),
            status: RefCell::new(SsrHardwareStatus::Available),
        }
    }

    /// Set the hardware status to be returned by get_status()
    pub fn set_status(&self, status: SsrHardwareStatus) {
        *self.status.borrow_mut() = status;
    }

    /// Check if a specific call was recorded
    pub fn has_call(&self, call: &HeaterCall) -> bool {
        self.calls.borrow().contains(call)
    }

    /// Get all recorded calls
    pub fn get_calls(&self) -> Vec<HeaterCall> {
        self.calls.borrow().clone()
    }

    /// Clear all recorded calls
    pub fn clear_calls(&self) {
        self.calls.borrow_mut().clear();
    }
}

impl Default for StubHeater {
    fn default() -> Self {
        Self::new()
    }
}

impl Heater for StubHeater {
    fn set_power(&mut self, duty: f32) -> Result<(), RoasterError> {
        self.calls.borrow_mut().push(HeaterCall::SetPower(duty));
        Ok(())
    }

    fn get_status(&self) -> SsrHardwareStatus {
        self.calls.borrow_mut().push(HeaterCall::GetStatus);
        *self.status.borrow()
    }

    fn rearm_hardware_status(&mut self) {
        self.calls
            .borrow_mut()
            .push(HeaterCall::RearmHardwareStatus);
        *self.status.borrow_mut() = SsrHardwareStatus::Available;
    }
}

// ============================================================================
// StubFan - Fan trait implementation with call history
// ============================================================================

/// Test stub for Fan that tracks all method calls and stores the
/// current fan speed.
///
/// # Example
///
/// ```rust
/// use libreroaster::common::StubFan;
/// use libreroaster::control::traits::Fan;
///
/// let mut fan = StubFan::new();
/// fan.set_speed(75.0).unwrap();
///
/// // Verify the speed was stored
/// assert_eq!(fan.get_speed(), 75.0);
/// ```
#[derive(Debug)]
pub struct StubFan {
    /// History of method calls made to this stub
    pub calls: RefCell<Vec<FanCall>>,
    /// Current fan speed (0.0 - 100.0)
    pub speed: RefCell<f32>,
}

impl StubFan {
    /// Create a new StubFan with default values.
    ///
    /// - calls: empty
    /// - speed: 0.0
    pub fn new() -> Self {
        Self {
            calls: RefCell::new(Vec::new()),
            speed: RefCell::new(0.0),
        }
    }

    /// Check if a specific call was recorded
    pub fn has_call(&self, call: &FanCall) -> bool {
        self.calls.borrow().contains(call)
    }

    /// Get all recorded calls
    pub fn get_calls(&self) -> Vec<FanCall> {
        self.calls.borrow().clone()
    }

    /// Clear all recorded calls
    pub fn clear_calls(&self) {
        self.calls.borrow_mut().clear();
    }
}

impl Default for StubFan {
    fn default() -> Self {
        Self::new()
    }
}

impl Fan for StubFan {
    fn set_speed(&mut self, duty: f32) -> Result<(), RoasterError> {
        self.calls.borrow_mut().push(FanCall::SetSpeed(duty));
        *self.speed.borrow_mut() = duty;
        Ok(())
    }

    fn emergency_set_speed(&mut self, percentage: f32) -> Result<(), RoasterError> {
        self.calls.borrow_mut().push(FanCall::SetSpeed(percentage));
        *self.speed.borrow_mut() = percentage;
        Ok(())
    }

    fn get_speed(&self) -> f32 {
        self.calls.borrow_mut().push(FanCall::GetSpeed);
        *self.speed.borrow()
    }
}

// ============================================================================
// StubThermometer - Thermometer trait implementation with configurable returns
// ============================================================================

/// Test stub for Thermometer that tracks calls and returns a
/// configurable temperature value.
///
/// # Example
///
/// ```rust
/// use libreroaster::common::StubThermometer;
/// use libreroaster::control::traits::Thermometer;
///
/// let mut thermometer = StubThermometer::with_temp(150.0);
/// let temp = thermometer.read_temperature().unwrap();
/// assert_eq!(temp, 150.0);
/// ```
#[derive(Debug)]
pub struct StubThermometer {
    /// History of method calls made to this stub
    pub calls: RefCell<Vec<ThermometerCall>>,
    /// Configurable temperature to return
    pub temp: RefCell<f32>,
}

impl StubThermometer {
    /// Create a new StubThermometer with the given initial temperature.
    ///
    /// - calls: empty
    /// - temp: initial_value
    pub fn with_temp(initial_temp: f32) -> Self {
        Self {
            calls: RefCell::new(Vec::new()),
            temp: RefCell::new(initial_temp),
        }
    }

    /// Set the temperature to be returned by read_temperature()
    pub fn set_temp(&self, temp: f32) {
        *self.temp.borrow_mut() = temp;
    }

    /// Check if a specific call was recorded
    pub fn has_call(&self, call: &ThermometerCall) -> bool {
        self.calls.borrow().contains(call)
    }

    /// Get all recorded calls
    pub fn get_calls(&self) -> Vec<ThermometerCall> {
        self.calls.borrow().clone()
    }

    /// Clear all recorded calls
    pub fn clear_calls(&self) {
        self.calls.borrow_mut().clear();
    }
}

impl Default for StubThermometer {
    fn default() -> Self {
        Self::with_temp(0.0)
    }
}

impl Thermometer for StubThermometer {
    fn read_temperature(&mut self) -> Result<f32, RoasterError> {
        self.calls
            .borrow_mut()
            .push(ThermometerCall::ReadTemperature);
        Ok(*self.temp.borrow())
    }
}
