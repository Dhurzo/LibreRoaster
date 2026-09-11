//! Output error types for the LibreRoaster response path.
//!
//! Defines `OutputError`, the single error enum returned by the formatters and
//! output controllers when serialisation or transport fails.

#[derive(Debug)]
/// Errors produced while formatting or emitting Artisan responses.
pub enum OutputError {
    /// Failed to serialise a value into the fixed-size wire buffer.
    Serialization,
    /// Underlying serial/USB/UART transport failure.
    SerialComm,
    /// A value supplied for formatting was invalid.
    InvalidData,
    /// The Embassy executor rejected scheduling the output task.
    Scheduler,
}
