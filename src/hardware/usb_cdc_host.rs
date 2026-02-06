#![cfg(target_arch = "x86_64")]

use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UsbCdcError {
    TransmissionError,
    ReceptionError,
    BufferOverflow,
    NotInitialized,
    NotSupported,
}

impl fmt::Display for UsbCdcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UsbCdcError::TransmissionError => write!(f, "USB CDC transmission error"),
            UsbCdcError::ReceptionError => write!(f, "USB CDC reception error"),
            UsbCdcError::BufferOverflow => write!(f, "USB CDC buffer overflow"),
            UsbCdcError::NotInitialized => write!(f, "USB CDC not initialized"),
            UsbCdcError::NotSupported => write!(f, "USB CDC not supported in this configuration"),
        }
    }
}

pub struct UsbCdcDriver;

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

    pub fn is_connected(&self) -> bool {
        false
    }
}

static mut USB_CDC_INSTANCE: Option<UsbCdcDriver> = None;

pub fn init_usb_cdc(_usb: ()) -> Result<(), UsbCdcError> {
    Ok(())
}

pub fn get_usb_cdc_driver() -> Option<&'static mut UsbCdcDriver> {
    None
}
