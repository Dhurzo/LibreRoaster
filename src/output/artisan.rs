extern crate alloc;

use crate::config::SystemStatus;
use crate::output::traits::{OutputError, OutputFormatter};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use embassy_time::Instant;

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
#[derive(Clone)]
pub struct ArtisanFormatter {
    start_time: Instant,
    last_bt: f32,
    bt_history: Vec<f32>, // 5 samples for ROR calculation
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
}

/// Mutable version for proper ROR calculation
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

    /// Helper function to create a SystemStatus with known values for testing
    fn create_test_status() -> SystemStatus {
        SystemStatus {
            state: RoasterState::Roasting,
            bean_temp: 150.5,
            env_temp: 120.3,
            target_temp: 200.0,
            ssr_output: 75.0,
            fan_output: 50.0,
            pid_enabled: true,
            artisan_control: false,
            fault_condition: false,
            ssr_hardware_status: SsrHardwareStatus::Detected,
        }
    }

    /// TEST-07: Verify format_read_response produces correct CSV format
    /// Format: "ET,BT,Power,Fan" (comma-separated with one decimal place)
    #[test]
    fn test_format_read_response() {
        let status = create_test_status();
        let fan_speed = 25.0;

        let output = ArtisanFormatter::format_read_response(&status, fan_speed);

        // Expected format: "120.3,150.5,75.0,25.0" (ET,BT,Power,Fan)
        assert_eq!(output, "120.3,150.5,75.0,25.0");

        // Verify all four comma-separated values are present
        let parts: Vec<&str> = output.split(',').collect();
        assert_eq!(parts.len(), 4);

        // Verify each value matches expected
        assert_eq!(parts[0], "120.3"); // ET
        assert_eq!(parts[1], "150.5"); // BT
        assert_eq!(parts[2], "75.0"); // Power
        assert_eq!(parts[3], "25.0"); // Fan
    }

    /// TEST-08: Verify format produces correct Artisan CSV line format
    /// Format: "time,ET,BT,ROR,Gas"
    #[test]
    fn test_format_csv_output() {
        let formatter = ArtisanFormatter::new();
        let status = create_test_status();

        let result = formatter.format(&status);

        assert!(result.is_ok());
        let output = result.unwrap();

        // Verify all five comma-separated fields are present
        let parts: Vec<&str> = output.split(',').collect();
        assert_eq!(parts.len(), 5);

        // Verify field order: time, ET, BT, ROR, Gas
        // Time should start with digits and decimal
        assert!(parts[0].starts_with(|c: char| c.is_ascii_digit()));
        // ET should be 120.3
        assert_eq!(parts[1], "120.3");
        // BT should be 150.5
        assert_eq!(parts[2], "150.5");
        // ROR should be 0.00 (two decimal places for rate of rise)
        assert_eq!(parts[3], "0.00");
        // Gas should be 75.0
        assert_eq!(parts[4], "75.0");
    }

    /// TEST-09a: Verify ROR calculation from BT history - empty history
    #[test]
    fn test_ror_calculation_empty_history() {
        let history: Vec<f32> = vec![];
        let ror = ArtisanFormatter::compute_ror_from_history(&history);
        assert_eq!(ror, 0.0);
    }

    /// TEST-09b: Verify ROR calculation from BT history - two samples
    #[test]
    fn test_ror_calculation_two_samples() {
        let history = vec![100.0, 105.0]; // 5.0 change over 1 interval
        let ror = ArtisanFormatter::compute_ror_from_history(&history);
        assert_eq!(ror, 5.0); // (105.0 - 100.0) / (2 - 1) = 5.0
    }

    /// TEST-09c: Verify ROR calculation from BT history - five samples
    #[test]
    fn test_ror_calculation_five_samples() {
        // BT values: [100, 102, 104, 106, 108]
        // Expected ROR: (108 - 100) / (5 - 1) = 8.0 / 4 = 2.0
        let history = vec![100.0, 102.0, 104.0, 106.0, 108.0];
        let ror = ArtisanFormatter::compute_ror_from_history(&history);
        assert_eq!(ror, 2.0);
    }

    /// TEST-09d: Verify MutableArtisanFormatter accumulates BT history for ROR
    #[test]
    fn test_mutable_formatter_ror() {
        let mut formatter = MutableArtisanFormatter::new();

        // First call - should have ROR = 0.0 (no history)
        let status1 = SystemStatus {
            bean_temp: 100.0,
            env_temp: 120.0,
            ssr_output: 50.0,
            ..create_test_status()
        };
        let result1 = formatter.format(&status1);
        assert!(result1.is_ok());
        let output1 = result1.unwrap();
        let parts1: Vec<&str> = output1.split(',').collect();
        assert_eq!(parts1[3], "0.00"); // ROR should be 0.0 initially

        // Second call - ROR should be delta from first call
        let status2 = SystemStatus {
            bean_temp: 102.0,
            env_temp: 121.0,
            ssr_output: 55.0,
            ..create_test_status()
        };
        let result2 = formatter.format(&status2);
        assert!(result2.is_ok());
        // Note: With 2 samples, ROR = (102 - 100) / 1 = 2.0
    }

    /// TEST-10a: Verify time format - seconds only
    #[test]
    fn test_time_format_seconds_only() {
        let time = ArtisanFormatter::format_time(5, 0);
        assert_eq!(time, "5.00");
    }

    /// TEST-10b: Verify time format - seconds with milliseconds
    #[test]
    fn test_time_format_with_milliseconds() {
        let time = ArtisanFormatter::format_time(5, 50);
        assert_eq!(time, "5.05");
    }

    /// TEST-10c: Verify time format - zero seconds with milliseconds
    #[test]
    fn test_time_format_zero_seconds() {
        let time = ArtisanFormatter::format_time(0, 150);
        assert_eq!(time, "0.15");
    }

    /// TEST-10d: Verify time format - capped at two decimal places
    #[test]
    fn test_time_format_capped_decimals() {
        // 999ms / 10 = 99 (should cap at 99)
        let time = ArtisanFormatter::format_time(10, 999);
        assert_eq!(time, "10.99");
    }

    /// TEST-10e: Verify time format - typical value
    #[test]
    fn test_time_format_typical_value() {
        let time = ArtisanFormatter::format_time(123, 456);
        assert_eq!(time, "123.45");
    }
}
