use crate::control::traits::Thermometer;
use crate::control::RoasterError;
use embedded_hal::spi::SpiDevice;

#[derive(Debug, Clone, Copy)]
pub enum Max31856Error {
    CommunicationError,
    FaultDetected,
    InvalidTemperature,
}

impl From<Max31856Error> for RoasterError {
    fn from(e: Max31856Error) -> Self {
        match e {
            Max31856Error::CommunicationError => RoasterError::SensorFault,
            Max31856Error::FaultDetected => RoasterError::SensorFault,
            Max31856Error::InvalidTemperature => RoasterError::TemperatureOutOfRange,
        }
    }
}

pub struct Max31856<SPI> {
    spi: SPI,
}

impl<SPI> Max31856<SPI>
where
    SPI: SpiDevice,
{
    pub fn new(spi: SPI) -> Result<Self, Max31856Error> {
        let mut max31856 = Max31856 { spi };

        // Initialize MAX31856
        max31856.write_register(0x80, 0x00)?; // Config register 0
        max31856.write_register(0x81, 0x03)?; // Config register 1 - Type K thermocouple
        max31856.write_register(0x82, 0x00)?; // Fault mask register

        Ok(max31856)
    }

    /// Configure for Type K thermocouple
    pub fn configure_type_k(&mut self) -> Result<(), Max31856Error> {
        self.write_register(0x81, 0x03)?; // Config register 1 - Type K thermocouple
        Ok(())
    }

    /// Synchronous read temperature
    pub fn read_temperature(&mut self) -> Result<f32, Max31856Error> {
        // Start one-shot conversion
        self.write_register(0x80, 0x80)?; // Set one-shot bit

        // Wait for conversion (160ms typical for MAX31856)
        // Using busy wait for synchronous context
        const DELAY_MS: u64 = 160;

        // Simple busy loop delay (approximate)
        for _ in 0..(DELAY_MS * 10000) {
            // 10k cycles per ms approx
            core::hint::spin_loop();
        }

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
        let mut operations = [embedded_hal::spi::Operation::Write(&[address, value])];

        match self.spi.transaction(&mut operations) {
            Ok(_) => Ok(()),
            Err(_) => Err(Max31856Error::CommunicationError),
        }
    }

    fn read_register(&mut self, address: u8) -> Result<u8, Max31856Error> {
        let mut rx_buffer = [0u8; 2];
        let mut operations = [
            embedded_hal::spi::Operation::Write(&[address | 0x80, 0x00]), // Read operation
            embedded_hal::spi::Operation::Read(&mut rx_buffer),
        ];

        match self.spi.transaction(&mut operations) {
            Ok(_) => Ok(rx_buffer[1]),
            Err(_) => Err(Max31856Error::CommunicationError),
        }
    }

    fn read_registers(&mut self, address: u8, count: usize) -> Result<[u8; 3], Max31856Error> {
        let mut rx_buffer = [0u8; 3];
        let tx = [address | 0x80; 3]; // Read operation

        let mut operations = [
            embedded_hal::spi::Operation::Write(&tx[..count]),
            embedded_hal::spi::Operation::Read(&mut rx_buffer[..count]),
        ];

        match self.spi.transaction(&mut operations) {
            Ok(_) => {
                let mut result = [0u8; 3];
                for i in 0..count.min(3) {
                    result[i] = rx_buffer[i];
                }
                Ok(result)
            }
            Err(_) => Err(Max31856Error::CommunicationError),
        }
    }
}

impl<SPI> Thermometer for Max31856<SPI>
where
    SPI: SpiDevice + Send,
{
    fn read_temperature(&mut self) -> Result<f32, RoasterError> {
        Self::read_temperature(self).map_err(|e| e.into())
    }
}
