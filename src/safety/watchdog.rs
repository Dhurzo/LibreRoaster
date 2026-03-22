/// Watchdog feeder errors exposed to higher-level services.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchdogError {
    /// Failed to initialize the underlying watchdog hardware.
    InitializationFailed,
    /// Hardware watchdog feed rejected the reset request.
    FeedFailed(&'static str),
    /// Watchdog service has not been registered yet.
    NotInitialized,
}

impl WatchdogError {
    /// Stable reason string for telemetry or status snapshots.
    pub fn reason(&self) -> &'static str {
        match self {
            WatchdogError::InitializationFailed => "watchdog_init",
            WatchdogError::FeedFailed(reason) => reason,
            WatchdogError::NotInitialized => "watchdog_unavailable",
        }
    }
}

/// Software watchdog implementation using embassy-time
///
/// This provides watchdog functionality without requiring ESP-IDF:
/// - A background task feeds the watchdog at regular intervals
/// - If the main loop stalls, the watchdog won't be fed and will trigger
/// - The actual "watchdog" is a counter that must be periodically reset
#[cfg(target_arch = "riscv32")]
mod software_watchdog {
    use super::WatchdogError;
    use core::sync::atomic::Ordering;
    use portable_atomic::AtomicU32;

    /// Counter that must be kept alive by feeding
    /// If this reaches 0, it means the system stalled
    static WATCHDOG_COUNTER: AtomicU32 = AtomicU32::new(0);

    /// Maximum missed feeds before panic
    const MAX_MISSED_FEEDS: u32 = 3;

    pub struct WatchdogFeeder {
        last_failure: Option<&'static str>,
    }

    impl WatchdogFeeder {
        pub fn initialize() -> Result<Self, WatchdogError> {
            // Initialize counter to max - 1 so first feed sets it to max
            WATCHDOG_COUNTER.store(MAX_MISSED_FEEDS - 1, Ordering::SeqCst);
            Ok(Self { last_failure: None })
        }

        /// Feed the watchdog - must be called regularly
        pub fn feed_async(&mut self, _bean_temp: f32) -> Result<(), WatchdogError> {
            let was_zero = WATCHDOG_COUNTER.swap(MAX_MISSED_FEEDS, Ordering::SeqCst);

            if was_zero == 0 {
                self.last_failure = Some("watchdog_timeout");
                return Err(WatchdogError::FeedFailed("watchdog_timeout"));
            }

            self.last_failure = None;
            Ok(())
        }

        pub fn last_failure_reason(&self) -> Option<&'static str> {
            self.last_failure
        }

        /// Check if watchdog is alive (for debugging)
        pub fn is_alive(&self) -> bool {
            WATCHDOG_COUNTER.load(Ordering::SeqCst) > 0
        }
    }
}

/// Host/PC implementation - no watchdog needed
#[cfg(not(target_arch = "riscv32"))]
mod stub {
    use super::WatchdogError;

    pub struct WatchdogFeeder;

    impl WatchdogFeeder {
        pub fn initialize() -> Result<Self, WatchdogError> {
            Ok(Self)
        }

        pub fn feed_async(&mut self, _bean_temp: f32) -> Result<(), WatchdogError> {
            Ok(())
        }

        pub fn last_failure_reason(&self) -> Option<&'static str> {
            None
        }
    }
}

#[cfg(target_arch = "riscv32")]
pub use software_watchdog::WatchdogFeeder;

#[cfg(not(target_arch = "riscv32"))]
pub use stub::WatchdogFeeder;
