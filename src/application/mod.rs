//! Application layer: builder, service container, control-loop tasks, and metrics.
//!
//! Wires the hardware and control subsystems into Embassy tasks and the
//! `ServiceContainer` singleton. Modules are gated so only embedded/host-test
//! builds pull in the task and builder code.

/// Application builder and orchestration entry point (embedded/host-test).
#[cfg(any(target_arch = "riscv32", feature = "test"))]
pub mod app_builder;
/// Command-channel saturation metrics.
pub mod queue_metrics;
/// Service container singleton owning `RoasterControl` and the channels.
pub mod service_container;
/// Control-loop stage instrumentation reporter.
pub mod stage_instrumentation;
/// Control-loop and dual-output Embassy tasks (embedded/host-test).
#[cfg(any(target_arch = "riscv32", feature = "test"))]
pub mod tasks;

/// Re-exports the application builder API.
#[cfg(any(target_arch = "riscv32", feature = "test"))]
pub use app_builder::*;
/// Re-exports the queue metrics API.
pub use queue_metrics::*;
/// Re-exports the service container API.
pub use service_container::*;
/// Re-exports the stage instrumentation API.
pub use stage_instrumentation::*;
/// Re-exports the task entry points (embedded only).
#[cfg(target_arch = "riscv32")]
pub use tasks::*;
