use core::fmt;

#[cfg(target_arch = "riscv32")]
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
#[cfg(target_arch = "riscv32")]
use embassy_sync::mutex::Mutex;
#[cfg(target_arch = "riscv32")]
use static_cell::StaticCell;

#[cfg(target_arch = "riscv32")]
use crate::hardware::static_sync::SyncCell;

#[cfg(target_arch = "riscv32")]
use embassy_time::{with_timeout, Duration as EmbassyDuration};

#[cfg(target_arch = "riscv32")]
use esp_hal::usb_serial_jtag::{UsbSerialJtag, UsbSerialJtagRx, UsbSerialJtagTx};

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
/// Bug #9 fix: the USB CDC driver is split into TX and RX halves with
/// independent mutexes, so the reader task waiting on `Read::read().await`
/// does not block the writer from emitting protocol responses or telemetry.
/// Previously a single `USB_CDC_MUTEX` guarded the whole `UsbSerialJtag`,
/// and the reader retained it across its `.await`, stalling every response
/// until the next inbound byte.
pub struct UsbCdcTxDriver {
    usb: UsbSerialJtagTx<'static, esp_hal::Async>,
}

#[cfg(target_arch = "riscv32")]
impl UsbCdcTxDriver {
    pub fn new(usb: UsbSerialJtagTx<'static, esp_hal::Async>) -> Self {
        Self { usb }
    }

    pub async fn write_bytes(&mut self, data: &[u8]) -> Result<(), UsbCdcError> {
        // Bug A2 (2026-07-25): `UsbSerialJtagTx::write_async` only completes
        // when the host reads from the endpoint. If the host disappears mid-
        // roast (Artisan killed, USB unplugged, …) the awaited write blocks
        // FOREVER with the TX mutex held, and every subsequent output line
        // (including telemetry that would have fallen back to UART) is
        // dropped silently because the channel fills. Bound the write at 50 ms
        // and treat the timeout as "line discarded" — the next telemetry tick
        // already carries a fresh sample, and the roaster cannot be allowed to
        // depend on a reader being present at all times.
        use embedded_io_async::Write;
        match with_timeout(
            EmbassyDuration::from_millis(50),
            Write::write(&mut self.usb, data),
        )
        .await
        {
            Ok(Ok(_)) => {}
            Ok(Err(_)) => return Err(UsbCdcError::TransmissionError),
            Err(_timeout) => return Err(UsbCdcError::TransmissionError),
        }
        match with_timeout(
            EmbassyDuration::from_millis(20),
            Write::flush(&mut self.usb),
        )
        .await
        {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(_)) => Err(UsbCdcError::TransmissionError),
            Err(_timeout) => Err(UsbCdcError::TransmissionError),
        }
    }
}

#[cfg(target_arch = "riscv32")]
pub struct UsbCdcRxDriver {
    usb: UsbSerialJtagRx<'static, esp_hal::Async>,
}

#[cfg(target_arch = "riscv32")]
impl UsbCdcRxDriver {
    pub fn new(usb: UsbSerialJtagRx<'static, esp_hal::Async>) -> Self {
        Self { usb }
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
static USB_CDC_TX_DRIVER: StaticCell<UsbCdcTxDriver> = StaticCell::new();
#[cfg(target_arch = "riscv32")]
static USB_CDC_TX_DRIVER_PTR: SyncCell<*mut UsbCdcTxDriver> = SyncCell::new(core::ptr::null_mut());
#[cfg(target_arch = "riscv32")]
static USB_CDC_RX_DRIVER: StaticCell<UsbCdcRxDriver> = StaticCell::new();
#[cfg(target_arch = "riscv32")]
static USB_CDC_RX_DRIVER_PTR: SyncCell<*mut UsbCdcRxDriver> = SyncCell::new(core::ptr::null_mut());

/// Async mutex guarding USB CDC TX only (Bug #9 split).
#[cfg(target_arch = "riscv32")]
static USB_CDC_TX_MUTEX: Mutex<CriticalSectionRawMutex, ()> = Mutex::new(());
/// Async mutex guarding USB CDC RX only (Bug #9 split).
#[cfg(target_arch = "riscv32")]
static USB_CDC_RX_MUTEX: Mutex<CriticalSectionRawMutex, ()> = Mutex::new(());

#[cfg(target_arch = "riscv32")]
pub fn init_usb_cdc(usb: UsbSerialJtag<'static, esp_hal::Blocking>) -> Result<(), UsbCdcError> {
    let usb_async = usb.into_async();
    let (rx_half, tx_half) = usb_async.split();
    let tx_driver = USB_CDC_TX_DRIVER.init(UsbCdcTxDriver::new(tx_half));
    let rx_driver = USB_CDC_RX_DRIVER.init(UsbCdcRxDriver::new(rx_half));
    // SAFETY: USB_CDC_*_DRIVER_PTR store raw pointers to drivers allocated
    // by StaticCell above. init_usb_cdc() runs in AppBuilder::build() before
    // any tasks start, so no concurrent access occurs. Each pointer is only
    // dereferenced inside its own mutex guard thereafter.
    unsafe {
        *USB_CDC_TX_DRIVER_PTR.get() = tx_driver as *mut UsbCdcTxDriver;
        *USB_CDC_RX_DRIVER_PTR.get() = rx_driver as *mut UsbCdcRxDriver;
    }
    Ok(())
}

#[cfg(not(target_arch = "riscv32"))]
pub fn init_usb_cdc(_usb: ()) -> Result<(), UsbCdcError> {
    Ok(())
}

/// Write bytes to USB CDC. Acquires the TX mutex only — RX may proceed
/// concurrently, so a reader blocked on `Read::read().await` does NOT stall
/// outbound protocol responses or continuous telemetry.
#[cfg(target_arch = "riscv32")]
pub async fn usb_cdc_write_bytes(data: &[u8]) -> Result<(), UsbCdcError> {
    let _guard = USB_CDC_TX_MUTEX.lock().await;
    // SAFETY: USB_CDC_TX_DRIVER_PTR points to a valid UsbCdcTxDriver
    // allocated by StaticCell in init_usb_cdc(). The TX mutex grants exclusive
    // &mut to the TX half; the RX half has its own mutex and may be held by
    // the reader task concurrently.
    let driver = unsafe { &mut *(*USB_CDC_TX_DRIVER_PTR.get()) };
    driver.write_bytes(data).await
}

/// Read bytes from USB CDC. Acquires the RX mutex only — TX may proceed
/// concurrently.
#[cfg(target_arch = "riscv32")]
pub async fn usb_cdc_read_bytes(buffer: &mut [u8]) -> Result<usize, UsbCdcError> {
    let _guard = USB_CDC_RX_MUTEX.lock().await;
    // SAFETY: same as usb_cdc_write_bytes, but for the RX half.
    let driver = unsafe { &mut *(*USB_CDC_RX_DRIVER_PTR.get()) };
    driver.read_bytes(buffer).await
}

#[cfg(not(target_arch = "riscv32"))]
pub async fn usb_cdc_write_bytes(_data: &[u8]) -> Result<(), UsbCdcError> {
    Ok(())
}

#[cfg(not(target_arch = "riscv32"))]
pub async fn usb_cdc_read_bytes(_buffer: &mut [u8]) -> Result<usize, UsbCdcError> {
    Ok(0)
}

#[cfg(test)]
#[cfg(not(target_arch = "riscv32"))]
pub fn get_usb_cdc_driver() -> Option<&'static mut UsbCdcDriver> {
    None
}
