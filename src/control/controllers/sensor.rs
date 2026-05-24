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
