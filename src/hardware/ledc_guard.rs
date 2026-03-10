use crate::config::constants::LEDC_GUARD_TIMEOUT_MS;
use core::hint::spin_loop;
use embassy_time::{Duration, Instant};
use portable_atomic::{AtomicBool, AtomicU16, Ordering};

static TIMEOUT_COUNTER: AtomicU16 = AtomicU16::new(0);

/// Error returned when the guard cannot be acquired within the timeout window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedcGuardError {
    channel: &'static str,
}

impl LedcGuardError {
    pub fn channel(&self) -> &'static str {
        self.channel
    }
}

pub struct LedcGuard {
    locked: AtomicBool,
}

/// Guard token that must be dropped before yielding across `await` points.
/// Holding the token longer than the timeout will raise `LedcGuardError::Timeout`.
pub struct LedcGuardToken<'a> {
    guard: &'a LedcGuard,
}

impl LedcGuard {
    pub const fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
        }
    }

    pub fn try_acquire(
        &self,
        channel_name: &'static str,
    ) -> Result<LedcGuardToken<'_>, LedcGuardError> {
        let timeout = Duration::from_millis(LEDC_GUARD_TIMEOUT_MS);
        let start = Instant::now();

        loop {
            if !self.locked.swap(true, Ordering::Acquire) {
                return Ok(LedcGuardToken { guard: self });
            }

            if Instant::now().duration_since(start) >= timeout {
                record_timeout(channel_name);
                return Err(LedcGuardError {
                    channel: channel_name,
                });
            }

            spin_loop();
        }
    }

    pub fn total_timeouts() -> u16 {
        TIMEOUT_COUNTER.load(Ordering::Relaxed)
    }
}

impl Drop for LedcGuardToken<'_> {
    fn drop(&mut self) {
        self.guard.locked.store(false, Ordering::Release);
    }
}

fn record_timeout(_channel_name: &str) {
    TIMEOUT_COUNTER.fetch_add(1, Ordering::Relaxed);
}

pub fn total_timeouts() -> u16 {
    LedcGuard::total_timeouts()
}
