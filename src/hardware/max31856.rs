use embedded_hal::digital::OutputPin;
use embedded_hal::spi::{SpiBus, SpiDevice};
use esp_hal::spi::master::Spi;
use esp_hal::spi::SpiMode;

#[derive(Debug, Clone, Copy)]
pub enum Max31856Error {
    CommunicationError,
    FaultDetected,
    InvalidTemperature,
}

pub struct Max31856<SPI, CS> {
    spi: SPI,
    cs: CS,
}

impl<SPI, CS> Max31856<SPI, CS>
where
    SPI: SpiDevice,
    CS: OutputPin,
{
    pub fn new(spi: SPI, mut cs: CS) -> Result<Self, Max31856Error> {
        let mut max31856 = Max31856 { spi, cs };

        // Initialize MAX31856
        max31856.write_register(0x80, 0x00)?; // Config register 0
        max31856.write_register(0x81, 0x03)?; // Config register 1 - Type K thermocouple
        max31856.write_register(0x82, 0x00)?; // Fault mask register

        Ok(max31856)
    }

    pub fn read_temperature(&mut self) -> Result<f32, Max31856Error> {
        // Start one-shot conversion
        self.write_register(0x80, 0x80)?; // Set one-shot bit

        // Wait for conversion to complete (about 160ms)
        embassy_time::Timer::after(embassy_time::Duration::from_millis(160)).await;

        // Read temperature registers
        let temp_data = self.read_registers(0x0C, 3)?;

        // Check for faults
        let fault = self.read_register(0x0F)?;
        if fault & 0x01 != 0 {
            return Err(Max31856Error::FaultDetected);
        }

        // Convert 24-bit temperature to Celsius
        let temp_raw =
            ((temp_data[0] as u32) << 16) | ((temp_data[1] as u32) << 8) | (temp_data[2] as u32);

        if temp_raw & 0x800000 != 0 {
            // Negative temperature
            let temp = (temp_raw as i32) >> 8;
            Ok(temp as f32 / 256.0)
        } else {
            // Positive temperature
            let temp = (temp_raw >> 8) as u32;
            Ok(temp as f32 / 256.0)
        }
    }

    fn write_register(&mut self, reg: u8, value: u8) -> Result<(), Max31856Error> {
        let _ = self.cs.set_low();
        let result = self.spi.write(&[reg | 0x80, value]); // Write bit set
        let _ = self.cs.set_high();

        match result {
            Ok(_) => Ok(()),
            Err(_) => Err(Max31856Error::CommunicationError),
        }
    }

    fn read_register(&mut self, reg: u8) -> Result<u8, Max31856Error> {
        let mut buffer = [0u8];
        let _ = self.cs.set_low();
        let result = self.spi.transfer(&mut [reg, 0], &mut buffer);
        let _ = self.cs.set_high();

        match result {
            Ok(_) => Ok(buffer[0]),
            Err(_) => Err(Max31856Error::CommunicationError),
        }
    }

    fn read_registers(&mut self, reg: u8, count: usize) -> Result<[u8; 3], Max31856Error> {
        let mut buffer = [0u8; 3];
        let _ = self.cs.set_low();

        let mut tx = [reg];
        for _ in 0..count {
            tx.push(0);
        }

        let result = self.spi.transfer(&tx, &mut buffer);
        let _ = self.cs.set_high();

        match result {
            Ok(_) => Ok(buffer),
            Err(_) => Err(Max31856Error::CommunicationError),
        }
    }
}

// Async version for Embassy
pub struct AsyncMax31856<SPI, CS> {
    inner: Max31856<SPI, CS>,
}

impl<SPI, CS> AsyncMax31856<SPI, CS>
where
    SPI: SpiDevice,
    CS: OutputPin,
{
    pub fn new(spi: SPI, cs: CS) -> Result<Self, Max31856Error> {
        Ok(AsyncMax31856 {
            inner: Max31856::new(spi, cs)?,
        })
    }

    pub async fn read_temperature(&mut self) -> Result<f32, Max31856Error> {
        // Start one-shot conversion
        self.inner.write_register(0x80, 0x80)?;

        // Wait for conversion to complete (about 160ms)
        embassy_time::Timer::after(embassy_time::Duration::from_millis(160)).await;

        // Read temperature registers
        let temp_data = self.inner.read_registers(0x0C, 3)?;

        // Check for faults
        let fault = self.inner.read_register(0x0F)?;
        if fault & 0x01 != 0 {
            return Err(Max31856Error::FaultDetected);
        }

        // Convert 24-bit temperature to Celsius
        let temp_raw =
            ((temp_data[0] as u32) << 16) | ((temp_data[1] as u32) << 8) | (temp_data[2] as u32);

        if temp_raw & 0x800000 != 0 {
            // Negative temperature
            let temp = (temp_raw as i32) >> 8;
            Ok(temp as f32 / 256.0)
        } else {
            // Positive temperature
            let temp = (temp_raw >> 8) as u32;
            Ok(temp as f32 / 256.0)
        }
    }
}
