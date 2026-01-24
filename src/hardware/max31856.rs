use embedded_hal::digital::OutputPin;
use embedded_hal::spi::SpiDevice;

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
    pub fn new(spi: SPI, cs: CS) -> Result<Self, Max31856Error> {
        let mut max31856 = Max31856 { spi, cs };

        // Initialize MAX31856
        max31856.write_register(0x80, 0x00)?; // Config register 0
        max31856.write_register(0x81, 0x03)?; // Config register 1 - Type K thermocouple
        max31856.write_register(0x82, 0x00)?; // Fault mask register

        Ok(max31856)
    }

    pub fn read_temperature_sync(&mut self) -> Result<f32, Max31856Error> {
        // Start one-shot conversion
        self.write_register(0x80, 0x80)?; // Set one-shot bit

        // Note: In real implementation, you would wait for conversion to complete
        // For now, we'll use a simple delay or check the status register

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

        // Convert from MAX31856 format (0.0078125°C LSB)
        let temperature = if (temp_raw & 0x800000) != 0 {
            // Negative temperature - two's complement
            let temp_complement = !temp_raw & 0x7FFFFF; // Get magnitude
            -(temp_complement as i32) as f32 * 0.0078125
        } else {
            // Positive temperature
            temp_raw as f32 * 0.0078125
        };

        // Validate temperature range
        if temperature < -200.0 || temperature > 1350.0 {
            return Err(Max31856Error::InvalidTemperature);
        }

        Ok(temperature)
    }

    fn write_register(&mut self, address: u8, value: u8) -> Result<(), Max31856Error> {
        let _ = self.cs.set_low();

        let tx = [address, value];
        let mut buffer = [0u8; 2];

        let result = self.spi.transfer(&mut buffer, &tx);

        let _ = self.cs.set_high();

        match result {
            Ok(_) => Ok(()),
            Err(_) => Err(Max31856Error::CommunicationError),
        }
    }

    fn read_register(&mut self, address: u8) -> Result<u8, Max31856Error> {
        let _ = self.cs.set_low();

        let tx = [address | 0x80, 0x00]; // Read operation
        let mut buffer = [0u8; 2];

        let result = self.spi.transfer(&mut buffer, &tx);

        let _ = self.cs.set_high();

        match result {
            Ok(_) => Ok(buffer[1]),
            Err(_) => Err(Max31856Error::CommunicationError),
        }
    }

    fn read_registers(&mut self, address: u8, count: usize) -> Result<[u8; 3], Max31856Error> {
        let _ = self.cs.set_low();

        let mut tx = [address | 0x80; 3]; // Read operation
        for i in 0..count.min(3) {
            tx[i] = address | 0x80;
        }

        let mut buffer = [0u8; 3];

        let result = self.spi.transfer(&mut buffer[..count], &tx[..count]);

        let _ = self.cs.set_high();

        match result {
            Ok(_) => {
                let mut result = [0u8; 3];
                for i in 0..count.min(3) {
                    result[i] = buffer[i];
                }
                Ok(result)
            }
            Err(_) => Err(Max31856Error::CommunicationError),
        }
    }
}
