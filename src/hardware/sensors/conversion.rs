use crate::control::RoasterError;
use crate::hardware::max31856::Max31856Error;
use embassy_time::Instant;
#[cfg(all(target_arch = "riscv32", not(feature = "simulated-sensors")))]
use embedded_hal::spi::SpiDevice;

#[cfg(all(target_arch = "riscv32", not(feature = "simulated-sensors")))]
use crate::hardware::max31856::{bt_spi::BtSpi, et_spi::EtSpi, Max31856};

#[cfg(feature = "simulated-sensors")]
use super::simulated::SimulatedSensorSource;

/// MAX31856 reports temperature using a 0.0078125°C LSB and two's-complement math.
/// The 19-bit temperature value occupies bits [23:5] of the 24-bit concatenated
/// register read (LTCB0<<16 | LTCB1<<8 | LTCB2). Shift right by 5 to align the
/// LSB to bit 0 before multiplying by the LSB weight.
pub const MAX31856_LSB: f32 = 0.0078125;

/// Maximum consecutive sensor read fallbacks before reporting error.
/// At 100ms control loop cadence, 5 fallbacks = 500ms of stale data.
const MAX_CONSECUTIVE_SENSOR_FALLBACKS: u8 = 5;

/// Exponential moving average alpha for temperature filtering.
/// 0.2 gives moderate smoothing — rejects single-bit SPI glitches (~0.25°C)
/// while keeping the filter responsive to real temperature changes.
const EMA_ALPHA: f32 = 0.2;

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
    #[cfg(all(target_arch = "riscv32", not(feature = "simulated-sensors")))]
    bean_sensor: Max31856<BtSpi>,
    #[cfg(all(target_arch = "riscv32", not(feature = "simulated-sensors")))]
    env_sensor: Max31856<EtSpi>,
    #[cfg(feature = "simulated-sensors")]
    simulated_source: SimulatedSensorSource,
    last_sample: Option<SensorSample>,
    bean_consecutive_fallbacks: u8,
    env_consecutive_fallbacks: u8,
    bean_filtered: f32,
    bean_filter_initialized: bool,
    env_filtered: f32,
    env_filter_initialized: bool,
}

impl SensorConversionHub {
    #[cfg(all(target_arch = "riscv32", not(feature = "simulated-sensors")))]
    pub fn new(bean_sensor: Max31856<BtSpi>, env_sensor: Max31856<EtSpi>) -> Self {
        Self {
            bean_sensor,
            env_sensor,
            last_sample: None,
            bean_consecutive_fallbacks: 0,
            env_consecutive_fallbacks: 0,
            bean_filtered: 0.0,
            bean_filter_initialized: false,
            env_filtered: 0.0,
            env_filter_initialized: false,
        }
    }

    #[cfg(feature = "simulated-sensors")]
    pub fn new_simulated(source: SimulatedSensorSource) -> Self {
        Self {
            simulated_source: source,
            last_sample: None,
            bean_consecutive_fallbacks: 0,
            env_consecutive_fallbacks: 0,
            bean_filtered: 0.0,
            bean_filter_initialized: false,
            env_filtered: 0.0,
            env_filter_initialized: false,
        }
    }

    #[cfg(not(target_arch = "riscv32"))]
    pub fn new() -> Self {
        Self {
            last_sample: None,
            bean_consecutive_fallbacks: 0,
            env_consecutive_fallbacks: 0,
            bean_filtered: 0.0,
            bean_filter_initialized: false,
            env_filtered: 0.0,
            env_filter_initialized: false,
        }
    }

    #[cfg(all(target_arch = "riscv32", not(feature = "simulated-sensors")))]
    #[allow(dead_code)]
    fn new_uninit() -> Self {
        panic!("from_fixture requires simulated-sensors feature or host target");
    }

    #[cfg(feature = "simulated-sensors")]
    #[allow(dead_code)]
    fn new_uninit() -> Self {
        Self::new_simulated(SimulatedSensorSource::default_curve())
    }

    #[cfg(not(target_arch = "riscv32"))]
    #[allow(dead_code)]
    fn new_uninit() -> Self {
        Self::new()
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
        let mut hub = Self::new_uninit();
        hub.sample_from_fixture(fixture)?;
        Ok(hub)
    }

    pub async fn sample(&mut self) -> Result<SensorSample, RoasterError> {
        #[cfg(all(target_arch = "riscv32", not(feature = "simulated-sensors")))]
        {
            let timestamp = Instant::now();
            let (bean, env) = self.sample_parallel().await;
            self.build_sample(timestamp, bean, env)
        }
        #[cfg(feature = "simulated-sensors")]
        {
            let timestamp = Instant::now();
            let (bean_temp, env_temp) = self.simulated_source.current_temperatures();
            let bean_result: SensorChannelResult = Ok((bean_temp, SensorFault::default()));
            let env_result: SensorChannelResult = Ok((env_temp, SensorFault::default()));
            self.build_sample(timestamp, bean_result, env_result)
        }
        #[cfg(all(not(target_arch = "riscv32"), not(feature = "simulated-sensors")))]
        {
            let timestamp = Instant::now();
            let sample = SensorSample::with_timestamp(timestamp);
            self.last_sample = Some(sample);
            Ok(sample)
        }
    }

    #[cfg(all(target_arch = "riscv32", not(feature = "simulated-sensors")))]
    #[allow(dead_code)]
    async fn read_bean_async(&mut self) -> SensorChannelResult {
        Self::read_sensor_async(&mut self.bean_sensor).await
    }

    #[cfg(all(target_arch = "riscv32", not(feature = "simulated-sensors")))]
    #[allow(dead_code)]
    async fn read_env_async(&mut self) -> SensorChannelResult {
        Self::read_sensor_async(&mut self.env_sensor).await
    }

    #[cfg(all(target_arch = "riscv32", not(feature = "simulated-sensors")))]
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

    #[cfg(all(target_arch = "riscv32", not(feature = "simulated-sensors")))]
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
        let (mut bean_temp, bean_fault) =
            Self::resolve_channel(SensorChannel::Bean, bean_result, previous, &mut bean_fb)?;
        self.bean_consecutive_fallbacks = bean_fb;

        if bean_fb == 0 && !bean_fault.has_fault() {
            if self.bean_filter_initialized {
                bean_temp = EMA_ALPHA * bean_temp + (1.0 - EMA_ALPHA) * self.bean_filtered;
            } else {
                self.bean_filter_initialized = true;
            }
            self.bean_filtered = bean_temp;
        }

        sample.bean_temp = bean_temp;
        sample.bean_fault = bean_fault;

        let mut env_fb = self.env_consecutive_fallbacks;
        let (mut env_temp, env_fault) =
            Self::resolve_channel(SensorChannel::Env, env_result, previous, &mut env_fb)?;
        self.env_consecutive_fallbacks = env_fb;

        if env_fb == 0 && !env_fault.has_fault() {
            if self.env_filter_initialized {
                env_temp = EMA_ALPHA * env_temp + (1.0 - EMA_ALPHA) * self.env_filtered;
            } else {
                self.env_filter_initialized = true;
            }
            self.env_filtered = env_temp;
        }

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

#[cfg(all(not(target_arch = "riscv32"), not(feature = "simulated-sensors")))]
impl Default for SensorConversionHub {
    fn default() -> Self {
        Self::new()
    }
}
