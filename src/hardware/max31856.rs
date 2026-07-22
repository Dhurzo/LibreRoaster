use crate::control::traits::Thermometer;
use crate::control::RoasterError;
use crate::hardware::sensors::conversion::convert_raw_temp;
use embassy_time::{Duration, Instant, Timer};
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

        // CR0 (0x80): CMODE=0 (normally off), 1SHOT=0, OCFAULT=01 (comparator
        // mode on bits 1:2), FILT50=1 (50 Hz notch filter, bit 0).
        // Bit layout: 0b0001_0001 = 0x11. The 50 Hz filter is selected by
        // CR0 bit 0 (not CR1 bit 3); conversion time maxes at 185 ms (datasheet).
        max31856.write_register(0x80, 0x11)?;
        // CR1 (0x81): AVGSEL=1 sample, TC TYPE = Type K (0011). Bits 3:0 = 0011.
        // The 50/60 Hz filter is NOT a CR1 field; the old `0x0B` value was
        // selecting "voltage mode with gain ×8" (TC TYPE = 1011) — no
        // thermocouple linearization, garbage temperatures even if the
        // readback had matched.
        max31856.write_register(0x81, 0x03)?;
        // Fault Mask (0x82): all faults enabled (0 = fault pin active on any fault)
        max31856.write_register(0x82, 0x00)?;

        // Verify config register was written by reading it back
        let cr1 = max31856.read_register(0x01).unwrap_or(0xFF);
        log::info!("MAX31856 init: wrote CR1=0x03, read back CR1=0x{:02X}", cr1);
        if cr1 != 0x03 {
            return Err(Max31856Error::CommunicationError {
                source: "cr1_readback_mismatch",
            });
        }

        // Perform boot self-test
        max31856.self_test()?;

        Ok(max31856)
    }

    /// Perform boot self-test to verify MAX31856 initialization and basic functionality
    pub fn self_test(&mut self) -> Result<(), Max31856Error> {
        log::info!("MAX31856: Starting boot self-test");

        // 1. Verify CR0 register (CMODE=0, FILT50=1). Must match the value
        //    written in `new()` (0x11 = bit 4 + bit 0, see init comment).
        let cr0 = self.read_register(0x00).unwrap_or(0xFF);
        log::info!("MAX31856 self-test: CR0=0x{:02X} (expected 0x11)", cr0);
        if cr0 != 0x11 {
            log::warn!(
                "MAX31856 self-test: CR0 mismatch (expected 0x11, got 0x{:02X})",
                cr0
            );
        }

        // 2. Verify MASK register (all faults enabled = 0x00)
        let mask = self.read_register(0x02).unwrap_or(0xFF);
        log::info!("MAX31856 self-test: MASK=0x{:02X} (expected 0x00)", mask);
        if mask != 0x00 {
            log::warn!(
                "MAX31856 self-test: MASK mismatch (expected 0x00, got 0x{:02X})",
                mask
            );
        }

        // 3. Check fault register for open circuit (informational only).
        // Per the MAX31856 datasheet, Open Circuit is bit 0 (value 0x01) —
        // the previous code checked 0x40 (which is actually TC Range, bit 6).
        let fault = self.read_register(0x0F).unwrap_or(0xFF);
        if fault & 0x01 != 0 {
            log::warn!(
                "MAX31856 self-test: Open circuit detected (fault=0x{:02X})",
                fault
            );
        } else {
            log::info!(
                "MAX31856 self-test: No open circuit fault (fault=0x{:02X})",
                fault
            );
        }

        // 4. Perform one-shot conversion and verify temperature
        self.trigger_conversion()?;
        log::info!(
            "MAX31856 self-test: Conversion triggered, waiting 185ms (50Hz conversion time)..."
        );

        // Spec F2.2: use Instant::elapsed() instead of fixed-iteration spin_loop.
        // 50 Hz conversion time is ~185 ms per datasheet. Busy-wait using wall-clock
        // time — this is more accurate than the fixed-iteration spin_loop.
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(185) {
            // spin_loop hint yields to the scheduler on a pre-emptive RTOS
            core::hint::spin_loop();
        }

        let reading = self.read_conversion_result()?;
        let temperature = convert_raw_temp(reading.raw_temp);

        log::info!(
            "MAX31856 self-test: Temperature reading = {:.1}°C (raw={:#010x}, fault=0x{:02X})",
            temperature,
            reading.raw_temp,
            reading.fault
        );

        // 5. Verify temperature is within reasonable ambient range (-50°C to 100°C)
        if !(-50.0..=100.0).contains(&temperature) {
            log::warn!(
                "MAX31856 self-test: Temperature out of expected range: {:.1}°C",
                temperature
            );
        } else {
            log::info!("MAX31856 self-test: Temperature within expected range");
        }

        log::info!("MAX31856: Boot self-test completed");
        Ok(())
    }

    pub fn configure_type_k(&mut self) -> Result<(), Max31856Error> {
        // Type K (TC TYPE = 0011). The 50 Hz filter lives in CR0 bit 0, not
        // in CR1; the old `0x0B` value selected voltage mode ×8 (TC TYPE = 1011).
        self.write_register(0x81, 0x03)?;
        Ok(())
    }

    fn read_conversion_block(&mut self) -> Result<Max31856Reading, Max31856Error> {
        // Read all 4 bytes (0x0C-0x0F) in single SPI burst for better performance
        let mut rx_buffer = [0u8; 4];
        let mut operations = [
            embedded_hal::spi::Operation::Write(&[0x0C & 0x7F]), // Address with read bit (A7=0)
            embedded_hal::spi::Operation::Read(&mut rx_buffer),  // Read 4 bytes continuously
        ];

        match self.spi.transaction(&mut operations) {
            Ok(_) => (),
            Err(_) => {
                return Err(Max31856Error::CommunicationError {
                    source: "spi_read_conversion_block_failed",
                })
            }
        }

        let raw_temp =
            ((rx_buffer[0] as u32) << 16) | ((rx_buffer[1] as u32) << 8) | (rx_buffer[2] as u32);
        let fault = rx_buffer[3];

        // Bug #6 mitigation: this line is HIT on EVERY sensor read (~6/s at the
        // default control cadence, plus once per sensor per tick). An `info!`
        // dump here floods the same physical UART/USB-Serial-JTAG channel that
        // carries the Artisan protocol — corrupting READ responses and
        // continuous telemetry in real sessions. Demoted to `debug!` so it
        // only appears under the `instrumentation` feature (which raises the
        // log level filter to Debug). A future, HW-validated fix installs a
        // dedicated log sink (see plan-informe F4 / LibreRoaster_11_Fixes_Criticos
        // fix #6).
        log::debug!(
            "MAX31856 raw: temp_reg=[0x{:02X},0x{:02X},0x{:02X}] fault=0x{:02X} raw_temp={:#010x}",
            rx_buffer[0],
            rx_buffer[1],
            rx_buffer[2],
            fault,
            raw_temp
        );

        Ok(Max31856Reading { raw_temp, fault })
    }

    /// Synchronous temperature read with busy-wait delay.
    ///
    /// # Deprecation
    ///
    /// This method uses a blocking spin loop (`spin_loop()` × 1,600,000 iterations)
    /// instead of an async timer. Use [`read_raw_temperature_async()`] in all
    /// Embassy task contexts. This sync variant exists only for non-async test
    /// harnesses and initialization paths.
    #[deprecated = "Use read_raw_temperature_async() in async contexts"]
    pub fn read_raw_temperature(&mut self) -> Result<Max31856Reading, Max31856Error> {
        // Bug #B29: the previous `0x80` value set CMODE (bit 7 = continuous
        // conversion) and cleared OCFAULT/FILT50 — masking open-circuit
        // faults and losing the 50 Hz filter. Use the same one-shot value
        // as the async path (0x51 = 1SHOT | FILT50 | OCFAULT=01).
        self.write_register(0x80, 0x51)?;

        // Match the async conversion wait (185 ms max at 50 Hz).
        const DELAY_MS: u64 = crate::config::constants::MAX31856_CONVERSION_TIME_MS;

        for _ in 0..(DELAY_MS * 10000) {
            core::hint::spin_loop();
        }

        self.read_conversion_block()
    }

    pub fn trigger_conversion(&mut self) -> Result<(), Max31856Error> {
        // CR0 with 1SHOT=1 (bit 6) triggers a single conversion in normally-off
        // mode. Preserve CMODE=0, OCFAULT settings from init, AND the 50 Hz
        // notch filter (bit 0 = FILT50). 0x51 = 0b0101_0001.
        // Bug #B1: the previous value 0x50 dropped FILT50 on every one-shot,
        // re-selecting 60 Hz mid-roast.
        self.write_register(0x80, 0x51)?;
        Ok(())
    }

    pub fn read_conversion_result(&mut self) -> Result<Max31856Reading, Max31856Error> {
        self.read_conversion_block()
    }

    pub async fn read_raw_temperature_async(&mut self) -> Result<Max31856Reading, Max31856Error> {
        // Trigger one-shot conversion in normally-off mode (CMODE=0, 1SHOT=1,
        // FILT50 preserved). Bug #B1: keep the 50 Hz filter (bit 0) on each shot.
        self.write_register(0x80, 0x51)?;

        // Bug #B1: 50 Hz conversion time is 185 ms max per datasheet. The
        // previous TEMPERATURE_READ_INTERVAL_MS (160 ms) wait could return the
        // *previous* conversion's result without any error indication.
        Timer::after(Duration::from_millis(
            crate::config::constants::MAX31856_CONVERSION_TIME_MS,
        ))
        .await;

        self.read_conversion_block()
    }

    #[allow(deprecated)]
    pub fn read_temperature(&mut self) -> Result<f32, Max31856Error> {
        let reading = self.read_raw_temperature()?;
        // MAX31856 Fault Register (0x0F): Open(0x01), OVUV(0x02), TC Low(0x04),
        // TC High(0x08), CJ Low(0x10), CJ High(0x20), TC Range(0x40), CJ Range(0x80).
        // Any bit set is a fault — the previous `& 0x7F` mask dropped CJ Range
        // (0x80), which is a legitimate cold-junction out-of-range fault.
        if reading.fault != 0 {
            return Err(Max31856Error::FaultDetected {
                source: "fault_bit_set",
            });
        }

        let temperature = convert_raw_temp(reading.raw_temp);

        if !(-200.0..=1350.0).contains(&temperature) {
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
        // MAX31856 Fault Register (0x0F): Open(0x01), OVUV(0x02), TC Low(0x04),
        // TC High(0x08), CJ Low(0x10), CJ High(0x20), TC Range(0x40), CJ Range(0x80).
        // Any bit set is a fault — previously masked with 0x7F, dropping CJ Range.
        if reading.fault != 0 {
            return Err(Max31856Error::FaultDetected {
                source: "fault_bit_set",
            });
        }

        let temperature = convert_raw_temp(reading.raw_temp);

        if !(-200.0..=1350.0).contains(&temperature) {
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
        // MAX31856 SPI: A7=0 for read, A7=1 for write.
        // Two separate phases: address then data.
        let mut rx_buffer = [0u8; 1];
        let mut operations = [
            embedded_hal::spi::Operation::Write(&[address & 0x7F]),
            embedded_hal::spi::Operation::Read(&mut rx_buffer),
        ];

        match self.spi.transaction(&mut operations) {
            Ok(_) => Ok(rx_buffer[0]),
            Err(_) => Err(Max31856Error::CommunicationError {
                source: "spi_read_failed",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spi_error_kind() {
        use embedded_hal::spi::Error as _;
        let err = Max31856Error::CommunicationError { source: "test" };
        assert!(matches!(err.kind(), embedded_hal::spi::ErrorKind::Other));
    }
}
