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

pub fn init_uart(uart0: esp_hal::peripherals::UART0) -> Result<(), UartError> {
    let config = Config::default().with_baudrate(115200);

    let uart = Uart::new(uart0, config).map_err(|_| UartError::TransmissionError)?;
    let uart = uart.into_async();

    let (rx, tx) = uart.split();

    let tx_static = UART_TX.init(tx);
    let rx_static = UART_RX.init(rx);

    let value = UART_DRIVER.init(UartDriver::new(tx_static, rx_static));
    unsafe { *UART_PTR.get() = core::ptr::NonNull::new_unchecked(value) };

    Ok(())
}

pub fn get_uart_driver() -> Option<&'static mut UartDriver> {
    unsafe {
        let ptr = UART_PTR.get();
        ptr.as_mut()
    }
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

pub fn init_uart(uart0: esp_hal::peripherals::UART0) -> Result<(), UartError> {
    let config = Config::default().with_baudrate(115200);

    let uart = Uart::new(uart0, config).map_err(|_| UartError::TransmissionError)?;
    let uart = uart.into_async();

    let (rx, tx) = uart.split();

    let tx_static = UART_TX.init(tx);
    let rx_static = UART_RX.init(rx);

    UART_DRIVER.init(UartDriver::new(tx_static, rx_static));

    Ok(())
}

pub fn get_uart_driver() -> Option<&'static mut UartDriver> {
    UART_DRIVER.get_mut()
}
