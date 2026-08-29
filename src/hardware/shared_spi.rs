//! Shared SPI bus abstractions for the MAX31856 thermocouples.
//!
//! `SharedSpiDevice` arbitrates a single `SpiBus` behind a `critical_section`
//! `Mutex<RefCell<_>>` so both amplifiers can share one SPI peripheral.
//! `SpiDeviceWithCs` layers per-device chip-select and the MAX31856 tCS hold.

use core::cell::RefCell;
use critical_section::Mutex;
use embedded_hal::digital::OutputPin;
use embedded_hal::spi::{ErrorType, Operation, SpiBus, SpiDevice};

/// Shared SPI error type (always `ErrorKind::Other`).
#[derive(Debug, Clone, Copy, Default)]
pub struct SpiError;

impl embedded_hal::spi::Error for SpiError {
    fn kind(&self) -> embedded_hal::spi::ErrorKind {
        embedded_hal::spi::ErrorKind::Other
    }
}

/// A `SpiDevice` that locks a shared `SpiBus` for the duration of a transaction.
pub struct SharedSpiDevice<'a, T> {
    /// Borrow of the shared SPI bus mutex.
    spi_bus: &'a Mutex<RefCell<T>>,
}

impl<'a, T> SharedSpiDevice<'a, T> {
    /// Wrap a shared SPI bus mutex as a `SpiDevice`.
    pub fn new(spi_bus: &'a Mutex<RefCell<T>>) -> Self {
        Self { spi_bus }
    }
}

impl<'a, T> ErrorType for SharedSpiDevice<'a, T>
where
    T: ErrorType,
{
    type Error = T::Error;
}

impl<'a, T> SpiDevice for SharedSpiDevice<'a, T>
where
    T: SpiBus,
{
    /// Run the full operation list under a single critical section, flushing
    /// before CS is released (see `SpiDeviceWithCs`).
    fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
        critical_section::with(|cs| {
            let mut bus = self.spi_bus.borrow(cs).borrow_mut();
            for op in operations {
                match op {
                    Operation::Write(buf) => {
                        bus.write(buf)?;
                    }
                    Operation::Read(buf) => {
                        bus.read(buf)?;
                    }
                    Operation::Transfer(read, write) => {
                        bus.transfer(read, write)?;
                    }
                    Operation::TransferInPlace(buf) => {
                        bus.transfer_in_place(buf)?;
                    }
                    Operation::DelayNs(ns) => {
                        // Cap at 1 ms to prevent unbounded executor blocking.
                        // Normal SPI inter-byte delays are 100–1000 ns (10–100 iterations);
                        // larger values would indicate a programming error.
                        let capped_ns = (*ns).min(1_000_000);
                        if *ns > 1_000_000 {
                            log::warn!("SPI DelayNs capped from {} ns to 1 ms", *ns);
                        }
                        let cycles = capped_ns / 10;
                        for _ in 0..cycles {
                            core::hint::spin_loop();
                        }
                    }
                }
            }
            // MANDATORY flush before CS is raised by SpiDeviceWithCs: without it, the
            // FIFO can still be draining while CS is asserted high, truncating the last
            // write bytes (e.g. MAX31856 register writes silently dropped → POR 0x000000
            // = 0.0°C with no failure flag).
            bus.flush()?;
            Ok(())
        })
    }
}

/// A `SpiDevice` that drives a per-device chip-select around the shared bus.
pub struct SpiDeviceWithCs<'a, T, CS> {
    /// Inner shared-bus device.
    spi: SharedSpiDevice<'a, T>,
    /// Chip-select output pin for this device.
    cs: CS,
}

impl<'a, T, CS> SpiDeviceWithCs<'a, T, CS>
where
    T: SpiBus,
    CS: OutputPin,
{
    /// Create a CS-gated device, asserting CS high (deselected) at init.
    pub fn new(spi_bus: &'a Mutex<RefCell<T>>, mut cs: CS) -> Self {
        let _ = cs.set_high();
        Self {
            spi: SharedSpiDevice::new(spi_bus),
            cs,
        }
    }
}

impl<'a, T, CS> ErrorType for SpiDeviceWithCs<'a, T, CS>
where
    T: ErrorType,
{
    type Error = T::Error;
}

impl<'a, T, CS> SpiDevice for SpiDeviceWithCs<'a, T, CS>
where
    T: SpiBus,
    CS: OutputPin,
{
    /// Assert CS, run operations on the shared bus, then release CS with the
    /// MAX31856 tCS hold delay.
    fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
        let _ = self.cs.set_low();

        let result = self.spi.transaction(operations);

        let _ = self.cs.set_high();

        // Add 1µs delay after CS goes high to meet MAX31856 tCS requirement (≥400ns between transactions on shared bus)
        // On ESP32-C3 at 160MHz, ~160+ cycles = ~1µs using spin_loop()
        for _ in 0..160 {
            core::hint::spin_loop();
        }

        result
    }
}
