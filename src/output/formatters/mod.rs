// Formatters module for Artisan protocol
//
// This module contains specialized formatters for different aspects of Artisan protocol:
// - Time formatting (time.rs)
// - ROR calculation (ror.rs)
// - CSV line formatting (csv.rs)
//
// Each formatter is self-contained and testable independently.

pub mod csv;
pub mod ror;
pub mod time;

// Re-export common types and functions for backward compatibility
pub use csv::{normalize_read_value, CsvFormatter};
pub use ror::RorCalculator;
pub use time::{format_time, TimeFormatter};
