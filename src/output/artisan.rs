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

    /// Calculate rate of rise using moving average
    fn calculate_ror(&mut self, current_bt: f32) -> f32 {
        // Add current temperature to history
        if self.bt_history.len() >= 5 {
            // Remove oldest if full
            self.bt_history.remove(0);
        }
        self.bt_history.push(current_bt);

        // Calculate ROR using last 2-5 points
        if self.bt_history.len() < 2 {
            0.0
        } else {
            let samples = self.bt_history.len();
            let first_bt = self.bt_history[0];
            let last_bt = self.bt_history[samples - 1];

            // ROR = (BT_current - BT_oldest) / (time_elapsed)
            // Assuming 1-second intervals between samples
            (last_bt - first_bt) / (samples as f32 - 1.0)
        }
    }
}

impl OutputFormatter for ArtisanFormatter {
    fn format(&self, status: &SystemStatus) -> Result<String, OutputError> {

        let elapsed_secs = self.start_time.elapsed().as_secs();

        let et = status.env_temp;
        let bt = status.bean_temp;

        let delta_bt = if self.last_bt != 0.0 {
            bt - self.last_bt
        } else {
            0.0
        };

        let power = status.ssr_output;

        let ror = delta_bt;

        let elapsed_ms = self.start_time.elapsed().as_millis() % 1000;
        let line = format!(
            "#{}.{:02},{:.1},{:.1},{:.2},{:.1},{:.2}",
            elapsed_secs,
            elapsed_ms / 10,
            et,
            bt,
            ror,
            power,
            delta_bt
        );

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


        let et = status.env_temp;
        let bt = status.bean_temp;


        let delta_bt = if self.last_bt != 0.0 {
            bt - self.last_bt
        } else {
            0.0
        };

        self.last_bt = bt;

        let ror = self.calculate_ror(bt);


        let power = status.ssr_output;

        let elapsed_ms = self.start_time.elapsed().as_millis() % 1000;
        let line = format!(
            "#{}.{:02},{:.1},{:.1},{:.2},{:.1},{:.2}",
            elapsed_secs,
            elapsed_ms / 10,
            et,
            bt,
            ror,
            power,
            delta_bt
        );

        Ok(line)
    }

    fn calculate_ror(&mut self, current_bt: f32) -> f32 {

        if self.bt_history.len() >= 5 {

            self.bt_history.remove(0);
        }
        self.bt_history.push(current_bt);

        if self.bt_history.len() < 2 {
            0.0
        } else {
            let samples = self.bt_history.len();
            let first_bt = self.bt_history[0];
            let last_bt = self.bt_history[samples - 1];

            // ROR = (BT_current - BT_oldest) / (time_elapsed)
            // Assuming 1-second intervals between samples
            (last_bt - first_bt) / (samples as f32 - 1.0)
        }
    }
}
