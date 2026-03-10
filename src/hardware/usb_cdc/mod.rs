pub mod driver;
pub mod tasks;

pub use driver::{UsbCdcDriver, UsbCdcError};
pub use tasks::usb_reader_task;

pub const USB_CDC_BAUD_RATE: u32 = 115200;

#[cfg(target_arch = "riscv32")]
pub fn initialize_usb_cdc_system(
    _usb_device: esp_hal::peripherals::USB_DEVICE,
) -> Result<(), UsbCdcError> {
    // For now, we'll skip USB CDC initialization until we can properly handle the lifetime issues
    Ok(())
}

#[cfg(not(target_arch = "riscv32"))]
pub fn initialize_usb_cdc_system(_usb: ()) -> Result<(), UsbCdcError> {
    Ok(())
}
