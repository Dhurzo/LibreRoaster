use core::cell::UnsafeCell;
use core::fmt;
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

impl Default for UartDriver {
    fn default() -> Self {
        Self::new()
    }
}

static UART_DRIVER: StaticCell<Option<UartDriver>> = StaticCell::new();

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

static UART_PTR: SyncPointer<core::ptr::NonNull<Option<UartDriver>>> =
    SyncPointer::new(core::ptr::NonNull::dangling());

pub fn init_uart(_uart0: (), _rx: (), _tx: ()) -> Result<(), UartError> {
    let value = UART_DRIVER.init(Some(UartDriver::new()));
    unsafe { *UART_PTR.get() = core::ptr::NonNull::new_unchecked(value) };
    Ok(())
}

pub fn get_uart_driver() -> Option<&'static mut UartDriver> {
    unsafe {
        let opt_ptr = &mut *UART_PTR.get();
        if let Some(ptr) = opt_ptr.as_mut() {
            return Some(ptr);
        }
        None
    }
}
