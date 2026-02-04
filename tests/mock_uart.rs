//! Mock UART driver for integration testing
//!
//! Provides a software implementation of the UART interface for testing
//! the Artisan+ protocol without requiring actual hardware.
//!
//! # Features
//!
//! - Simulates UART RX buffer for incoming Artisan commands
//! - Simulates UART TX buffer for outgoing responses
//! - Tracks transmission state and data
//! - Enables testing complete command → response flow
//!
//! # Usage Example
//!
//! ```rust
//! use libreroaster::hardware::uart::{UartDriver, UartError};
//! use crate::MockUartDriver;
//!
//! // Create mock with incoming command
//! let mut mock_uart = MockUartDriver::new("READ\r\n");
//!
//! // Read command from buffer
//! let mut buffer = [0u8; 64];
//! let bytes_read = mock_uart.read_bytes(&mut buffer).unwrap();
//! let command = core::str::from_utf8(&buffer[..bytes_read]).unwrap();
//! assert_eq!(command, "READ\r\n");
//!
//! // Write response
//! mock_uart.write_bytes(b"120.3,150.5,75.0,25.0\r\n").unwrap();
//!
//! // Verify transmitted data
//! let transmitted = mock_uart.get_transmitted();
//! assert!(transmitted.contains("120.3"));
//! ```

#![cfg(all(test, not(target_arch = "riscv32")))]
#![allow(non_snake_case)]

extern crate std;

use std::println;
use std::string::String;
use std::vec;
use std::vec::Vec;

use libreroaster::hardware::uart::{UartDriver, UartError};

/// Mock implementation of UartDriver for testing
///
/// Simulates UART communication for Artisan+ protocol testing
/// without requiring actual hardware.
pub struct MockUartDriver {
    /// Incoming data buffer (simulates RX)
    rx_buffer: Vec<u8>,
    /// Outgoing data buffer (simulates TX)
    tx_buffer: Vec<u8>,
    /// Current read position in RX buffer
    read_index: usize,
    /// Whether we've seen EOF marker
    eof: bool,
}

impl MockUartDriver {
    /// Create a new MockUartDriver with initial RX data
    ///
    /// # Arguments
    ///
    /// * `rx_data` - Initial data to place in RX buffer
    ///
    /// # Example
    ///
    /// ```rust
    /// let mock = MockUartDriver::new("READ\r\n");
    /// ```
    pub fn new(rx_data: &str) -> Self {
        Self {
            rx_buffer: rx_data.as_bytes().to_vec(),
            tx_buffer: Vec::new(),
            read_index: 0,
            eof: false,
        }
    }

    /// Get the received data as a String
    pub fn get_received_data(&self) -> String {
        String::from_utf8_lossy(&self.rx_buffer).to_string()
    }

    /// Get transmitted data as a String
    pub fn get_transmitted(&self) -> String {
        String::from_utf8_lossy(&self.tx_buffer).to_string()
    }

    /// Check if there's more data available to read
    pub fn has_data(&self) -> bool {
        self.read_index < self.rx_buffer.len() || (!self.rx_buffer.is_empty() && !self.eof)
    }

    /// Get remaining data length
    pub fn remaining_data_len(&self) -> usize {
        self.rx_buffer.len().saturating_sub(self.read_index)
    }

    /// Clear both buffers
    pub fn clear(&mut self) {
        self.rx_buffer.clear();
        self.tx_buffer.clear();
        self.read_index = 0;
        self.eof = false;
    }

    /// Add more RX data (for simulating streaming input)
    pub fn add_rx_data(&mut self, data: &str) {
        self.rx_buffer.extend(data.as_bytes());
    }

    /// Get number of bytes transmitted
    pub fn tx_byte_count(&self) -> usize {
        self.tx_buffer.len()
    }

    /// Get number of bytes received (available to read)
    pub fn rx_byte_count(&self) -> usize {
        self.rx_buffer.len()
    }
}

/// Implement mock read_bytes method
impl MockUartDriver {
    /// Read bytes from the RX buffer
    ///
    /// # Arguments
    ///
    /// * `buffer` - Buffer to read into
    ///
    /// # Returns
    ///
    /// Number of bytes read, or error
    pub fn read_bytes(&mut self, buffer: &mut [u8]) -> Result<usize, UartError> {
        if self.read_index >= self.rx_buffer.len() {
            // No more data available
            return Err(UartError::ReceptionError);
        }

        let available = self.rx_buffer.len() - self.read_index;
        let to_read = buffer.len().min(available);

        buffer[..to_read]
            .copy_from_slice(&self.rx_buffer[self.read_index..self.read_index + to_read]);
        self.read_index += to_read;

        Ok(to_read)
    }
}

/// Implement mock write_bytes method
impl MockUartDriver {
    /// Write bytes to the TX buffer
    ///
    /// # Arguments
    ///
    /// * `data` - Data to write
    ///
    /// # Returns
    ///
    /// Success or error
    pub fn write_bytes(&mut self, data: &[u8]) -> Result<(), UartError> {
        self.tx_buffer.extend(data);
        Ok(())
    }
}

// ============================================================================
// Integration Tests
// ============================================================================

/// TEST-MOCK-01: Basic mock UART functionality
#[test]
fn test_mock_uart_basic() {
    println!("TEST-MOCK-01: Basic mock UART functionality");

    // Create mock with initial data
    let mock = MockUartDriver::new("READ\r\n");

    // Verify initial state
    assert_eq!(mock.rx_byte_count(), 6, "Should have 6 bytes received");
    assert_eq!(mock.tx_byte_count(), 0, "Should have no transmitted bytes");
    assert!(mock.has_data(), "Should have data available");
    assert_eq!(
        mock.remaining_data_len(),
        6,
        "Should have 6 bytes remaining"
    );

    // Verify received data
    let received = mock.get_received_data();
    assert_eq!(received, "READ\r\n", "Should have correct received data");

    println!("   ✅ Mock UART created with correct initial state");
}

/// TEST-MOCK-02: Read bytes from mock UART
#[test]
fn test_mock_uart_read_bytes() {
    println!("TEST-MOCK-02: Read bytes from mock UART");

    let mut mock = MockUartDriver::new("READ\r\n");

    // Read first 4 bytes
    let mut buffer = [0u8; 4];
    let bytes_read = mock.read_bytes(&mut buffer).unwrap();
    assert_eq!(bytes_read, 4, "Should read 4 bytes");
    assert_eq!(&buffer[..4], b"READ", "Should have 'READ'");

    // Read remaining 2 bytes
    let mut buffer2 = [0u8; 4];
    let bytes_read2 = mock.read_bytes(&mut buffer2).unwrap();
    assert_eq!(bytes_read2, 2, "Should read 2 bytes");
    assert_eq!(&buffer2[..2], b"\r\n", "Should have '\\r\\n'");

    // Should have no more data
    assert!(!mock.has_data(), "Should have no more data");
    assert_eq!(
        mock.remaining_data_len(),
        0,
        "Should have 0 bytes remaining"
    );

    // Attempting to read more should error
    let mut buffer3 = [0u8; 4];
    let result = mock.read_bytes(&mut buffer3);
    assert!(result.is_err(), "Reading empty buffer should error");

    println!("   ✅ Read bytes functionality works correctly");
}

/// TEST-MOCK-03: Write bytes to mock UART
#[test]
fn test_mock_uart_write_bytes() {
    println!("TEST-MOCK-03: Write bytes to mock UART");

    let mut mock = MockUartDriver::new("");

    // Write response
    let response = b"120.3,150.5,75.0,25.0\r\n";
    let result = mock.write_bytes(response);
    assert!(result.is_ok(), "Write should succeed");

    // Verify transmitted
    assert_eq!(
        mock.tx_byte_count(),
        response.len(),
        "Should have correct byte count"
    );
    let transmitted = mock.get_transmitted();
    assert_eq!(
        transmitted, "120.3,150.5,75.0,25.0\r\n",
        "Should have correct transmitted data"
    );

    // Write more data
    mock.write_bytes(b"OK\r\n").unwrap();
    assert_eq!(
        mock.tx_byte_count(),
        response.len() + 4,
        "Should accumulate bytes"
    );

    println!("   ✅ Write bytes functionality works correctly");
}

/// TEST-MOCK-04: Command → response flow simulation
#[test]
fn test_command_response_flow() {
    println!("TEST-MOCK-04: Command → response flow simulation");

    use libreroaster::input::parser::parse_artisan_command;
    use libreroaster::output::artisan::ArtisanFormatter;
    use libreroaster::output::OutputFormatter;

    // Step 1: Artisan sends READ command
    let mut mock = MockUartDriver::new("READ\r\n");
    println!("   Step 1: Artisan sends 'READ\\r\\n'");

    // Step 2: System reads command
    let mut buffer = [0u8; 64];
    let bytes_read = mock.read_bytes(&mut buffer).unwrap();
    let command_str = core::str::from_utf8(&buffer[..bytes_read]).unwrap();
    assert_eq!(command_str, "READ\r\n", "Should receive correct command");
    println!("   Step 2: System reads command: {:?}", command_str);

    // Step 3: Parse command
    let parse_result = parse_artisan_command(command_str);
    assert!(parse_result.is_ok(), "Command should parse");
    println!("   Step 3: Command parsed: {:?}", parse_result.unwrap());

    // Step 4: Format response
    let status = create_mock_status();
    let response = ArtisanFormatter::format_read_response(&status, 25.0);
    let response_bytes = response.as_bytes();
    println!("   Step 4: Response formatted: {}", response);

    // Step 5: Transmit response
    mock.write_bytes(response_bytes).unwrap();
    mock.write_bytes(b"\r\n").unwrap(); // Add terminator
    let transmitted = mock.get_transmitted();
    assert!(transmitted.contains("120.3"), "Should contain ET");
    assert!(transmitted.contains("150.5"), "Should contain BT");
    println!("   Step 5: Response transmitted: {}", transmitted);

    println!("   ✅ Complete command → response flow verified");
}

/// TEST-MOCK-05: Multiple commands in sequence
#[test]
fn test_multiple_commands() {
    println!("TEST-MOCK-05: Multiple commands in sequence");

    use libreroaster::input::parser::parse_artisan_command;

    // Simulate multiple commands
    let mut mock = MockUartDriver::new("READ\r\nOT1 75\r\nIO3 50\r\n");

    let expected_commands = ["READ", "OT1 75", "IO3 50"];

    for (i, expected) in expected_commands.iter().enumerate() {
        // Read command
        let mut buffer = [0u8; 64];
        let bytes_read = mock.read_bytes(&mut buffer).unwrap();
        let command = core::str::from_utf8(&buffer[..bytes_read]).unwrap();

        // Should have command plus \r\n
        let command_trimmed = command.trim_end();
        assert!(
            command.starts_with(expected),
            "Command {} should start with '{}', got '{}'",
            i + 1,
            expected,
            command
        );

        // Parse command
        let result = parse_artisan_command(command_trimmed);
        assert!(result.is_ok(), "Command {} should parse", i + 1);

        println!("   ✅ Command {} processed: '{}'", i + 1, command_trimmed);
    }

    assert!(!mock.has_data(), "All commands should be consumed");

    println!("   ✅ Multiple commands handled correctly");
}

/// TEST-MOCK-06: Mock UART with streaming data
#[test]
fn test_mock_uart_streaming() {
    println!("TEST-MOCK-06: Mock UART with streaming data");

    let mut mock = MockUartDriver::new("READ");

    // Initially has data
    assert!(mock.has_data(), "Should have initial data");

    // Read partial
    let mut buffer = [0u8; 2];
    let bytes_read = mock.read_bytes(&mut buffer).unwrap();
    assert_eq!(bytes_read, 2, "Should read 2 bytes");
    assert_eq!(buffer[..bytes_read], b"RE", "Should have 'RE'");

    // Add more data (simulating streaming)
    mock.add_rx_data("\r\nOT1 50");

    // Should still have data
    assert!(mock.has_data(), "Should have more data after streaming");

    // Read next chunk
    let mut buffer2 = [0u8; 8];
    let bytes_read2 = mock.read_bytes(&mut buffer2).unwrap();
    assert_eq!(bytes_read2, 8, "Should read remaining and new data");
    assert_eq!(
        core::str::from_utf8(&buffer2[..bytes_read2]).unwrap(),
        "AD\r\nOT1 5"
    );

    println!("   ✅ Streaming data simulation works correctly");
}

/// Helper to create mock SystemStatus
fn create_mock_status() -> libreroaster::config::SystemStatus {
    libreroaster::config::SystemStatus {
        state: libreroaster::config::RoasterState::Stable,
        bean_temp: 150.5,
        env_temp: 120.3,
        target_temp: 200.0,
        ssr_output: 75.0,
        fan_output: 50.0,
        pid_enabled: true,
        artisan_control: false,
        fault_condition: false,
        ssr_hardware_status: libreroaster::config::SsrHardwareStatus::Available,
    }
}

/// TEST-MOCK-07: Buffer management
#[test]
fn test_mock_uart_buffer_management() {
    println!("TEST-MOCK-07: Buffer management");

    let mut mock = MockUartDriver::new("TEST");

    // Initial state
    assert_eq!(mock.rx_byte_count(), 4);
    assert_eq!(mock.tx_byte_count(), 0);

    // Write some data
    mock.write_bytes(b"RESP").unwrap();
    assert_eq!(mock.tx_byte_count(), 4);

    // Clear buffers
    mock.clear();

    // Verify cleared
    assert_eq!(mock.rx_byte_count(), 0);
    assert_eq!(mock.tx_byte_count(), 0);
    assert!(!mock.has_data(), "Should have no data after clear");

    println!("   ✅ Buffer management works correctly");
}
