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
    /// Create new Artisan+ formatter
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            last_bt: 0.0,
            bt_history: Vec::new(),
        }
    }

    /// Reset formatter (call when roast starts)
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
        // Calculate elapsed seconds since start
        let elapsed_secs = self.start_time.elapsed().as_secs();

        // Get temperatures
        let et = status.env_temp;
        let bt = status.bean_temp;

        // Calculate delta BT (change from previous)
        let delta_bt = if self.last_bt != 0.0 {
            bt - self.last_bt
        } else {
            0.0
        };

        // Get power output (SSR percentage)
        let power = status.ssr_output;

        // Note: ROR calculation would need mutable access,
        // for now using delta_BT as approximation
        let ror = delta_bt;

        // Format according to Artisan+ protocol
        // Format: #time,ET,BT,ROR,Power,DeltaBT
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

/// Mutable version for proper ROR calculation
pub struct MutableArtisanFormatter {
    start_time: Instant,
    last_bt: f32,
    bt_history: Vec<f32>,
}

impl MutableArtisanFormatter {
    /// Create new mutable Artisan+ formatter
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            last_bt: 0.0,
            bt_history: Vec::new(),
        }
    }

    /// Reset formatter (call when roast starts)
    pub fn reset(&mut self) {
        self.start_time = Instant::now();
        self.last_bt = 0.0;
        self.bt_history.clear();
    }

    /// Format system status into Artisan+ protocol string
    pub fn format(&mut self, status: &SystemStatus) -> Result<String, OutputError> {
        // Calculate elapsed seconds since start
        let elapsed_secs = self.start_time.elapsed().as_secs();

        // Get temperatures
        let et = status.env_temp;
        let bt = status.bean_temp;

        // Calculate delta BT
        let delta_bt = if self.last_bt != 0.0 {
            bt - self.last_bt
        } else {
            0.0
        };

        // Update last BT
        self.last_bt = bt;

        // Calculate ROR using moving average
        let ror = self.calculate_ror(bt);

        // Get power output
        let power = status.ssr_output;

        // Format according to Artisan+ protocol
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
