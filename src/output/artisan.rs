extern crate alloc;

/// Artisan standard CSV protocol formatter
///
/// Implements the standard Artisan serial protocol format:
/// time,ET,BT,ROR,Gas
///
/// Fields:
/// - time: Seconds since roast start
/// - ET: Environment temperature (°C)
/// - BT: Bean temperature (°C)  
/// - ROR: Rate of rise (°C/s) - calculated as moving average
/// - Gas: SSR output percentage (0-100) as heater control
use crate::config::SystemStatus;
use crate::output::traits::{OutputError, OutputFormatter};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use embassy_time::Instant;

#[derive(Clone)]
pub struct ArtisanFormatter {
    start_time: Instant,
    last_bt: f32,
    bt_history: Vec<f32>,
}

impl ArtisanFormatter {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            last_bt: 0.0,
            bt_history: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        self.start_time = Instant::now();
        self.last_bt = 0.0;
        self.bt_history.clear();
    }

    fn calculate_delta_bt(current_bt: f32, last_bt: f32) -> f32 {
        if last_bt != 0.0 {
            current_bt - last_bt
        } else {
            0.0
        }
    }

    fn update_bt_history(history: &mut Vec<f32>, current_bt: f32) {
        if history.len() >= 5 {
            history.remove(0);
        }
        history.push(current_bt);
    }

    fn compute_ror_from_history(history: &[f32]) -> f32 {
        if history.len() < 2 {
            0.0
        } else {
            let samples = history.len();
            let first_bt = history[0];
            let last_bt = history[samples - 1];

            // ROR = (BT_current - BT_oldest) / (time_elapsed)
            // Assuming 1-second intervals between samples
            (last_bt - first_bt) / (samples as f32 - 1.0)
        }
    }

    fn format_time(elapsed_secs: u64, elapsed_ms: u64) -> String {
        format!("{}.{:02}", elapsed_secs, elapsed_ms / 10)
    }

    fn format_artisan_line(time_str: &str, et: f32, bt: f32, ror: f32, gas: f32) -> String {
        format!("{},{:.1},{:.1},{:.2},{:.1}", time_str, et, bt, ror, gas)
    }

    fn normalize_read_value(value: f32) -> f32 {
        if value.is_finite() {
            value
        } else {
            0.0
        }
    }
}

impl OutputFormatter for ArtisanFormatter {
    fn format(&self, status: &SystemStatus) -> Result<String, OutputError> {
        let elapsed_secs = self.start_time.elapsed().as_secs();
        let elapsed_ms = self.start_time.elapsed().as_millis() % 1000;

        let et = status.env_temp;
        let bt = status.bean_temp;
        let gas = status.ssr_output; // SSR output as gas control

        let delta_bt = Self::calculate_delta_bt(bt, self.last_bt);
        let ror = delta_bt;

        let time_str = Self::format_time(elapsed_secs, elapsed_ms);
        let line = Self::format_artisan_line(&time_str, et, bt, ror, gas);

        Ok(line)
    }
}

impl ArtisanFormatter {
    pub fn format_read_response(status: &SystemStatus, fan_speed: f32) -> String {
        let et = Self::normalize_read_value(status.env_temp);
        let bt = Self::normalize_read_value(status.bean_temp);
        let heater = Self::normalize_read_value(status.ssr_output);
        let fan = Self::normalize_read_value(fan_speed);
        format!(
            "{:.1},{:.1},{:.1},{:.1}",
            et,     // ET
            bt,     // BT
            heater, // Power (heater)
            fan     // Fan
        )
    }

    pub fn format_read_response_full(status: &SystemStatus) -> String {
        let et = Self::normalize_read_value(status.env_temp);
        let bt = Self::normalize_read_value(status.bean_temp);
        let heater = Self::normalize_read_value(status.ssr_output);
        let fan = Self::normalize_read_value(status.fan_output);
        // 4-value format: ET, BT, HEATER, FAN
        format!(
            "{:.1},{:.1},{:.1},{:.1}",
            et,     // ET
            bt,     // BT
            heater, // Heater
            fan     // Fan
        )
    }

    pub fn format_status_response(status: &SystemStatus) -> String {
        let et = Self::normalize_read_value(status.env_temp);
        let bt = Self::normalize_read_value(status.bean_temp);
        let heater = Self::normalize_read_value(status.ssr_output);
        let fan = Self::normalize_read_value(status.fan_output);
        let watchdog_flag = if status.watchdog_feed_ok { 1 } else { 0 };
        let failure_count = status.watchdog_consecutive_failures;
        let failure_reason = status.watchdog_last_failure.unwrap_or("none");
        let guard_timeouts = status.ledc_guard_timeouts;
        let regression_flag = if status.overtemp_regression_active {
            1
        } else {
            0
        };
        let pv = Self::normalize_read_value(status.pv);
        let mv = Self::normalize_read_value(status.mv);
        let integrator_value = Self::normalize_read_value(status.integrator_value);
        let derivative_value = Self::normalize_read_value(status.derivative_rate);
        let saturation_flag = if status.saturation_active { 1 } else { 0 };
        let integrator_clamp_flag = if status.integrator_clamped { 1 } else { 0 };
        let derivative_available_flag = if status.derivative_available { 1 } else { 0 };

        format!(
            "{:.1},{:.1},{:.1},{:.1},{},{},{},{},{},{:.1},{:.1},{:.1},{:.2},{},{},{}",
            et,
            bt,
            heater,
            fan,
            watchdog_flag,
            failure_count,
            failure_reason,
            guard_timeouts,
            regression_flag,
            pv,
            mv,
            integrator_value,
            derivative_value,
            saturation_flag,
            integrator_clamp_flag,
            derivative_available_flag
        )
    }

    pub fn format_chan_ack(channel: u16) -> String {
        format!("#{}", channel)
    }

    pub fn format_err(code: u8, message: &str) -> String {
        format!("ERR {} {}", code, message)
    }
}

pub struct MutableArtisanFormatter {
    start_time: Instant,
    last_bt: f32,
    bt_history: Vec<f32>,
}

impl MutableArtisanFormatter {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            last_bt: 0.0,
            bt_history: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        self.start_time = Instant::now();
        self.last_bt = 0.0;
        self.bt_history.clear();
    }

    pub fn format(&mut self, status: &SystemStatus) -> Result<String, OutputError> {
        let elapsed_secs = self.start_time.elapsed().as_secs();
        let elapsed_ms = self.start_time.elapsed().as_millis() % 1000;

        let et = status.env_temp;
        let bt = status.bean_temp;
        let gas = status.ssr_output; // SSR output as gas control

        let ror = self.calculate_ror(bt);

        let time_str = ArtisanFormatter::format_time(elapsed_secs, elapsed_ms);
        let line = ArtisanFormatter::format_artisan_line(&time_str, et, bt, ror, gas);

        Ok(line)
    }

    fn calculate_ror(&mut self, current_bt: f32) -> f32 {
        if self.last_bt == 0.0 {
            self.last_bt = current_bt;
            ArtisanFormatter::update_bt_history(&mut self.bt_history, current_bt);
            return 0.0;
        }

        if current_bt == self.last_bt {
            self.last_bt = current_bt;
            return 0.0;
        }

        self.last_bt = current_bt;
        ArtisanFormatter::update_bt_history(&mut self.bt_history, current_bt);
        ArtisanFormatter::compute_ror_from_history(&self.bt_history)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{RoasterState, SsrHardwareStatus, SystemStatus};
    use alloc::vec;

    fn create_test_status() -> SystemStatus {
        SystemStatus {
            state: RoasterState::Stable,
            bean_temp: 150.5,
            env_temp: 120.3,
            target_temp: 200.0,
            ssr_output: 75.0,
            fan_output: 50.0,
            pid_enabled: true,
            artisan_control: false,
            fault_condition: false,
            ssr_hardware_status: SsrHardwareStatus::Available,
            ssr_last_duty_delta_ticks: 0,
            ssr_retry_count: 0,
            ssr_cycle_guard_busy_until_ms: 0,
            watchdog_feed_ok: true,
            watchdog_last_failure: None,
            watchdog_consecutive_failures: 0,
            ledc_guard_timeouts: 0,
            overtemp_regression_active: false,
            ..SystemStatus::default()
        }
    }

    fn create_instrumented_status() -> SystemStatus {
        let mut status = create_test_status();
        status.pv = 150.5;
        status.mv = 88.5;
        status.integrator_value = 37.1;
        status.derivative_rate = -0.42;
        status.saturation_active = true;
        status.integrator_clamped = true;
        status.derivative_available = true;
        status
    }

    #[test]
    fn test_format_read_response() {
        let status = create_test_status();
        let fan_speed = 25.0;

        let output = ArtisanFormatter::format_read_response(&status, fan_speed);

        assert_eq!(output, "120.3,150.5,75.0,25.0");

        let parts: Vec<&str> = output.split(',').collect();
        assert_eq!(parts.len(), 4);

        assert_eq!(parts[0], "120.3");
        assert_eq!(parts[1], "150.5");
        assert_eq!(parts[2], "75.0");
        assert_eq!(parts[3], "25.0");
    }

    #[test]
    fn test_format_read_response_out_of_range_values() {
        let mut status = create_test_status();
        status.ssr_output = 123.46;
        let fan_speed = -7.6;

        let output = ArtisanFormatter::format_read_response(&status, fan_speed);

        assert_eq!(output, "120.3,150.5,123.5,-7.6");
        assert_eq!(output.split(',').count(), 4);
    }

    #[test]
    fn test_format_read_response_invalid_values() {
        let mut status = create_test_status();
        status.env_temp = f32::NAN;
        status.ssr_output = f32::INFINITY;
        let fan_speed = 25.0;

        let output = ArtisanFormatter::format_read_response(&status, fan_speed);

        assert_eq!(output, "0.0,150.5,0.0,25.0");
        assert_eq!(output.split(',').count(), 4);
    }

    #[test]
    fn test_format_status_response_columns_order() {
        let mut status = create_instrumented_status();
        status.watchdog_feed_ok = false;
        status.watchdog_consecutive_failures = 3;
        status.watchdog_last_failure = Some("timeout");
        status.ledc_guard_timeouts = 7;
        status.overtemp_regression_active = true;
        status.ssr_output = 88.0;
        status.fan_output = 42.0;

        let output = ArtisanFormatter::format_status_response(&status);

        let parts: Vec<&str> = output.split(',').collect();
        assert_eq!(parts.len(), 16);

        assert_eq!(parts[0], "120.3");
        assert_eq!(parts[1], "150.5");
        assert_eq!(parts[2], "88.0");
        assert_eq!(parts[3], "42.0");
        assert_eq!(parts[4], "0");
        assert_eq!(parts[5], "3");
        assert_eq!(parts[6], "timeout");
        assert_eq!(parts[7], "7");
        assert_eq!(parts[8], "1");
        assert_eq!(parts[9], "150.5");
        assert_eq!(parts[10], "88.5");
        assert_eq!(parts[11], "37.1");
        assert_eq!(parts[12], "-0.42");
        assert_eq!(parts[13], "1");
        assert_eq!(parts[14], "1");
        assert_eq!(parts[15], "1");
    }

    #[test]
    fn test_format_status_response_flags_reflect_system_status() {
        let mut status = create_test_status();
        status.saturation_active = false;
        status.integrator_clamped = true;
        status.derivative_available = false;

        let output = ArtisanFormatter::format_status_response(&status);
        let parts: Vec<&str> = output.split(',').collect();

        assert_eq!(parts.len(), 16);
        assert_eq!(parts[13], "0");
        assert_eq!(parts[14], "1");
        assert_eq!(parts[15], "0");
    }

    #[test]
    fn test_format_status_response_derivative_integrator_values_reflect_system_status() {
        let mut status = create_test_status();
        status.integrator_value = 51.2;
        status.derivative_rate = 0.73;
        status.saturation_active = true;
        status.integrator_clamped = false;
        status.derivative_available = true;

        let output = ArtisanFormatter::format_status_response(&status);
        let parts: Vec<&str> = output.split(',').collect();

        assert_eq!(parts.len(), 16);
        assert_eq!(parts[11], "51.2");
        assert_eq!(parts[12], "0.73");
        assert_eq!(parts[13], "1");
        assert_eq!(parts[14], "0");
        assert_eq!(parts[15], "1");
    }

    #[test]
    fn test_format_status_response_none_reason() {
        let status = create_test_status();
        let output = ArtisanFormatter::format_status_response(&status);

        assert!(output.contains(",none,"));
        let parts: Vec<&str> = output.split(',').collect();
        assert_eq!(parts.len(), 16);
        assert_eq!(parts[12], "0.00");
        assert_eq!(parts[13], "0");
        assert_eq!(parts[14], "0");
        assert_eq!(parts[15], "0");
    }

    #[test]
    fn test_format_csv_output() {
        let formatter = ArtisanFormatter::new();
        let status = create_test_status();

        let result = formatter.format(&status);

        assert!(result.is_ok());
        let output = match result {
            Ok(val) => val,
            Err(e) => {
                log::error!("Failed to process Artisan output (result): {:?}", e);
                panic!("Artisan output processing failed");
            }
        };

        let parts: Vec<&str> = output.split(',').collect();
        assert_eq!(parts.len(), 5);

        assert!(parts[0].starts_with(|c: char| c.is_ascii_digit()));
        assert_eq!(parts[1], "120.3");
        assert_eq!(parts[2], "150.5");
        assert_eq!(parts[3], "0.00");
        assert_eq!(parts[4], "75.0");
    }

    #[test]
    fn test_ror_calculation_empty_history() {
        let history: Vec<f32> = vec![];
        let ror = ArtisanFormatter::compute_ror_from_history(&history);
        assert_eq!(ror, 0.0);
    }

    #[test]
    fn test_ror_calculation_two_samples() {
        let history = vec![100.0, 105.0];
        let ror = ArtisanFormatter::compute_ror_from_history(&history);
        assert_eq!(ror, 5.0);
    }

    #[test]
    fn test_ror_calculation_five_samples() {
        let history = vec![100.0, 102.0, 104.0, 106.0, 108.0];
        let ror = ArtisanFormatter::compute_ror_from_history(&history);
        assert_eq!(ror, 2.0);
    }

    #[test]
    fn test_mutable_formatter_ror() {
        let mut formatter = MutableArtisanFormatter::new();

        let status1 = SystemStatus {
            bean_temp: 100.0,
            env_temp: 120.0,
            ssr_output: 50.0,
            ..create_test_status()
        };
        let result1 = formatter.format(&status1);
        assert!(result1.is_ok());
        let output1 = match result1 {
            Ok(val) => val,
            Err(e) => {
                log::error!("Failed to process Artisan output (result1): {:?}", e);
                panic!("Artisan output processing failed");
            }
        };
        let parts1: Vec<&str> = output1.split(',').collect();
        assert_eq!(parts1[3], "0.00");

        let status2 = SystemStatus {
            bean_temp: 102.0,
            env_temp: 121.0,
            ssr_output: 55.0,
            ..create_test_status()
        };
        let result2 = formatter.format(&status2);
        assert!(result2.is_ok());
    }

    #[test]
    fn test_time_format_seconds_only() {
        let time = ArtisanFormatter::format_time(5, 0);
        assert_eq!(time, "5.00");
    }

    #[test]
    fn test_time_format_with_milliseconds() {
        let time = ArtisanFormatter::format_time(5, 50);
        assert_eq!(time, "5.05");
    }

    #[test]
    fn test_time_format_zero_seconds() {
        let time = ArtisanFormatter::format_time(0, 150);
        assert_eq!(time, "0.15");
    }

    #[test]
    fn test_time_format_capped_decimals() {
        let time = ArtisanFormatter::format_time(10, 999);
        assert_eq!(time, "10.99");
    }

    #[test]
    fn test_time_format_typical_value() {
        let time = ArtisanFormatter::format_time(123, 456);
        assert_eq!(time, "123.45");
    }

    #[test]
    fn test_format_chan_ack() {
        let result = ArtisanFormatter::format_chan_ack(1200);
        assert_eq!(result, "#1200");
    }

    #[test]
    fn test_format_chan_ack_various_values() {
        assert_eq!(ArtisanFormatter::format_chan_ack(1), "#1");
        assert_eq!(ArtisanFormatter::format_chan_ack(9999), "#9999");
        assert_eq!(ArtisanFormatter::format_chan_ack(0), "#0");
    }

    #[test]
    fn test_format_err() {
        let result = ArtisanFormatter::format_err(1, "Unknown command");
        assert_eq!(result, "ERR 1 Unknown command");
    }

    #[test]
    fn test_format_err_various() {
        assert_eq!(
            ArtisanFormatter::format_err(2, "Invalid value"),
            "ERR 2 Invalid value"
        );
        assert_eq!(ArtisanFormatter::format_err(0, "Success"), "ERR 0 Success");
    }

    #[test]
    fn test_format_read_response_four_values() {
        let status = create_test_status();
        let response = ArtisanFormatter::format_read_response_full(&status);

        let parts: Vec<&str> = response.split(',').collect();
        assert_eq!(parts.len(), 4, "READ response must have exactly 4 values");
    }

    #[test]
    fn test_format_read_response_full_uses_status_values() {
        let mut status = create_test_status();
        status.env_temp = 125.5;
        status.bean_temp = 155.7;
        status.fan_output = 60.0;
        status.ssr_output = 80.0;

        let response = ArtisanFormatter::format_read_response_full(&status);

        let parts: Vec<&str> = response.split(',').collect();

        assert_eq!(parts[0], "125.5", "ET should use env_temp");
        assert_eq!(parts[1], "155.7", "BT should use bean_temp");
        assert_eq!(parts[2], "80.0", "Heater should use ssr_output");
        assert_eq!(parts[3], "60.0", "Fan should use fan_output");
    }

    #[test]
    fn test_format_read_response_full_invalid_values() {
        let mut status = create_test_status();
        status.bean_temp = f32::NEG_INFINITY;
        status.fan_output = f32::NAN;

        let response = ArtisanFormatter::format_read_response_full(&status);

        assert_eq!(response, "120.3,0.0,75.0,0.0");
        assert_eq!(response.split(',').count(), 4);
    }

    #[test]
    fn test_format_read_response_full_one_decimal_format() {
        let mut status = create_test_status();
        status.fan_output = 75.0;
        status.ssr_output = 100.0;

        let response = ArtisanFormatter::format_read_response_full(&status);

        let parts: Vec<&str> = response.split(',').collect();

        assert_eq!(parts[2], "100.0", "Heater must show one decimal (100.0)");
        assert_eq!(parts[3], "75.0", "Fan must show one decimal (75.0)");
    }
}
