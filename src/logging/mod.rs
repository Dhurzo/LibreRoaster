//! Logging infrastructure for LibreRoaster
//!
//! Provides ring-buffer roast logging and traceability.
//!
//! ## Architecture
//!
//! Uses the standard `log` facade; no direct esp_println output to the
//! protocol port — logs must never corrupt the Artisan protocol stream.

/// Edge-triggered log gating for persistent safety conditions.
pub mod edge_log_gate;
/// Ring-buffer roast data logger for Artisan reconnect dumps.
pub mod roast_logger;
/// Command/event traceability across the control pipeline.
pub mod traceability;
