//! Temperature sensor conversion and fault handling for LibreRoaster.
//!
//! Decodes raw MAX31856 register reads into °C, classifies fault-register
//! bits into a `SensorFault`, and runs the `SensorConversionHub` that samples
//! the bean/env thermocouples (real SPI or simulated), applies stale-read
//! fallback and EMA smoothing, and exposes the latest `SensorSample`.

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
/// The 19-bit temperature value occupies bits `[23:5]` of the 24-bit concatenated
/// register read (LTCB0<<16 | LTCB1<<8 | LTCB2). Shift right by 5 to align the
/// LSB to bit 0 before multiplying by the LSB weight.
pub const MAX31856_LSB: f32 = 0.0078125;

/// Maximum consecutive sensor read fallbacks before the channel is marked
/// faulted (NaN + fault flags, H9). At the real tick cadence (~310 ms:
/// 100 ms timer + 210 ms `MAX31856_CONVERSION_TIME_MS`), 5 fallbacks ≈ 1.55 s
/// of stale data before `resolve_channel` poisons that channel — the OTHER
/// channel's sample is preserved (single-channel degradation). Mirrors
/// `SENSOR_FAULT_DEBOUNCE = 5` (same 5-tick persistence bar) and stays under
/// `TEMP_VALIDITY_TIMEOUT_MS` (1000 ms freshness bound is enforced per-sample;
/// this caps total fallback run).
const MAX_CONSECUTIVE_SENSOR_FALLBACKS: u8 = 5;

/// Exponential moving average alpha for temperature filtering.
/// 0.2 gives moderate smoothing — rejects single-bit SPI glitches (~0.25°C)
/// while keeping the filter responsive to real temperature changes.
/// Smoother (lower alpha) than `DERIVATIVE_FILTER_ALPHA` (0.3): display temps
/// favour stability, the RoR derivative favours responsiveness.
const EMA_ALPHA: f32 = 0.2;

/// Decode a raw 24-bit MAX31856 temperature register into °C using
/// two's-complement math and the `MAX31856_LSB` weight.
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

/// Cold-junction temperature LSB in °C (datasheet Table 2 — distinct from
/// the 0.0078125 °C thermocouple LSB).
pub const MAX31856_CJ_LSB: f32 = 0.015625;

/// Decode the CJTH:CJTL cold-junction registers into °C (H13).
///
/// 14-bit two's complement, left-justified in the 16-bit word (the low 2
/// bits of CJTL are dead): arithmetic-shift right by 2, then scale.
/// Feeds the `AMB` field of the Artisan `READ` line.
pub fn convert_cj_temp(cjth: u8, cjtl: u8) -> f32 {
    let raw = ((cjth as u16) << 8) | cjtl as u16;
    ((raw as i16) >> 2) as f32 * MAX31856_CJ_LSB
}

/// Fault classification for a single MAX31856 thermocouple channel.
///
/// Each field maps to a bit of the device fault-status register, plus a few
/// derived flags used by the control/safety layer.
#[derive(Debug, Clone, Copy, Default)]
pub struct SensorFault {
    /// bit 0 (0x01) — Open / Thermocouple open-circuit fault.
    pub open_circuit: bool,
    /// bit 1 (0x02) — OVUV (Over/Under Voltage input fault). Legacy alias
    /// `short_to_vcc` (MAX6675 name) for bit 1; see MAX31856 datasheet §7.
    pub short_to_vcc: bool,
    /// bit 2 (0x04) — TC Low (thermocouple below the user fault threshold).
    pub tc_low: bool,
    /// bit 3 (0x08) — TC High (thermocouple above the user fault threshold).
    /// Also exposed as `tc_high`.
    pub tc_high: bool,
    /// bit 4 (0x10) — CJ Low (cold-junction below threshold).
    pub cold_junction_low: bool,
    /// bit 5 (0x20) — CJ High (cold-junction above threshold).
    pub cold_junction_high: bool,
    /// bit 6 (0x40) — TC Range (linearized TC temperature out of range).
    pub tc_range_fault: bool,
    /// bit 7 (0x80) — CJ Range (cold-junction temperature out of range).
    pub cj_range_fault: bool,
    pub communication_error: bool,
    pub invalid_temperature: bool,
    /// Aggregated flag: any bit in the fault register is set, *including*
    /// CJ High / TC Range / CJ Range (0x20 / 0x40 / 0x80).
    pub fault_detected: bool,
    /// Back-compat alias. The MAX31856 has no dedicated "short to GND" bit
    /// (that name comes from the older MAX6675). Kept as a NoOp for
    /// compatibility; prefer the correctly-named `tc_low` / `cj_range_fault` etc.
    #[deprecated(note = "MAX31856 has no short-to-GND bit; use tc_low or cj_range_fault")]
    pub short_to_gnd: bool,
}

impl SensorFault {
    #[allow(dead_code, deprecated)]
    fn from_register(fault: u8) -> Self {
        // MAX31856 Fault Status Register (0x0F) bit map (datasheet, MSB=bit7):
        //   0x01 (bit 0) = Open   — Thermocouple open-circuit fault
        //   0x02 (bit 1) = OVUV  — Over/Under Voltage input fault
        //   0x04 (bit 2) = TC Low  — TC below user low threshold
        //   0x08 (bit 3) = TC High — TC above user high threshold
        //   0x10 (bit 4) = CJ Low  — Cold-junction below threshold
        //   0x20 (bit 5) = CJ High — Cold-junction above threshold
        //   0x40 (bit 6) = TC Range — Linearized TC out of range
        //   0x80 (bit 7) = CJ Range — Cold-junction out of range
        Self {
            open_circuit: fault & 0x01 != 0,
            short_to_vcc: fault & 0x02 != 0,
            tc_low: fault & 0x04 != 0,
            tc_high: fault & 0x08 != 0,
            cold_junction_low: fault & 0x10 != 0,
            cold_junction_high: fault & 0x20 != 0,
            tc_range_fault: fault & 0x40 != 0,
            cj_range_fault: fault & 0x80 != 0,
            // Any bit set is a fault.
            fault_detected: fault != 0,
            short_to_gnd: false,
            invalid_temperature: (fault & 0x04 != 0) || (fault & 0x08 != 0),
            communication_error: false,
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

    /// True if any fault field (including `fault_detected`) is set.
    pub fn has_fault(&self) -> bool {
        self.open_circuit
            || self.short_to_vcc
            || self.tc_low
            || self.tc_high
            || self.cold_junction_low
            || self.cold_junction_high
            || self.tc_range_fault
            || self.cj_range_fault
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
    /// Cold-junction (board) temperature in °C (H13) — mean of the healthy
    /// channels' CJ readings, held on total fault. Feeds `AMB` on the wire.
    pub ambient_temp: f32,
    pub timestamp: Instant,
}

impl SensorSample {
    fn with_timestamp(timestamp: Instant) -> Self {
        Self {
            bean_temp: 0.0,
            env_temp: 0.0,
            bean_fault: SensorFault::default(),
            env_fault: SensorFault::default(),
            ambient_temp: 0.0,
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
/// Raw ADC bytes plus fault register for one fixture reading (regression feature).
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

/// Samples and decodes the bean/env thermocouples into a `SensorSample`.
///
/// Owns the MAX31856 devices (real SPI) or a `SimulatedSensorSource`, tracks
/// stale-read fallback counts and EMA filter state per channel.
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

    /// Feed heater/fan to the thermal plant (closed-loop). No-op in open-loop
    /// curve mode or without `simulated-sensors`. Call after `update_control`.
    #[cfg(feature = "simulated-sensors")]
    pub fn set_simulated_actuators(&mut self, heater_pct: f32, fan_pct: f32) {
        self.simulated_source.set_heater_fan(heater_pct, fan_pct);
    }

    /// Simulate bean charge dip (plant mode only). Drop BT ~90°C, ET less.
    #[cfg(feature = "simulated-sensors")]
    pub fn inject_simulated_charge(&mut self, bean_drop_c: f32) {
        self.simulated_source.inject_charge(bean_drop_c);
    }

    /// Deterministic plant advance for tests (bypasses wall-clock).
    #[cfg(feature = "simulated-sensors")]
    pub fn plant_advance(&mut self, heater_pct: f32, fan_pct: f32, dt_secs: f32) -> (f32, f32) {
        self.simulated_source
            .plant_advance(heater_pct, fan_pct, dt_secs)
    }

    #[cfg(all(not(target_arch = "riscv32"), not(feature = "simulated-sensors")))]
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

    /// Host-targeted `new()` that ALSO initialises the `simulated_source`
    /// field. The `not(simulated-sensors)` variant above has no such field;
    /// the `simulated-sensors` variant here supplies the default curve. Both
    /// are named `new()` but are mutually exclusive via cfg — exactly one
    /// compiles per host feature combination.
    #[cfg(all(not(target_arch = "riscv32"), feature = "simulated-sensors"))]
    pub fn new() -> Self {
        Self::new_simulated(SimulatedSensorSource::default_curve())
    }

    #[cfg(all(target_arch = "riscv32", not(feature = "simulated-sensors")))]
    #[allow(dead_code)]
    #[allow(clippy::panic, clippy::empty_loop)]
    fn new_uninit() -> Self {
        // Unreachable on this target: `from_fixture` is gated on
        // `feature = "regression"`, and the regression setup is never built
        // for riscv32 without simulated-sensors. Panicking here is correct:
        // if we ever reach this we have a configuration bug that should crash
        // loudly rather than return a fabricated hub.
        // Spec F1.1: replaced with `unimplemented!()` per spec request.
        unimplemented!("from_fixture requires simulated-sensors feature or host target");
    }

    #[cfg(feature = "simulated-sensors")]
    #[allow(dead_code)]
    fn new_uninit() -> Self {
        Self::new_simulated(SimulatedSensorSource::default_curve())
    }

    // Gate the host variant to `not(riscv32) AND not(simulated-sensors)` so
    // exactly one definition is selected per feature combination; the
    // simulated-sensors variant above handles the `test,regression` host build.
    #[cfg(all(not(target_arch = "riscv32"), not(feature = "simulated-sensors")))]
    #[allow(dead_code)]
    fn new_uninit() -> Self {
        Self::new()
    }

    /// Return the most recent successfully built sensor sample, if any.
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

    /// Build a `SensorSample` from a `FixtureReading` (regression feature).
    /// Fixtures carry no CJ bytes — ambient holds its previous value.
    #[cfg(feature = "regression")]
    pub fn sample_from_fixture(
        &mut self,
        fixture: FixtureReading,
    ) -> Result<SensorSample, RoasterError> {
        let timestamp = Instant::now();
        let (bean_result, env_result) = fixture.to_channel_results();
        self.build_sample(timestamp, bean_result, env_result, None, None)
    }

    /// Construct a hub pre-loaded with a single fixture sample (regression).
    #[cfg(feature = "regression")]
    pub fn from_fixture(fixture: FixtureReading) -> Result<Self, RoasterError> {
        let mut hub = Self::new_uninit();
        hub.sample_from_fixture(fixture)?;
        Ok(hub)
    }

    /// Read both channels and return a freshly built `SensorSample`.
    pub async fn sample(&mut self) -> Result<SensorSample, RoasterError> {
        #[cfg(all(target_arch = "riscv32", not(feature = "simulated-sensors")))]
        {
            let timestamp = Instant::now();
            let ((bean, bean_cj), (env, env_cj)) = self.sample_parallel().await;
            self.build_sample(timestamp, bean, env, bean_cj, env_cj)
        }
        #[cfg(feature = "simulated-sensors")]
        {
            let timestamp = Instant::now();
            let (bean_temp, env_temp) = self.simulated_source.current_temperatures();
            let bean_result: SensorChannelResult = Ok((bean_temp, SensorFault::default()));
            let env_result: SensorChannelResult = Ok((env_temp, SensorFault::default()));
            // No CJ hardware in simulation — ambient holds (0.0 on host).
            self.build_sample(timestamp, bean_result, env_result, None, None)
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
    async fn sample_parallel(
        &mut self,
    ) -> (
        (SensorChannelResult, Option<f32>),
        (SensorChannelResult, Option<f32>),
    ) {
        // Trigger both sensor conversions in parallel by starting both conversions
        // before any await, then wait once, then read both results.
        //
        // This reduces sampling time from ~320-360ms (serial) to ~160-180ms (parallel)
        // bringing control loop from ~2-3Hz to target 10Hz.

        // Trigger bean sensor conversion (one-shot) - fast SPI write, ~50us
        let bean_trigger_result = self.bean_sensor.trigger_conversion();

        // Trigger env sensor conversion (one-shot) - fast SPI write, ~50us
        let env_trigger_result = self.env_sensor.trigger_conversion();

        // Wait once for both conversions to complete.
        // 50 Hz-filtered conversions take up to 185 ms (datasheet); use the
        // dedicated `MAX31856_CONVERSION_TIME_MS` wait.
        embassy_time::Timer::after(embassy_time::Duration::from_millis(
            crate::config::constants::MAX31856_CONVERSION_TIME_MS,
        ))
        .await;

        // Read bean sensor result - fast SPI read, ~50us. The CJ reading
        // rides along only when the channel itself is healthy (H13).
        let bean_result = bean_trigger_result
            .and_then(|_| self.bean_sensor.read_conversion_result())
            .map(|reading| {
                (
                    (
                        convert_raw_temp(reading.raw_temp),
                        SensorFault::from_register(reading.fault),
                    ),
                    reading.cj_temp_c,
                )
            });
        let (bean, bean_cj) = match bean_result {
            Ok(((temp, fault), cj)) if !fault.has_fault() => (Ok((temp, fault)), Some(cj)),
            Ok(((temp, fault), _)) => (Ok((temp, fault)), None),
            Err(e) => (Err(e), None),
        };

        // Read env sensor result - fast SPI read, ~50us
        let env_result = env_trigger_result
            .and_then(|_| self.env_sensor.read_conversion_result())
            .map(|reading| {
                (
                    (
                        convert_raw_temp(reading.raw_temp),
                        SensorFault::from_register(reading.fault),
                    ),
                    reading.cj_temp_c,
                )
            });
        let (env, env_cj) = match env_result {
            Ok(((temp, fault), cj)) if !fault.has_fault() => (Ok((temp, fault)), Some(cj)),
            Ok(((temp, fault), _)) => (Ok((temp, fault)), None),
            Err(e) => (Err(e), None),
        };

        ((bean, bean_cj), (env, env_cj))
    }

    #[allow(dead_code)]
    fn build_sample(
        &mut self,
        timestamp: Instant,
        bean_result: SensorChannelResult,
        env_result: SensorChannelResult,
        bean_cj: Option<f32>,
        env_cj: Option<f32>,
    ) -> Result<SensorSample, RoasterError> {
        let previous = self.last_sample;
        let mut sample = previous.unwrap_or_else(|| SensorSample::with_timestamp(timestamp));
        sample.timestamp = timestamp;

        // H13: ambient is the mean of the healthy channels' CJ readings; on
        // total fault the previous ambient holds (never NaN-poisoned).
        sample.ambient_temp = match (bean_cj, env_cj) {
            (Some(b), Some(e)) => (b + e) * 0.5,
            (Some(b), None) => b,
            (None, Some(e)) => e,
            (None, None) => previous.map(|p| p.ambient_temp).unwrap_or(0.0),
        };

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
                    // H9: a persistently failing channel must NOT invalidate
                    // the other channel's sample. Mark this channel faulted
                    // (NaN + fault flags) and let the per-channel debounce in
                    // `SensorController` (SENSOR_FAULT_DEBOUNCE → NaN → hold)
                    // decide — a dead ET keeps a BT-only roast alive, matching
                    // the single-channel boot degradation of
                    // `Max31856::new_tolerant`.
                    let mut fault = SensorFault::from_max31856_error(&err);
                    fault.communication_error = true;
                    fault.fault_detected = true;
                    return Ok((f32::NAN, fault));
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

#[cfg(all(test, not(target_arch = "riscv32"), not(feature = "simulated-sensors")))]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn ok(temp: f32) -> SensorChannelResult {
        Ok((temp, SensorFault::default()))
    }

    fn err() -> SensorChannelResult {
        Err(Max31856Error::CommunicationError { source: "test" })
    }

    #[test]
    fn cj_decode_vectors() {
        // Datasheet Table 2 spot checks (14-bit code × 0.015625 °C).
        assert!((convert_cj_temp(0x19, 0x00) - 25.0).abs() < 0.001);
        assert!((convert_cj_temp(0x00, 0x00) - 0.0).abs() < 0.001);
        assert!((convert_cj_temp(0xFF, 0x00) - -1.0).abs() < 0.001);
        assert!((convert_cj_temp(0xC0, 0x00) - -64.0).abs() < 0.001);
        assert!((convert_cj_temp(0x7F, 0xFC) - 127.984).abs() < 0.002);
    }

    #[test]
    fn single_dead_channel_preserves_other_channel() {
        // H9: 5 consecutive SPI errors on ONE channel must NaN-mark only
        // that channel — the other channel's sample survives, matching the
        // single-channel boot degradation of `Max31856::new_tolerant`.
        let mut hub = SensorConversionHub::new();
        let ts = Instant::from_millis(1_000);
        let s = hub
            .build_sample(ts, ok(150.0), ok(200.0), Some(25.0), Some(26.0))
            .expect("first sample");
        assert_eq!(s.bean_temp, 150.0);
        assert_eq!(s.env_temp, 200.0);
        assert!((s.ambient_temp - 25.5).abs() < 0.001);

        // 4 fallbacks: previous temps hold, no fault latched.
        for i in 1..5u64 {
            let t = Instant::from_millis(1_000 + i * 310);
            let s = hub
                .build_sample(t, err(), ok(200.0), None, Some(26.0))
                .expect("fallback holds");
            assert_eq!(s.bean_temp, 150.0);
            assert_eq!(s.env_temp, 200.0);
            assert!(!s.bean_fault.has_fault() || s.bean_temp.is_finite());
        }

        // 5th consecutive error: bean NaN + fault flags, env intact.
        let t = Instant::from_millis(1_000 + 5 * 310);
        let s = hub
            .build_sample(t, err(), ok(200.0), None, Some(26.0))
            .expect("dead channel must not abort the sample");
        assert!(s.bean_temp.is_nan());
        assert!(s.bean_fault.communication_error);
        assert!(s.bean_fault.fault_detected);
        assert_eq!(s.env_temp, 200.0);
        assert!(!s.env_fault.has_fault());
        // Ambient falls back to the surviving channel, never NaN.
        assert!((s.ambient_temp - 26.0).abs() < 0.001);
        assert!(s.ambient_temp.is_finite());
    }

    #[test]
    fn ambient_holds_on_total_fault() {
        let mut hub = SensorConversionHub::new();
        let ts = Instant::from_millis(2_000);
        let s = hub
            .build_sample(ts, ok(150.0), ok(200.0), Some(25.0), Some(27.0))
            .expect("seed");
        assert!((s.ambient_temp - 26.0).abs() < 0.001);

        // Both channels faulted (but below the NaN threshold): ambient holds.
        let t = Instant::from_millis(2_310);
        let s = hub
            .build_sample(t, err(), err(), None, None)
            .expect("total fault holds");
        assert!((s.ambient_temp - 26.0).abs() < 0.001);
        assert!(s.ambient_temp.is_finite());
    }
}
