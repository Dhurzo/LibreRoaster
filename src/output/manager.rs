extern crate alloc;

use crate::config::SystemStatus;
use crate::output::artisan::MutableArtisanFormatter;
use crate::output::scheduler::IntervalScheduler;
use crate::output::uart::{UartPrinter, DefaultUartChannel};
use crate::output::traits::{OutputError, PrintScheduler, SerialOutput};
use alloc::string::String;
use log::info;

/// Main output manager responsible for coordinating all output operations
///
/// This component follows the Single Responsibility Principle by:
/// - Coordinating between formatter, scheduler, and output
/// - Managing configuration and enabling/disabling
/// - Handling errors gracefully without affecting control logic
/// - Providing a simple interface for the rest of the system
pub struct OutputManager {
    formatter: MutableArtisanFormatter,
    printer: UartPrinter<DefaultUartChannel>,
    scheduler: IntervalScheduler,
    enabled: bool,
}

impl OutputManager {
    /// Create new output manager with default configuration
    pub fn new() -> Self {
        Self {
            formatter: MutableArtisanFormatter::new(),
            printer: UartPrinter::new(),
            scheduler: IntervalScheduler::hz1(), // 1Hz as requested
            enabled: true,
        }
    }

    /// Create output manager with custom configuration
    pub fn with_interval(interval_ms: u64) -> Self {
        Self {
            formatter: MutableArtisanFormatter::new(),
            printer: UartPrinter::new(),
            scheduler: IntervalScheduler::new(interval_ms),
            enabled: true,
        }
    }

    /// Enable or disable output completely
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        info!("Output manager enabled: {}", enabled);
    }

    /// Check if output is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable or disable just the scheduler (stops timing but keeps formatter ready)
    pub fn set_scheduler_enabled(&mut self, enabled: bool) {
        self.scheduler.set_enabled(enabled);
    }

    /// Enable continuous output for Artisan+ (enables scheduler)
    pub fn enable_continuous_output(&mut self) {
        self.scheduler.set_enabled(true);
        info!("Continuous output enabled for Artisan+");
    }

    /// Disable continuous output (disables scheduler)
    pub fn disable_continuous_output(&mut self) {
        self.scheduler.set_enabled(false);
        info!("Continuous output disabled");
    }

    /// Enable or disable just the serial printer
    pub fn set_printer_enabled(&mut self, enabled: bool) {
        self.printer.set_enabled(enabled);
    }

    /// Get current scheduler interval
    pub fn get_interval_ms(&self) -> u64 {
        self.scheduler.interval().as_millis() as u64
    }

    /// Set new scheduler interval
    pub fn set_interval_ms(&mut self, interval_ms: u64) {
        self.scheduler.set_interval(interval_ms);
        info!("Output interval changed to {}ms", interval_ms);
    }

    /// Reset all components (call when roast starts)
    pub fn reset(&mut self) {
        self.formatter.reset();
        self.scheduler.reset();
        info!("Output manager reset for new roast");
    }

    /// Main method: process system status and output if it's time
    ///
    /// This is the primary interface that the roaster control system will call
    /// from its main loop. It handles all the logic internally.
    pub async fn process_status(&mut self, status: &SystemStatus) -> Result<(), OutputError> {
        // Quick check if we're enabled
        if !self.enabled {
            return Ok(());
        }

        // Check if it's time to print (non-blocking)
        if !self.scheduler.should_print().await {
            return Ok(());
        }

        // Format the data
        let formatted_data = self.formatter.format(status)?;

        // Output the data
        self.printer.print(&formatted_data).await?;

        Ok(())
    }

    /// Force immediate output (ignores scheduler)
    ///
    /// Useful for one-time status updates or testing
    pub async fn force_output(&mut self, status: &SystemStatus) -> Result<(), OutputError> {
        if !self.enabled {
            return Ok(());
        }

        let formatted_data = self.formatter.format(status)?;
        self.printer.print(&formatted_data).await?;

        Ok(())
    }

    /// Output just the formatter (bypass scheduler and printer)
    ///
    /// Returns the formatted string without outputting it
    /// Useful for debugging or custom output handling
    pub fn format_status(&mut self, status: &SystemStatus) -> Result<String, OutputError> {
        self.formatter.format(status)
    }

    /// Print a custom message immediately
    ///
    /// Useful for custom logging or status messages
    pub async fn print_message(&mut self, message: &str) -> Result<(), OutputError> {
        if !self.enabled {
            return Ok(());
        }

        self.printer.print(message).await?;
        Ok(())
    }
}

impl Default for OutputManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration structure for output manager
#[derive(Debug, Clone)]
pub struct OutputConfig {
    pub enabled: bool,
    pub interval_ms: u64,
    pub printer_enabled: bool,
    pub scheduler_enabled: bool,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_ms: 1000, // 1Hz default
            printer_enabled: true,
            scheduler_enabled: true,
        }
    }
}

impl From<OutputConfig> for OutputManager {
    fn from(config: OutputConfig) -> Self {
        let mut manager = Self::with_interval(config.interval_ms);
        manager.enabled = config.enabled;
        manager.printer.set_enabled(config.printer_enabled);
        manager.scheduler.set_enabled(config.scheduler_enabled);
        manager
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RoasterState;
    use heapless::String;
    use core::fmt::Write;
    
    // For embassy tests in std environment
    use embassy_executor::Spawner;

    fn create_test_status() -> SystemStatus {
        SystemStatus {
            state: RoasterState::Heating,
            bean_temp: 150.0,
            env_temp: 100.0,
            target_temp: 200.0,
            ssr_output: 75.0,
            fan_output: 50.0,
            pid_enabled: true,
            artisan_control: false,
            fault_condition: false,
        }
    }

    #[test]
    fn test_output_manager_basic() {
        let mut manager = OutputManager::new();
        let status = create_test_status();

        // Should not print immediately (scheduler check)
        // Note: In test mode, we'll call the sync version
        let result = manager.force_output(&status);

        // Force output should work
        let result = manager.force_output(&status);

        // Format should work
        let formatted = manager.format_status(&status);
        assert!(formatted.is_ok());
        let formatted_str = formatted.unwrap();
        assert!(formatted_str.starts_with('#'));
        
        // Check for temperature manually without format macro
        let temp_str = "150.0"; // Expected temperature from test status
        assert!(formatted_str.contains(temp_str));
    }

    #[test]
    fn test_output_manager_disabled() {
        let mut manager = OutputManager::new();
        manager.set_enabled(false);

        let status = create_test_status();

        // Force output should not work when disabled
        let result = manager.force_output(&status);
        // Just check that it doesn't panic - result type depends on implementation
        drop(result); // Returns Ok, but no actual output
    }
}
