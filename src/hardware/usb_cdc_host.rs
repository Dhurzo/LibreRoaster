//! Stub USB CDC implementation for native testing
//!
//! This module provides stub implementations for USB CDC functionality
//! when running on native (non-embedded) targets for testing purposes.

#![cfg(target_arch = "x86_64")]

use core::fmt;

/// Stub error type for USB CDC operations on native targets
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UsbCdcError {
    TransmissionError,
    ReceptionError,
    BufferOverflow,
    NotInitialized,
    NotSupported,
}

impl fmt::Display for UsbCdcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UsbCdcError::TransmissionError => write!(f, "USB CDC transmission error"),
            UsbCdcError::ReceptionError => write!(f, "USB CDC reception error"),
            UsbCdcError::BufferOverflow => write!(f, "USB CDC buffer overflow"),
            UsbCdcError::NotInitialized => write!(f, "USB CDC not initialized"),
            UsbCdcError::NotSupported => write!(f, "USB CDC not supported in this configuration"),
        }
    }
}

/// Stub driver for USB CDC on native targets
pub struct UsbCdcDriver;

impl UsbCdcDriver {
    /// Create a new stub driver
    pub fn new() -> Result<Self, UsbCdcError> {
        Ok(Self)
    }

    /// Stub write - always succeeds
    pub async fn write_bytes(&mut self, _data: &[u8]) -> Result<(), UsbCdcError> {
        Ok(())
    }

    /// Stub read - always returns 0 bytes
    pub async fn read_bytes(&mut self, _buffer: &mut [u8]) -> Result<usize, UsbCdcError> {
        Ok(0)
    }

    /// Stub connection check - always returns false
    pub fn is_connected(&self) -> bool {
        false
    }
}

static mut USB_CDC_INSTANCE: Option<UsbCdcDriver> = None;

/// Initialize USB CDC (stub)
pub fn init_usb_cdc(_usb: ()) -> Result<(), UsbCdcError> {
    Ok(())
}

/// Get USB CDC driver (stub - always returns None)
pub fn get_usb_cdc_driver() -> Option<&'static mut UsbCdcDriver> {
    None
}
