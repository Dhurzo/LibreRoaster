use core::fmt;

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

pub struct UartDriver;

impl UartDriver {
    pub fn new() -> Self {
        Self
    }

    pub async fn write_bytes(&mut self, _data: &[u8]) -> Result<(), UartError> {
        Err(UartError::TransmissionError)
    }

    pub async fn read_bytes(&mut self, _buffer: &mut [u8]) -> Result<usize, UartError> {
        Err(UartError::ReceptionError)
    }
}

static mut UART_INSTANCE: Option<UartDriver> = None;

pub fn init_uart(_uart0: ()) -> Result<(), UartError> {
    critical_section::with(|_| unsafe {
        UART_INSTANCE = Some(UartDriver::new());
    });

    Ok(())
}

pub fn get_uart_driver() -> Option<&'static mut UartDriver> {
    #[allow(static_mut_refs)]
    unsafe {
        UART_INSTANCE.as_mut()
    }
}
