use crate::control::RoasterError;
use crate::hardware::max31856::Max31856Error;
use embassy_time::Instant;
#[cfg(target_arch = "riscv32")]
use embedded_hal::spi::SpiDevice;

#[cfg(target_arch = "riscv32")]
use crate::hardware::max31856::{bt_spi::BtSpi, et_spi::EtSpi, Max31856};

/// MAX31856 reports temperature using a 0.0078125°C LSB and two's-complement math.
/// The 19-bit temperature value occupies bits [23:5] of the 24-bit concatenated
/// register read (LTCB0<<16 | LTCB1<<8 | LTCB2). Shift right by 5 to align the
/// LSB to bit 0 before multiplying by the LSB weight.
pub const MAX31856_LSB: f32 = 0.0078125;

/// Maximum consecutive sensor read fallbacks before reporting error.
/// At 100ms control loop cadence, 5 fallbacks = 500ms of stale data.
const MAX_CONSECUTIVE_SENSOR_FALLBACKS: u8 = 5;

pub fn convert_raw_temp(raw_temp: u32) -> f32 {
    if (raw_temp & 0x800000) != 0 {
        // Sign bit (bit 23, which is bit 18 of the 19-bit value) set → negative.
        // Extract 19-bit two's complement absolute value after right-shifting.
        let temp_shifted = raw_temp >> 5;
        let temp_complement = (!temp_shifted & 0x7FFFF) + 1;
        -(temp_complement as f32) * MAX31856_LSB
    } else {
        (raw_temp >> 5) as f32 * MAX31856_LSB
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SensorFault {
    pub open_circuit: bool,
    pub short_to_vcc: bool,
    pub short_to_gnd: bool,
    pub cold_junction_high: bool,
    pub cold_junction_low: bool,
    pub communication_error: bool,
    pub invalid_temperature: bool,
    pub fault_detected: bool,
}

impl SensorFault {
    #[allow(dead_code)]
    fn from_register(fault: u8) -> Self {
        let has_fault = fault & 0x1F != 0;
        Self {
            open_circuit: fault & 0x01 != 0,
            short_to_vcc: fault & 0x02 != 0,
            short_to_gnd: fault & 0x04 != 0,
            cold_junction_high: fault & 0x08 != 0,
            cold_junction_low: fault & 0x10 != 0,
            fault_detected: has_fault,
            ..Default::default()
        }
    }

    #[allow(dead_code)]
    fn from_max31856_error(error: &Max31856Error) -> Self {
        match error {
            Max31856Error::CommunicationError { .. } => Self {
                communication_error: true,
                ..Default::default()
            },
            Max31856Error::FaultDetected { .. } => Self {
                fault_detected: true,
                ..Default::default()
            },
            Max31856Error::InvalidTemperature { .. } => Self {
                invalid_temperature: true,
                ..Default::default()
            },
        }
    }

    pub fn has_fault(&self) -> bool {
        self.open_circuit
            || self.short_to_vcc
            || self.short_to_gnd
            || self.cold_junction_high
            || self.cold_junction_low
            || self.communication_error
            || self.invalid_temperature
            || self.fault_detected
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SensorSample {
    pub bean_temp: f32,
    pub env_temp: f32,
    pub bean_fault: SensorFault,
    pub env_fault: SensorFault,
    pub timestamp: Instant,
}

impl SensorSample {
    fn with_timestamp(timestamp: Instant) -> Self {
        Self {
            bean_temp: 0.0,
            env_temp: 0.0,
            bean_fault: SensorFault::default(),
            env_fault: SensorFault::default(),
            timestamp,
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum SensorChannel {
    Bean,
    Env,
}

#[allow(dead_code)]
type SensorChannelResult = Result<(f32, SensorFault), Max31856Error>;

#[cfg(feature = "regression")]
#[derive(Clone, Copy)]
pub struct FixtureReading {
    pub bean_adc: [u8; 3],
    pub bean_fault: u8,
    pub env_adc: [u8; 3],
    pub env_fault: u8,
}

#[cfg(feature = "regression")]
impl FixtureReading {
    fn to_channel_results(self) -> (SensorChannelResult, SensorChannelResult) {
        (
            SensorConversionHub::channel_result_from_bytes(self.bean_adc, self.bean_fault),
            SensorConversionHub::channel_result_from_bytes(self.env_adc, self.env_fault),
        )
    }
}

pub struct SensorConversionHub {
    #[cfg(target_arch = "riscv32")]
    bean_sensor: Max31856<BtSpi>,
    #[cfg(target_arch = "riscv32")]
    env_sensor: Max31856<EtSpi>,
    last_sample: Option<SensorSample>,
    bean_consecutive_fallbacks: u8,
    env_consecutive_fallbacks: u8,
}

impl SensorConversionHub {
    #[cfg(target_arch = "riscv32")]
    pub fn new(bean_sensor: Max31856<BtSpi>, env_sensor: Max31856<EtSpi>) -> Self {
        Self {
            bean_sensor,
            env_sensor,
            last_sample: None,
            bean_consecutive_fallbacks: 0,
            env_consecutive_fallbacks: 0,
        }
    }

    #[cfg(not(target_arch = "riscv32"))]
    pub fn new() -> Self {
        Self {
            last_sample: None,
            bean_consecutive_fallbacks: 0,
            env_consecutive_fallbacks: 0,
        }
    }

    pub fn last_sample(&self) -> Option<SensorSample> {
        self.last_sample
    }

    #[cfg(feature = "regression")]
    fn channel_result_from_bytes(adc_bytes: [u8; 3], fault: u8) -> SensorChannelResult {
        let raw_temp =
            ((adc_bytes[0] as u32) << 16) | ((adc_bytes[1] as u32) << 8) | (adc_bytes[2] as u32);

        let temperature = convert_raw_temp(raw_temp);
        let sensor_fault = SensorFault::from_register(fault);

        Ok((temperature, sensor_fault))
    }

    #[cfg(feature = "regression")]
    pub fn sample_from_fixture(
        &mut self,
        fixture: FixtureReading,
    ) -> Result<SensorSample, RoasterError> {
        let timestamp = Instant::now();
        let (bean_result, env_result) = fixture.to_channel_results();
        self.build_sample(timestamp, bean_result, env_result)
    }

    #[cfg(feature = "regression")]
    pub fn from_fixture(fixture: FixtureReading) -> Result<Self, RoasterError> {
        let mut hub = Self::new();
        hub.sample_from_fixture(fixture)?;
        Ok(hub)
    }

    pub async fn sample(&mut self) -> Result<SensorSample, RoasterError> {
        #[cfg(target_arch = "riscv32")]
        {
            let timestamp = Instant::now();
            let (bean, env) = self.sample_parallel().await;
            self.build_sample(timestamp, bean, env)
        }
        #[cfg(not(target_arch = "riscv32"))]
        {
            let timestamp = Instant::now();
            let sample = SensorSample::with_timestamp(timestamp);
            self.last_sample = Some(sample);
            Ok(sample)
        }
    }

    #[cfg(target_arch = "riscv32")]
    #[allow(dead_code)]
    async fn read_bean_async(&mut self) -> SensorChannelResult {
        Self::read_sensor_async(&mut self.bean_sensor).await
    }

    #[cfg(target_arch = "riscv32")]
    #[allow(dead_code)]
    async fn read_env_async(&mut self) -> SensorChannelResult {
        Self::read_sensor_async(&mut self.env_sensor).await
    }

    #[cfg(target_arch = "riscv32")]
    #[allow(dead_code)]
    async fn read_sensor_async<SPI>(sensor: &mut Max31856<SPI>) -> SensorChannelResult
    where
        SPI: SpiDevice,
    {
        let reading = sensor.read_raw_temperature_async().await?;
        Ok((
            convert_raw_temp(reading.raw_temp),
            SensorFault::from_register(reading.fault),
        ))
    }

    #[cfg(target_arch = "riscv32")]
    async fn sample_parallel(&mut self) -> (SensorChannelResult, SensorChannelResult) {
        // Trigger both sensor conversions in parallel by starting both conversions
        // before any await, then wait once, then read both results.
        //
        // This reduces sampling time from ~320-360ms (serial) to ~160-180ms (parallel)
        // bringing control loop from ~2-3Hz to target 10Hz.

        // Trigger bean sensor conversion (one-shot) - fast SPI write, ~50us
        let bean_trigger_result = self.bean_sensor.trigger_conversion();

        // Trigger env sensor conversion (one-shot) - fast SPI write, ~50us
        let env_trigger_result = self.env_sensor.trigger_conversion();

        // Wait once for both conversions to complete (TEMPERATURE_READ_INTERVAL_MS is the typical conversion time)
        embassy_time::Timer::after(embassy_time::Duration::from_millis(
            crate::config::constants::TEMPERATURE_READ_INTERVAL_MS as u64,
        ))
        .await;

        // Read bean sensor result - fast SPI read, ~50us
        let bean_result = bean_trigger_result
            .and_then(|_| self.bean_sensor.read_conversion_result())
            .map(|reading| {
                (
                    convert_raw_temp(reading.raw_temp),
                    SensorFault::from_register(reading.fault),
                )
            });

        // Read env sensor result - fast SPI read, ~50us
        let env_result = env_trigger_result
            .and_then(|_| self.env_sensor.read_conversion_result())
            .map(|reading| {
                (
                    convert_raw_temp(reading.raw_temp),
                    SensorFault::from_register(reading.fault),
                )
            });

        (bean_result, env_result)
    }

    #[allow(dead_code)]
    fn build_sample(
        &mut self,
        timestamp: Instant,
        bean_result: SensorChannelResult,
        env_result: SensorChannelResult,
    ) -> Result<SensorSample, RoasterError> {
        let previous = self.last_sample;
        let mut sample = previous.unwrap_or_else(|| SensorSample::with_timestamp(timestamp));
        sample.timestamp = timestamp;

        let mut bean_fb = self.bean_consecutive_fallbacks;
        let (bean_temp, bean_fault) =
            Self::resolve_channel(SensorChannel::Bean, bean_result, previous, &mut bean_fb)?;
        self.bean_consecutive_fallbacks = bean_fb;
        sample.bean_temp = bean_temp;
        sample.bean_fault = bean_fault;

        let mut env_fb = self.env_consecutive_fallbacks;
        let (env_temp, env_fault) =
            Self::resolve_channel(SensorChannel::Env, env_result, previous, &mut env_fb)?;
        self.env_consecutive_fallbacks = env_fb;
        sample.env_temp = env_temp;
        sample.env_fault = env_fault;

        self.last_sample = Some(sample);
        Ok(sample)
    }

    #[allow(dead_code)]
    fn resolve_channel(
        channel: SensorChannel,
        result: SensorChannelResult,
        previous: Option<SensorSample>,
        consecutive_fallbacks: &mut u8,
    ) -> Result<(f32, SensorFault), RoasterError> {
        match result {
            Ok(tuple) => {
                *consecutive_fallbacks = 0;
                Ok(tuple)
            }
            Err(err) => {
                *consecutive_fallbacks = consecutive_fallbacks.saturating_add(1);
                if *consecutive_fallbacks >= MAX_CONSECUTIVE_SENSOR_FALLBACKS {
                    return Err(RoasterError::HardwareError {
                        source: Some("consecutive_sensor_fallbacks_exceeded"),
                    });
                }
                let fallback_temp = match (channel, previous) {
                    (_, Some(prev)) => match channel {
                        SensorChannel::Bean => prev.bean_temp,
                        SensorChannel::Env => prev.env_temp,
                    },
                    (SensorChannel::Bean, None) => 0.0,
                    (SensorChannel::Env, None) => 0.0,
                };
                let fault = SensorFault::from_max31856_error(&err);
                Ok((fallback_temp, fault))
            }
        }
    }
}

#[cfg(not(target_arch = "riscv32"))]
impl Default for SensorConversionHub {
    fn default() -> Self {
        Self::new()
    }
}
