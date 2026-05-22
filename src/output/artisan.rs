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
// Artisan protocol formatter for LibreRoaster
//
// This module handles formatting of temperature data and commands according to the
// Artisan roasting software protocol. Delegates to specialized formatters.
//
// # Memory Strategy
//
// This module is classified as **HOT PATH**:
// - All formatting operations occur during real-time temperature reporting
// - Delegates to formatters module with heapless types
// - No dynamic memory allocation during normal operation
//
// ## Memory Usage
//
// - `BT_HISTORY_SIZE`: Fixed history for BT temperature tracking (5 samples)
// - `REPORT_BUFFER_SIZE`: Temperature report formatting (32 chars)
// - `TIME_FORMAT_SIZE`: Time formatting (8 chars)
use crate::config::SystemStatus;
use crate::memory::{
    BT_HISTORY_SIZE, REPORT_BUFFER_SIZE, RESPONSE_BUFFER_SIZE, ROR_FILTER_ALPHA, ROR_MIN_SAMPLES,
    TIME_FORMAT_SIZE,
};
use crate::output::formatters::{CsvFormatter, RorCalculator, TimeFormatter};
use crate::output::traits::{OutputError, OutputFormatter};
use core::cell::RefCell;
use core::fmt::Write;
use embassy_time::Instant;
use heapless::{Deque, String as HeaplessString};

#[derive(Clone)]
pub struct ArtisanFormatter {
    start_time: Instant,
    // RefCell is safe here because ArtisanFormatter is only accessed from the
    // control_loop_task under EmbassyMutex, which guarantees single-threaded
    // access. No re-entrant borrow is possible in the Embassy cooperative model.
    ror_calculator: RefCell<RorCalculator>,
}

impl Default for ArtisanFormatter {
    fn default() -> Self {
        Self {
            start_time: Instant::now(),
            ror_calculator: RefCell::new(RorCalculator::new()),
        }
    }
}

impl ArtisanFormatter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.start_time = Instant::now();
        self.ror_calculator.borrow_mut().reset();
    }

    fn format_time(elapsed_secs: u64, elapsed_ms: u64) -> HeaplessString<TIME_FORMAT_SIZE> {
        let mut buf = HeaplessString::<TIME_FORMAT_SIZE>::new();
        let _ = core::write!(&mut buf, "{}.{:02}", elapsed_secs, elapsed_ms / 10);
        buf
    }

    fn format_artisan_line(
        time_str: &str,
        et: f32,
        bt: f32,
        ror: f32,
        gas: f32,
    ) -> HeaplessString<REPORT_BUFFER_SIZE> {
        let mut buf = HeaplessString::<REPORT_BUFFER_SIZE>::new();
        let _ = core::write!(
            &mut buf,
            "{},{:.1},{:.1},{:.2},{:.1}",
            time_str,
            et,
            bt,
            ror,
            gas
        );
        buf
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
    fn format(
        &self,
        status: &SystemStatus,
    ) -> Result<HeaplessString<REPORT_BUFFER_SIZE>, OutputError> {
        let elapsed_secs = self.start_time.elapsed().as_secs();
        let elapsed_ms = self.start_time.elapsed().as_millis() % 1000;

        let et = status.env_temp;
        let bt = status.bean_temp;
        let gas = status.ssr_output; // SSR output as gas control

        let ror = self.ror_calculator.borrow_mut().calculate_ror(bt);
        let time_str = TimeFormatter::format_time(elapsed_secs, elapsed_ms);
        let line = CsvFormatter::format_artisan_line(&time_str, et, bt, ror, gas);

        Ok(line)
    }
}

impl ArtisanFormatter {
    #[deprecated = "Use format_read_response_full() instead"]
    pub fn format_read_response(
        status: &SystemStatus,
        fan_speed: f32,
    ) -> HeaplessString<REPORT_BUFFER_SIZE> {
        let et = Self::normalize_read_value(status.env_temp);
        let bt = Self::normalize_read_value(status.bean_temp);
        let heater = Self::normalize_read_value(status.ssr_output);
        let fan = Self::normalize_read_value(fan_speed);
        let mut buf = HeaplessString::<REPORT_BUFFER_SIZE>::new();
        let _ = core::write!(
            &mut buf,
            "{:.1},{:.1},{:.1},{:.1}",
            et,     // ET
            bt,     // BT
            heater, // Power (heater)
            fan     // Fan
        );
        buf
    }

    pub fn format_read_response_full(status: &SystemStatus) -> HeaplessString<REPORT_BUFFER_SIZE> {
        let amb = Self::normalize_read_value(
            status
                .temperature_settings
                .convert_to_display(status.ambient_temp),
        );
        let et = Self::normalize_read_value(
            status
                .temperature_settings
                .convert_to_display(status.env_temp),
        );
        let bt = Self::normalize_read_value(
            status
                .temperature_settings
                .convert_to_display(status.bean_temp),
        );
        // TC4 standard format: AMBIENT,ET,BT,CHAN3,CHAN4
        // When PID is ON (TC4 PID mode), Artisan expects 3 extra fields:
        //   res[5] = Heater duty %, res[6] = Fan duty %, res[7] = SV (setpoint temp)
        // Heater/fan are percentages (0-100), NOT temperatures — never convert to °F.
        let mut buf = HeaplessString::<REPORT_BUFFER_SIZE>::new();
        if status.pid_enabled {
            let heater = Self::normalize_read_value(status.ssr_output);
            let fan = Self::normalize_read_value(status.fan_output);
            let sv = Self::normalize_read_value(
                status
                    .temperature_settings
                    .convert_to_display(status.target_temp),
            );
            let _ = core::write!(
                &mut buf,
                "{:.1},{:.1},{:.1},0.0,0.0,{:.1},{:.1},{:.1}",
                amb,
                et,
                bt,
                heater,
                fan,
                sv,
            );
        } else {
            let _ = core::write!(&mut buf, "{:.1},{:.1},{:.1},0.0,0.0", amb, et, bt,);
        }
        buf
    }

    /// Format STATUS response with 18 CSV fields.
    ///
    /// Buffer capacity: RESPONSE_BUFFER_SIZE=512 bytes.
    /// The STATUS line consists of 18 fields (ET, BT, heater, fan, watchdog flags,
    /// failure reason, PID state, and latency metrics). This must fit in 512 bytes.
    pub fn format_status_response(status: &SystemStatus) -> HeaplessString<RESPONSE_BUFFER_SIZE> {
        let et = Self::normalize_read_value(
            status
                .temperature_settings
                .convert_to_display(status.env_temp),
        );
        let bt = Self::normalize_read_value(
            status
                .temperature_settings
                .convert_to_display(status.bean_temp),
        );
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
        let pv =
            Self::normalize_read_value(status.temperature_settings.convert_to_display(status.pv));
        let mv =
            Self::normalize_read_value(status.temperature_settings.convert_to_display(status.mv));
        let integrator_value = Self::normalize_read_value(
            status
                .temperature_settings
                .convert_to_display(status.integrator_value),
        );
        let derivative_value = Self::normalize_read_value(
            status
                .temperature_settings
                .convert_to_display(status.derivative_rate),
        );
        let saturation_flag = if status.saturation_active { 1 } else { 0 };
        let integrator_clamp_flag = if status.integrator_clamped { 1 } else { 0 };
        let derivative_available_flag = if status.derivative_available { 1 } else { 0 };
        let command_latency = status.command_latency_us;
        let max_command_latency = status.max_command_latency_us;
        let temp_scale_indicator = if status.temperature_settings.is_fahrenheit() {
            1u8
        } else {
            0u8
        };
        let fault_flag = if status.fault_condition { 1 } else { 0 };

        let mut buf = HeaplessString::<RESPONSE_BUFFER_SIZE>::new();
        // Safety: Verify buffer capacity before writing to catch overflow bugs in development.
        // STATUS response with 20 fields must fit in RESPONSE_BUFFER_SIZE=512 bytes.
        debug_assert!(buf.capacity() >= RESPONSE_BUFFER_SIZE);
        let _ = core::write!(
            &mut buf,
            "{:.1},{:.1},{:.1},{:.1},{},{},{},{},{},{:.1},{:.1},{:.1},{:.2},{},{},{},{},{},{},{}",
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
            derivative_available_flag,
            command_latency,
            max_command_latency,
            temp_scale_indicator,
            fault_flag
        );
        buf
    }

    pub fn format_chan_ack(channel: u16) -> HeaplessString<REPORT_BUFFER_SIZE> {
        let mut buf = HeaplessString::<REPORT_BUFFER_SIZE>::new();
        let _ = core::write!(&mut buf, "#{}", channel);
        buf
    }

    pub fn format_err(code: u8, message: &str) -> HeaplessString<RESPONSE_BUFFER_SIZE> {
        let mut buf = HeaplessString::<RESPONSE_BUFFER_SIZE>::new();
        let _ = core::write!(&mut buf, "ERR {} {}", code, message);
        buf
    }

    // ROR Calculation Helper Functions (public for use by MutableArtisanFormatter)

    pub fn calculate_weighted_ror(history: &[f32]) -> f32 {
        if history.len() < ROR_MIN_SAMPLES {
            return 0.0;
        }

        let mut weighted_sum = 0.0;
        let mut weight_sum = 0.0;

        // Linear weighting: recent samples count more
        for (i, &temp) in history.iter().enumerate() {
            let weight = (i + 1) as f32; // Linear progression: 1, 2, 3, ..., n
            weighted_sum += temp * weight;
            weight_sum += weight;
        }

        let weighted_temp = weighted_sum / weight_sum;
        (weighted_temp - history[0]) / (history.len() - 1) as f32
    }

    pub fn apply_iir_filter(instantaneous_ror: f32, last_filtered: f32, alpha: f32) -> f32 {
        // IIR filter: y[n] = alpha * x[n] + (1 - alpha) * y[n-1]
        alpha * instantaneous_ror + (1.0 - alpha) * last_filtered
    }

    pub fn is_temperature_outlier(current_temp: f32, history: &[f32]) -> bool {
        if history.len() < 3 {
            return false;
        }

        let mean = history.iter().sum::<f32>() / history.len() as f32;
        let variance = history
            .iter()
            .map(|&v| {
                let diff = v - mean;
                diff * diff
            })
            .sum::<f32>()
            / history.len() as f32;
        let std_dev = libm::sqrtf(variance);

        // 2-sigma rule: values more than 2 standard deviations from mean are outliers
        (current_temp - mean).abs() > 2.0 * std_dev
    }
}

pub struct MutableArtisanFormatter {
    start_time: Instant,
    last_bt: f32,
    bt_history: Deque<f32, BT_HISTORY_SIZE>,
    timestamp_history: Deque<Instant, BT_HISTORY_SIZE>,
    last_filtered_ror: f32,
}

impl Default for MutableArtisanFormatter {
    fn default() -> Self {
        Self {
            start_time: Instant::now(),
            last_bt: 0.0,
            bt_history: Deque::<f32, BT_HISTORY_SIZE>::new(),
            timestamp_history: Deque::<Instant, BT_HISTORY_SIZE>::new(),
            last_filtered_ror: 0.0,
        }
    }
}

impl MutableArtisanFormatter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn format(
        &mut self,
        status: &SystemStatus,
    ) -> Result<HeaplessString<REPORT_BUFFER_SIZE>, OutputError> {
        let elapsed_secs = self.start_time.elapsed().as_secs();
        let elapsed_ms = self.start_time.elapsed().as_millis() % 1000;

        // Use original Celsius for ROR calculation
        let bt_c = status.bean_temp;
        let ror = self.calculate_ror(bt_c, Instant::now());

        // Convert temperatures for display
        let et = status
            .temperature_settings
            .convert_to_display(status.env_temp);
        let bt_display = status.temperature_settings.convert_to_display(bt_c);
        let gas = status.ssr_output; // SSR output as gas control

        let time_str = ArtisanFormatter::format_time(elapsed_secs, elapsed_ms);
        let line = ArtisanFormatter::format_artisan_line(&time_str, et, bt_display, ror, gas);

        Ok(line)
    }

    fn calculate_ror(&mut self, current_bt: f32, now: Instant) -> f32 {
        if self.last_bt == 0.0 {
            self.last_bt = current_bt;
            Self::update_bt_history_with_timestamp(
                &mut self.bt_history,
                &mut self.timestamp_history,
                current_bt,
                now,
            );
            return 0.0;
        }

        if current_bt == self.last_bt {
            self.last_bt = current_bt;
            return 0.0;
        }

        // Check for outliers before updating history
        let (front, back) = self.bt_history.as_slices();
        if !ArtisanFormatter::is_temperature_outlier(current_bt, front)
            && !ArtisanFormatter::is_temperature_outlier(current_bt, back)
        {
            self.last_bt = current_bt;
            Self::update_bt_history_with_timestamp(
                &mut self.bt_history,
                &mut self.timestamp_history,
                current_bt,
                now,
            );
        } else {
            // Skip this outlier reading, return last filtered value
            return self.last_filtered_ror;
        }

        // Compute ROR from Deque history using as_slices
        let (bt_front, bt_back) = self.bt_history.as_slices();
        let (ts_front, ts_back) = self.timestamp_history.as_slices();
        let combined_len = bt_front.len() + bt_back.len();
        if combined_len < ROR_MIN_SAMPLES {
            return 0.0;
        }

        // Build temp arrays for ROR calculation
        let mut bt_arr = [0.0f32; BT_HISTORY_SIZE];
        for (i, &v) in bt_front.iter().enumerate() {
            bt_arr[i] = v;
        }
        for (i, &v) in bt_back.iter().enumerate() {
            bt_arr[bt_front.len() + i] = v;
        }

        let mut ts_arr = [Instant::from_millis(0); BT_HISTORY_SIZE];
        for (i, &v) in ts_front.iter().enumerate() {
            ts_arr[i] = v;
        }
        for (i, &v) in ts_back.iter().enumerate() {
            ts_arr[ts_front.len() + i] = v;
        }

        let usable_bt = &bt_arr[..combined_len];
        let usable_ts = &ts_arr[..combined_len];

        // Calculate ROR using actual elapsed time
        let ror = Self::compute_ror_with_timestamps(usable_bt, usable_ts);

        // Apply IIR filter for smoothing
        let filtered_ror =
            ArtisanFormatter::apply_iir_filter(ror, self.last_filtered_ror, ROR_FILTER_ALPHA);

        self.last_filtered_ror = filtered_ror;
        filtered_ror
    }

    fn update_bt_history_with_timestamp(
        bt_history: &mut Deque<f32, BT_HISTORY_SIZE>,
        timestamp_history: &mut Deque<Instant, BT_HISTORY_SIZE>,
        current_bt: f32,
        now: Instant,
    ) {
        if bt_history.len() >= BT_HISTORY_SIZE {
            let _ = bt_history.pop_front();
        }
        let _ = bt_history.push_back(current_bt);

        if timestamp_history.len() >= BT_HISTORY_SIZE {
            let _ = timestamp_history.pop_front();
        }
        let _ = timestamp_history.push_back(now);
    }

    fn compute_ror_with_timestamps(bt: &[f32], timestamps: &[Instant]) -> f32 {
        if bt.len() < 2 {
            return 0.0;
        }

        let first_bt = bt[0];
        let last_bt = bt[bt.len() - 1];
        let first_ts = timestamps[0];
        let last_ts = timestamps[timestamps.len() - 1];

        let time_elapsed_secs = (last_ts.duration_since(first_ts).as_secs() as f32)
            + (last_ts.duration_since(first_ts).as_millis() as f32) / 1000.0;
        if time_elapsed_secs > 0.0 {
            (last_bt - first_bt) / time_elapsed_secs
        } else {
            0.0
        }
    }
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;
    use crate::config::{
        RoasterState, SsrHardwareStatus, SystemStatus, TemperatureScale, TemperatureSettings,
    };

    fn create_test_status() -> SystemStatus {
        SystemStatus {
            state: RoasterState::Stable,
            bean_temp: 150.5,
            env_temp: 120.3,
            target_temp: 200.0,
            ssr_output: 75.0,
            fan_output: 50.0,
            pid_enabled: false,
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
            temperature_settings: TemperatureSettings::new(),
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
        status.command_latency_us = 1250;
        status.max_command_latency_us = 5000;

        let output = ArtisanFormatter::format_status_response(&status);

        let parts: Vec<&str> = output.split(',').collect();
        assert_eq!(parts.len(), 20);

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
        assert_eq!(parts[16], "1250");
        assert_eq!(parts[17], "5000");
        assert_eq!(parts[18], "0");
        assert_eq!(parts[19], "0"); // fault_condition = false
    }

    #[test]
    fn test_format_status_response_flags_reflect_system_status() {
        let mut status = create_test_status();
        status.saturation_active = false;
        status.integrator_clamped = true;
        status.derivative_available = false;

        let output = ArtisanFormatter::format_status_response(&status);
        let parts: Vec<&str> = output.split(',').collect();

        assert_eq!(parts.len(), 20);
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

        assert_eq!(parts.len(), 20);
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
        assert_eq!(parts.len(), 20);
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
    fn test_format_read_response_tc4_format() {
        let status = create_test_status();
        let response = ArtisanFormatter::format_read_response_full(&status);

        let parts: Vec<&str> = response.split(',').collect();
        assert_eq!(
            parts.len(),
            5,
            "TC4 READ must have exactly 5 values (AMB,ET,BT,CHAN3,CHAN4)"
        );
    }

    #[test]
    fn test_format_read_response_full_uses_tc4_order() {
        let mut status = create_test_status();
        status.env_temp = 125.5;
        status.bean_temp = 155.7;
        status.ambient_temp = 25.0;

        // Test Celsius (default) — TC4 order: AMB,ET,BT,0,0
        let response = ArtisanFormatter::format_read_response_full(&status);
        let parts: Vec<&str> = response.split(',').collect();
        assert_eq!(parts.len(), 5);
        assert_eq!(
            parts[0], "25.0",
            "AMB should be ambient_temp (first per TC4)"
        );
        assert_eq!(parts[1], "125.5", "ET should be env_temp (second per TC4)");
        assert_eq!(parts[2], "155.7", "BT should be bean_temp (third per TC4)");
        assert_eq!(parts[3], "0.0", "CHAN3 placeholder");
        assert_eq!(parts[4], "0.0", "CHAN4 placeholder");

        // Test Fahrenheit conversion
        status
            .temperature_settings
            .set_scale(TemperatureScale::Fahrenheit);
        let response_f = ArtisanFormatter::format_read_response_full(&status);
        let parts_f: Vec<&str> = response_f.split(',').collect();
        // 25.0°C = 77.0°F, 125.5°C = 257.9°F, 155.7°C = 312.3°F
        assert_eq!(parts_f[0], "77.0", "AMB converted to Fahrenheit");
        assert_eq!(parts_f[1], "257.9", "ET converted to Fahrenheit");
        assert_eq!(parts_f[2], "312.3", "BT converted to Fahrenheit");
        assert_eq!(parts_f[3], "0.0", "CHAN3 unchanged");
        assert_eq!(parts_f[4], "0.0", "CHAN4 unchanged");
    }

    #[test]
    fn test_tc4_read_invalid_values_normalized() {
        let mut status = create_test_status();
        status.bean_temp = f32::NEG_INFINITY;

        let response = ArtisanFormatter::format_read_response_full(&status);
        let parts: Vec<&str> = response.split(',').collect();
        assert_eq!(parts.len(), 5);
        assert_eq!(parts[0], "0.0", "AMB default");
        assert_eq!(parts[1], "120.3", "ET valid");
        assert_eq!(parts[2], "0.0", "BT normalized from -inf");
        assert_eq!(parts[3], "0.0");
        assert_eq!(parts[4], "0.0");
    }

    #[test]
    fn test_tc4_read_one_decimal_format() {
        let status = create_test_status();
        let response = ArtisanFormatter::format_read_response_full(&status);
        let parts: Vec<&str> = response.split(',').collect();
        assert_eq!(parts.len(), 5);
        assert_eq!(parts[0], "0.0", "AMB shows one decimal");
        assert_eq!(parts[1], "120.3", "ET shows one decimal");
        assert_eq!(parts[2], "150.5", "BT shows one decimal");
    }

    // ── TC4 READ with PID data (8-value format) ──────────────────

    #[test]
    fn test_read_response_pid_disabled_5_values() {
        let mut status = create_test_status();
        // pid_enabled is false by default in create_test_status
        status.bean_temp = 155.7;
        status.env_temp = 125.5;
        status.ambient_temp = 25.0;

        let response = ArtisanFormatter::format_read_response_full(&status);
        let parts: Vec<&str> = response.split(',').collect();
        assert_eq!(parts.len(), 5, "PID off → 5 values (AMB,ET,BT,CHAN3,CHAN4)");
    }

    #[test]
    fn test_read_response_pid_enabled_8_values() {
        let mut status = create_test_status();
        status.pid_enabled = true;
        status.bean_temp = 155.7;
        status.env_temp = 125.5;
        status.target_temp = 200.0;
        status.ssr_output = 75.0;
        status.fan_output = 50.0;

        let response = ArtisanFormatter::format_read_response_full(&status);
        let parts: Vec<&str> = response.split(',').collect();
        assert_eq!(
            parts.len(),
            8,
            "PID on → 8 values (AMB,ET,BT,CHAN3,CHAN4,heater,fan,SV)"
        );
    }

    #[test]
    fn test_read_response_pid_values_at_correct_indices() {
        let mut status = create_test_status();
        status.pid_enabled = true;
        status.env_temp = 125.5;
        status.bean_temp = 155.7;
        status.target_temp = 200.0;
        status.ssr_output = 75.0;
        status.fan_output = 50.0;

        let response = ArtisanFormatter::format_read_response_full(&status);
        let parts: Vec<&str> = response.split(',').collect();
        assert_eq!(parts.len(), 8);
        assert_eq!(parts[0], "0.0", "AMB");
        assert_eq!(parts[1], "125.5", "ET");
        assert_eq!(parts[2], "155.7", "BT");
        assert_eq!(parts[3], "0.0", "CHAN3");
        assert_eq!(parts[4], "0.0", "CHAN4");
        assert_eq!(parts[5], "75.0", "Heater % (TC4 res[5])");
        assert_eq!(parts[6], "50.0", "Fan % (TC4 res[6])");
        assert_eq!(parts[7], "200.0", "SV setpoint (TC4 res[7])");
    }

    #[test]
    fn test_read_response_pid_sv_respects_fahrenheit() {
        let mut status = create_test_status();
        status.pid_enabled = true;
        status.env_temp = 125.5;
        status.bean_temp = 155.7;
        status.target_temp = 200.0; // 200°C = 392°F
        status.ssr_output = 75.0;
        status.fan_output = 50.0;
        status
            .temperature_settings
            .set_scale(TemperatureScale::Fahrenheit);

        let response = ArtisanFormatter::format_read_response_full(&status);
        let parts: Vec<&str> = response.split(',').collect();
        assert_eq!(parts.len(), 8);
        // Temperatures converted: 125.5°C=257.9°F, 155.7°C=312.3°F, 200°C=392.0°F
        assert_eq!(parts[1], "257.9", "ET converted to °F");
        assert_eq!(parts[2], "312.3", "BT converted to °F");
        assert_eq!(parts[7], "392.0", "SV converted to °F");
        // Heater/fan are percentages — NOT converted
        assert_eq!(parts[5], "75.0", "Heater % must not be converted");
        assert_eq!(parts[6], "50.0", "Fan % must not be converted");
    }

    #[test]
    fn test_format_status_response_celsius() {
        let mut status = create_instrumented_status();
        status.watchdog_feed_ok = false;
        status.watchdog_consecutive_failures = 3;
        status.watchdog_last_failure = Some("timeout");
        status.ledc_guard_timeouts = 7;
        status.overtemp_regression_active = true;
        status.ssr_output = 88.0;
        status.fan_output = 42.0;
        status.command_latency_us = 1250;
        status.max_command_latency_us = 5000;

        let response = ArtisanFormatter::format_status_response(&status);
        let parts: Vec<&str> = response.split(',').collect();
        assert_eq!(parts.len(), 20);

        assert_eq!(parts[0], "120.3", "ET in Celsius");
        assert_eq!(parts[1], "150.5", "BT in Celsius");
        assert_eq!(parts[2], "88.0", "Heater unchanged");
        assert_eq!(parts[3], "42.0", "Fan unchanged");
        assert_eq!(parts[4], "0");
        assert_eq!(parts[5], "3");
        assert_eq!(parts[6], "timeout");
        assert_eq!(parts[7], "7");
        assert_eq!(parts[8], "1");
        assert_eq!(parts[9], "150.5", "PV in Celsius");
        assert_eq!(parts[10], "88.5", "MV in Celsius");
        assert_eq!(parts[11], "37.1", "Integrator in Celsius");
        assert_eq!(parts[12], "-0.42", "Derivative in Celsius");
        assert_eq!(parts[13], "1");
        assert_eq!(parts[14], "1");
        assert_eq!(parts[15], "1");
        assert_eq!(parts[16], "1250");
        assert_eq!(parts[17], "5000");
        assert_eq!(parts[18], "0");
        assert_eq!(parts[19], "0", "fault_condition should be false");
    }

    #[test]
    fn test_format_status_response_fahrenheit() {
        let mut status = create_instrumented_status();
        status.watchdog_feed_ok = false;
        status.watchdog_consecutive_failures = 3;
        status.watchdog_last_failure = Some("timeout");
        status.ledc_guard_timeouts = 7;
        status.overtemp_regression_active = true;
        status.ssr_output = 88.0;
        status.fan_output = 42.0;
        status.command_latency_us = 1250;
        status.max_command_latency_us = 5000;
        status
            .temperature_settings
            .set_scale(TemperatureScale::Fahrenheit);

        let response = ArtisanFormatter::format_status_response(&status);
        let parts: Vec<&str> = response.split(',').collect();
        assert_eq!(parts.len(), 20);

        // 120.3°C = 248.5°F, 150.5°C = 302.9°F
        assert_eq!(parts[0], "248.5", "ET converted to Fahrenheit");
        assert_eq!(parts[1], "302.9", "BT converted to Fahrenheit");
        assert_eq!(parts[2], "88.0", "Heater unchanged");
        assert_eq!(parts[3], "42.0", "Fan unchanged");
        assert_eq!(parts[9], "302.9", "PV converted to Fahrenheit");
        assert_eq!(parts[10], "191.3", "MV converted to Fahrenheit (88.5°C)");
        assert_eq!(
            parts[11], "98.8",
            "Integrator converted to Fahrenheit (37.1°C)"
        );
        assert_eq!(
            parts[12], "31.24",
            "Derivative converted to Fahrenheit (-0.42°C)"
        );
    }
}
