// Sub-controllers for roaster control.
//
// Each controller owns a focused slice of responsibility:
// - sensor:    temperature sampling, derivative filtering, fault detection
// - actuator:  heater/fan hardware writes, SSR cycle guard
// - safety:    emergency state, overtemp regression flag
// - dispatch:  command routing, PID management, handler orchestration

pub mod actuator;
pub mod dispatch;
pub mod safety;
pub mod sensor;

pub use actuator::ActuatorController;
pub use dispatch::{CommandDispatcher, CommandDispatchResult};
pub use safety::SafetyController;
pub use sensor::SensorController;
