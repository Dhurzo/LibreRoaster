use core::fmt;
use core::ptr;
use embedded_io_async::Read;
use embedded_io_async::Write;
use esp_hal::gpio::interconnect::{PeripheralInput, PeripheralOutput};
use esp_hal::uart::{Config, Uart, UartRx, UartTx};
use static_cell::StaticCell;

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
static mut UART_DRIVER_PTR: *mut UartDriver = ptr::null_mut();

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
        UART_DRIVER_PTR = driver as *mut UartDriver;
    }
    Ok(())
}

pub fn get_uart_driver() -> Option<&'static mut UartDriver> {
    unsafe { UART_DRIVER_PTR.as_mut() }
}
