//! Channel-prefixed logging macros
//!
//! Provides `log_channel!` macro for adding [USB], [UART], [SYSTEM] prefixes to logs.
//! Uses esp_println for direct UART0 output.

#[derive(Clone, Copy, Debug)]
pub enum Channel {
    Usb,
    Uart,
    System,
}

impl core::fmt::Display for Channel {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Channel::Usb => write!(f, "USB"),
            Channel::Uart => write!(f, "UART"),
            Channel::System => write!(f, "SYSTEM"),
        }
    }
}

#[macro_export]
macro_rules! log_channel {
    ($channel:expr, $($arg:tt)*) => {
        esp_println::println!("[{}] {}", $channel, format_args!($($arg)*))
    };
}
