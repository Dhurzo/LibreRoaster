//! Memory constants for LibreRoaster — heapless buffer sizes.
//!
//! All sizes are `heapless` capacities (bytes or elements) chosen to fit the
//! ESP32-C3's ~72 KB heap budget while avoiding truncation on the wire. Each
//! constant notes its worst-case payload and RAM cost; see `docs/ARCHITECTURE.md` §13
//! and `src/memory/strategy.rs` for the overall budget.

/// Hot-path error wire format — `ERR <token>[:<source>]`. 128 B covers the
/// longest `ERR handler_failed <token>:<source>` plus `safety_fault` reason
/// without truncation; hot path, no alloc.
pub const ERROR_MSG_MAX_LEN: usize = 128;

/// Artisan command line buffer — 64 B fits `PROFILE`/`FANPROFILE` token chunks
/// (`OT1 100`, `CHAN 12345`) at the 255 B transport ceiling split into tokens.
pub const ARTISAN_CMD_MAX_LEN: usize = 64;

/// Temperature report CSV line (`#<t>,ET,BT,…`). 64 B covers `REPORT_BUFFER_SIZE`
/// formatted by `ArtisanFormatter::format_read_response_full`.
pub const REPORT_BUFFER_SIZE: usize = 64;

/// BT (Bean Temperature) history buffer size.
/// Stored samples for BT temp tracking. The weighted ROR calculation
/// uses all available samples up to this limit for linear weighting.
/// Alias of `ROR_WINDOW_SIZE` (10 × 4 B ≈ 40 B).
pub const BT_HISTORY_SIZE: usize = ROR_WINDOW_SIZE;

/// Stage/state name tag (e.g. `Heating`, `Error`). 16 B covers the longest
/// `RoasterState` debug name plus `TRACE` prefix.
pub const STAGE_NAME_MAX_LEN: usize = 16;

/// Non-critical status wire line (`#OK`, `STATUS …`). 64 B mirrors
/// `REPORT_BUFFER_SIZE`; not on the hot error path.
pub const STATUS_MSG_MAX_LEN: usize = 64;

/// UART/USB RX command buffer — 256 B matches the transport's 255 B ceiling
/// (`transport_tasks` heapless::String<256>) plus delimiter normalisation.
pub const COMMAND_BUFFER_SIZE: usize = 256;

/// UART/USB TX response buffer — 512 B holds the largest multi-field
/// `STATUS` CSV (≈ 200 B) plus `#DUMP` row (128 B) with headroom.
pub const RESPONSE_BUFFER_SIZE: usize = 512;

/// Policy-engine diagnostic line (init-time only). 96 B covers
/// `PolicyOutcome` reason strings; not in the hot tick.
pub const POLICY_MSG_MAX_LEN: usize = 96;

/// Max tokens after splitting an Artisan line on `; , =` / space. 8 covers
/// `PROFILE;0,0;30,150;60,200;90,225` style bursts; longer lines use FIFO.
pub const PARSE_TOKENS_MAX: usize = 8;

/// Single parameter/token scratch (e.g. `"250.0"`, `"OT1"`). 32 B covers
/// `heapless::String<256>` token slices with margin for `PID;GAIN` triples.
pub const PARAM_VALUE_MAX_LEN: usize = 32;

/// Instrumentation TRACE JSON line (`stage,elapsed,guard,wd`). 128 B covers
/// `stage_instrumentation::StageReporter` output at `TRACE_EVENT_MAX_LEN=256`
/// truncated to heapless.
pub const INSTRUMENT_BUFFER_SIZE: usize = 128;

/// Roast profile name (init-time config). 32 B covers `PROFILE` tag plus
/// user label; stored once per `FanProfile`/`RoastProfile`.
pub const PROFILE_NAME_MAX_LEN: usize = 32;

/// Safety event queue depth — 16 slots for `ArtisanCommand` / `RoasterCommand`
/// without alloc; bounds tick latency (see `ServiceContainer::ARTISAN_CMD_CHANNEL_SIZE`).
pub const SAFETY_EVENT_QUEUE_SIZE: usize = 16;

/// Real-time log line (`log` crate). 96 B covers `warn!`/`info!` formatting
/// outside the hot path; larger lines are truncated by `heapless`.
pub const LOG_MSG_MAX_LEN: usize = 96;

/// Diagnostic dump line (full `SystemStatus` debug). 256 B covers
/// `format_status_response` worst case; used by `STATUS`/`DUMP` handlers.
pub const DIAGNOSTIC_MSG_MAX_LEN: usize = 256;

/// Sensor/control calibration scratch. 64 B covers `MAX31856` register block
/// (8 B) plus conversion text; init-time only.
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

/// Safety-critical error wire line (`SAFETY …` / `ERR safety_fault …`).
/// 128 B matches `ERROR_MSG_MAX_LEN`; separate name for audit grep.
pub const SAFETY_ERROR_MSG_MAX_LEN: usize = 128;

/// ROR sliding window — 10 samples × 4 B ≈ 40 B per deque at 1 Hz
/// (≈ 10 s window). Matches `BT_HISTORY_SIZE`.
pub const ROR_WINDOW_SIZE: usize = 10;

/// IIR filter alpha for ROR smoothing — 0.25 balances SPI glitch rejection
/// vs. responsiveness (0.1–0.4 typical; higher = smoother).
pub const ROR_FILTER_ALPHA: f32 = 0.25;

/// Minimum samples before a valid ROR is reported. 2 is the slope floor
/// (single delta); fewer would mask the first tick after `START`.
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
