use core::cell::RefCell;
use critical_section::Mutex;
use embedded_hal::digital::OutputPin;
use embedded_hal::spi::{ErrorType, Operation, SpiBus, SpiDevice};

#[derive(Debug, Clone, Copy, Default)]
pub struct SpiError;

impl embedded_hal::spi::Error for SpiError {
    fn kind(&self) -> embedded_hal::spi::ErrorKind {
        embedded_hal::spi::ErrorKind::Other
    }
}

pub struct SharedSpiDevice<'a, T> {
    spi_bus: &'a Mutex<RefCell<T>>,
}

impl<'a, T> SharedSpiDevice<'a, T> {
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
                        let cycles = (*ns as u64) / 10;
                        for _ in 0..cycles {
                            core::hint::spin_loop();
                        }
                    }
                }
            }
            Ok(())
        })
    }
}

pub struct SpiDeviceWithCs<'a, T, CS> {
    spi: SharedSpiDevice<'a, T>,
    cs: CS,
}

impl<'a, T, CS> SpiDeviceWithCs<'a, T, CS>
where
    T: SpiBus,
    CS: OutputPin,
{
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
    fn transaction(&mut self, operations: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
        let _ = self.cs.set_low();

        let result = self.spi.transaction(operations);

        let _ = self.cs.set_high();

        result
    }
}
