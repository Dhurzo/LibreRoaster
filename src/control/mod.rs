//! Control layer for the roaster: state machine, PID, safety policies, command handlers.
//!
//! `roaster_control` is the single writer to hardware and owns the focused controllers in `controllers/`.
//! `handlers/` evaluate Artisan commands into policy outcomes (`policies.rs`) that it applies.
//! `pid` is the bean-temperature PID controller; `traits` defines the hardware port traits.

/// Shared error type (`RoasterError`) and the `RoasterCommandHandler` trait.
pub mod abstractions;
/// Step-test PID autotune (pure logic, DIFF E4).
pub mod autotune;
/// State of one batch of beans: charge detection, markers, RoR-follow (CORE-4).
pub mod batch;
/// Focused controllers: sensor, actuator (heater+fan), safety and command dispatch.
pub mod controllers;
/// Artisan/TC4 command handlers producing policy outcomes.
pub mod handlers;
/// Who drives the heater, derived from the stored flags (CORE-2, pure logic).
pub mod mode;
/// Bean-temperature PID controller with anti-windup protection.
pub mod pid;
/// Policy outcome types and the manual/safety policy traits.
pub mod policies;
/// Central `RoasterControl` facade: state machine, safety latches, single hardware writer.
pub mod roaster_control;
/// RoR-follow setpoint generator (pure logic, DIFF E3).
pub mod ror_follow;
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
