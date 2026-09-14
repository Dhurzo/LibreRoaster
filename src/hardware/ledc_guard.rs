//! Short-timeout spin guard arbitrating LEDC register access.
//!
//! Ensures only one task touches an LEDC channel at a time on the single-core
//! Embassy runtime. Held for <1 us around a register write; contention beyond
//! `LEDC_GUARD_TIMEOUT_MS` is reported instead of deadlocked.

use crate::config::constants::LEDC_GUARD_TIMEOUT_MS;
use core::cell::Cell;
use core::hint::spin_loop;
use embassy_time::{Duration, Instant};
use portable_atomic::{AtomicU16, Ordering};

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

/// Single-owner spin guard for LEDC channel register access.
pub struct LedcGuard {
    locked: Cell<bool>,
}

/// Guard token that must be dropped before yielding across `await` points.
/// Holding the token longer than the timeout will raise `LedcGuardError::Timeout`.
pub struct LedcGuardToken<'a> {
    guard: &'a LedcGuard,
}

impl LedcGuard {
    /// Create an unlocked guard.
    pub const fn new() -> Self {
        Self {
            locked: Cell::new(false),
        }
    }

    /// Attempts to acquire the guard with a short timeout.
    ///
    /// Not re-entrant: calling `try_acquire` from the same task before
    /// `drop`-ing the `LedcGuardToken` will spin until `LEDC_GUARD_TIMEOUT_MS`
    /// and return `Err` — by design, to surface the logic bug.
    ///
    /// # Notes on spin-loop safety (single-core Embassy)
    ///
    /// On the ESP32-C3 with Embassy cooperative multitasking, the lock holder
    /// cannot be preempted — only one task executes at a time. The guard is held
    /// for <1 μs (register write). Contention can only occur if the *same* task
    /// re-enters `try_acquire` before releasing the token — which is a logic bug.
    /// The timeout provides an escape hatch for that case without deadlocking.
    ///
    /// If porting to a preemptive RTOS, replace `spin_loop()` with a yield/sleep.
    pub fn try_acquire(
        &self,
        channel_name: &'static str,
    ) -> Result<LedcGuardToken<'_>, LedcGuardError> {
        let timeout = Duration::from_millis(LEDC_GUARD_TIMEOUT_MS);
        let start = Instant::now();

        loop {
            if !self.locked.get() {
                self.locked.set(true);
                return Ok(LedcGuardToken { guard: self });
            }

            if Instant::now().saturating_duration_since(start) >= timeout {
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

impl Default for LedcGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for LedcGuardToken<'_> {
    fn drop(&mut self) {
        self.guard.locked.set(false);
    }
}

// SAFETY: LedcGuard is used in a single-core ESP32-C3 environment with Embassy
// cooperative multitasking. The Cell<bool> is only accessed from a single
// thread (the current task) and the lock/unlock operations are protected
// by spin loops and timeouts. This is safe because:
// 1. ESP32-C3 is single-core - no true concurrency
// 2. Embassy uses cooperative multitasking - no preemption
// 3. The critical section is extremely short (<1 μs)
// 4. LedcGuard instances are typically static or confined to a single task
unsafe impl Sync for LedcGuard {}

fn record_timeout(_channel_name: &str) {
    TIMEOUT_COUNTER.fetch_add(1, Ordering::Relaxed);
}

pub fn total_timeouts() -> u16 {
    LedcGuard::total_timeouts()
}
