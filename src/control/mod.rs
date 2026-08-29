//! Control layer for the roaster: state machine, PID, safety policies, command handlers.
//!
//! `roaster_control` is the single writer to hardware and owns the focused controllers in `controllers/`.
//! `handlers/` evaluate Artisan commands into policy outcomes (`policies.rs`) that it applies.
//! `pid` is the bean-temperature PID controller; `traits` defines the hardware port traits.

/// Shared error type (`RoasterError`) and the `RoasterCommandHandler` trait.
pub mod abstractions;
/// Focused controllers: sensor, actuator (heater+fan), safety and command dispatch.
pub mod controllers;
/// Artisan/TC4 command handlers producing policy outcomes.
pub mod handlers;
/// Bean-temperature PID controller with anti-windup protection.
pub mod pid;
/// Policy outcome types and the manual/safety policy traits.
pub mod policies;
/// Central `RoasterControl` facade: state machine, safety latches, single hardware writer.
pub mod roaster_control;
/// SSR zero-cross cycle guard (`SsrCycleGuard`) pacing heater cycles.
pub mod ssr_scheduler;
/// Hardware port traits (heater, fan, thermometer) for dependency injection.
pub mod traits;
/// Re-export the handler trait and shared error type.
pub use abstractions::{RoasterCommandHandler, RoasterError};

/// Re-export all items from `abstractions`.
pub use abstractions::*;
/// Re-export all command handlers.
pub use handlers::*;
/// Re-export the public API of `roaster_control` (`RoasterControl`).
pub use roaster_control::*;
/// Re-export the SSR zero-cross cycle guard.
pub use ssr_scheduler::SsrCycleGuard;
