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

/// Bug #9 fix: split the UART driver into two independent halves with their
/// own mutexes. Previously a single `UART_MUTEX` guarded both halves, and the
/// reader task retained it across its `.await` (only released when bytes
/// actually arrived), indefinitely blocking the writer. That meant every
/// protocol response arrived one poll late and unsolicited telemetry could
/// stall forever on an idle line. With separate mutexes the RX task can wait
/// for incoming bytes while the TX task concurrently emits responses and
/// continuous telemetry.
pub struct UartTxDriver {
    tx: UartTx<'static, esp_hal::Async>,
}

impl UartTxDriver {
    pub fn new(tx: UartTx<'static, esp_hal::Async>) -> Self {
        Self { tx }
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
}

pub struct UartRxDriver {
    rx: UartRx<'static, esp_hal::Async>,
}

impl UartRxDriver {
    pub fn new(rx: UartRx<'static, esp_hal::Async>) -> Self {
        Self { rx }
    }

    pub async fn read_bytes(&mut self, buffer: &mut [u8]) -> Result<usize, UartError> {
        Read::read(&mut self.rx, buffer)
            .await
            .map_err(|_| UartError::ReceptionError)
    }
}

static UART_TX_DRIVER: StaticCell<UartTxDriver> = StaticCell::new();
static UART_TX_DRIVER_PTR: SyncCell<*mut UartTxDriver> = SyncCell::new(ptr::null_mut());
static UART_RX_DRIVER: StaticCell<UartRxDriver> = StaticCell::new();
static UART_RX_DRIVER_PTR: SyncCell<*mut UartRxDriver> = SyncCell::new(ptr::null_mut());

/// Async mutex guarding TX only. RX may proceed concurrently.
static UART_TX_MUTEX: Mutex<CriticalSectionRawMutex, ()> = Mutex::new(());
/// Async mutex guarding RX only. TX may proceed concurrently.
static UART_RX_MUTEX: Mutex<CriticalSectionRawMutex, ()> = Mutex::new(());

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
    let tx_driver = UART_TX_DRIVER.init(UartTxDriver::new(tx_half));
    let rx_driver = UART_RX_DRIVER.init(UartRxDriver::new(rx_half));
    // SAFETY: UART_*_DRIVER_PTR store raw pointers to drivers allocated by
    // StaticCell above. init_uart() runs in AppBuilder::build() before any
    // tasks start, so no concurrent access occurs. Each pointer is only
    // dereferenced inside its own mutex guard thereafter.
    unsafe {
        *UART_TX_DRIVER_PTR.get() = tx_driver as *mut UartTxDriver;
        *UART_RX_DRIVER_PTR.get() = rx_driver as *mut UartRxDriver;
    }
    Ok(())
}

/// Write bytes to UART. Acquires the TX mutex only — RX may proceed
/// concurrently, so the reader task blocking on `Read::read(...).await` does
/// NOT prevent a response or telemetry line from being emitted.
///
/// SAFETY: The `&mut UartTxDriver` only exists inside the locked scope and
/// does not cross an `.await` outside it; the raw pointer is valid because
/// `init_uart()` ran before any tasks started.
pub async fn uart_write_bytes(data: &[u8]) -> Result<(), UartError> {
    let _guard = UART_TX_MUTEX.lock().await;
    // SAFETY: UART_TX_DRIVER_PTR points to a valid UartTxDriver allocated
    // by StaticCell in init_uart(). The TX mutex guarantees exclusive &mut
    // access to the TX half; the RX half has its own mutex and may be
    // concurrently held by the reader task.
    let driver = unsafe { &mut *(*UART_TX_DRIVER_PTR.get()) };
    driver.write_bytes(data).await
}

/// Read bytes from UART. Acquires the RX mutex only — TX may proceed
/// concurrently, so a writer emitting a response does NOT block while we
/// wait for the next inbound byte.
///
/// SAFETY: Same as `uart_write_bytes` — the mutable reference is only alive
/// inside the RX mutex-locked scope.
pub async fn uart_read_bytes(buffer: &mut [u8]) -> Result<usize, UartError> {
    let _guard = UART_RX_MUTEX.lock().await;
    // SAFETY: Same reasoning as uart_write_bytes, but for the RX half.
    let driver = unsafe { &mut *(*UART_RX_DRIVER_PTR.get()) };
    driver.read_bytes(buffer).await
}

#[cfg(test)]
/// Internal accessor for tests that need to interact with the UART driver
/// directly. The driver is now split, so the old single accessor is gone;
/// this stub returns None to keep any historical caller compiling.
pub fn get_uart_driver() -> Option<&'static ()> {
    None
}
