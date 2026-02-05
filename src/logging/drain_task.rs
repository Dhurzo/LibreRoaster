//! UART Drain Task - Logging to UART0
//!
//! This module provides channel-prefixed logging that outputs directly to UART0.
//!
//! ## Architecture
//!
//! Instead of a complex defmt-rtt + drain task pipeline, this implementation uses
//! esp_println::println! directly in the log_channel! macro:
//!
//! ```rust
//! #[macro_export]
//! macro_rules! log_channel {
//!     ($channel:expr, $($arg:tt)*) => {
//!         esp_println::println!("[{}] {}", $channel, format_args!($($arg)*))
//!     };
//! }
//! ```
//!
//! ## Why This Approach?
//!
//! **defmt-rtt limitations:**
//! - RTT (Real Time Transfer) is designed for host-side reading via debugger
//! - No public API for embedded device to read from RTT buffer
//! - Complex integration requiring access to internal RTT control block
//!
//! **esp_println advantages:**
//! - Direct UART0 output at 115200 baud
//! - No buffering or drain task needed
//! - Simple, reliable, well-tested
//! - GPIO20 (TX), GPIO21 (RX)
//!
//! ## Usage
//!
//! ```rust
//! use crate::logging::channel::Channel;
//! use crate::log_channel;
//!
//! log_channel!(Channel::Usb, "RX: READ");
//! log_channel!(Channel::Uart, "TX: 185.2,192.3,...");
//! ```
//!
//! ## Output Examples
//!
//! ```text
//! [USB] RX: READ
//! [USB] TX: 185.2,192.3,-1.0,-1.0,24.5,45,75
//! [SYSTEM] Temperature: 185.5°C
//! ```
//!
//! ## Performance
//!
//! - Blocking call (10-100μs per log)
//! - Suitable for debugging and development
//! - For production with strict timing, consider bbqueue + async drain task

pub use crate::log_channel;
pub use crate::logging::channel::Channel;

pub mod channel {
    pub use crate::logging::channel::Channel;
}
