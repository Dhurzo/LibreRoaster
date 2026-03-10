#![cfg(all(test, not(target_arch = "riscv32")))]

//! Thin shim so integration tests can continue importing the shared stubs
//! without duplicating their implementations.
//!
//! The real definitions now live in `libreroaster::common`, and the shim now
//! also re-exports the helper that wires a `SensorConversionHub` into
//! `RoasterControl` so helper suites can build the control consistently.

pub use libreroaster::common::{
    collect_output, reset_channels, FanCall, HeaterCall, StubFan, StubHeater, StubThermometer,
    ThermometerCall,
};

pub use libreroaster::hardware::sensors::SensorConversionHub;

pub use libreroaster::control::traits::{Fan, Heater, Thermometer};
use libreroaster::control::RoasterControl;
pub use libreroaster::control::RoasterError;

pub fn build_test_control(
    heater: Box<dyn Heater + Send>,
    fan: Box<dyn Fan + Send>,
) -> RoasterControl {
    RoasterControl::new(heater, fan, SensorConversionHub::new()).expect("test control should build")
}
