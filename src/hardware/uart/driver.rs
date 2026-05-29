use core::fmt;
use core::ptr;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embedded_io_async::Read;
use embedded_io_async::Write;
use esp_hal::gpio::interconnect::{PeripheralInput, PeripheralOutput};
use esp_hal::uart::{Config, Uart, UartRx, UartTx};
use static_cell::StaticCell;

use crate::hardware::static_sync::SyncCell;

#[derive(Debug, Clone, PartialEq)]
pub enum UartError {
    TransmissionError,
    ReceptionError,
    BufferOverflow,
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

pub struct UartDriver {
    tx: UartTx<'static, esp_hal::Async>,
    rx: UartRx<'static, esp_hal::Async>,
}

impl UartDriver {
    pub fn new(tx: UartTx<'static, esp_hal::Async>, rx: UartRx<'static, esp_hal::Async>) -> Self {
        Self { tx, rx }
    }

    pub async fn write_bytes(&mut self, data: &[u8]) -> Result<(), UartError> {
        Write::write(&mut self.tx, data)
            .await
            .map_err(|_| UartError::TransmissionError)?;
        Write::flush(&mut self.tx)
            .await
            .map_err(|_| UartError::TransmissionError)?;
        Ok(())
    }

    pub async fn read_bytes(&mut self, buffer: &mut [u8]) -> Result<usize, UartError> {
        Read::read(&mut self.rx, buffer)
            .await
            .map_err(|_| UartError::ReceptionError)
    }
}

static UART_DRIVER: StaticCell<UartDriver> = StaticCell::new();
static UART_DRIVER_PTR: SyncCell<*mut UartDriver> = SyncCell::new(ptr::null_mut());

/// Async mutex that guards UART driver access across tasks.
/// Prevents multiple tasks from simultaneously holding `&mut UartDriver`.
static UART_MUTEX: Mutex<CriticalSectionRawMutex, ()> = Mutex::new(());

pub fn init_uart(
    uart0: esp_hal::peripherals::UART0<'static>,
    rx: impl PeripheralInput<'static>,
    tx: impl PeripheralOutput<'static>,
) -> Result<(), UartError> {
    let uart = Uart::new(uart0, Config::default())
        .map_err(|_| UartError::InitError)?
        .with_rx(rx)
        .with_tx(tx)
        .into_async();

    let (rx_half, tx_half) = uart.split();
    let driver = UART_DRIVER.init(UartDriver::new(tx_half, rx_half));
    unsafe {
        *UART_DRIVER_PTR.get() = driver as *mut UartDriver;
    }
    Ok(())
}

/// Write bytes to UART. Acquires the async mutex to ensure exclusive access.
///
/// SAFETY: The `&mut UartDriver` is only constructed inside the mutex-locked
/// scope and does not exist across any `.await` boundary outside the locked
/// section. The raw pointer is guaranteed valid because `init_uart()` runs
/// before any tasks start.
pub async fn uart_write_bytes(data: &[u8]) -> Result<(), UartError> {
    let _guard = UART_MUTEX.lock().await;
    // SAFETY: UART_DRIVER_PTR points to a valid UartDriver allocated by StaticCell.
    // init_uart() is called in AppBuilder::build() before any tasks start.
    let driver = unsafe { &mut *(*UART_DRIVER_PTR.get()) };
    driver.write_bytes(data).await
}

/// Read bytes from UART. Acquires the async mutex to ensure exclusive access.
///
/// SAFETY: Same as `uart_write_bytes` — the mutable reference is only alive
/// inside the mutex-locked scope.
pub async fn uart_read_bytes(buffer: &mut [u8]) -> Result<usize, UartError> {
    let _guard = UART_MUTEX.lock().await;
    // SAFETY: Same as uart_write_bytes — init runs before tasks.
    let driver = unsafe { &mut *(*UART_DRIVER_PTR.get()) };
    driver.read_bytes(buffer).await
}

#[cfg(test)]
/// Internal accessor for tests that need to interact with the UART driver directly.
pub fn get_uart_driver() -> Option<&'static mut UartDriver> {
    unsafe { (*UART_DRIVER_PTR.get()).as_mut() }
}
