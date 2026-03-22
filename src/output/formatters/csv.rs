// CSV formatter for Artisan protocol responses
//
// This module handles CSV line formatting for Artisan protocol.
// All formatting operations use heapless types to ensure predictable memory usage.
//
// # Memory Strategy
//
// - `REPORT_BUFFER_SIZE`: CSV line formatting (32 chars)
// - No dynamic memory allocation during formatting
//
// ## Memory Usage
//
// - CSV buffer: REPORT_BUFFER_SIZE chars

use crate::memory::REPORT_BUFFER_SIZE;
use core::fmt::Write;
use heapless::String as HeaplessString;

/// CSV formatter for Artisan protocol lines
pub struct CsvFormatter;

impl CsvFormatter {
    /// Format Artisan CSV line with time, ET, BT, ROR, and gas values
    ///
    /// Format: "time,ET,BT,ROR,gas"
    ///
    /// # Arguments
    ///
    /// * `time_str` - Formatted time string
    /// * `et` - Environment temperature
    /// * `bt` - Bean temperature
    /// * `ror` - Rate of Rise
    /// * `gas` - SSR output percentage (heater control)
    ///
    /// # Returns
    ///
    /// Formatted CSV line with heapless allocation
    pub fn format_artisan_line(
        time_str: &str,
        et: f32,
        bt: f32,
        ror: f32,
        gas: f32,
    ) -> HeaplessString<REPORT_BUFFER_SIZE> {
        let mut buf = HeaplessString::<REPORT_BUFFER_SIZE>::new();
        let _ = core::write!(
            &mut buf,
            "{},{:.1},{:.1},{:.2},{:.1}",
            time_str,
            et,
            bt,
            ror,
            gas
        );
        buf
    }

    /// Normalize a read value to prevent NaN or Infinity
    ///
    /// Returns 0.0 if value is not finite, otherwise returns value unchanged
    ///
    /// # Arguments
    ///
    /// * `value` - Value to normalize
    ///
    /// # Returns
    ///
    /// Normalized value (0.0 if not finite, otherwise value)
    pub fn normalize_read_value(value: f32) -> f32 {
        if value.is_finite() {
            value
        } else {
            0.0
        }
    }
}

impl Default for CsvFormatter {
    fn default() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_artisan_line() {
        let line = CsvFormatter::format_artisan_line("123.45", 150.5, 175.3, 2.5, 50.0);
        assert_eq!(line.as_str(), "123.45,150.5,175.3,2.50,50.0");
    }

    #[test]
    fn test_format_artisan_line_with_negative_ror() {
        let line = CsvFormatter::format_artisan_line("10.00", 100.0, 98.5, -1.5, 0.0);
        assert_eq!(line.as_str(), "10.00,100.0,98.5,-1.50,0.0");
    }

    #[test]
    fn test_normalize_read_value_finite() {
        let value = CsvFormatter::normalize_read_value(123.45);
        assert_eq!(value, 123.45);
    }

    #[test]
    fn test_normalize_read_value_infinity() {
        let value = CsvFormatter::normalize_read_value(f32::INFINITY);
        assert_eq!(value, 0.0);
    }

    #[test]
    fn test_normalize_read_value_nan() {
        let value = CsvFormatter::normalize_read_value(f32::NAN);
        assert_eq!(value, 0.0);
    }

    #[test]
    fn test_normalize_read_value_negative_infinity() {
        let value = CsvFormatter::normalize_read_value(f32::NEG_INFINITY);
        assert_eq!(value, 0.0);
    }
}
