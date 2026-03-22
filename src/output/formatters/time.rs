// Time formatting for Artisan protocol
//
// This module handles time string formatting for Artisan protocol responses.
// All formatting operations use heapless types to ensure predictable memory usage.
//
// # Memory Strategy
//
// - `TIME_FORMAT_SIZE`: Time formatting (8 chars)
// - No dynamic memory allocation during formatting
//
// ## Memory Usage
//
// - Formatting buffer: TIME_FORMAT_SIZE chars

use crate::memory::TIME_FORMAT_SIZE;
use core::fmt::Write;
use heapless::String as HeaplessString;

/// Time formatter for Artisan protocol
pub struct TimeFormatter;

impl TimeFormatter {
    /// Format time as seconds with 2-digit millisecond precision
    ///
    /// Format: "seconds.MM" where MM is milliseconds / 10
    /// Example: "123.45" (123 seconds, 45 centiseconds)
    ///
    /// # Arguments
    ///
    /// * `elapsed_secs` - Elapsed seconds since roast start
    /// * `elapsed_ms` - Milliseconds within the current second (0-999)
    ///
    /// # Returns
    ///
    /// Formatted time string with heapless allocation
    pub fn format_time(elapsed_secs: u64, elapsed_ms: u64) -> HeaplessString<TIME_FORMAT_SIZE> {
        let mut buf = HeaplessString::<TIME_FORMAT_SIZE>::new();
        let _ = core::write!(&mut buf, "{}.{:02}", elapsed_secs, elapsed_ms / 10);
        buf
    }
}

impl Default for TimeFormatter {
    fn default() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_time_zero() {
        let time_str = TimeFormatter::format_time(0, 0);
        assert_eq!(time_str.as_str(), "0.00");
    }

    #[test]
    fn test_format_time_seconds_only() {
        let time_str = TimeFormatter::format_time(123, 0);
        assert_eq!(time_str.as_str(), "123.00");
    }

    #[test]
    fn test_format_time_with_milliseconds() {
        let time_str = TimeFormatter::format_time(45, 456);
        assert_eq!(time_str.as_str(), "45.45");
    }

    #[test]
    fn test_format_time_carry_over() {
        let time_str = TimeFormatter::format_time(60, 999);
        assert_eq!(time_str.as_str(), "60.99");
    }

    #[test]
    fn test_format_time_large_value() {
        let time_str = TimeFormatter::format_time(3600, 234);
        assert_eq!(time_str.as_str(), "3600.23");
    }
}
