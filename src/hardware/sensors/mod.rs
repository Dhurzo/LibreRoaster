//! Thermocouple sensor subsystem: raw conversion, fault decoding, and the
//! `SensorConversionHub` that produces `SensorSample`s. The `simulated`
//! sub-module (feature `simulated-sensors`) supplies synthetic roast curves
//! for host/L3 testing without real MAX31856 hardware.

/// Real thermocouple conversion and the `SensorConversionHub`.
pub mod conversion;
#[cfg(feature = "simulated-sensors")]
/// Synthetic roast-curve temperature source (feature `simulated-sensors`).
pub mod simulated;

/// Re-exports of the public conversion API.
pub use conversion::{convert_raw_temp, SensorConversionHub, SensorFault, SensorSample};
#[cfg(feature = "simulated-sensors")]
/// Re-exports of the simulated-sensor API (feature `simulated-sensors`).
pub use simulated::{CurvePoint, RoastCurve, SimulatedSensorSource};
