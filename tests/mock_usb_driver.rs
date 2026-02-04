//! Mock USB CDC driver for integration testing
//!
//! Provides a software implementation of the USB CDC interface for testing
//! the Artisan+ protocol without requiring actual ESP32-C3 hardware.
//!
//! # Features
//!
//! - Simulates USB CDC RX buffer for incoming Artisan commands
//! - Simulates USB CDC TX buffer for outgoing responses
//! - Tracks transmission state and data
//! - Enables testing complete command → response flow
//! - Supports testing error conditions (buffer overflow, transmission errors)
//!
//! # Usage Example
//!
//! ```rust
//! use libreroaster::hardware::usb_cdc::driver::UsbCdcDriver;
//! use crate::MockUsbCdcDriver;
//!
//! // Create mock with incoming command
//! let mut mock_usb = MockUsbCdcDriver::new();
//!
//! // Simulate receiving READ command
//! mock_usb.push_rx_data(b"READ\r\n");
//!
//! // Read command from buffer
//! let mut buffer = [0u8; 64];
//! let bytes_read = mock_usb.read_bytes(&mut buffer).unwrap();
//! let command = core::str::from_utf8(&buffer[..bytes_read]).unwrap();
//! assert_eq!(command, "READ\r\n");
//!
//! // Write response
//! mock_usb.write_bytes(b"120.3,150.5,75.0,25.0\r\n").unwrap();
//!
//! // Verify transmitted data
//! let transmitted = mock_usb.get_transmitted();
//! assert!(transmitted.contains("120.3"));
//! ```

#![cfg(all(test, not(target_arch = "riscv32")))]
#![allow(non_snake_case)]

extern crate std;

use std::println;
use std::string::String;
use std::vec::Vec;

use libreroaster::hardware::usb_cdc::driver::{UsbCdcDriver, UsbCdcError};

/// Mock implementation of UsbCdcDriver for testing
///
/// Simulates USB CDC communication for Artisan+ protocol testing
/// without requiring actual ESP32-C3 hardware.
pub struct MockUsbCdcDriver {
    /// Incoming data buffer (simulates RX)
    rx_buffer: Vec<u8>,
    /// Outgoing data buffer (simulates TX)
    tx_buffer: Vec<u8>,
    /// Current read position in RX buffer
    read_index: usize,
    /// Whether transmission has failed
    transmit_error: bool,
    /// Whether reception has failed
    receive_error: bool,
    /// Track connection state
    connected: bool,
    /// Track if buffer overflow should occur
    overflow_on_next_read: bool,
    /// Maximum buffer size for overflow testing
    max_buffer_size: usize,
}

impl MockUsbCdcDriver {
    /// Create a new MockUsbCdcDriver
    ///
    /// # Arguments
    ///
    /// * `max_rx_size` - Maximum RX buffer size (default 256 for USB CDC)
    ///
    /// # Example
    ///
    /// ```rust
    /// let mock = MockUsbCdcDriver::new();
    /// ```
    pub fn new() -> Self {
        Self {
            rx_buffer: Vec::new(),
            tx_buffer: Vec::new(),
            read_index: 0,
            transmit_error: false,
            receive_error: false,
            connected: true,
            overflow_on_next_read: false,
            max_buffer_size: 256,
        }
    }

    /// Create with initial RX data
    pub fn with_data(rx_data: &str) -> Self {
        Self {
            rx_buffer: rx_data.as_bytes().to_vec(),
            tx_buffer: Vec::new(),
            read_index: 0,
            transmit_error: false,
            receive_error: false,
            connected: true,
            overflow_on_next_read: false,
            max_buffer_size: 256,
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
        self.read_index < self.rx_buffer.len()
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
        self.transmit_error = false;
        self.receive_error = false;
        self.connected = true;
    }

    /// Add more RX data (for simulating streaming input)
    pub fn push_rx_data(&mut self, data: &str) {
        self.rx_buffer.extend(data.as_bytes());
    }

    /// Add more RX data as bytes
    pub fn push_rx_bytes(&mut self, data: &[u8]) {
        self.rx_buffer.extend(data);
    }

    /// Get number of bytes transmitted
    pub fn tx_byte_count(&self) -> usize {
        self.tx_buffer.len()
    }

    /// Get number of bytes received (available to read)
    pub fn rx_byte_count(&self) -> usize {
        self.rx_buffer.len()
    }

    /// Simulate connection state
    pub fn set_connected(&mut self, connected: bool) {
        self.connected = connected;
    }

    /// Simulate transmission error on next write
    pub fn set_transmit_error(&mut self, error: bool) {
        self.transmit_error = error;
    }

    /// Simulate reception error on next read
    pub fn set_receive_error(&mut self, error: bool) {
        self.receive_error = error;
    }

    /// Simulate buffer overflow on next read
    pub fn set_overflow(&mut self, overflow: bool) {
        self.overflow_on_next_read = overflow;
    }

    /// Set maximum buffer size for overflow testing
    pub fn set_max_buffer_size(&mut self, size: usize) {
        self.max_buffer_size = size;
    }

    /// Inject a large payload to test buffer overflow
    pub fn inject_large_payload(&mut self, size: usize) {
        self.rx_buffer.clear();
        self.rx_buffer.resize(size, b'A');
        self.read_index = 0;
    }
}

/// Implement mock read_bytes method
impl MockUsbCdcDriver {
    /// Read bytes from the RX buffer
    ///
    /// # Arguments
    ///
    /// * `buffer` - Buffer to read into
    ///
    /// # Returns
    ///
    /// Number of bytes read, or error
    pub fn read_bytes(&mut self, buffer: &mut [u8]) -> Result<usize, UsbCdcError> {
        // Check for simulated error
        if self.receive_error {
            return Err(UsbCdcError::ReceptionError);
        }

        if self.read_index >= self.rx_buffer.len() {
            // No more data available - return EOF condition
            return Ok(0);
        }

        let available = self.rx_buffer.len() - self.read_index;
        let to_read = buffer.len().min(available);

        // Check for overflow condition
        if self.overflow_on_next_read && self.rx_buffer.len() > self.max_buffer_size {
            self.overflow_on_next_read = false;
            return Err(UsbCdcError::BufferOverflow);
        }

        buffer[..to_read]
            .copy_from_slice(&self.rx_buffer[self.read_index..self.read_index + to_read]);
        self.read_index += to_read;

        Ok(to_read)
    }
}

/// Implement mock write_bytes method
impl MockUsbCdcDriver {
    /// Write bytes to the TX buffer
    ///
    /// # Arguments
    ///
    /// * `data` - Data to write
    ///
    /// # Returns
    ///
    /// Success or error
    pub fn write_bytes(&mut self, data: &[u8]) -> Result<(), UsbCdcError> {
        // Check for simulated error
        if self.transmit_error {
            return Err(UsbCdcError::TransmissionError);
        }

        self.tx_buffer.extend(data);
        Ok(())
    }
}

/// Implement mock is_connected method
impl MockUsbCdcDriver {
    /// Check if USB CDC is connected
    pub fn is_connected_mock(&self) -> bool {
        self.connected
    }
}

// ============================================================================
// ============================================================================
// INTEGRATION TESTS
// ============================================================================
// ============================================================================

/// TEST-USB-MOCK-01: Basic mock USB CDC functionality
#[test]
fn test_mock_usb_basic() {
    println!("TEST-USB-MOCK-01: Basic mock USB CDC functionality");

    // Create mock with initial data
    let mut mock = MockUsbCdcDriver::new();
    mock.push_rx_data("READ\r\n");

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

    println!("   ✅ Mock USB CDC created with correct initial state");
}

/// TEST-USB-MOCK-02: Read bytes from mock USB CDC
#[test]
fn test_mock_usb_read_bytes() {
    println!("TEST-USB-MOCK-02: Read bytes from mock USB CDC");

    let mut mock = MockUsbCdcDriver::with_data("READ\r\n");

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

    // Reading empty should return Ok(0) not error
    let mut buffer3 = [0u8; 4];
    let result = mock.read_bytes(&mut buffer3);
    assert!(result.is_ok(), "Reading empty buffer should return Ok(0)");
    assert_eq!(result.unwrap(), 0, "Should read 0 bytes");

    println!("   ✅ Read bytes functionality works correctly");
}

/// TEST-USB-MOCK-03: Write bytes to mock USB CDC
#[test]
fn test_mock_usb_write_bytes() {
    println!("TEST-USB-MOCK-03: Write bytes to mock USB CDC");

    let mut mock = MockUsbCdcDriver::new();

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

/// TEST-USB-MOCK-04: Complete command → response flow simulation
#[test]
fn test_usb_command_response_flow() {
    println!("TEST-USB-MOCK-04: Complete command → response flow simulation");

    use libreroaster::input::parser::parse_artisan_command;
    use libreroaster::output::artisan::ArtisanFormatter;
    use libreroaster::output::OutputFormatter;

    // Step 1: Artisan sends READ command via USB
    let mut mock = MockUsbCdcDriver::with_data("READ\r\n");
    println!("   Step 1: Artisan sends 'READ\\r\\n' via USB CDC");

    // Step 2: System reads command
    let mut buffer = [0u8; 64];
    let bytes_read = mock.read_bytes(&mut buffer).unwrap();
    let command_str = core::str::from_utf8(&buffer[..bytes_read]).unwrap();
    assert_eq!(command_str, "READ\r\n", "Should receive correct command");
    println!("   Step 2: System reads command: {:?}", command_str);

    // Step 3: Parse command
    let parse_result = parse_artisan_command(command_str.trim_end());
    assert!(parse_result.is_ok(), "Command should parse");
    println!("   Step 3: Command parsed: {:?}", parse_result.unwrap());

    // Step 4: Format response
    let status = create_mock_status();
    let response = ArtisanFormatter::format_read_response(&status, 25.0);
    let response_bytes = response.as_bytes();
    println!("   Step 4: Response formatted: {}", response);

    // Step 5: Transmit response via USB CDC
    mock.write_bytes(response_bytes).unwrap();
    mock.write_bytes(b"\r\n").unwrap(); // Add terminator
    let transmitted = mock.get_transmitted();
    assert!(transmitted.contains("120.3"), "Should contain ET");
    assert!(transmitted.contains("150.5"), "Should contain BT");
    println!(
        "   Step 5: Response transmitted via USB CDC: {}",
        transmitted
    );

    println!("   ✅ Complete USB CDC command → response flow verified");
}

/// TEST-USB-MOCK-05: Multiple commands in sequence via USB
#[test]
fn test_usb_multiple_commands() {
    println!("TEST-USB-MOCK-05: Multiple commands in sequence via USB");

    use libreroaster::input::parser::parse_artisan_command;

    // Simulate multiple commands via USB CDC (one at a time)
    let mut mock = MockUsbCdcDriver::with_data("READ\r\n");

    // Process first command
    let mut buffer = [0u8; 64];
    let bytes_read = mock.read_bytes(&mut buffer).unwrap();
    let command = core::str::from_utf8(&buffer[..bytes_read]).unwrap();
    let command_trimmed = command.trim_end();
    assert!(
        command_trimmed.starts_with("READ"),
        "First command should be READ"
    );
    let result = parse_artisan_command(command_trimmed);
    assert!(result.is_ok(), "READ command should parse");
    println!("   ✅ Command 1 processed: 'READ'");

    // Second command
    mock.push_rx_data("OT1 75\r\n");
    let bytes_read = mock.read_bytes(&mut buffer).unwrap();
    let command = core::str::from_utf8(&buffer[..bytes_read]).unwrap();
    let command_trimmed = command.trim_end();
    assert!(
        command_trimmed.starts_with("OT1"),
        "Second command should be OT1"
    );
    let result = parse_artisan_command(command_trimmed);
    assert!(result.is_ok(), "OT1 75 command should parse");
    println!("   ✅ Command 2 processed: 'OT1 75'");

    // Third command
    mock.push_rx_data("IO3 50\r\n");
    let bytes_read = mock.read_bytes(&mut buffer).unwrap();
    let command = core::str::from_utf8(&buffer[..bytes_read]).unwrap();
    let command_trimmed = command.trim_end();
    assert!(
        command_trimmed.starts_with("IO3"),
        "Third command should be IO3"
    );
    let result = parse_artisan_command(command_trimmed);
    assert!(result.is_ok(), "IO3 50 command should parse");
    println!("   ✅ Command 3 processed: 'IO3 50'");

    assert!(!mock.has_data(), "All commands should be consumed");

    println!("   ✅ Multiple USB commands handled correctly");
}

/// TEST-USB-MOCK-06: USB CDC with streaming data
#[test]
fn test_usb_streaming_data() {
    println!("TEST-USB-MOCK-06: USB CDC with streaming data");

    let mut mock = MockUsbCdcDriver::with_data("READ");

    // Initially has data
    assert!(mock.has_data(), "Should have initial data");

    // Read partial
    let mut buffer = [0u8; 2];
    let bytes_read = mock.read_bytes(&mut buffer).unwrap();
    assert_eq!(bytes_read, 2, "Should read 2 bytes");
    assert_eq!(&buffer[..bytes_read], b"RE", "Should have 'RE'");

    // Add more data (simulating streaming)
    mock.push_rx_data("\r\nOT1 50");

    // Should still have data
    assert!(mock.has_data(), "Should have more data after streaming");

    // Read next chunk (8 bytes: "AD\r\nOT1 ")
    let mut buffer2 = [0u8; 8];
    let bytes_read2 = mock.read_bytes(&mut buffer2).unwrap();
    assert_eq!(bytes_read2, 8, "Should read remaining and new data");
    assert_eq!(
        core::str::from_utf8(&buffer2[..bytes_read2]).unwrap(),
        "AD\r\nOT1 "
    );

    // Verify remaining data
    assert_eq!(
        mock.remaining_data_len(),
        2,
        "Should have 2 bytes remaining"
    );

    println!("   ✅ Streaming data simulation works correctly");
}

/// TEST-USB-MOCK-07: USB CDC error conditions
#[test]
fn test_usb_error_conditions() {
    println!("TEST-USB-MOCK-07: USB CDC error conditions");

    // Test transmission error
    let mut mock = MockUsbCdcDriver::new();
    mock.set_transmit_error(true);
    let result = mock.write_bytes(b"TEST\r\n");
    assert!(result.is_err(), "Should error when transmit error set");
    assert_eq!(result.unwrap_err(), UsbCdcError::TransmissionError);

    // Test reception error
    let mut mock2 = MockUsbCdcDriver::new();
    mock2.push_rx_data("TEST\r\n");
    mock2.set_receive_error(true);
    let mut buffer = [0u8; 64];
    let result = mock2.read_bytes(&mut buffer);
    assert!(result.is_err(), "Should error when receive error set");
    assert_eq!(result.unwrap_err(), UsbCdcError::ReceptionError);

    println!("   ✅ Error conditions handled correctly");
}

/// TEST-USB-MOCK-08: USB CDC buffer overflow handling
#[test]
fn test_usb_buffer_overflow() {
    println!("TEST-USB-MOCK-08: USB CDC buffer overflow handling");

    let mut mock = MockUsbCdcDriver::new();
    mock.set_max_buffer_size(64);

    // Inject payload larger than max buffer
    mock.inject_large_payload(128);

    assert_eq!(mock.rx_byte_count(), 128, "Should have 128 bytes");

    // Reading should trigger overflow error
    mock.set_overflow(true);
    let mut buffer = [0u8; 256];
    let result = mock.read_bytes(&mut buffer);
    assert!(result.is_err(), "Should error on buffer overflow");
    assert_eq!(result.unwrap_err(), UsbCdcError::BufferOverflow);

    println!("   ✅ Buffer overflow handling works correctly");
}

/// TEST-USB-MOCK-09: USB CDC connection state
#[test]
fn test_usb_connection_state() {
    println!("TEST-USB-MOCK-09: USB CDC connection state");

    let mut mock = MockUsbCdcDriver::new();

    // Default connected
    assert!(mock.is_connected_mock(), "Should be connected by default");

    // Simulate disconnect
    mock.set_connected(false);
    assert!(!mock.is_connected_mock(), "Should be disconnected");

    // Reconnect
    mock.set_connected(true);
    assert!(mock.is_connected_mock(), "Should be connected again");

    println!("   ✅ Connection state management works correctly");
}

/// TEST-USB-MOCK-10: USB CDC buffer management
#[test]
fn test_usb_buffer_management() {
    println!("TEST-USB-MOCK-10: USB CDC buffer management");

    let mut mock = MockUsbCdcDriver::with_data("TEST");

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
    assert!(
        mock.is_connected_mock(),
        "Should still be connected after clear"
    );

    println!("   ✅ Buffer management works correctly");
}

/// TEST-USB-MOCK-11: Partial reads across buffer boundaries
#[test]
fn test_usb_partial_reads() {
    println!("TEST-USB-MOCK-11: Partial reads across buffer boundaries");

    let mut mock = MockUsbCdcDriver::with_data("READ\r\n");
    let mut all_data = Vec::new();

    // Read in small chunks
    let mut buffer = [0u8; 2];
    while mock.has_data() {
        let bytes_read = mock.read_bytes(&mut buffer).unwrap();
        if bytes_read > 0 {
            all_data.extend_from_slice(&buffer[..bytes_read]);
        }
    }

    let result = core::str::from_utf8(&all_data).unwrap();
    assert_eq!(result, "READ\r\n", "Should reconstruct complete data");

    println!("   ✅ Partial reads correctly reconstruct data");
}

/// TEST-USB-MOCK-12: Zero-length operations
#[test]
fn test_usb_zero_length_operations() {
    println!("TEST-USB-MOCK-12: Zero-length operations");

    let mut mock = MockUsbCdcDriver::new();

    // Zero-length write should succeed
    let result = mock.write_bytes(&[]);
    assert!(result.is_ok(), "Zero-length write should succeed");
    assert_eq!(mock.tx_byte_count(), 0);

    // Zero-length read should return Ok(0)
    let mut buffer = [0u8; 4];
    let result = mock.read_bytes(&mut buffer);
    assert!(result.is_ok(), "Zero-length read should succeed");
    assert_eq!(result.unwrap(), 0);

    println!("   ✅ Zero-length operations handled correctly");
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
