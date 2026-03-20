use crate::control::traits::{AsyncThermometer, Thermometer};
use crate::control::RoasterError;
use crate::hardware::sensors::conversion::convert_raw_temp;
use embassy_time::{Duration, Timer};
use embedded_hal::spi::SpiDevice;

/// Type aliases for concrete SPI device types used in this application
/// These allow storing Max31856 with concrete types to enable async methods
#[allow(dead_code)]
#[cfg(target_arch = "riscv32")]
pub mod bt_spi {
    use crate::hardware::shared_spi::SpiDeviceWithCs;
    // Bean Temperature SPI type - using 'static for simplicity
    pub type BtSpi = SpiDeviceWithCs<
        'static,
        esp_hal::spi::master::Spi<'static, esp_hal::Blocking>,
        esp_hal::gpio::Output<'static>,
    >;
}

#[allow(dead_code)]
#[cfg(target_arch = "riscv32")]
pub mod et_spi {
    use crate::hardware::shared_spi::SpiDeviceWithCs;
    // Environment Temperature SPI type - using 'static for simplicity
    pub type EtSpi = SpiDeviceWithCs<
        'static,
        esp_hal::spi::master::Spi<'static, esp_hal::Blocking>,
        esp_hal::gpio::Output<'static>,
    >;
}

#[derive(Debug, Clone, Copy)]
pub enum Max31856Error {
    CommunicationError { source: &'static str },
    FaultDetected { source: &'static str },
    InvalidTemperature { source: &'static str },
}

/// Raw conversion payload returned by the MAX31856 driver.
pub struct Max31856Reading {
    pub raw_temp: u32,
    pub fault: u8,
}

impl embedded_hal::spi::Error for Max31856Error {
    fn kind(&self) -> embedded_hal::spi::ErrorKind {
        match self {
            Max31856Error::CommunicationError { .. } => embedded_hal::spi::ErrorKind::Other,
            Max31856Error::FaultDetected { .. } => embedded_hal::spi::ErrorKind::Other,
            Max31856Error::InvalidTemperature { .. } => embedded_hal::spi::ErrorKind::Other,
        }
    }
}

impl From<Max31856Error> for RoasterError {
    fn from(e: Max31856Error) -> Self {
        match e {
            Max31856Error::CommunicationError { source } => RoasterError::SensorFault {
                source: Some(source),
            },
            Max31856Error::FaultDetected { source } => RoasterError::SensorFault {
                source: Some(source),
            },
            Max31856Error::InvalidTemperature { source } => RoasterError::TemperatureOutOfRange {
                source: Some(source),
            },
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

        max31856.write_register(0x80, 0x00)?; // Config register 0
        max31856.write_register(0x81, 0x03)?; // Config register 1 - Type K thermocouple
        max31856.write_register(0x82, 0x00)?; // Fault mask register

        Ok(max31856)
    }

    pub fn configure_type_k(&mut self) -> Result<(), Max31856Error> {
        self.write_register(0x81, 0x03)?;
        Ok(())
    }

    fn read_conversion_block(&mut self) -> Result<Max31856Reading, Max31856Error> {
        let temp_data = self.read_registers(0x0C, 3)?;
        let fault = self.read_register(0x0F)?;
        let raw_temp =
            ((temp_data[0] as u32) << 16) | ((temp_data[1] as u32) << 8) | (temp_data[2] as u32);

        Ok(Max31856Reading { raw_temp, fault })
    }

    pub fn read_raw_temperature(&mut self) -> Result<Max31856Reading, Max31856Error> {
        self.write_register(0x80, 0x80)?; // Set one-shot bit

        const DELAY_MS: u64 = 160;

        for _ in 0..(DELAY_MS * 10000) {
            core::hint::spin_loop();
        }

        self.read_conversion_block()
    }

    pub async fn read_raw_temperature_async(&mut self) -> Result<Max31856Reading, Max31856Error> {
        self.write_register(0x80, 0x80)?; // Set one-shot bit

        Timer::after(Duration::from_millis(160)).await;

        self.read_conversion_block()
    }

    pub fn read_temperature(&mut self) -> Result<f32, Max31856Error> {
        let reading = self.read_raw_temperature()?;
        if reading.fault & 0x01 != 0 {
            return Err(Max31856Error::FaultDetected {
                source: "fault_bit_set",
            });
        }

        let temperature = convert_raw_temp(reading.raw_temp);

        if temperature < -200.0 || temperature > 1350.0 {
            return Err(Max31856Error::InvalidTemperature {
                source: "value_out_of_range",
            });
        }

        Ok(temperature)
    }

    /// Async temperature read using embassy-time Timer instead of blocking spin loop.
    /// This prevents blocking the async executor during the 160ms conversion delay.
    pub async fn read_temperature_async(&mut self) -> Result<f32, Max31856Error> {
        let reading = self.read_raw_temperature_async().await?;
        if reading.fault & 0x01 != 0 {
            return Err(Max31856Error::FaultDetected {
                source: "fault_bit_set",
            });
        }

        let temperature = convert_raw_temp(reading.raw_temp);

        if temperature < -200.0 || temperature > 1350.0 {
            return Err(Max31856Error::InvalidTemperature {
                source: "value_out_of_range",
            });
        }

        Ok(temperature)
    }

    /// Async temperature read with retry logic.
    /// Attempts up to max_retries + 1 times (so max_retries=2 means 3 total attempts).
    /// Waits fixed 10ms between retries using embassy-time Timer.
    pub async fn read_with_retry(&mut self, max_retries: u8) -> Result<f32, Max31856Error> {
        let mut last_error = Max31856Error::CommunicationError {
            source: "retry_limit_reached",
        };

        for attempt in 0..=max_retries {
            match self.read_temperature_async().await {
                Ok(temp) => return Ok(temp),
                Err(e) => {
                    last_error = e;
                    // Wait 10ms before retry (not on last attempt)
                    if attempt < max_retries {
                        Timer::after(Duration::from_millis(10)).await;
                    }
                }
            }
        }

        Err(last_error)
    }

    fn write_register(&mut self, address: u8, value: u8) -> Result<(), Max31856Error> {
        let mut operations = [embedded_hal::spi::Operation::Write(&[address, value])];

        match self.spi.transaction(&mut operations) {
            Ok(_) => Ok(()),
            Err(_) => Err(Max31856Error::CommunicationError {
                source: "spi_write_failed",
            }),
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
            Err(_) => Err(Max31856Error::CommunicationError {
                source: "spi_read_failed",
            }),
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
            Err(_) => Err(Max31856Error::CommunicationError {
                source: "spi_read_multiple_failed",
            }),
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

impl<SPI> AsyncThermometer for Max31856<SPI>
where
    SPI: SpiDevice + Send,
{
    async fn read_temperature_async(&mut self) -> Result<f32, RoasterError> {
        // Use read_with_retry for reliability (max_retries=2 = 3 attempts)
        Self::read_with_retry(self, 2).await.map_err(|e| e.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spi_error_kind() {
        let err = Max31856Error::CommunicationError {
            source: "test",
        };
        assert!(matches!(err.kind(), embedded_hal::spi::ErrorKind::Other));
    }
}

