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
    ror_exceeded_count: u8,
    consecutive_fault_count: u8,
}

impl SensorController {
    pub fn new(sensor_hub: SensorConversionHub) -> Self {
        Self {
            sensor_hub,
            last_temp_read: None,
            last_pv_sample: None,
            last_pv_sample_time: None,
            last_filtered_derivative: 0.0,
            ror_exceeded_count: 0,
            consecutive_fault_count: 0,
        }
    }

    pub async fn read_sensors(&mut self, status: &mut SystemStatus) -> Result<(), RoasterError> {
        let sample = self.sensor_hub.sample().await?;
        let has_fault = sample.bean_fault.has_fault() || sample.env_fault.has_fault();
        // F4.11 (Gap #3): debounce before latching.
        self.apply_fault_debounce(has_fault, status);
        self.update_temperatures(
            sample.bean_temp,
            sample.env_temp,
            sample.bean_fault,
            sample.env_fault,
            sample.timestamp,
            status,
        )
    }

    /// F4.11 (Gap #3): Debounce sensor fault reads. A single faulty sample
    /// increments the counter but does NOT latch the fault — a transient SPI
    /// glitch therefore self-clears on the next clean read. Only after
    /// `SENSOR_FAULT_DEBOUNCE` consecutive faulty samples do we set
    /// `fault_condition = true`, protecting against a real wiring or
    /// open-thermocouple fault while avoiding spurious lockouts from a
    /// one-off bus error. Other paths that set `fault_condition` (overtemp in
    /// `update_temperatures`, manual emergency, RWDT) are not affected.
    pub fn apply_fault_debounce(&mut self, has_fault: bool, status: &mut SystemStatus) {
        if has_fault {
            self.consecutive_fault_count = self.consecutive_fault_count.saturating_add(1);
            if self.consecutive_fault_count >= SENSOR_FAULT_DEBOUNCE {
                status.fault_condition = true;
            }
        } else {
            self.consecutive_fault_count = 0;
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

        status.bean_temp = bean_temp + BT_THERMOCOUPLE_OFFSET;
        status.env_temp = env_temp + ET_THERMOCOUPLE_OFFSET;
        self.last_temp_read = Some(current_time);

        // Only check overtemp against valid sensors (ignore faulted ones)
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

        // If a sensor is faulted, decide whether to poison the temperature
        // with NaN. Bug B7: the F4.11 debouncer only protects
        // `status.fault_condition` — but the PID downstream rejects NaN PV by
        // triggering an *emergency* (see update_control NaN guard). Poisoning
        // bean_temp on the FIRST fault sample therefore turned a single
        // transient SPI glitch into a latched emergency immediately, even
        // though the next 160 ms read might have been clean. Only poison the
        // value once the fault has persisted across SENSOR_FAULT_DEBOUNCE
        // consecutive samples; until then hold the last valid temperature so
        // the PID and emergency guards keep operating on real data and the
        // debounce machinery has a chance to do its job.
        if bean_fault.has_fault() && self.consecutive_fault_count >= SENSOR_FAULT_DEBOUNCE {
            status.bean_temp = f32::NAN;
            // else: hold the last valid value already in status.bean_temp.
        }
        if env_fault.has_fault() && self.consecutive_fault_count >= SENSOR_FAULT_DEBOUNCE {
            status.env_temp = f32::NAN;
            // else: hold the last valid value already in status.env_temp.
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
            let duration = current_time.duration_since(prev_time);
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
            self.ror_exceeded_count = 0;
            return Ok(());
        }

        if status.derivative_rate > MAX_BT_RATE_OF_RISE {
            self.ror_exceeded_count = self.ror_exceeded_count.saturating_add(1);
            warn!(
                "BT RoR {:.2}°C/s exceeds limit {:.1}°C/s (count {}/{})",
                status.derivative_rate,
                MAX_BT_RATE_OF_RISE,
                self.ror_exceeded_count,
                ROR_EXCEEDED_CONSECUTIVE_LIMIT
            );
            if self.ror_exceeded_count >= ROR_EXCEEDED_CONSECUTIVE_LIMIT {
                return Err(RoasterError::TemperatureOutOfRange {
                    source: Some("rate_of_rise_exceeded"),
                });
            }
        } else {
            self.ror_exceeded_count = 0;
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

        // Bug B7: the FIRST few faulted samples must NOT poison bean_temp
        // (the F4.11 debounce protects against a transient SPI glitch). Run
        // the same faulted read fewer than SENSOR_FAULT_DEBOUNCE times and
        // assert the last valid value is held (here the offset-adjusted
        // OVERTEMP_THRESHOLD, NOT NaN). The overtemp guard must still be
        // skipped because the channel is signal-faulted.
        for _ in 0..(SENSOR_FAULT_DEBOUNCE - 1) {
            let result = ctrl.update_temperatures(
                OVERTEMP_THRESHOLD,
                120.0,
                bean_fault,
                no_fault,
                now,
                &mut status,
            );
            assert!(result.is_ok());
            assert!(
                !status.bean_temp.is_nan(),
                "B7: pre-debounce fault must hold the last valid value, not NaN"
            );
            // Apply the debouncer as the production read path does so
            // consecutive_fault_count advances toward the threshold.
            ctrl.apply_fault_debounce(true, &mut status);
        }

        // After SENSOR_FAULT_DEBOUNCE consecutive faults the temperature IS
        // poisoned, matching the F4.11 latch. Drive the debouncer one more
        // time so consecutive_fault_count reaches the threshold, then the
        // next faulted read poisons bean_temp.
        ctrl.apply_fault_debounce(true, &mut status);
        let result = ctrl.update_temperatures(
            OVERTEMP_THRESHOLD,
            120.0,
            bean_fault,
            no_fault,
            now,
            &mut status,
        );
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

        // Bug B7: a SINGLE faulty ET read must not poison env_temp. The PID
        // downstream treats NaN as an emergency; debouncing before poisoning
        // prevents a transient SPI glitch from latching one.
        ctrl.update_temperatures(150.0, 120.0, no_fault, env_fault, now, &mut status)
            .unwrap();
        assert!(!status.bean_temp.is_nan());
        assert!(
            !status.env_temp.is_nan(),
            "B7: first faulty ET read must hold the last valid value, not NaN"
        );

        // Drive the debouncer to the threshold, then the next faulted read
        // poisons env_temp (matching the F4.11 latch).
        for _ in 0..SENSOR_FAULT_DEBOUNCE {
            ctrl.apply_fault_debounce(true, &mut status);
        }
        ctrl.update_temperatures(150.0, 120.0, no_fault, env_fault, now, &mut status)
            .unwrap();
        assert!(!status.bean_temp.is_nan());
        assert!(status.env_temp.is_nan());
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
        assert_eq!(ctrl.ror_exceeded_count, 1);

        // Reset to a rate below MAX_BT_RATE_OF_RISE (0.5°C/s)
        status.derivative_rate = 0.3;
        let r2 = ctrl.check_rate_of_rise(&status);
        assert!(r2.is_ok());
        assert_eq!(ctrl.ror_exceeded_count, 0);
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
        assert_eq!(ctrl.ror_exceeded_count, 1);
        assert!(ctrl.check_rate_of_rise(&status).is_ok());
        assert_eq!(ctrl.ror_exceeded_count, 2);
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
        assert_eq!(ctrl.ror_exceeded_count, 1);
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

    // ── F4.11 (Gap #3): fault_condition debounce ─────────────────────────

    #[test]
    fn fault_debounce_single_transient_does_not_latch() {
        // A single transient faulty read must NOT latch fault_condition —
        // the most common false trigger is a one-off SPI glitch.
        let hub = SensorConversionHub::new();
        let mut ctrl = SensorController::new(hub);
        let mut status = make_status();

        ctrl.apply_fault_debounce(true, &mut status);
        assert_eq!(ctrl.consecutive_fault_count, 1);
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

        // 3 faulty reads (still below threshold of 5)
        for _ in 0..3 {
            ctrl.apply_fault_debounce(true, &mut status);
        }
        assert_eq!(ctrl.consecutive_fault_count, 3);
        assert!(!status.fault_condition);

        // A single clean read resets the counter entirely
        ctrl.apply_fault_debounce(false, &mut status);
        assert_eq!(ctrl.consecutive_fault_count, 0);
        assert!(!status.fault_condition);
    }

    #[test]
    fn fault_debounce_latches_after_threshold_consecutive_faults() {
        let hub = SensorConversionHub::new();
        let mut ctrl = SensorController::new(hub);
        let mut status = make_status();

        // Below threshold: no latch
        for _ in 0..(SENSOR_FAULT_DEBOUNCE - 1) {
            ctrl.apply_fault_debounce(true, &mut status);
        }
        assert_eq!(ctrl.consecutive_fault_count, SENSOR_FAULT_DEBOUNCE - 1);
        assert!(!status.fault_condition);

        // Nth consecutive fault: latch
        ctrl.apply_fault_debounce(true, &mut status);
        assert_eq!(ctrl.consecutive_fault_count, SENSOR_FAULT_DEBOUNCE);
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
            ctrl.apply_fault_debounce(true, &mut status);
        }
        assert_eq!(ctrl.consecutive_fault_count, 205);
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
            ctrl.apply_fault_debounce(i % 2 == 0, &mut status);
        }
        assert!(
            !status.fault_condition,
            "Intermittent fault must not latch fault_condition"
        );
    }
}
