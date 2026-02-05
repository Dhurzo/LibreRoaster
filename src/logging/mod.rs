//! Logging infrastructure for LibreRoaster
//!
//! Provides channel-prefixed logging that outputs to UART0 using esp_println.
//! Channel prefixes: [USB], [UART], [SYSTEM]
//!
//! ## Architecture
//!
//! Uses esp_println::println! directly for reliable UART0 output.
//! Simpler than defmt-rtt + drain task pipeline.
//!
//! ## Modules
//!
//! - `channel`: Channel enum and log_channel! macro
//! - `drain_task`: Documentation and architectural decisions

pub mod channel;
pub mod drain_task;
