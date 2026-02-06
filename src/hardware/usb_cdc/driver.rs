use core::fmt;

#[cfg(target_arch = "riscv32")]
use esp_hal::usb_serial_jtag::UsbSerialJtag;

#[cfg(target_arch = "riscv32")]
use embedded_io::Read;

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

#[cfg(target_arch = "riscv32")]
pub struct UsbCdcDriver {
    usb: UsbSerialJtag<'static, esp_hal::Blocking>,
}

#[cfg(target_arch = "riscv32")]
impl UsbCdcDriver {
    pub fn new(usb: UsbSerialJtag<'static, esp_hal::Blocking>) -> Self {
        Self { usb }
    }

    pub async fn write_bytes(&mut self, data: &[u8]) -> Result<(), UsbCdcError> {
        self.usb.write(data).map_err(|_| UsbCdcError::TransmissionError)?;
        Ok(())
    }

    pub async fn read_bytes(&mut self, buffer: &mut [u8]) -> Result<usize, UsbCdcError> {
        self.usb.read(buffer).map_err(|_| UsbCdcError::ReceptionError)
    }

    pub fn is_connected(&self) -> bool {
        false
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

    pub fn is_connected(&self) -> bool {
        false
    }
}

static mut USB_CDC_INSTANCE: Option<UsbCdcDriver> = None;

#[cfg(target_arch = "riscv32")]
pub fn init_usb_cdc(usb: UsbSerialJtag<'static, esp_hal::Blocking>) -> Result<(), UsbCdcError> {
    critical_section::with(|_| unsafe {
        USB_CDC_INSTANCE = Some(UsbCdcDriver::new(usb));
    });
    Ok(())
}

#[cfg(not(target_arch = "riscv32"))]
pub fn init_usb_cdc(_usb: ()) -> Result<(), UsbCdcError> {
    Ok(())
}

#[cfg(target_arch = "riscv32")]
pub fn get_usb_cdc_driver() -> Option<&'static mut UsbCdcDriver> {
    #[allow(static_mut_refs)]
    unsafe {
        USB_CDC_INSTANCE.as_mut()
    }
}

#[cfg(not(target_arch = "riscv32"))]
pub fn get_usb_cdc_driver() -> Option<&'static mut UsbCdcDriver> {
    None
}
