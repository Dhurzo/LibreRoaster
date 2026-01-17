extern crate alloc;

use crate::config::SystemStatus;
use alloc::string::String;

/// Error types for output operations
#[derive(Debug)]
pub enum OutputError {
    /// Failed to serialize/format data
    Serialization,
    /// Serial communication error
    SerialComm,
    /// Invalid data received
    InvalidData,
    /// Scheduler timing error
    Scheduler,
}

/// Trait for formatting data into different output formats
pub trait OutputFormatter {
    /// Format system status into output string
    fn format(&self, status: &SystemStatus) -> Result<String, OutputError>;
}

/// Trait for managing print timing and scheduling
pub trait PrintScheduler {
    /// Check if it's time to print (non-blocking)
    #[allow(async_fn_in_trait)]
    async fn should_print(&mut self) -> bool;

    /// Reset scheduler (typically when roast starts)
    fn reset(&mut self);
}

/// Trait for output destinations
pub trait SerialOutput {
    /// Output formatted data (non-blocking)
    #[allow(async_fn_in_trait)]
    async fn print(&mut self, data: &str) -> Result<(), OutputError>;
}
