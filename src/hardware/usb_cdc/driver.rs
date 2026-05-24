use core::fmt;

#[cfg(target_arch = "riscv32")]
use static_cell::StaticCell;

#[cfg(target_arch = "riscv32")]
use crate::hardware::static_sync::SyncCell;

#[cfg(target_arch = "riscv32")]
use esp_hal::usb_serial_jtag::UsbSerialJtag;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UsbCdcError {
    TransmissionError,
    ReceptionError,
    BufferOverflow,
    NotInitialized,
    NotSupported,
    WouldBlock,
}

impl fmt::Display for UsbCdcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UsbCdcError::TransmissionError => write!(f, "USB CDC transmission error"),
            UsbCdcError::ReceptionError => write!(f, "USB CDC reception error"),
            UsbCdcError::BufferOverflow => write!(f, "USB CDC buffer overflow"),
            UsbCdcError::NotInitialized => write!(f, "USB CDC not initialized"),
            UsbCdcError::NotSupported => write!(f, "USB CDC not supported in this configuration"),
            UsbCdcError::WouldBlock => write!(f, "USB CDC write blocked - back-pressure"),
        }
    }
}

#[cfg(target_arch = "riscv32")]
pub struct UsbCdcDriver {
    usb: UsbSerialJtag<'static, esp_hal::Async>,
}

#[cfg(target_arch = "riscv32")]
impl UsbCdcDriver {
    pub fn new(usb: UsbSerialJtag<'static, esp_hal::Async>) -> Self {
        Self { usb }
    }

    pub async fn write_bytes(&mut self, data: &[u8]) -> Result<(), UsbCdcError> {
        use embedded_io_async::Write;
        Write::write(&mut self.usb, data)
            .await
            .map_err(|_| UsbCdcError::TransmissionError)?;
        Write::flush(&mut self.usb)
            .await
            .map_err(|_| UsbCdcError::TransmissionError)?;
        Ok(())
    }

    pub async fn read_bytes(&mut self, buffer: &mut [u8]) -> Result<usize, UsbCdcError> {
        use embedded_io_async::Read;
        Read::read(&mut self.usb, buffer)
            .await
            .map_err(|_| UsbCdcError::ReceptionError)
    }
}

#[cfg(not(target_arch = "riscv32"))]
pub struct UsbCdcDriver;

#[cfg(not(target_arch = "riscv32"))]
impl UsbCdcDriver {
    pub fn new() -> Result<Self, UsbCdcError> {
        Ok(Self)
    }

    pub async fn write_bytes(&mut self, _data: &[u8]) -> Result<(), UsbCdcError> {
        Ok(())
    }

    pub async fn read_bytes(&mut self, _buffer: &mut [u8]) -> Result<usize, UsbCdcError> {
        Ok(0)
    }
}

#[cfg(target_arch = "riscv32")]
static USB_CDC_DRIVER: StaticCell<UsbCdcDriver> = StaticCell::new();

#[cfg(target_arch = "riscv32")]
static USB_CDC_DRIVER_PTR: SyncCell<*mut UsbCdcDriver> = SyncCell::new(core::ptr::null_mut());

#[cfg(target_arch = "riscv32")]
pub fn init_usb_cdc(usb: UsbSerialJtag<'static, esp_hal::Blocking>) -> Result<(), UsbCdcError> {
    let driver = USB_CDC_DRIVER.init(UsbCdcDriver::new(usb.into_async()));
    unsafe {
        *USB_CDC_DRIVER_PTR.get() = driver as *mut UsbCdcDriver;
    }
    Ok(())
}

#[cfg(not(target_arch = "riscv32"))]
pub fn init_usb_cdc(_usb: ()) -> Result<(), UsbCdcError> {
    Ok(())
}

#[cfg(target_arch = "riscv32")]
pub fn get_usb_cdc_driver() -> Option<&'static mut UsbCdcDriver> {
    unsafe { (*USB_CDC_DRIVER_PTR.get()).as_mut() }
}

#[cfg(not(target_arch = "riscv32"))]
pub fn get_usb_cdc_driver() -> Option<&'static mut UsbCdcDriver> {
    None
}
