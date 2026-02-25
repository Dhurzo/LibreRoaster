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

// SAFETY: StaticCell provides compile-time memory reservation, preventing use-after-free.
// Initialized once during early boot in single-threaded context before any async tasks start.
static UART_INSTANCE: StaticCell<Option<UartDriver>> = StaticCell::new();

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

// Store a raw pointer to the Option<UartDriver> after initialization for later access
// SAFETY: Only written once during init, wrapped in UnsafeCell for safe interior mutability
static UART_PTR: SyncPointer<core::ptr::NonNull<Option<UartDriver>>> =
    SyncPointer::new(core::ptr::NonNull::dangling());

pub fn init_uart(uart0: esp_hal::peripherals::UART0) -> Result<(), UartError> {
    let config = Config::default().with_baudrate(115200);

    // Create UART in blocking mode first, then convert to async
    let uart = Uart::new(uart0, config).map_err(|_| UartError::TransmissionError)?;

    // Convert to async mode - this enables non-blocking I/O
    let uart = uart.into_async();

    let (rx, tx) = uart.split();

    // Extend lifetime using transmute (unsafe but necessary for static storage)
    let tx_static = unsafe {
        core::mem::transmute::<UartTx<esp_hal::Async>, UartTx<'static, esp_hal::Async>>(tx)
    };
    let rx_static = unsafe {
        core::mem::transmute::<UartRx<esp_hal::Async>, UartRx<'static, esp_hal::Async>>(rx)
    };

    // SAFETY: init() called once during early UART initialization.
    // Stored pointer lives for the duration of the program.
    let value = UART_INSTANCE.init(Some(UartDriver::new(tx_static, rx_static)));
    // Store the pointer for later retrieval
    // SAFETY: value is &'static mut Option<UartDriver>, converting to NonNull is safe.
    // Only called once during early boot in single-threaded context.
    unsafe { *UART_PTR.get() = core::ptr::NonNull::new_unchecked(value) };

    Ok(())
}

pub fn get_uart_driver() -> Option<&'static mut UartDriver> {
    // SAFETY: UART_PTR is set once during init_uart() before any async tasks run.
    // The StaticCell guarantees the memory is valid for the program duration.
    // Only called after UART initialization in single-threaded context.
    unsafe {
        let opt_ptr = &mut *UART_PTR.get();
        if let Some(ptr) = opt_ptr.as_mut() {
            return Some(ptr);
        }
        None
    }
}
