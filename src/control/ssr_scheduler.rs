//! SSR cycle guard enforcing the minimum inter-command interval.
//!
//! `SsrCycleGuard` tracks the timestamp of the last SSR cycle and reports
//! whether a new cycle may start yet (`SSR_CYCLE_GUARD_MS`), preventing command
//! bursts that violate the SSR datasheet timing.

use crate::config::constants::SSR_CYCLE_GUARD_MS;
use embassy_time::{Duration, Instant};

/// Guards SSR cycles so commands follow the datasheet minimum interval.
#[derive(Debug, Clone, Copy)]
pub struct SsrCycleGuard {
    last_cycle: Option<Instant>,
    guard_duration: Duration,
}

impl SsrCycleGuard {
    /// Creates a guard with `guard_duration` = `SSR_CYCLE_GUARD_MS`.
    pub fn new() -> Self {
        Self {
            last_cycle: None,
            guard_duration: Duration::from_millis(SSR_CYCLE_GUARD_MS as u64),
        }
    }

    /// `Ok(now)` if a cycle may start, else `Err(busy_until)`.
    pub fn next_cycle_allowed(&self, now: Instant) -> Result<Instant, Instant> {
        let busy_until = self.busy_until();
        if busy_until <= now {
            Ok(now)
        } else {
            Err(busy_until)
        }
    }

    /// Records `now` as the start of an SSR cycle.
    pub fn mark_cycle(&mut self, now: Instant) {
        self.last_cycle = Some(now);
    }

    /// Returns the instant the guard next permits a cycle (epoch if never cycled).
    pub fn busy_until(&self) -> Instant {
        if let Some(last) = self.last_cycle {
            last + self.guard_duration
        } else {
            Instant::from_micros(0)
        }
    }
}

impl Default for SsrCycleGuard {
    fn default() -> Self {
        Self::new()
    }
}
