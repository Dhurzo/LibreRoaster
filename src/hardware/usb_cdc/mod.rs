pub mod driver;
pub mod tasks;

pub use driver::{UsbCdcDriver, UsbCdcError};
pub use tasks::{usb_reader_task, usb_writer_task};

pub const USB_CDC_BAUD_RATE: u32 = 115200;

#[cfg(target_arch = "riscv32")]
pub fn initialize_usb_cdc_system(
    usb_device: esp_hal::peripherals::USB_DEVICE,
) -> Result<(), UsbCdcError> {
    use esp_hal::usb_serial_jtag::UsbSerialJtag;

    let usb_serial_jtag = UsbSerialJtag::new(usb_device);

    let usb_static = unsafe {
        core::mem::transmute::<UsbSerialJtag<'_, esp_hal::Blocking>, UsbSerialJtag<'static, esp_hal::Blocking>>(usb_serial_jtag)
    };

    driver::init_usb_cdc(usb_static)
}

#[cfg(not(target_arch = "riscv32"))]
pub fn initialize_usb_cdc_system(_usb: ()) -> Result<(), UsbCdcError> {
    Ok(())
}
