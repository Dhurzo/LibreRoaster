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

/// Stub implementation for embedded targets
/// Note: The original ESP-IDF watchdog implementation requires ESP-IDF libraries
/// that need additional build configuration. This is a software-only stub.
#[cfg(target_arch = "riscv32")]
mod stub {
    use super::WatchdogError;
    use embassy_time::Instant;

    pub struct WatchdogFeeder {
        last_feed: Option<Instant>,
        last_failure: Option<&'static str>,
    }

    impl WatchdogFeeder {
        pub fn initialize() -> Result<Self, WatchdogError> {
            Ok(Self {
                last_feed: None,
                last_failure: None,
            })
        }

        pub fn feed_async(&mut self, _bean_temp: f32) -> Result<(), WatchdogError> {
            self.last_feed = Some(Instant::now());
            self.last_failure = None;
            Ok(())
        }

        pub fn last_failure_reason(&self) -> Option<&'static str> {
            self.last_failure
        }
    }
}

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

pub use stub::WatchdogFeeder;
