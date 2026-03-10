use core::cell::UnsafeCell;
use core::fmt;
use embedded_io_async::Read;
use embedded_io_async::Write;
use esp_hal::uart::{Config, Uart, UartRx, UartTx};
use static_cell::StaticCell;

#[derive(Debug, Clone, PartialEq)]
pub enum UartError {
    TransmissionError,
    ReceptionError,
    BufferOverflow,
}

impl fmt::Display for UartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UartError::TransmissionError => write!(f, "UART transmission error"),
            UartError::ReceptionError => write!(f, "UART reception error"),
            UartError::BufferOverflow => write!(f, "UART buffer overflow"),
        }
    }
}

/// Async UART driver using embassy-compatible traits
/// Uses embedded_io_async::Read and embedded_io_async::Write for non-blocking I/O
pub struct UartDriver {
    tx: UartTx<'static, esp_hal::Async>,
    rx: UartRx<'static, esp_hal::Async>,
}

impl UartDriver {
    pub fn new(tx: UartTx<'static, esp_hal::Async>, rx: UartRx<'static, esp_hal::Async>) -> Self {
        Self { tx, rx }
    }

    /// Write bytes asynchronously using embedded_io_async::Write trait
    pub async fn write_bytes(&mut self, data: &[u8]) -> Result<(), UartError> {
        Write::write(&mut self.tx, data)
            .await
            .map_err(|_| UartError::TransmissionError)?;
        Write::flush(&mut self.tx)
            .await
            .map_err(|_| UartError::TransmissionError)?;
        Ok(())
    }

    /// Read bytes asynchronously using embedded_io_async::Read trait
    pub async fn read_bytes(&mut self, buffer: &mut [u8]) -> Result<usize, UartError> {
        Read::read(&mut self.rx, buffer)
            .await
            .map_err(|_| UartError::ReceptionError)
    }
}

static UART_TX: StaticCell<UartTx<'static, esp_hal::Async>> = StaticCell::new();
static UART_RX: StaticCell<UartRx<'static, esp_hal::Async>> = StaticCell::new();
static UART_DRIVER: StaticCell<UartDriver> = StaticCell::new();

struct SyncPointer<T>(UnsafeCell<T>);

unsafe impl<T> Sync for SyncPointer<T> {}

impl<T> SyncPointer<T> {
    const fn new(ptr: T) -> Self {
        Self(UnsafeCell::new(ptr))
    }

    fn get(&self) -> *mut T {
        self.0.get()
    }
}

static UART_PTR: SyncPointer<core::ptr::NonNull<UartDriver>> =
    SyncPointer::new(core::ptr::NonNull::dangling());

pub fn init_uart(_uart0: esp_hal::peripherals::UART0) -> Result<(), UartError> {
    // For now, we'll skip UART initialization until we can properly handle the lifetime issues
    // In a real implementation, we would need to use a different approach that doesn't require 'static
    Ok(())
}

pub fn get_uart_driver() -> Option<&'static mut UartDriver> {
    unsafe {
        // StaticCell doesn't have get_mut(), we need to use a different approach
        // For now, return None until we can properly implement this
        None
    }
}
