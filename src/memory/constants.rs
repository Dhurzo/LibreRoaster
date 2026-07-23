//! Memory constants for LibreRoaster
//!
//! This module defines standard sizes for heapless buffers,
//! ensuring consistency and predictability in memory usage.

/// Maximum size for error messages in hot paths
///
/// Used for errors that may occur during normal operation
/// in critical paths where heap allocation is not allowed.
pub const ERROR_MSG_MAX_LEN: usize = 128;

/// Maximum size for Artisan commands in hot paths
///
/// Used for Artisan command and response formatting
/// in the real-time communication path.
pub const ARTISAN_CMD_MAX_LEN: usize = 64;

/// Temperature report buffer size
///
/// Used for temperature data formatting sent to
/// Artisan or other monitoring systems.
pub const REPORT_BUFFER_SIZE: usize = 64;

/// BT (Bean Temperature) history buffer size.
/// Stored samples for BT temp tracking. The weighted ROR calculation
/// uses all available samples up to this limit for linear weighting.
pub const BT_HISTORY_SIZE: usize = ROR_WINDOW_SIZE;

/// Maximum size for stage or state names
///
/// Used to identify current roaster state
/// in reports and logs.
pub const STAGE_NAME_MAX_LEN: usize = 16;

/// Maximum size for system status messages
///
/// Used for periodic system status reports
/// that are not critical errors.
pub const STATUS_MSG_MAX_LEN: usize = 64;

/// UART/USB command buffer size
///
/// Used for processing commands received via
/// serial or USB communication.
pub const COMMAND_BUFFER_SIZE: usize = 256;

/// UART/USB response buffer size
///
/// Used to build command responses
/// without dynamic allocation.
pub const RESPONSE_BUFFER_SIZE: usize = 512;

/// Maximum size for control policy messages
///
/// Used in policy modules where messages
/// may be generated during initialization.
pub const POLICY_MSG_MAX_LEN: usize = 96;

/// Command parsing buffer size
///
/// Used during Artisan command parsing
/// to hold tokens and parameters.
pub const PARSE_TOKENS_MAX: usize = 8;

/// Maximum size for parameter values
///
/// Used for temporary parameter storage
/// during command parsing and processing.
pub const PARAM_VALUE_MAX_LEN: usize = 32;

/// Instrumentation buffer size
///
/// Used for collecting system metrics
/// and instrumentation data.
pub const INSTRUMENT_BUFFER_SIZE: usize = 128;

/// Maximum size for roast profile names
///
/// Used during initialization to store
/// configuration profile names.
pub const PROFILE_NAME_MAX_LEN: usize = 32;

/// Maximum capacity for safety event queue
///
/// Used for handling safety events without
/// real-time allocations.
pub const SAFETY_EVENT_QUEUE_SIZE: usize = 16;

/// Real-time logging buffer size
///
/// Used for log message formatting that
/// may occur at any time.
pub const LOG_MSG_MAX_LEN: usize = 96;

/// Maximum size for diagnostic messages
///
/// Used for system diagnostic reports
/// that may include detailed information.
pub const DIAGNOSTIC_MSG_MAX_LEN: usize = 256;

/// Calibration buffer size
///
/// Used during sensor and control
/// system calibration operations.
pub const CALIBRATION_BUFFER_SIZE: usize = 64;

/// Time formatting buffer size
///
/// Used for timestamp formatting in seconds and milliseconds
/// for protocols like Artisan.
///
/// Bug B31: the previous value was 8 bytes, which fits `"{}.{:02}"` only up
/// to 99 999 s (≈27.7 h of continuous streaming). At 100 000+ s the `write!`
/// returns `Err` and the timestamp buffer is silently left truncated — the
/// upstream `try_send` swallows the failure. 16 bytes gives comfortable
/// headroom (up to 9 999 999 s ≈ 115 days) without measurable memory cost.
pub const TIME_FORMAT_SIZE: usize = 16;

/// Safety module error message size
///
/// Used for critical error messages that may
/// occur during safety operations.
pub const SAFETY_ERROR_MSG_MAX_LEN: usize = 128;

/// ROR (Rate of Rise) window size
///
/// Number of temperature samples used to calculate
/// the temperature rate of change with a sliding window.
pub const ROR_WINDOW_SIZE: usize = 10;

/// IIR filter coefficient for ROR
///
/// Alpha for the IIR filter that smooths ROR calculation.
/// Typical values: 0.1-0.4 (higher = smoother, less sensitive)
pub const ROR_FILTER_ALPHA: f32 = 0.25;

/// Minimum samples for ROR calculation
///
/// Minimum number of samples required in history
/// before a valid ROR value can be calculated.
pub const ROR_MIN_SAMPLES: usize = 2;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_sanity() {
        // Verify that constants are reasonable (checked at compile time)
        const {
            assert!(ERROR_MSG_MAX_LEN > 0);
        }
        const {
            assert!(ERROR_MSG_MAX_LEN <= 1024);
        }

        const {
            assert!(ARTISAN_CMD_MAX_LEN > 0);
        }
        const {
            assert!(ARTISAN_CMD_MAX_LEN <= 256);
        }

        const {
            assert!(REPORT_BUFFER_SIZE > 0);
        }
        const {
            assert!(REPORT_BUFFER_SIZE <= 256);
        }

        const {
            assert!(BT_HISTORY_SIZE > 0);
        }
        const {
            assert!(BT_HISTORY_SIZE <= 32);
        }

        const {
            assert!(COMMAND_BUFFER_SIZE >= RESPONSE_BUFFER_SIZE / 2);
        }
        const {
            assert!(RESPONSE_BUFFER_SIZE <= 1024);
        }

        // Verify ROR constants
        const {
            assert!(ROR_WINDOW_SIZE >= ROR_MIN_SAMPLES);
        }
        const {
            assert!(ROR_FILTER_ALPHA > 0.0 && ROR_FILTER_ALPHA < 1.0);
        }
        const {
            assert!(ROR_MIN_SAMPLES >= 2);
        }

        // Verify that sizes are powers of 2 or commonly used multiples
        const {
            assert!(ERROR_MSG_MAX_LEN.is_multiple_of(8) || ERROR_MSG_MAX_LEN == 128);
        }
        const {
            assert!(ARTISAN_CMD_MAX_LEN.is_multiple_of(8) || ARTISAN_CMD_MAX_LEN == 64);
        }
    }
}
