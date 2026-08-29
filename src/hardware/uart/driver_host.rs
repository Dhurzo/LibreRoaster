//! Host (non-embedded) stub of the UART driver.
//!
//! Compiles under `not(target_arch = "riscv32")` for host-side tests. The
//! read/write functions always return errors and `UartDriver` carries no real
//! peripheral; only the signatures match the embedded driver.

use core::fmt;
use static_cell::StaticCell;

use crate::hardware::static_sync::SyncCell;

/// Errors returned by the host UART stub (always returned, never real I/O).
#[derive(Debug, Clone, PartialEq)]
pub enum UartError {
    /// Stub: transmission always fails on host.
    TransmissionError,
    /// Stub: reception always fails on host.
    ReceptionError,
    /// Stub: buffer overflow placeholder.
    BufferOverflow,
    /// Stub: init always succeeds (no peripheral).
    InitError,
}

impl fmt::Display for UartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UartError::TransmissionError => write!(f, "UART transmission error"),
            UartError::ReceptionError => write!(f, "UART reception error"),
            UartError::BufferOverflow => write!(f, "UART buffer overflow"),
            UartError::InitError => write!(f, "UART initialization error"),
        }
    }
}

/// Host stub UART driver with no underlying peripheral.
pub struct UartDriver;

impl UartDriver {
    /// Construct the host stub driver.
    pub fn new() -> Self {
        Self
    }

    /// Host stub: always returns `TransmissionError`.
    pub async fn write_bytes(&mut self, _data: &[u8]) -> Result<(), UartError> {
        Err(UartError::TransmissionError)
    }

    /// Host stub: always returns `ReceptionError`.
    pub async fn read_bytes(&mut self, _buffer: &mut [u8]) -> Result<usize, UartError> {
        Err(UartError::ReceptionError)
    }
}

impl Default for UartDriver {
    fn default() -> Self {
        Self::new()
    }
}

static UART_DRIVER: StaticCell<Option<UartDriver>> = StaticCell::new();

static UART_PTR: SyncCell<core::ptr::NonNull<Option<UartDriver>>> =
    SyncCell::new(core::ptr::NonNull::dangling());

/// Host stub: allocate the (no-op) driver and return `Ok`.
pub fn init_uart(_uart0: (), _rx: (), _tx: ()) -> Result<(), UartError> {
    let value = UART_DRIVER.init(Some(UartDriver::new()));
    unsafe { *UART_PTR.get() = core::ptr::NonNull::new_unchecked(value) };
    Ok(())
}

/// Write bytes to UART (host stub — always returns TransmissionError).
pub async fn uart_write_bytes(_data: &[u8]) -> Result<(), UartError> {
    Err(UartError::TransmissionError)
}

/// Read bytes from UART (host stub — always returns ReceptionError).
pub async fn uart_read_bytes(_buffer: &mut [u8]) -> Result<usize, UartError> {
    Err(UartError::ReceptionError)
}

#[cfg(test)]
/// Internal accessor for tests that need to interact with the UART driver directly.
pub fn get_uart_driver() -> Option<&'static mut UartDriver> {
    unsafe {
        let opt_ptr = &mut *UART_PTR.get();
        if let Some(ptr) = opt_ptr.as_mut() {
            return Some(ptr);
        }
        None
    }
}
