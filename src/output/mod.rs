//! Artisan output / wire-formatting subsystem for LibreRoaster.
//!
//! Provides the TC4-compatible formatters (`artisan`) and the shared
//! `OutputError` type (`traits`) used across the response path.

pub mod artisan;
/// Shared output error type.
pub mod traits;

/// Core Artisan formatters re-exported for the application layer.
pub use artisan::{ArtisanFormatter, MutableArtisanFormatter};
/// Output error type re-exported for the application layer.
pub use traits::OutputError;
