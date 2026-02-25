extern crate alloc;

use crate::config::SystemStatus;
use alloc::string::String;

#[derive(Debug)]
pub enum OutputError {
    Serialization,
    SerialComm,
    InvalidData,
    Scheduler,
}

pub trait OutputFormatter {
    fn format(&self, status: &SystemStatus) -> Result<String, OutputError>;
}

pub trait PrintScheduler {
    #[allow(async_fn_in_trait)]
    async fn should_print(&mut self) -> bool;

    fn reset(&mut self);
}

pub trait SerialOutput {
    #[allow(async_fn_in_trait)]
    async fn print(&mut self, data: &str) -> Result<(), OutputError>;
}
