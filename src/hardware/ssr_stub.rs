//! Host-build stub for SSR control.
//!
//! Mirrors the `SsrError` type from `hardware::ssr_logic` so host-target code
//! (which replaces `hardware::ssr` with this stub) keeps compiling and the
//! `embedded_hal::digital::Error` contract intact.

#[derive(Debug, Clone, Copy, PartialEq)]
/// SSR control error (host stub mirror of `ssr_logic::SsrError`).
pub enum SsrError {
    /// GPIO write (SSR pin) failed.
    OutputError { source: &'static str },
    /// GPIO read (detection pin) failed.
    InputError { source: &'static str },
    /// Heat source not detected despite commanded duty.
    HeatSourceNotDetected { source: &'static str },
    /// LEDC PWM write or duty verification failed.
    PwmError { source: &'static str },
}

impl embedded_hal::digital::Error for SsrError {
    fn kind(&self) -> embedded_hal::digital::ErrorKind {
        embedded_hal::digital::ErrorKind::Other
    }
}
