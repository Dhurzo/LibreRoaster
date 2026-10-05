//! Safety subsystems for LibreRoaster.
//!
//! Houses the dual-layer watchdog (software telemetry + hardware RTC WDT) and
//! the over-temperature regression runner used to self-verify the hardware
//! safety path on embedded targets.

/// Panic-path heater cut-off (GPIO10 LOW before the backtrace).
pub mod panic_guard;
/// Over-temperature regression self-test runner.
pub mod regression;
/// Dual-layer watchdog (software + hardware RTC WDT).
pub mod watchdog;
