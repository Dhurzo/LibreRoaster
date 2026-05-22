pub mod conversion;
#[cfg(feature = "simulated-sensors")]
pub mod simulated;

pub use conversion::{convert_raw_temp, SensorConversionHub, SensorFault, SensorSample};
#[cfg(feature = "simulated-sensors")]
pub use simulated::{CurvePoint, RoastCurve, SimulatedSensorSource};
