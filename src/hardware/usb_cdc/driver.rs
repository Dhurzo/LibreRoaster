use core::fmt;

#[cfg(target_arch = "riscv32")]
use static_cell::StaticCell;

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
    usb: UsbSerialJtag<'static, esp_hal::Blocking>,
}

#[cfg(target_arch = "riscv32")]
impl UsbCdcDriver {
    pub fn new(usb: UsbSerialJtag<'static, esp_hal::Blocking>) -> Self {
        Self { usb }
    }

    /// Check if the USB TX buffer can accept more data.
    /// Returns true if ready to write, false if congested (back-pressure).
    pub fn is_write_ready(&self) -> bool {
        // USB Serial JTAG has a 64-byte TX FIFO
        // We can't directly check FIFO status in blocking mode,
        // but we can use this as a hook for future implementation
        // Currently returns true - actual back-pressure handled in writer task
        true
    }

    /// Write bytes with back-pressure awareness.
    /// Returns WouldBlock if the write cannot complete immediately.
    pub async fn write_bytes(&mut self, data: &[u8]) -> Result<(), UsbCdcError> {
        // Try to write - if it fails, return WouldBlock to trigger back-pressure
        match self.usb.write(data) {
            Ok(()) => Ok(()),
            Err(_) => Err(UsbCdcError::WouldBlock),
        }
    }

    /// Write bytes with timeout for back-pressure detection.
    /// If write takes longer than expected, treats as congestion.
    pub async fn write_bytes_with_timeout(
        &mut self,
        data: &[u8],
        timeout_ticks: u64,
    ) -> Result<(), UsbCdcError> {
        use embassy_time::{Duration, Timer};

        // Try write with polling for back-pressure
        let mut remaining = data.len();
        let mut offset = 0;

        while remaining > 0 {
            // Check if we should yield due to potential congestion
            if !self.is_write_ready() {
                return Err(UsbCdcError::WouldBlock);
            }

            match self.usb.write(&data[offset..offset + 1]) {
                Ok(()) => {
                    offset += 1;
                    remaining -= 1;
                }
                Err(_) => {
                    // Brief yield to allow USB hardware to process
                    Timer::after(Duration::from_ticks(timeout_ticks)).await;
                    // After timeout, treat as congestion
                    return Err(UsbCdcError::WouldBlock);
                }
            }
        }

        Ok(())
    }

    pub async fn read_bytes(&mut self, buffer: &mut [u8]) -> Result<usize, UsbCdcError> {
        let result = self
            .usb
            .read(buffer)
            .map_err(|_| UsbCdcError::ReceptionError);
        result
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

    /// Check if the USB TX buffer can accept more data.
    /// Always returns true for non-riscv32 targets.
    pub fn is_write_ready(&self) -> bool {
        true
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

#[cfg(target_arch = "riscv32")]
static USB_CDC_DRIVER: StaticCell<UsbCdcDriver> = StaticCell::new();

#[cfg(target_arch = "riscv32")]
pub fn init_usb_cdc(usb: UsbSerialJtag<'static, esp_hal::Blocking>) -> Result<(), UsbCdcError> {
    USB_CDC_DRIVER.init(UsbCdcDriver::new(usb));
    Ok(())
}

#[cfg(not(target_arch = "riscv32"))]
pub fn init_usb_cdc(_usb: ()) -> Result<(), UsbCdcError> {
    Ok(())
}

#[cfg(target_arch = "riscv32")]
pub fn get_usb_cdc_driver() -> Option<&'static mut UsbCdcDriver> {
    USB_CDC_DRIVER.get_mut()
}

#[cfg(not(target_arch = "riscv32"))]
pub fn get_usb_cdc_driver() -> Option<&'static mut UsbCdcDriver> {
    None
}
