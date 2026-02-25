use crate::config::constants::SSR_CYCLE_GUARD_MS;
use embassy_time::{Duration, Instant};

/// Guards SSR cycles so commands follow the datasheet minimum interval.
#[derive(Debug, Clone, Copy)]
pub struct SsrCycleGuard {
    last_cycle: Option<Instant>,
    guard_duration: Duration,
}

impl SsrCycleGuard {
    pub fn new() -> Self {
        Self {
            last_cycle: None,
            guard_duration: Duration::from_millis(SSR_CYCLE_GUARD_MS as u64),
        }
    }

    pub fn next_cycle_allowed(&self, now: Instant) -> Result<Instant, Instant> {
        let busy_until = self.busy_until();
        if busy_until <= now {
            Ok(now)
        } else {
            Err(busy_until)
        }
    }

    pub fn mark_cycle(&mut self, now: Instant) {
        self.last_cycle = Some(now);
    }

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
