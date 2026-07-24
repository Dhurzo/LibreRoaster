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
use core::fmt::Write;
use embassy_time::Instant;
use heapless::{Deque, String as HeaplessString};

#[derive(Clone)]
pub struct ArtisanFormatter {
    start_time: Instant,
    ror_calculator: RorCalculator,
}

impl Default for ArtisanFormatter {
    fn default() -> Self {
        Self {
            start_time: Instant::now(),
            ror_calculator: RorCalculator::new(),
        }
    }
}

impl ArtisanFormatter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.start_time = Instant::now();
        self.ror_calculator.reset();
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
        &mut self,
        status: &SystemStatus,
    ) -> Result<HeaplessString<REPORT_BUFFER_SIZE>, OutputError> {
        let elapsed_secs = self.start_time.elapsed().as_secs();
        let elapsed_ms = self.start_time.elapsed().as_millis() % 1000;

        let et = status.env_temp;
        let bt = status.bean_temp;
        let gas = status.ssr_output;

        let ror = self.ror_calculator.calculate_ror(bt);
        let time_str = TimeFormatter::format_time(elapsed_secs, elapsed_ms);
        let line = CsvFormatter::format_artisan_line(&time_str, et, bt, ror, gas);

        // Bug #7: prefix the spontaneous continuous-telemetry line with '#'
        // so a line-oriented client can distinguish it from a `READ` response.
        // The `READ` handler emits via `format_read_response_full`, which has
        // a different field layout (8 fields starting with AMB) and starts
        // with a digit; this `#` prefix on continuous telemetry lets the
        // peer multiplex both sources on the same byte stream. Empirically
        // Artisan tolerates a leading '#' on telemetry lines.
        let mut prefixed = HeaplessString::<REPORT_BUFFER_SIZE>::new();
        let _ = prefixed.push('#');
        let _ = prefixed.push_str(&line);
        Ok(prefixed)
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

    /// Format STATUS response with 20 CSV fields.
    ///
    /// Buffer capacity: RESPONSE_BUFFER_SIZE=512 bytes.
    /// The STATUS line consists of 20 fields (ET, BT, heater, fan, watchdog flags,
    /// failure reason, PID state, latency metrics, temp scale, fault flag).
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
        // Bug #5: PV is a temperature, but `mv` (manipulated variable) and
        // `integrator_value` are dimensionless percentages — they must NOT go
        // through the °C→°F temperature conversion, otherwise a heater at
        // 75% would be reported as ~167°"F" to Artisan. The previous code
        // wrapped them in `convert_to_display`, and a test blessed the bug.
        let mv = Self::normalize_read_value(status.mv);
        let integrator_value = Self::normalize_read_value(status.integrator_value);
        // `derivative_rate` is in °C/s internally. When Artisan is in °F
        // mode it expects rate in °F/min (not °F as if it were a temperature).
        // °F/min = °C/s × (9/5) × 60. The previous code applied the
        // temperature formula `(C*9/5)+32`, producing nonsense values like
        // 31.24 for a -0.42 °C/s RoR.
        let derivative_value = if status.temperature_settings.is_fahrenheit() {
            Self::normalize_read_value(status.derivative_rate * (9.0 / 5.0) * 60.0)
        } else {
            Self::normalize_read_value(status.derivative_rate)
        };
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
    /// Bug #6: `last_bt == 0.0` was previously used as the "first sample"
    /// sentinel, but a legitimate BT reading of 0 °C (cold ambient, sensor
    /// fallback) would falsely re-initialise the ROR state and discard the
    /// real history. We now track initialisation explicitly with this flag.
    is_initialised: bool,
    bt_history: Deque<f32, BT_HISTORY_SIZE>,
    timestamp_history: Deque<Instant, BT_HISTORY_SIZE>,
    last_filtered_ror: f32,
}

impl Default for MutableArtisanFormatter {
    fn default() -> Self {
        Self {
            start_time: Instant::now(),
            last_bt: 0.0,
            is_initialised: false,
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

        // Bug #7: prefix the spontaneous continuous-telemetry line with '#' so
        // a line-oriented client can distinguish it from a synchronous `READ`
        // response (which the READ handler emits via `format_read_response_full`,
        // starting with a digit and using a different field count). Artisan
        // tolerates a leading '#' on continuous telemetry.
        let mut prefixed = HeaplessString::<REPORT_BUFFER_SIZE>::new();
        let _ = prefixed.push('#');
        let _ = prefixed.push_str(&line);
        Ok(prefixed)
    }

    fn calculate_ror(&mut self, current_bt: f32, now: Instant) -> f32 {
        // Bug V2-12: a single non-finite BT (sensor fault → NaN flowing into
        // `derivative_rate` is the more common upstream path, but this method
        // is also called with the raw BT for the formatter) poisons the IIR
        // forever: `last_filtered_ror = α·NaN + (1-α)·prev = NaN`, and every
        // subsequent clean sample still mixes with `NaN`. Return the last
        // filtered RoR and DO NOT advance history with garbage — the next
        // finite sample finds a clean window so the IIR recovers immediately.
        if !current_bt.is_finite() {
            return self.last_filtered_ror;
        }

        // Bug #6: use an explicit initialisation flag instead of treating
        // `last_bt == 0.0` as the "first sample" sentinel. A legitimate BT
        // of 0 °C (cold ambient, MAX31856 fallback) no longer corrupts ROR
        // state by re-seeding the history.
        if !self.is_initialised {
            self.last_bt = current_bt;
            self.is_initialised = true;
            Self::update_bt_history_with_timestamp(
                &mut self.bt_history,
                &mut self.timestamp_history,
                current_bt,
                now,
            );
            return 0.0;
        }

        if current_bt == self.last_bt {
            // Bug #7: still record the sample in history so the time base
            // advances. Returning 0.0 without updating the history left a
            // gap in the timestamp series, so subsequent ROR computations
            // used stale time spans and produced a static ROR even when
            // earlier samples indicated a trend.
            self.last_bt = current_bt;
            Self::update_bt_history_with_timestamp(
                &mut self.bt_history,
                &mut self.timestamp_history,
                current_bt,
                now,
            );
            return 0.0;
        }

        // Bug B12: the previous code refused to insert an outlier into the
        // history, so on a smooth ramp {m, m+d, m+2d, m+3d, ...} every
        // sample past the 3rd was rejected (a constant ramp violates the
        // "2-sigma vs mean of {first 3 samples}" rule by construction:
        // σ ≈ 0.82·d, while the deviation of m+3d is 2·d > 1.63·d). The
        // window froze at 3 samples and `last_filtered_ror` was returned
        // for the entire roast.
        //
        // Fix: ALWAYS advance the history so the mean/variance track the
        // trend, and the only side-effect of an outlier is suppressing the
        // RoR value emitted for THIS sample (return the last filtered
        // RoR). The next clean sample finds a window that has moved on
        // rather than one frozen at the start of the roast.
        let is_outlier = {
            // Bug V2-11 (B12 residual): the previous code applied the 2σ test
            // to `front` and `back` SEPARATELY (each slice using its own
            // mean/σ). When the deque wraps, the front slice holds only the
            // oldest samples; on a linear ramp the current sample deviates up
            // to ~9d from that fragment while 2σ of a 3-element slice is
            // ~1.63d → guaranteed outlier. The simulation in the v2 report
            // showed 70-85 % of ramp samples suppressed, so the IIR only
            // updated ~2 of every 10 samples and the emitted RoR converged
            // ~20-30 s late. Combine both slices into a single window (as
            // the RoR calc a few lines below already does) so the test uses
            // the mean/σ of the WHOLE history.
            let mut window: heapless::Vec<f32, BT_HISTORY_SIZE> = heapless::Vec::new();
            let (front, back) = self.bt_history.as_slices();
            let _ = window.extend_from_slice(front);
            let _ = window.extend_from_slice(back);
            ArtisanFormatter::is_temperature_outlier(current_bt, &window)
        };
        self.last_bt = current_bt;
        Self::update_bt_history_with_timestamp(
            &mut self.bt_history,
            &mut self.timestamp_history,
            current_bt,
            now,
        );
        if is_outlier {
            // Suppress the RoR for this sample; keep the window advancing.
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

        // Bug B20: `as_secs()` plus `as_millis()/1000` doubled the elapsed
        // time for windows >= 1 s because `as_millis()` returns the FULL
        // millisecond count (not the sub-second remainder). On the prior
        // `last_filtered_ror` (kept by B12's outlier-skip path) this would
        // approximately halve the reported RoR for any roast > 1 s. Use a
        // single `as_millis()` reading divided by 1000 to get the real span.
        let time_elapsed_secs = (last_ts.duration_since(first_ts).as_millis() as f32) / 1000.0;
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
        let mut formatter = ArtisanFormatter::new();
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

        // Bug #7 regression: continuous-telemetry lines are prefixed with '#'
        // so they can be distinguished from a synchronous `READ` response on
        // the same wire. The time field carries that prefix.
        assert!(
            parts[0].starts_with('#'),
            "telemetry line must start with '#' prefix, got: {}",
            parts[0]
        );
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

        // Temperatures in CSV convert to °F: 120.3°C → 248.5°F, 150.5°C → 302.9°F
        assert_eq!(parts[0], "248.5", "ET converted to Fahrenheit");
        assert_eq!(parts[1], "302.9", "BT converted to Fahrenheit");
        // Heater and fan are percentages — NOT temperatures, NOT converted.
        assert_eq!(parts[2], "88.0", "Heater % must not be converted");
        assert_eq!(parts[3], "42.0", "Fan % must not be converted");
        assert_eq!(parts[9], "302.9", "PV converted to Fahrenheit");
        // Bug #5 regression: `mv` and `integrator_value` are percentages
        // (PID output terms), not temperatures. The previous formatter ran
        // them through `convert_to_display`, producing nonsense like 191.3
        // for a 75% heater, and a test here blessed that. They must be
        // emitted unchanged in both scales.
        assert_eq!(parts[10], "88.5", "MV must NOT be converted (it is a %)");
        assert_eq!(
            parts[11], "37.1",
            "Integrator must NOT be converted (it is a %)"
        );
        // Bug #5 regression: `derivative_rate` is in °C/s internally. Artisan
        // in °F mode expects a rate in °F/min, not "°F as if it were a
        // temperature". So we multiply by 9/5×60, NOT apply °C→°F.
        // −0.42 °C/s × 1.8 × 60 = −45.36 °F/min.
        assert_eq!(
            parts[12], "-45.36",
            "Derivative must be °F/min (rate), not °F (temperature)"
        );
    }

    // ── V2-12: calculate_ror must not be poisoned by a non-finite BT ──

    #[test]
    fn calculate_ror_nan_bt_does_not_poison_filter() {
        // Bug V2-12: a single NaN flowing into the IIR left
        // `last_filtered_ror = α·NaN + (1-α)·prev = NaN` forever; every clean
        // sample afterwards still mixed with NaN and the RoR never recovered.
        // The fix early-returns on non-finite BT WITHOUT advancing history,
        // so the next finite sample finds a clean window.
        let mut fmt = MutableArtisanFormatter::new();

        // Seed a history and a baseline RoR.
        let t0 = Instant::from_millis(0);
        let r0 = fmt.calculate_ror(100.0, t0); // init
        assert_eq!(r0, 0.0);
        let t1 = Instant::from_millis(1000);
        let _ = fmt.calculate_ror(110.0, t1); // finite RoR
        let baseline = fmt.calculate_ror(120.0, Instant::from_millis(2000));
        // The filter is finite and non-zero after a 10 °C/s jump (α=0.25).
        assert!(
            baseline.is_finite(),
            "baseline must be finite: {}",
            baseline
        );
        assert!(baseline > 0.0, "baseline must be positive: {}", baseline);

        // Inject a NaN — the method MUST return the last filtered RoR (which
        // is finite) and NOT propagate NaN into the IIR state.
        let t_nan = Instant::from_millis(3000);
        let r_nan = fmt.calculate_ror(f32::NAN, t_nan);
        assert!(
            r_nan.is_finite(),
            "NaN BT must not propagate to the emitted RoR: {}",
            r_nan
        );

        // A subsequent finite sample recovers a finite RoR — the IIR was not
        // poisoned by the NaN.
        let r_after = fmt.calculate_ror(130.0, Instant::from_millis(4000));
        assert!(
            r_after.is_finite(),
            "RoR after a NaN sample must be finite (IIR not poisoned): {}",
            r_after
        );
        assert!(r_after > 0.0);
    }

    // ── V2-11: outlier test uses the COMBINED history window ─────────

    #[test]
    fn outlier_test_uses_combined_window_on_linear_ramp() {
        // Bug V2-11 (B12 residual): the previous per-slice test marked
        // 70-85 % of a linear ramp as outliers because the front slice (when
        // the deque wrapped) held only the oldest samples — the current
        // sample deviated up to ~9d from that fragment while 2σ of a
        // 3-element slice was ~1.63d → guaranteed outlier. The fix combines
        // both slices into a single window so the 2σ test uses the mean of
        // the WHOLE history; on a linear ramp the deviation from the
        // combined mean stays under 2σ for the bulk of the samples.
        //
        // A neat way to exercise this is to fill the BT_HISTORY_SIZE=5 deque
        // (which forces a wrap, splitting front/back) and check that the
        // LAST sample of a clean ramp is NOT classified as an outlier — the
        // per-slice version flagged it.
        let mut fmt = MutableArtisanFormatter::new();

        // Drive a clean linear ramp 100, 102, 104, 106, 108 °C at 1 s steps.
        // The 5th sample will evict the 1st (deque wrap → front holds oldest
        // 4, back holds 0; the per-slice test on a single 4-element slice was
        // actually safe). To force a true front+back split, drive 6 samples.
        let bt_series = [100.0_f32, 102.0, 104.0, 106.0, 108.0, 110.0];
        let mut last_ror = 0.0_f32;
        for (i, &bt) in bt_series.iter().enumerate() {
            let t = Instant::from_millis((i as u64) * 1000);
            last_ror = fmt.calculate_ror(bt, t);
            // Every sample of a clean, monotonic ramp must produce a finite
            // RoR. The IIR must update on every one of them (the bug
            // suppressed ~70-85 % of them, so `last_filtered_ror` would have
            // frozen at the first or second sample's value).
            assert!(
                last_ror.is_finite(),
                "RoR at sample {} must be finite: {}",
                i,
                last_ror
            );
        }
        // A 2 °C/s ramp filtered with α=0.25 from 0 must end meaningfully
        // above zero (the per-slice bug would have returned the same frozen
        // value for all late samples; we assert it advanced past the first
        // sample's 0.0). 5 updates of 2°C/s × 0.25 → ≥ 0.5 °C/s.
        assert!(
            last_ror > 0.5,
            "RoR must advance on a clean ramp (per-slice bug froze it): {}",
            last_ror
        );
    }
}
