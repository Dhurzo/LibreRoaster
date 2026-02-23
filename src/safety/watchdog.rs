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

#[cfg(target_arch = "riscv32")]
mod target {
    use super::WatchdogError;
    use crate::config::WATCHDOG_FEED_INTERVAL_MS;
    use core::ffi::c_void;
    use core::ptr;
    use embassy_time::Instant;

    const ESP_OK: i32 = 0;
    const ESP_FAIL: i32 = -1;
    const ESP_ERR_INVALID_ARG: i32 = -3;
    const ESP_ERR_INVALID_STATE: i32 = -4;

    const fn timeout_seconds_from_feed_interval() -> u32 {
        let seconds = (WATCHDOG_FEED_INTERVAL_MS + 999) / 1000;
        if seconds < 1 {
            1
        } else {
            seconds as u32
        }
    }

    extern "C" {
        fn esp_task_wdt_init(timeout_s: u32, panic: bool) -> i32;
        fn esp_task_wdt_add(task: *mut c_void) -> i32;
        fn esp_task_wdt_reset() -> i32;
        fn esp_task_wdt_delete(task: *mut c_void) -> i32;
        fn esp_task_wdt_deinit() -> i32;
    }

    pub struct WatchdogFeeder {
        last_feed: Option<Instant>,
        last_failure: Option<&'static str>,
    }

    impl WatchdogFeeder {
        pub fn initialize() -> Result<Self, WatchdogError> {
            let timeout = timeout_seconds_from_feed_interval();
            let init = unsafe { esp_task_wdt_init(timeout, true) };
            if init != ESP_OK {
                return Err(WatchdogError::InitializationFailed);
            }

            let added = unsafe { esp_task_wdt_add(ptr::null_mut()) };
            if added != ESP_OK {
                unsafe { esp_task_wdt_deinit() };
                return Err(WatchdogError::InitializationFailed);
            }

            Ok(Self {
                last_feed: None,
                last_failure: None,
            })
        }

        pub fn feed_async(&mut self, _bean_temp: f32) -> Result<(), WatchdogError> {
            let res = unsafe { esp_task_wdt_reset() };
            if res == ESP_OK {
                self.last_feed = Some(Instant::now());
                self.last_failure = None;
                Ok(())
            } else {
                let reason = map_feed_reason(res);
                self.last_failure = Some(reason);
                Err(WatchdogError::FeedFailed(reason))
            }
        }

        pub fn last_failure_reason(&self) -> Option<&'static str> {
            self.last_failure
        }
    }

    impl Drop for WatchdogFeeder {
        fn drop(&mut self) {
            let _ = unsafe { esp_task_wdt_delete(ptr::null_mut()) };
            let _ = unsafe { esp_task_wdt_deinit() };
        }
    }

    fn map_feed_reason(code: i32) -> &'static str {
        match code {
            ESP_ERR_INVALID_STATE => "watchdog_invalid_state",
            ESP_ERR_INVALID_ARG => "watchdog_invalid_arg",
            ESP_FAIL => "watchdog_feed_failed",
            _ => "watchdog_feed_error",
        }
    }
}

#[cfg(not(target_arch = "riscv32"))]
mod shim {
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
pub use target::WatchdogFeeder;

#[cfg(not(target_arch = "riscv32"))]
pub use shim::WatchdogFeeder;
