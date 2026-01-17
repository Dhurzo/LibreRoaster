use crate::output::traits::{OutputError, SerialOutput};
use log::info;

/// Serial output implementation using esp-println
///
/// This implementation uses the existing logging infrastructure
/// to output formatted data to the ESP32's USB serial interface.
pub struct SerialPrinter {
    enabled: bool,
}

impl SerialPrinter {
    /// Create new serial printer
    pub fn new() -> Self {
        Self { enabled: true }
    }

    /// Enable or disable serial output
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if serial output is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for SerialPrinter {
    fn default() -> Self {
        Self::new()
    }
}

impl SerialOutput for SerialPrinter {
    async fn print(&mut self, data: &str) -> Result<(), OutputError> {
        if !self.enabled {
            return Ok(());
        }

        // Use info! macro which outputs to serial via esp-println
        // This leverages the existing logging infrastructure
        info!("{}", data);

        // Note: In a production environment, you might want to use
        // direct UART access for more control, but this approach
        // integrates well with the existing logging system

        Ok(())
    }
}

/// Alternative direct UART implementation for more control
///
/// This is kept as a comment for future reference if needed:
///
/// ```rust
/// use esp_idf_hal::uart;
/// use esp_idf_hal::prelude::*;
///
/// pub struct DirectSerialPrinter {
///     uart: uart::UartDriver<'static>,
///     enabled: bool,
/// }
///
/// impl DirectSerialPrinter {
///     pub fn new() -> Result<Self, OutputError> {
///         // UART configuration for ESP32-C3
///         let config = uart::config::Config::new().baudrate(Hertz(115200));
///         let uart = uart::UartDriver::new(
///             uart::UartPort::Uart0,
///             // TX pin, RX pin, etc.
///         )?;
///         
///         Ok(Self {
///             uart,
///             enabled: true,
///         })
///     }
/// }
///
/// impl SerialOutput for DirectSerialPrinter {
///     async fn print(&mut self, data: &str) -> Result<(), OutputError> {
///         if !self.enabled {
///             return Ok(());
///         }
///         
///         let data_with_newline = format!("{}\n", data);
///         self.uart.write(data_with_newline.as_bytes())?;
///         
///         Ok(())
///     }
/// }
/// ```

/// Mock serial printer for testing
#[cfg(test)]
pub struct MockSerialPrinter {
    output: heapless::Vec<String, 100>,
    enabled: bool,
}

#[cfg(test)]
impl MockSerialPrinter {
    pub fn new() -> Self {
        Self {
            output: heapless::Vec::new(),
            enabled: true,
        }
    }

    pub fn get_output(&self) -> &[String] {
        &self.output
    }

    pub fn clear(&mut self) {
        self.output.clear();
    }
}

#[cfg(test)]
impl SerialOutput for MockSerialPrinter {
    async fn print(&mut self, data: &str) -> Result<(), OutputError> {
        if !self.enabled {
            return Ok(());
        }

        if self.output.push(data.to_string()).is_err() {
            return Err(OutputError::Serialization);
        }

        Ok(())
    }
}
