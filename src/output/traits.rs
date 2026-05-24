use crate::config::SystemStatus;
use crate::memory::REPORT_BUFFER_SIZE;
use heapless::String as HeaplessString;

#[derive(Debug)]
pub enum OutputError {
    Serialization,
    SerialComm,
    InvalidData,
    Scheduler,
}

pub trait OutputFormatter {
    fn format(
        &self,
        status: &SystemStatus,
    ) -> Result<HeaplessString<REPORT_BUFFER_SIZE>, OutputError>;
}
