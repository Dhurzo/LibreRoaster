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
        format!(
            "{:.1},{:.1},{:.1},{:.1}",
            status.env_temp,   // ET
            status.bean_temp,  // BT
            status.ssr_output, // Power (heater)
            fan_speed          // Fan
        )
    }

    pub fn format_read_response_full(status: &SystemStatus) -> String {
        // NOTE: These values are placeholders for future ET2/BT2 thermocouple support.
        // store value for future et2 and bt2 support
        // When additional thermocouples are added, update these positions.
        format!(
            "{:.1},{:.1},-1,-1,-1,{:.1},{:.1}\r\n",
            status.env_temp,   // ET
            status.bean_temp,  // BT
            status.fan_output, // Fan
            status.ssr_output  // Heater
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

        let _delta_bt = ArtisanFormatter::calculate_delta_bt(bt, self.last_bt);
        self.last_bt = bt;

        let ror = self.calculate_ror(bt);

        let time_str = ArtisanFormatter::format_time(elapsed_secs, elapsed_ms);
        let line = ArtisanFormatter::format_artisan_line(&time_str, et, bt, ror, gas);

        Ok(line)
    }

    fn calculate_ror(&mut self, current_bt: f32) -> f32 {
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
        }
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
        status.ssr_output = 123.45;
        let fan_speed = -7.6;

        let output = ArtisanFormatter::format_read_response(&status, fan_speed);

        assert_eq!(output, "120.3,150.5,123.5,-7.6");
        assert_eq!(output.split(',').count(), 4);
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
    fn test_format_read_response_seven_values() {
        let status = create_test_status();
        let response = ArtisanFormatter::format_read_response_full(&status);

        let parts: Vec<&str> = response.trim_end().split(',').collect();
        assert_eq!(parts.len(), 7, "READ response must have exactly 7 values");
    }

    #[test]
    fn test_unused_channels_return_negative_one() {
        let status = create_test_status();
        let response = ArtisanFormatter::format_read_response_full(&status);

        let parts: Vec<&str> = response.trim_end().split(',').collect();

        assert_eq!(parts[2], "-1", "ET2 placeholder should be -1");
        assert_eq!(parts[3], "-1", "BT2 placeholder should be -1");
        assert_eq!(parts[4], "-1", "ambient placeholder should be -1");
    }

    #[test]
    fn test_response_terminates_with_crlf() {
        let status = create_test_status();
        let response = ArtisanFormatter::format_read_response_full(&status);

        assert!(
            response.ends_with("\r\n"),
            "READ response must terminate with CRLF"
        );
    }

    #[test]
    fn test_format_read_response_full_uses_status_values() {
        let mut status = create_test_status();
        status.env_temp = 125.5;
        status.bean_temp = 155.7;
        status.fan_output = 60.0;
        status.ssr_output = 80.0;

        let response = ArtisanFormatter::format_read_response_full(&status);

        let parts: Vec<&str> = response.trim_end().split(',').collect();

        assert_eq!(parts[0], "125.5", "ET should use env_temp");
        assert_eq!(parts[1], "155.7", "BT should use bean_temp");
        assert_eq!(parts[5], "60.0", "Fan should use fan_output");
        assert_eq!(parts[6], "80.0", "Heater should use ssr_output");
    }

    #[test]
    fn test_format_read_response_full_one_decimal_format() {
        let mut status = create_test_status();
        status.fan_output = 75.0;
        status.ssr_output = 100.0;

        let response = ArtisanFormatter::format_read_response_full(&status);

        let parts: Vec<&str> = response.trim_end().split(',').collect();

        assert_eq!(parts[5], "75.0", "Fan must show one decimal (75.0)");
        assert_eq!(parts[6], "100.0", "Heater must show one decimal (100.0)");
    }
}
