#![cfg(all(test, not(target_arch = "riscv32")))]

//! Thin shim so integration tests can continue importing the shared stubs
//! without duplicating their implementations.
//!
//! The real definitions now live in `libreroaster::common`, and the shim now
//! also re-exports the helper that wires a `SensorConversionHub` into
//! `RoasterControl` so helper suites can build the control consistently.

pub use libreroaster::common::{StubFan, StubHeater};

pub use libreroaster::hardware::sensors::SensorConversionHub;

use libreroaster::application::service_container::ServiceContainer;
use libreroaster::control::traits::{Fan, Heater};
use libreroaster::control::RoasterControl;
use libreroaster::input::ArtisanInput;

/// Build a `RoasterControl` from stub heater/fan and a fresh `SensorConversionHub`.
#[allow(clippy::expect_used)]
pub fn build_test_control(
    heater: Box<dyn Heater + Send>,
    fan: Box<dyn Fan + Send>,
) -> RoasterControl {
    RoasterControl::new(heater, fan, SensorConversionHub::new()).expect("test control should build")
}

/// Init the global `ServiceContainer` with stub roaster + artisan input.
#[allow(dead_code, clippy::expect_used)]
pub fn init_test_service_container() {
    let roaster = build_test_control(Box::new(StubHeater::new()), Box::new(StubFan::new()));
    let artisan_input = ArtisanInput::new().expect("ArtisanInput should build");
    ServiceContainer::init_roaster(roaster);
    ServiceContainer::init_artisan_input(artisan_input);
}
