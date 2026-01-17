extern crate alloc;

use crate::config::SystemStatus;
use crate::output::traits::{OutputError, OutputFormatter};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use embassy_time::Instant;

/// Artisan+ protocol formatter
///
/// Implements the standard Artian+ serial protocol format:
/// #time,ET,BT,ROR,Power,DeltaBT
///
/// Fields:
/// - time: Seconds since roast start
/// - ET: Environment temperature (°C)
/// - BT: Bean temperature (°C)  
/// - ROR: Rate of rise (°C/s) - calculated as moving average
/// - Power: SSR output percentage (0-100)
/// - DeltaBT: BT change from previous reading
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

    fn calculate_ror(&mut self, current_bt: f32) -> f32 {
        Self::update_bt_history(&mut self.bt_history, current_bt);
        Self::compute_ror_from_history(&self.bt_history)
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

    fn format_artisan_line(
        time_str: &str,
        et: f32,
        bt: f32,
        ror: f32,
        power: f32,
        delta_bt: f32,
    ) -> String {
        format!(
            "#{}{:.1},{:.1},{:.2},{:.1},{:.2}",
            time_str, et, bt, ror, power, delta_bt
        )
    }
}

impl OutputFormatter for ArtisanFormatter {
    fn format(&self, status: &SystemStatus) -> Result<String, OutputError> {
        let elapsed_secs = self.start_time.elapsed().as_secs();
        let elapsed_ms = self.start_time.elapsed().as_millis() % 1000;

        let et = status.env_temp;
        let bt = status.bean_temp;
        let power = status.ssr_output;

        let delta_bt = Self::calculate_delta_bt(bt, self.last_bt);
        let ror = delta_bt;

        let time_str = Self::format_time(elapsed_secs, elapsed_ms);
        let line = Self::format_artisan_line(&time_str, et, bt, ror, power, delta_bt);

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
        let power = status.ssr_output;

        let delta_bt = ArtisanFormatter::calculate_delta_bt(bt, self.last_bt);
        self.last_bt = bt;

        let ror = self.calculate_ror(bt);

        let time_str = ArtisanFormatter::format_time(elapsed_secs, elapsed_ms);
        let line = ArtisanFormatter::format_artisan_line(&time_str, et, bt, ror, power, delta_bt);

        Ok(line)
    }

    fn calculate_ror(&mut self, current_bt: f32) -> f32 {
        ArtisanFormatter::update_bt_history(&mut self.bt_history, current_bt);
        ArtisanFormatter::compute_ror_from_history(&self.bt_history)
    }
}
