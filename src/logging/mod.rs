//! Logging infrastructure for LibreRoaster
//!
//! Provides ring-buffer roast logging and traceability.
//!
//! ## Architecture
//!
//! Uses the standard `log` facade; no direct esp_println output to the
//! protocol port (Bug #6 — logs must never corrupt the Artisan protocol
//! stream). Bug M12 (2026-07-26): the `channel` module and `log_channel!`
//! macro (which wrote via `esp_println::println!` straight to the protocol
//! port) had ZERO invocations anywhere and were deleted.

pub mod roast_logger;
pub mod traceability;
