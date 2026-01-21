use core::fmt;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embedded_io::{Read, Write};
use esp_hal::uart::{Config, Uart, UartRx, UartTx};

#[derive(Debug)]
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

pub struct UartDriver {
    tx: UartTx<'static, esp_hal::Blocking>,
    rx: UartRx<'static, esp_hal::Blocking>,
}

impl UartDriver {
    pub fn new(
        tx: UartTx<'static, esp_hal::Blocking>,
        rx: UartRx<'static, esp_hal::Blocking>,
    ) -> Self {
        Self { tx, rx }
    }

    pub async fn write_bytes(&mut self, data: &[u8]) -> Result<(), UartError> {
        self.tx
            .write_all(data)
            .map_err(|_| UartError::TransmissionError)?;
        self.tx.flush().map_err(|_| UartError::TransmissionError)?;
        Ok(())
    }

    pub async fn read_bytes(&mut self, buffer: &mut [u8]) -> Result<usize, UartError> {
        self.rx.read(buffer).map_err(|_| UartError::ReceptionError)
    }
}

static UART_DRIVER: CriticalSectionRawMutex = CriticalSectionRawMutex::new();
static mut UART_INSTANCE: Option<UartDriver> = None;

pub fn init_uart(uart0: esp_hal::peripherals::UART0) -> Result<(), UartError> {
    let config = Config::default().with_baudrate(115200);

    let uart = Uart::new(uart0, config).map_err(|_| UartError::TransmissionError)?;

    let (rx, tx) = uart.split();

    // Extend lifetime using transmute (unsafe but necessary for static storage)
    let tx_static = unsafe {
        core::mem::transmute::<UartTx<esp_hal::Blocking>, UartTx<'static, esp_hal::Blocking>>(tx)
    };
    let rx_static = unsafe {
        core::mem::transmute::<UartRx<esp_hal::Blocking>, UartRx<'static, esp_hal::Blocking>>(rx)
    };

    critical_section::with(|_| unsafe {
        UART_INSTANCE = Some(UartDriver::new(tx_static, rx_static));
    });

    Ok(())
}

pub fn get_uart_driver() -> Option<&'static mut UartDriver> {
    unsafe { UART_INSTANCE.as_mut() }
}
