use crate::output::traits::PrintScheduler;
use embassy_time::{Duration, Instant};

/// Fixed interval scheduler for consistent timing
///
/// This scheduler triggers at regular intervals (default 1Hz)
/// and provides non-blocking timing checks suitable for
/// use in async contexts.
pub struct IntervalScheduler {
    interval: Duration,
    last_print: Instant,
    enabled: bool,
}

impl IntervalScheduler {
    /// Create new interval scheduler with custom interval
    pub fn new(interval_ms: u64) -> Self {
        Self {
            interval: Duration::from_millis(interval_ms),
            last_print: Instant::now(),
            enabled: true,
        }
    }

    /// Create 1Hz scheduler (1000ms interval)
    pub fn hz1() -> Self {
        Self::new(1000)
    }

    /// Create 10Hz scheduler (100ms interval)
    pub fn hz10() -> Self {
        Self::new(100)
    }

    /// Enable or disable the scheduler
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if enabled {
            self.last_print = Instant::now();
        }
    }

    /// Check if scheduler is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get the current interval duration
    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// Set new interval duration
    pub fn set_interval(&mut self, interval_ms: u64) {
        self.interval = Duration::from_millis(interval_ms);
    }

    /// Force next print to happen immediately
    pub fn trigger_now(&mut self) {
        self.last_print = Instant::now() - self.interval;
    }

    /// Get time until next print
    pub fn time_until_next(&self) -> Duration {
        let elapsed = self.last_print.elapsed();
        if elapsed >= self.interval {
            Duration::from_millis(0)
        } else {
            self.interval - elapsed
        }
    }
}

impl PrintScheduler for IntervalScheduler {
    async fn should_print(&mut self) -> bool {
        if !self.enabled {
            return false;
        }

        let now = Instant::now();
        let elapsed = now - self.last_print;

        if elapsed >= self.interval {
            self.last_print = now;
            true
        } else {
            false
        }
    }

    fn reset(&mut self) {
        self.last_print = Instant::now();
    }
}

impl Default for IntervalScheduler {
    fn default() -> Self {
        Self::hz1()
    }
}

/// Adaptive scheduler that adjusts frequency based on roaster state
///
/// This scheduler provides higher frequency output during certain
/// phases (like heating) and lower frequency during others.
pub struct AdaptiveScheduler {
    base_interval: Duration,
    heating_interval: Duration,
    last_print: Instant,
    enabled: bool,
}

impl AdaptiveScheduler {
    /// Create new adaptive scheduler
    pub fn new(base_interval_ms: u64, heating_interval_ms: u64) -> Self {
        Self {
            base_interval: Duration::from_millis(base_interval_ms),
            heating_interval: Duration::from_millis(heating_interval_ms),
            last_print: Instant::now(),
            enabled: true,
        }
    }

    /// Check if should print based on roaster state
    pub async fn should_print_with_state(&mut self, is_heating: bool) -> bool {
        if !self.enabled {
            return false;
        }

        let interval = if is_heating {
            self.heating_interval
        } else {
            self.base_interval
        };

        let now = Instant::now();
        let elapsed = now - self.last_print;

        if elapsed >= interval {
            self.last_print = now;
            true
        } else {
            false
        }
    }

    /// Enable or disable scheduler
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if enabled {
            self.last_print = Instant::now();
        }
    }
}

impl PrintScheduler for AdaptiveScheduler {
    async fn should_print(&mut self) -> bool {
        // Default to base interval when no state provided
        self.should_print_with_state(false).await
    }

    fn reset(&mut self) {
        self.last_print = Instant::now();
    }
}

/// Mock scheduler for testing
#[cfg(test)]
pub struct MockScheduler {
    enabled: bool,
    call_count: usize,
}

#[cfg(test)]
impl MockScheduler {
    pub fn new() -> Self {
        Self {
            enabled: true,
            call_count: 0,
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn get_call_count(&self) -> usize {
        self.call_count
    }

    /// Force should_print to return true
    pub fn force_print(&mut self) -> bool {
        if self.enabled {
            self.call_count += 1;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
impl PrintScheduler for MockScheduler {
    async fn should_print(&mut self) -> bool {
        if self.enabled {
            self.call_count += 1;
            true
        } else {
            false
        }
    }

    fn reset(&mut self) {
        self.call_count = 0;
    }
}
