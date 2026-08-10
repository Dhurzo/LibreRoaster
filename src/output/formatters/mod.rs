// Formatters module for Artisan protocol
//
// This module contains specialized formatters for different aspects of Artisan protocol:
// - Time formatting (time.rs)
// - CSV line formatting (csv.rs)
//
// Each formatter is self-contained and testable independently.
//
// Bug M12 (2026-08-10): `ror.rs` (`RorCalculator`) was removed — it computed
// a timestamp-less `(last - first) / (n - 1)` rate that assumed 1 s sampling
// and was only consumed by the dead `OutputFormatter for ArtisanFormatter`
// impl. The production RoR lives in `MutableArtisanFormatter` (timestamped,
// IIR-filtered, ×60 scaled).

pub mod csv;
pub mod time;

// Re-export common types and functions for backward compatibility
pub use csv::CsvFormatter;
pub use time::TimeFormatter;
