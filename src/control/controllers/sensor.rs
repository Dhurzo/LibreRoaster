use crate::config::*;
use crate::control::RoasterError;
use crate::hardware::sensors::{SensorConversionHub, SensorSample};
use embassy_time::Instant;
use log::warn;

const DERIVATIVE_FILTER_ALPHA: f32 = 0.3;

pub struct SensorController {
    sensor_hub: SensorConversionHub,
    last_temp_read: Option<Instant>,
    last_pv_sample: Option<f32>,
    last_pv_sample_time: Option<Instant>,
    last_filtered_derivative: f32,
    ror_exceeded_count: u8,
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
        }
    }

    pub async fn read_sensors(&mut self, status: &mut SystemStatus) -> Result<(), RoasterError> {
        let sample = self.sensor_hub.sample().await?;
        let has_fault = sample.bean_fault.has_fault() || sample.env_fault.has_fault();
        if has_fault {
            status.fault_condition = true;
        }
        self.update_temperatures(
            sample.bean_temp,
            sample.env_temp,
            sample.bean_fault,
            sample.env_fault,
            sample.timestamp,
            status,
        )
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

        // If a sensor is faulted, mark its temperature as NaN so PID rejects it
        if bean_fault.has_fault() {
            status.bean_temp = f32::NAN;
        }
        if env_fault.has_fault() {
            status.env_temp = f32::NAN;
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
        if !status.derivative_available || !status.pid_enabled {
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

        status.derivative_rate = 2.0;
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
    fn check_rate_of_rise_skipped_when_pid_disabled() {
        let hub = SensorConversionHub::new();
        let mut ctrl = SensorController::new(hub);
        let mut status = make_status();
        status.pid_enabled = false;
        status.derivative_rate = 10.0;
        status.derivative_available = true;

        let result = ctrl.check_rate_of_rise(&status);

        assert!(result.is_ok());
        assert_eq!(ctrl.ror_exceeded_count, 0);
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
}
