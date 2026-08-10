use crate::config::*;
use crate::control::RoasterError;
use crate::hardware::sensors::{SensorConversionHub, SensorSample};
use embassy_time::Instant;
use log::warn;

const DERIVATIVE_FILTER_ALPHA: f32 = 0.3;

/// F4.11 (Gap #3): Number of consecutive faulty sensor reads required before
/// latching `status.fault_condition = true`. A single transient SPI glitch
/// (the most common false trigger) must NOT permanently latch a fault, which
/// would force manual recovery. The fault is only latched once it persists
/// across N consecutive samples — confirming a real wiring/open-thermocouple
/// condition rather than a momentary bus error.
///
/// Note: this debouncer gates only the *sensor-read* path of `fault_condition`.
/// Other paths (overtemp in `update_temperatures`, manual emergency, RWDT) set
/// `fault_condition = true` independently and are not affected.
pub const SENSOR_FAULT_DEBOUNCE: u8 = 5;

pub struct SensorController {
    sensor_hub: SensorConversionHub,
    last_temp_read: Option<Instant>,
    last_pv_sample: Option<f32>,
    last_pv_sample_time: Option<Instant>,
    last_filtered_derivative: f32,
    /// Bug M4 (2026-07-25): dedicated sample pair for the BT-only RoR guard.
    /// The previous design keyed the runaway guard on `status.derivative_rate`
    /// which is the PV (BT or ET) derivative chosen by `update_pid_control`.
    /// With `PID;CHAN;1` (ET as PV — supported and tested downstream), the
    /// 0.5 °C/s threshold (calibrated for the sluggish BT) is applied to ET
    /// (which climbs much faster), causing a latched emergency on a healthy
    /// roast while a real BT runaway goes unguarded. Keep the guard on BT
    /// always and the PID feed on whatever PV is configured; they are now
    /// independent measurements.
    last_bt_guard_sample: Option<(f32, Instant)>,
    bt_guard_derivative: f32,
    /// Bug R1 (2026-07-26): debounce counter for the legacy PV-RoR check
    /// (`check_rate_of_rise`). Previously a SINGLE `ror_exceeded_count` was
    /// shared with the BT-only runaway guard (`check_bt_rate`, bug M4): with
    /// `PID;CHAN;1` (ET as PV) a healthy ET tick reset the shared counter
    /// every tick, so a genuine BT runaway never accumulated to
    /// `ROR_EXCEEDED_CONSECUTIVE_LIMIT` — the BT guard was silently neutered
    /// in the exact configuration it was built for. Each guard now owns its
    /// own counter.
    pv_ror_exceeded_count: u8,
    /// Bug R1: dedicated counter for the BT-only runaway guard
    /// (`check_bt_rate`). Independent of `pv_ror_exceeded_count`.
    bt_ror_exceeded_count: u8,
    // Bug V2-3 / B7 residual: per-channel fault counters. A single shared
    // counter fed with `bean_fault || env_fault` was defeated by a chronically
    // disconnected ET (a single-probe configuration the code itself supports):
    // the shared counter sat permanently >= threshold, so the FIRST transient
    // BT glitch immediately met `bean_fault && count >= DEBOUNCE` and poisoned
    // bean_temp with NaN in the same tick — exactly the spurious emergency B7
    // was supposed to eliminate. Each channel now debounces against its OWN
    // counter, so a faulted-but-unused channel cannot arm the other channel's
    // NaN decision.
    consecutive_bean_faults: u8,
    consecutive_env_faults: u8,
}

impl SensorController {
    pub fn new(sensor_hub: SensorConversionHub) -> Self {
        Self {
            sensor_hub,
            last_temp_read: None,
            last_pv_sample: None,
            last_pv_sample_time: None,
            last_filtered_derivative: 0.0,
            last_bt_guard_sample: None,
            bt_guard_derivative: 0.0,
            pv_ror_exceeded_count: 0,
            bt_ror_exceeded_count: 0,
            consecutive_bean_faults: 0,
            consecutive_env_faults: 0,
        }
    }

    pub async fn read_sensors(&mut self, status: &mut SystemStatus) -> Result<(), RoasterError> {
        let sample = self.sensor_hub.sample().await?;
        // F4.11 (Gap #3) + V2-3: debounce each channel against its own counter.
        self.apply_fault_debounce(
            sample.bean_fault.has_fault(),
            sample.env_fault.has_fault(),
            status,
        );
        self.update_temperatures(
            sample.bean_temp,
            sample.env_temp,
            sample.bean_fault,
            sample.env_fault,
            sample.timestamp,
            status,
        )
    }

    /// F4.11 (Gap #3) + V2-3: per-channel debounce. Each channel increments
    /// its own counter only on its own fault and resets only on its own clean
    /// read, so a chronically faulted-but-unused channel cannot push the other
    /// channel's counter to the NaN threshold.
    ///
    /// Bug P2 (2026-08-03): `fault_condition` now latches ONLY when the
    /// chronically faulted channel is the ACTIVE PID input (`pid_channel`):
    /// 1 = ET, anything else = BT (see the PV selector in
    /// `RoasterControl::update_control`). The single-probe configuration the
    /// code explicitly supports (V2-3/B7: BT-only with ET unplugged) used to
    /// latch the GLOBAL fault after 5 ET-fault ticks, which rejected every
    /// subsequent START/OT1/PREHEAT with `fault_condition_active` — the
    /// device became inoperable while the fault was in a channel the control
    /// loop never reads. An unused channel's persistent fault still advances
    /// its own debounce counter (so switching `pid_channel` to it re-arms the
    /// latch) and still poisons its own temperature with NaN, but it no
    /// longer blocks the whole device. Other paths that set
    /// `fault_condition` (overtemp in `update_temperatures`, manual
    /// emergency, RWDT) are not affected.
    pub fn apply_fault_debounce(
        &mut self,
        bean_fault: bool,
        env_fault: bool,
        status: &mut SystemStatus,
    ) {
        // The PID input selector in `update_control` treats `pid_channel == 1`
        // as ET and EVERY other value as BT — mirror it here so the latch
        // decision tracks exactly which channel the control loop consumes.
        let bean_is_pv = status.pid_channel != 1;
        let env_is_pv = status.pid_channel == 1;
        if bean_fault {
            self.consecutive_bean_faults = self.consecutive_bean_faults.saturating_add(1);
            if self.consecutive_bean_faults >= SENSOR_FAULT_DEBOUNCE && bean_is_pv {
                status.fault_condition = true;
            }
        } else {
            self.consecutive_bean_faults = 0;
        }
        if env_fault {
            self.consecutive_env_faults = self.consecutive_env_faults.saturating_add(1);
            if self.consecutive_env_faults >= SENSOR_FAULT_DEBOUNCE && env_is_pv {
                status.fault_condition = true;
            }
        } else {
            self.consecutive_env_faults = 0;
        }
    }

    pub fn update_temperatures(
        &mut self,
        bean_temp: f32,
        env_temp: f32,
        bean_fault: crate::hardware::sensors::conversion::SensorFault,
        env_fault: crate::hardware::sensors::conversion::SensorFault,
        current_time: Instant,
        status: &mut SystemStatus,
    ) -> Result<(), RoasterError> {
        // Only validate temperature for channels without fault.
        // A sensor with open thermocouple (e.g. ET not connected) may return
        // garbage temperatures; we should not let that invalidate the entire read.
        if !bean_fault.has_fault() && !Self::is_temperature_valid(bean_temp) {
            return Err(RoasterError::TemperatureOutOfRange {
                source: Some("temperature_out_of_valid_range"),
            });
        }
        if !env_fault.has_fault() && !Self::is_temperature_valid(env_temp) {
            return Err(RoasterError::TemperatureOutOfRange {
                source: Some("temperature_out_of_valid_range"),
            });
        }

        // Bug V2-2 / B7 residual: "hold last value" was NOT implemented. The
        // previous code wrote `status.bean_temp = bean_temp + OFFSET`
        // UNCONDITIONALLY here, BEFORE the fault gate below. So during the
        // pre-debounce window the value "held" was THIS faulted sample's raw
        // garbage (typically 0 from an open thermocouple), not the last valid
        // one — and the comment claiming "hold the last valid value" was
        // false. The PID/RoR/overtemp guards then operated on that garbage
        // for up to ~0.5-0.8 s until the 5th fault sample finally poisoned
        // the value with NaN.
        //
        // Fix: write `status.*_temp` ONLY when the channel is not faulted;
        // if the channel is faulted AND its own debounce counter has reached
        // the threshold, poison with NaN; otherwise leave the previous value
        // in `status.*_temp` untouched — a REAL hold of the last valid reading.
        if !bean_fault.has_fault() {
            status.bean_temp = bean_temp + BT_THERMOCOUPLE_OFFSET;
        } else if self.consecutive_bean_faults >= SENSOR_FAULT_DEBOUNCE {
            status.bean_temp = f32::NAN;
        }
        // else: status.bean_temp keeps the last valid value (real hold).

        if !env_fault.has_fault() {
            status.env_temp = env_temp + ET_THERMOCOUPLE_OFFSET;
        } else if self.consecutive_env_faults >= SENSOR_FAULT_DEBOUNCE {
            status.env_temp = f32::NAN;
        }
        // else: status.env_temp keeps the last valid value (real hold).

        // Bug B-Q (2026-08-04): only mark the read as fresh when at least one
        // channel delivered a clean sample. When BOTH channels are faulted the
        // sample carried no new information (the V2-2 hold keeps the last
        // valid temperature in `status.*_temp`), so refreshing
        // `last_temp_read` here would make `update_control`'s staleness guard
        // treat the held value as fresh and let the PID keep integrating
        // against stale data — worst case at boot: PV stuck at 0.0 while the
        // heater ramps toward 100 %. Freezing the timestamp sends the PID
        // into the stale-hold branch (`is_stale`), which holds the last
        // APPLIED output instead of ramping. A single-channel fault (e.g. the
        // supported BT-only config with ET unplugged) still refreshes the
        // timestamp because the other channel is usable.
        if !(bean_fault.has_fault() && env_fault.has_fault()) {
            self.last_temp_read = Some(current_time);
        }

        // Only check overtemp against valid sensors (ignore faulted ones).
        // A faulted channel did not overwrite status.*_temp above, so this
        // gate is only re-tested on a freshly-written clean reading.
        if !bean_fault.has_fault() && status.bean_temp >= OVERTEMP_THRESHOLD {
            return Err(RoasterError::TemperatureOutOfRange {
                source: Some("overtemp_detected"),
            });
        }
        if !env_fault.has_fault() && status.env_temp >= OVERTEMP_THRESHOLD {
            return Err(RoasterError::TemperatureOutOfRange {
                source: Some("overtemp_detected"),
            });
        }

        Ok(())
    }

    pub fn refresh_filtered_derivative(
        &mut self,
        current_pv: f32,
        current_time: Instant,
        status: &mut SystemStatus,
    ) {
        let mut derivative_rate = 0.0;
        let mut has_valid_rate = false;

        if let (Some(prev_pv), Some(prev_time)) = (self.last_pv_sample, self.last_pv_sample_time) {
            let duration = current_time.saturating_duration_since(prev_time);
            let dt_secs = duration.as_micros() as f32 * 1e-6;
            if dt_secs > 0.0 {
                let delta_temp = current_pv - prev_pv;
                if delta_temp.is_finite() {
                    let instantaneous_rate = delta_temp / dt_secs;
                    if instantaneous_rate.is_finite() {
                        derivative_rate = DERIVATIVE_FILTER_ALPHA * instantaneous_rate
                            + (1.0 - DERIVATIVE_FILTER_ALPHA) * self.last_filtered_derivative;
                        if derivative_rate.is_finite() {
                            has_valid_rate = true;
                            self.last_filtered_derivative = derivative_rate;
                        }
                    }
                }
            }
        }

        if has_valid_rate {
            status.derivative_rate = derivative_rate;
            status.derivative_available = true;
        } else {
            status.derivative_rate = 0.0;
            status.derivative_available = false;
        }

        self.last_pv_sample = Some(current_pv);
        self.last_pv_sample_time = Some(current_time);
    }

    pub fn last_sensor_sample(&self) -> Option<SensorSample> {
        self.sensor_hub.last_sample()
    }

    pub fn last_temp_read(&self) -> Option<Instant> {
        self.last_temp_read
    }

    pub fn is_temperature_valid(temp: f32) -> bool {
        (MIN_VALID_TEMP..=MAX_VALID_TEMP).contains(&temp)
    }

    pub fn check_rate_of_rise(&mut self, status: &SystemStatus) -> Result<(), RoasterError> {
        if !status.derivative_available {
            self.pv_ror_exceeded_count = 0;
            return Ok(());
        }

        if status.derivative_rate > MAX_BT_RATE_OF_RISE {
            self.pv_ror_exceeded_count = self.pv_ror_exceeded_count.saturating_add(1);
            warn!(
                "BT RoR {:.2}°C/s exceeds limit {:.1}°C/s (count {}/{})",
                status.derivative_rate,
                MAX_BT_RATE_OF_RISE,
                self.pv_ror_exceeded_count,
                ROR_EXCEEDED_CONSECUTIVE_LIMIT
            );
            if self.pv_ror_exceeded_count >= ROR_EXCEEDED_CONSECUTIVE_LIMIT {
                // Bug M10 (2026-08-10): reset the debounce counter before
                // firing, exactly like `check_bt_rate` does. Without this the
                // counter stayed pinned at the limit through the latched
                // period and the recovery, so the NEXT roast tripped on a
                // SINGLE tick above the threshold — no 3-sample confirmation.
                self.pv_ror_exceeded_count = 0;
                return Err(RoasterError::TemperatureOutOfRange {
                    source: Some("rate_of_rise_exceeded"),
                });
            }
        } else {
            self.pv_ror_exceeded_count = 0;
        }

        Ok(())
    }

    /// Bug M4 (2026-07-25): refresh the BT-only rate-of-rise dedicated for the
    /// runaway guard. `update_temperatures` already gates BT in
    /// `status.bean_temp`, and `update_control` calls this every tick with
    /// the canonical BT reading (independent of the active PV channel). The
    /// IIR-filtered slope is consumed by the guard below (`check_bt_rate`),
    /// never by the PID feed path that uses `refresh_filtered_derivative`.
    ///
    /// Returns `None` on the first sample (no prior pair), when BT is
    /// non-finite (NaN during the post-debounce poison window), or when the
    /// clock would have to run backwards (saturating zero).
    pub fn refresh_bt_guard_derivative(&mut self, bt: f32, now: Instant) -> Option<f32> {
        if !bt.is_finite() {
            return None;
        }
        let out = self.last_bt_guard_sample.and_then(|(prev_bt, prev_time)| {
            let dt = now.saturating_duration_since(prev_time).as_micros() as f32 * 1e-6;
            if dt > 0.0 {
                self.bt_guard_derivative = DERIVATIVE_FILTER_ALPHA * ((bt - prev_bt) / dt)
                    + (1.0 - DERIVATIVE_FILTER_ALPHA) * self.bt_guard_derivative;
                Some(self.bt_guard_derivative)
            } else {
                None
            }
        });
        self.last_bt_guard_sample = Some((bt, now));
        out
    }

    /// Bug M4 (2026-07-25): the BT-only runaway guard. Threshold semantics
    /// match `check_rate_of_rise` (consecutive-count-style debounce) but
    /// consume the BT-only derivative so a healthy ET-as-PV roast never
    /// trips the BT guard while a genuine BT runaway still does.
    pub fn check_bt_rate(&mut self, bt_rate: f32) -> Result<(), RoasterError> {
        if bt_rate > MAX_BT_RATE_OF_RISE {
            self.bt_ror_exceeded_count = self.bt_ror_exceeded_count.saturating_add(1);
            warn!(
                "BT RoR guard {:.2}°C/s exceeds limit {:.1}°C/s (count {}/{})",
                bt_rate,
                MAX_BT_RATE_OF_RISE,
                self.bt_ror_exceeded_count,
                ROR_EXCEEDED_CONSECUTIVE_LIMIT
            );
            if self.bt_ror_exceeded_count >= ROR_EXCEEDED_CONSECUTIVE_LIMIT {
                self.bt_ror_exceeded_count = 0;
                return Err(RoasterError::TemperatureOutOfRange {
                    source: Some("bt_rate_of_rise_exceeded"),
                });
            }
        } else {
            self.bt_ror_exceeded_count = 0;
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::hardware::sensors::{SensorConversionHub, SensorFault};

    fn make_status() -> SystemStatus {
        SystemStatus::default()
    }

    #[test]
    fn update_temperatures_valid_values() {
        let hub = SensorConversionHub::new();
        let mut ctrl = SensorController::new(hub);
        let mut status = make_status();
        let fault = SensorFault::default();
        let now = embassy_time::Instant::now();

        ctrl.update_temperatures(150.0, 120.0, fault, fault, now, &mut status)
            .unwrap();

        assert_eq!(status.bean_temp, 150.0 + BT_THERMOCOUPLE_OFFSET);
        assert_eq!(status.env_temp, 120.0 + ET_THERMOCOUPLE_OFFSET);
    }

    #[test]
    fn update_temperatures_bt_overtemp() {
        let hub = SensorConversionHub::new();
        let mut ctrl = SensorController::new(hub);
        let mut status = make_status();
        let fault = SensorFault::default();
        let now = embassy_time::Instant::now();

        let result =
            ctrl.update_temperatures(OVERTEMP_THRESHOLD, 120.0, fault, fault, now, &mut status);

        assert!(result.is_err());
    }

    #[test]
    fn update_temperatures_et_overtemp() {
        let hub = SensorConversionHub::new();
        let mut ctrl = SensorController::new(hub);
        let mut status = make_status();
        let fault = SensorFault::default();
        let now = embassy_time::Instant::now();

        let result =
            ctrl.update_temperatures(150.0, OVERTEMP_THRESHOLD, fault, fault, now, &mut status);

        assert!(result.is_err());
    }

    #[test]
    fn update_temperatures_nan_bt_no_fault() {
        let hub = SensorConversionHub::new();
        let mut ctrl = SensorController::new(hub);
        let mut status = make_status();
        let fault = SensorFault::default();
        let now = embassy_time::Instant::now();

        let result = ctrl.update_temperatures(f32::NAN, 120.0, fault, fault, now, &mut status);

        assert!(result.is_err());
    }

    #[test]
    fn update_temperatures_faulted_bt_skips_overtemp() {
        let hub = SensorConversionHub::new();
        let mut ctrl = SensorController::new(hub);
        let mut status = make_status();
        let now = embassy_time::Instant::now();
        let no_fault = SensorFault::default();
        let bean_fault = SensorFault {
            fault_detected: true,
            ..SensorFault::default()
        };

        // V2-2 fix: a faulted BT channel must NOT overwrite status.bean_temp
        // with its raw garbage. Seed a valid prior reading, then drive faulted
        // samples that carry a *distinct* garbage value (999.0 — out of the
        // valid range yet still not NaN) below the debounce threshold. The
        // last valid value (150.0 + offset) must be retained — NOT 999.0 and
        // NOT NaN — proving the "hold last value" comment now tells the truth.
        ctrl.update_temperatures(150.0, 120.0, no_fault, no_fault, now, &mut status)
            .expect("seed clean reading");
        let last_valid_bt = status.bean_temp;

        for _ in 0..(SENSOR_FAULT_DEBOUNCE - 1) {
            let result =
                ctrl.update_temperatures(999.0, 120.0, bean_fault, no_fault, now, &mut status);
            assert!(result.is_ok());
            assert!(
                !status.bean_temp.is_nan(),
                "B7/V2-2: pre-debounce fault must hold the last valid value, not NaN"
            );
            assert_eq!(
                status.bean_temp, last_valid_bt,
                "V2-2: pre-debounce fault must NOT overwrite bean_temp with the \
                 faulted sample's raw value"
            );
            // The overtemp guard must still be skipped because the channel is
            // signal-faulted (no error even though 999.0 > OVERTEMP_THRESHOLD).
            // Drive the debouncer as the production read path does so the bean
            // counter advances toward the threshold.
            ctrl.apply_fault_debounce(true, false, &mut status);
        }

        // Drive the debouncer one more time so consecutive_bean_faults reaches
        // the threshold, then the next faulted read poisons bean_temp.
        ctrl.apply_fault_debounce(true, false, &mut status);
        let result = ctrl.update_temperatures(999.0, 120.0, bean_fault, no_fault, now, &mut status);
        assert!(result.is_ok());
        assert!(status.bean_temp.is_nan());
    }

    #[test]
    fn update_temperatures_faulted_et_is_nan() {
        let hub = SensorConversionHub::new();
        let mut ctrl = SensorController::new(hub);
        let mut status = make_status();
        let now = embassy_time::Instant::now();
        let no_fault = SensorFault::default();
        let env_fault = SensorFault {
            fault_detected: true,
            ..SensorFault::default()
        };

        // V2-2 fix: seed a valid env_temp, then drive faulted ET samples with
        // a distinct garbage value (888.0) below the debounce threshold. The
        // last valid value must be retained — NOT 888.0 and NOT NaN.
        ctrl.update_temperatures(150.0, 120.0, no_fault, no_fault, now, &mut status)
            .expect("seed clean reading");
        let last_valid_et = status.env_temp;

        // Bug B7: a SINGLE faulty ET read must not poison env_temp. The PID
        // downstream treats NaN as an emergency; debouncing before poisoning
        // prevents a transient SPI glitch from latching one.
        ctrl.update_temperatures(150.0, 888.0, no_fault, env_fault, now, &mut status)
            .unwrap();
        assert!(!status.bean_temp.is_nan());
        assert!(
            !status.env_temp.is_nan(),
            "B7: first faulty ET read must hold the last valid value, not NaN"
        );
        assert_eq!(
            status.env_temp, last_valid_et,
            "V2-2: pre-debounce ET fault must NOT overwrite env_temp with the \
             faulted sample's raw value"
        );

        // Drive the debouncer to the threshold, then the next faulted read
        // poisons env_temp (matching the F4.11 latch).
        for _ in 0..SENSOR_FAULT_DEBOUNCE {
            ctrl.apply_fault_debounce(false, true, &mut status);
        }
        ctrl.update_temperatures(150.0, 888.0, no_fault, env_fault, now, &mut status)
            .unwrap();
        assert!(!status.bean_temp.is_nan());
        assert!(status.env_temp.is_nan());
    }

    // V2-3: per-channel counters — a chronically faulted ET must NOT push the
    // BT counter to the NaN threshold, so the first transient BT glitch does
    // NOT poison BT in the same tick.
    #[test]
    fn single_chronic_env_fault_does_not_poison_bean_on_bt_glitch() {
        let hub = SensorConversionHub::new();
        let mut ctrl = SensorController::new(hub);
        let mut status = make_status();
        let now = embassy_time::Instant::now();
        let no_fault = SensorFault::default();
        let bean_fault = SensorFault {
            fault_detected: true,
            ..SensorFault::default()
        };
        let env_fault = SensorFault {
            fault_detected: true,
            ..SensorFault::default()
        };

        // Seed a valid prior reading on both channels.
        ctrl.update_temperatures(150.0, 120.0, no_fault, no_fault, now, &mut status)
            .expect("seed");
        let last_valid_bt = status.bean_temp;

        // ET chronically faulted well past the threshold (10 ticks). BT stays
        // clean throughout, so the env counter saturates but the bean counter
        // stays 0 because BT is not faulted.
        for _ in 0..10 {
            ctrl.apply_fault_debounce(false, true, &mut status);
        }
        // Bug P2 (2026-08-03): with the default `pid_channel = 2` (BT is the
        // PV), a chronic ET fault must NOT latch the GLOBAL fault_condition —
        // the old behaviour bricked single-probe configs by rejecting every
        // START/OT1/PREHEAT. The env counter is at threshold (so switching
        // pid_channel to 1 re-arms the latch) but the device stays operable.
        assert!(
            !status.fault_condition,
            "P2: a chronically faulted non-PV channel must not latch fault_condition"
        );
        assert_eq!(ctrl.consecutive_bean_faults, 0);
        assert!(ctrl.consecutive_env_faults >= SENSOR_FAULT_DEBOUNCE);

        // First transient BT glitch: with a SHARED counter the bean NaN
        // decision would have fired immediately. With per-channel counters,
        // the BT counter has just incremented to 1 (< DEBOUNCE), so bean_temp
        // retains its last valid value — no NaN, no emergency this tick.
        ctrl.apply_fault_debounce(true, true, &mut status);
        ctrl.update_temperatures(999.0, 888.0, bean_fault, env_fault, now, &mut status)
            .expect("faulted read");
        assert_eq!(
            status.bean_temp, last_valid_bt,
            "V2-3: BT must hold its last valid value on the first BT glitch even \
             while ET is chronically faulted (per-channel debounce)"
        );
        assert!(
            status.env_temp.is_nan(),
            "ET already past debounce poisons now"
        );
    }

    #[test]
    fn check_rate_of_rise_below_threshold() {
        let hub = SensorConversionHub::new();
        let mut ctrl = SensorController::new(hub);
        let mut status = make_status();
        status.pid_enabled = true;
        status.derivative_rate = 2.0;
        status.derivative_available = true;

        let result = ctrl.check_rate_of_rise(&status);

        assert!(result.is_ok());
    }

    #[test]
    fn check_rate_of_rise_above_threshold_resets_on_ok() {
        let hub = SensorConversionHub::new();
        let mut ctrl = SensorController::new(hub);
        let mut status = make_status();
        status.pid_enabled = true;
        status.derivative_rate = 10.0;
        status.derivative_available = true;

        let r1 = ctrl.check_rate_of_rise(&status);
        assert!(r1.is_ok());
        assert_eq!(ctrl.pv_ror_exceeded_count, 1);

        // Reset to a rate below MAX_BT_RATE_OF_RISE (0.5°C/s)
        status.derivative_rate = 0.3;
        let r2 = ctrl.check_rate_of_rise(&status);
        assert!(r2.is_ok());
        assert_eq!(ctrl.pv_ror_exceeded_count, 0);
    }

    #[test]
    fn check_rate_of_rise_triggers_emergency_on_third() {
        let hub = SensorConversionHub::new();
        let mut ctrl = SensorController::new(hub);
        let mut status = make_status();
        status.pid_enabled = true;
        status.derivative_rate = 10.0;
        status.derivative_available = true;

        assert!(ctrl.check_rate_of_rise(&status).is_ok());
        assert_eq!(ctrl.pv_ror_exceeded_count, 1);
        assert!(ctrl.check_rate_of_rise(&status).is_ok());
        assert_eq!(ctrl.pv_ror_exceeded_count, 2);
        assert!(ctrl.check_rate_of_rise(&status).is_err());
    }

    #[test]
    fn check_rate_of_rise_runs_even_when_pid_disabled() {
        let hub = SensorConversionHub::new();
        let mut ctrl = SensorController::new(hub);
        let mut status = make_status();
        status.pid_enabled = false;
        status.derivative_rate = 10.0;
        status.derivative_available = true;

        assert!(ctrl.check_rate_of_rise(&status).is_ok());
        assert_eq!(ctrl.pv_ror_exceeded_count, 1);
    }

    /// Bug R1 (2026-07-26): the BT runaway guard and the legacy PV-RoR check
    /// must use SEPARATE debounce counters. Previously one shared counter
    /// meant a healthy ET-as-PV tick (which resets the PV counter every tick)
    /// also reset the BT guard counter, so a genuine BT runaway never reached
    /// `ROR_EXCEEDED_CONSECUTIVE_LIMIT` in `PID;CHAN;1` mode.
    #[test]
    fn bt_guard_counter_independent_of_pv_check() {
        let hub = SensorConversionHub::new();
        let mut ctrl = SensorController::new(hub);

        // Healthy PV (ET as PV, e.g.): the legacy check resets its own counter.
        let mut status = make_status();
        status.pid_enabled = true;
        status.derivative_rate = 0.1; // below MAX_BT_RATE_OF_RISE
        status.derivative_available = true;
        assert!(ctrl.check_rate_of_rise(&status).is_ok());
        assert_eq!(ctrl.pv_ror_exceeded_count, 0);

        // BT is genuinely running away: its guard must accumulate despite the
        // healthy PV checks interleaved between ticks.
        assert!(ctrl.check_bt_rate(1.0).is_ok());
        assert_eq!(ctrl.bt_ror_exceeded_count, 1);
        assert!(ctrl.check_rate_of_rise(&status).is_ok()); // healthy ET tick
        assert_eq!(ctrl.pv_ror_exceeded_count, 0);
        assert!(
            ctrl.bt_ror_exceeded_count == 1,
            "healthy PV tick must NOT reset the BT guard counter (R1)"
        );
        assert!(ctrl.check_bt_rate(1.0).is_ok());
        assert_eq!(ctrl.bt_ror_exceeded_count, 2);
        // Third consecutive BT violation trips the guard even though the PV
        // path kept resetting its own (independent) counter.
        assert!(ctrl.check_bt_rate(1.0).is_err());
    }

    #[test]
    fn is_temperature_valid_within_range() {
        assert!(SensorController::is_temperature_valid(25.0));
        assert!(SensorController::is_temperature_valid(MIN_VALID_TEMP));
        assert!(SensorController::is_temperature_valid(MAX_VALID_TEMP));
    }

    #[test]
    fn is_temperature_valid_nan() {
        assert!(!SensorController::is_temperature_valid(f32::NAN));
    }

    #[test]
    fn is_temperature_valid_out_of_range() {
        assert!(!SensorController::is_temperature_valid(
            MIN_VALID_TEMP - 1.0
        ));
        assert!(!SensorController::is_temperature_valid(
            MAX_VALID_TEMP + 1.0
        ));
    }

    // ── F4.11 (Gap #3) + V2-3: fault_condition debounce, per-channel ──────

    #[test]
    fn fault_debounce_single_transient_does_not_latch() {
        // A single transient faulty read must NOT latch fault_condition —
        // the most common false trigger is a one-off SPI glitch.
        let hub = SensorConversionHub::new();
        let mut ctrl = SensorController::new(hub);
        let mut status = make_status();

        ctrl.apply_fault_debounce(true, false, &mut status);
        assert_eq!(ctrl.consecutive_bean_faults, 1);
        assert_eq!(ctrl.consecutive_env_faults, 0);
        assert!(
            !status.fault_condition,
            "Single faulty read must not latch fault_condition"
        );
    }

    #[test]
    fn fault_debounce_resets_on_clean_read_after_glitches() {
        let hub = SensorConversionHub::new();
        let mut ctrl = SensorController::new(hub);
        let mut status = make_status();

        // 3 faulty reads on BT (still below threshold of 5)
        for _ in 0..3 {
            ctrl.apply_fault_debounce(true, false, &mut status);
        }
        assert_eq!(ctrl.consecutive_bean_faults, 3);
        assert_eq!(ctrl.consecutive_env_faults, 0);
        assert!(!status.fault_condition);

        // A single clean read resets the counter entirely
        ctrl.apply_fault_debounce(false, false, &mut status);
        assert_eq!(ctrl.consecutive_bean_faults, 0);
        assert!(!status.fault_condition);
    }

    #[test]
    fn fault_debounce_latches_after_threshold_consecutive_faults() {
        let hub = SensorConversionHub::new();
        let mut ctrl = SensorController::new(hub);
        let mut status = make_status();

        // Below threshold: no latch
        for _ in 0..(SENSOR_FAULT_DEBOUNCE - 1) {
            ctrl.apply_fault_debounce(true, false, &mut status);
        }
        assert_eq!(ctrl.consecutive_bean_faults, SENSOR_FAULT_DEBOUNCE - 1);
        assert!(!status.fault_condition);

        // Nth consecutive fault: latch
        ctrl.apply_fault_debounce(true, false, &mut status);
        assert_eq!(ctrl.consecutive_bean_faults, SENSOR_FAULT_DEBOUNCE);
        assert!(
            status.fault_condition,
            "After {} consecutive faults, fault_condition must be latched",
            SENSOR_FAULT_DEBOUNCE
        );
    }

    #[test]
    fn fault_debounce_persists_above_threshold() {
        // Counter uses saturating_add so it never overflows u8. Once latched,
        // repeated faults keep fault_condition true. 205 iterations = count 205.
        let hub = SensorConversionHub::new();
        let mut ctrl = SensorController::new(hub);
        let mut status = make_status();

        for _ in 0..(SENSOR_FAULT_DEBOUNCE + 200) {
            ctrl.apply_fault_debounce(true, false, &mut status);
        }
        assert_eq!(ctrl.consecutive_bean_faults, 205);
        assert!(status.fault_condition);
    }

    #[test]
    fn fault_debounce_alternating_faults_never_latches() {
        // A condition that's intermittent (every other read faulty) should
        // never accumulate past the threshold and never latch.
        let hub = SensorConversionHub::new();
        let mut ctrl = SensorController::new(hub);
        let mut status = make_status();

        for i in 0..20 {
            ctrl.apply_fault_debounce(i % 2 == 0, false, &mut status);
        }
        assert!(
            !status.fault_condition,
            "Intermittent fault must not latch fault_condition"
        );
    }

    #[test]
    fn fault_debounce_independent_channels() {
        // V2-3: a fault on one channel must not advance the other's counter.
        let hub = SensorConversionHub::new();
        let mut ctrl = SensorController::new(hub);
        let mut status = make_status();

        // 10 ticks with only ET faulted: bean counter stays 0, and (Bug P2:
        // default pid_channel=2 → BT is the PV) the GLOBAL fault_condition
        // must NOT latch — a single-probe (BT-only) configuration stays
        // operable with ET unplugged.
        for _ in 0..10 {
            ctrl.apply_fault_debounce(false, true, &mut status);
        }
        assert_eq!(ctrl.consecutive_bean_faults, 0);
        assert!(ctrl.consecutive_env_faults >= SENSOR_FAULT_DEBOUNCE);
        assert!(
            !status.fault_condition,
            "P2: ET fault must not latch fault_condition while BT is the PV"
        );

        // A clean ET tick resets env; a subsequent BT-only fault advances
        // bean alone.
        ctrl.apply_fault_debounce(false, false, &mut status);
        assert_eq!(ctrl.consecutive_env_faults, 0);
        ctrl.apply_fault_debounce(true, false, &mut status);
        assert_eq!(ctrl.consecutive_bean_faults, 1);
        assert_eq!(ctrl.consecutive_env_faults, 0);
    }

    // ── Bug P2 (2026-08-03): fault_condition latches only on the ACTIVE PV ──

    #[test]
    fn env_fault_latch_does_not_arm_when_pid_channel_is_bt() {
        // Single-probe (BT-only) configuration: ET is chronically faulted but
        // `pid_channel = 2` (the default) means the control loop never reads
        // ET. The GLOBAL latch must NOT arm — otherwise the device rejects
        // every START/OT1/PREHEAT and is inoperable until a power cycle.
        let hub = SensorConversionHub::new();
        let mut ctrl = SensorController::new(hub);
        let mut status = make_status();
        status.pid_channel = 2; // default — BT is the PV

        for _ in 0..(SENSOR_FAULT_DEBOUNCE + 2) {
            ctrl.apply_fault_debounce(false, true, &mut status);
        }
        assert!(
            !status.fault_condition,
            "P2: chronic ET fault must not latch while pid_channel = 2 (BT PV)"
        );
        assert!(
            ctrl.consecutive_env_faults >= SENSOR_FAULT_DEBOUNCE,
            "the env debounce counter must still accumulate for a future channel switch"
        );
    }

    #[test]
    fn env_fault_latch_arms_when_pid_channel_is_env() {
        // `PID;CHAN;1` (ET as PV): a chronic ET fault IS a fault of the active
        // input — the latch must arm exactly as before.
        let hub = SensorConversionHub::new();
        let mut ctrl = SensorController::new(hub);
        let mut status = make_status();
        status.pid_channel = 1; // ET is the PV

        for _ in 0..SENSOR_FAULT_DEBOUNCE {
            ctrl.apply_fault_debounce(false, true, &mut status);
        }
        assert!(
            status.fault_condition,
            "P2: chronic ET fault must latch while pid_channel = 1 (ET PV)"
        );
    }

    #[test]
    fn bean_fault_latch_arms_when_pid_channel_is_bt() {
        // Default configuration: BT is the PV, so a chronic BT fault latches.
        let hub = SensorConversionHub::new();
        let mut ctrl = SensorController::new(hub);
        let mut status = make_status();
        status.pid_channel = 2;

        for _ in 0..SENSOR_FAULT_DEBOUNCE {
            ctrl.apply_fault_debounce(true, false, &mut status);
        }
        assert!(status.fault_condition);
    }

    // ── Bug B-Q (2026-08-04): a fully-faulted read must not refresh the ────
    // ── freshness timestamp (`last_temp_read`) ─────────────────────────────

    #[test]
    fn both_channels_faulted_does_not_refresh_last_temp_read() {
        // Bug B-Q: when BOTH channels are faulted the sample carries no new
        // information (V2-2 hold keeps the last valid temperature). Refreshing
        // `last_temp_read` made `update_control`'s staleness guard treat the
        // held value as fresh, letting the PID integrate against stale data —
        // worst case at boot: PV stuck at 0.0 with the heater ramping toward
        // 100 %. The timestamp must stay frozen so the PID enters the
        // stale-hold branch instead.
        let hub = SensorConversionHub::new();
        let mut ctrl = SensorController::new(hub);
        let mut status = make_status();
        let no_fault = SensorFault::default();
        let bean_fault = SensorFault {
            fault_detected: true,
            ..SensorFault::default()
        };
        let env_fault = SensorFault {
            fault_detected: true,
            ..SensorFault::default()
        };

        // Seed a clean reading — this establishes `last_temp_read`.
        let t0 = embassy_time::Instant::from_millis(1_000);
        ctrl.update_temperatures(150.0, 120.0, no_fault, no_fault, t0, &mut status)
            .expect("seed clean reading");
        assert_eq!(ctrl.last_temp_read(), Some(t0));

        // A fully-faulted read at t1 must NOT move the timestamp.
        let t1 = embassy_time::Instant::from_millis(2_000);
        ctrl.update_temperatures(999.0, 888.0, bean_fault, env_fault, t1, &mut status)
            .expect("faulted read is not an error");
        assert_eq!(
            ctrl.last_temp_read(),
            Some(t0),
            "B-Q: a fully-faulted read must not refresh last_temp_read"
        );

        // A single clean channel (BT-only config with ET unplugged) still
        // refreshes the timestamp — the remaining channel is usable.
        let t2 = embassy_time::Instant::from_millis(3_000);
        ctrl.update_temperatures(151.0, 888.0, no_fault, env_fault, t2, &mut status)
            .expect("partially-faulted read is not an error");
        assert_eq!(
            ctrl.last_temp_read(),
            Some(t2),
            "B-Q: a partially-faulted read (one clean channel) must refresh the timestamp"
        );
    }
}
