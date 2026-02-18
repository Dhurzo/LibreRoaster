use core::fmt;
use embedded_io_async::Write;
use embedded_io_async::Read;
use esp_hal::uart::{Config, Uart, UartRx, UartTx};

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
    pub fn new(
        tx: UartTx<'static, esp_hal::Async>,
        rx: UartRx<'static, esp_hal::Async>,
    ) -> Self {
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

static mut UART_INSTANCE: Option<UartDriver> = None;

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

    critical_section::with(|_| unsafe {
        UART_INSTANCE = Some(UartDriver::new(tx_static, rx_static));
    });

    Ok(())
}

pub fn get_uart_driver() -> Option<&'static mut UartDriver> {
    // Allow this static_mut_ref warning as it's necessary for embedded systems
    #[allow(static_mut_refs)]
    unsafe {
        UART_INSTANCE.as_mut()
    }
}
