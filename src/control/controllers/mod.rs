//! Focused sub-controllers that decompose `RoasterControl`.
//!
//! Each owns one slice of the control loop: `sensor` samples temperatures and
//! runs the rate-of-rise guards, `actuator` writes heater/fan hardware through
//! the SSR cycle guard and slew limiter, `safety` fronts the emergency policy,
//! and `dispatch` routes commands and manages the PID.

// Sub-controllers for roaster control.
//
// Each controller owns a focused slice of responsibility:
// - sensor:    temperature sampling, derivative filtering, fault detection
// - actuator:  heater/fan hardware writes, SSR cycle guard
// - safety:    emergency state, overtemp regression flag
// - dispatch:  command routing, PID management, handler orchestration

/// Heater/fan actuation: SSR cycle guard, slew limit, emergency sequence.
pub mod actuator;
/// Command routing facade and PID management.
pub mod dispatch;
/// Emergency-state facade over the safety command handler.
pub mod safety;
/// Temperature sampling, derivative filtering, and fault debounce.
pub mod sensor;

/// Heater/fan hardware actuation controller.
pub use actuator::ActuatorController;
/// Command dispatch outcome and routing facade.
pub use dispatch::{CommandDispatchResult, CommandDispatcher};
/// Emergency-state facade.
pub use safety::SafetyController;
/// Temperature sampling and rate-of-rise guard state.
pub use sensor::SensorController;
