pub mod driver;
pub mod tasks;

pub use driver::{UsbCdcDriver, UsbCdcError};
pub use tasks::usb_reader_task;

pub const USB_CDC_BAUD_RATE: u32 = 115200;

#[cfg(target_arch = "riscv32")]
pub fn initialize_usb_cdc_system(
    usb_device: esp_hal::peripherals::USB_DEVICE<'static>,
) -> Result<(), UsbCdcError> {
    let usb = esp_hal::usb_serial_jtag::UsbSerialJtag::new(usb_device);
    driver::init_usb_cdc(usb)
}

#[cfg(not(target_arch = "riscv32"))]
pub fn initialize_usb_cdc_system(_usb: ()) -> Result<(), UsbCdcError> {
    Ok(())
}
