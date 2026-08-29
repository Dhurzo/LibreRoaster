//! Safety subsystems for LibreRoaster.
//!
//! Houses the dual-layer watchdog (software telemetry + hardware RTC WDT) and
//! the over-temperature regression runner used to self-verify the hardware
//! safety path on embedded targets.

/// Over-temperature regression self-test runner.
pub mod regression;
/// Dual-layer watchdog (software + hardware RTC WDT).
pub mod watchdog;
