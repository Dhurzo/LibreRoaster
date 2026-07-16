/// Dual-layer watchdog: software counter (telemetry) + hardware RTC WDT (CPU reset).
/// The software watchdog provides status telemetry via the STATUS command.
/// The hardware watchdog resets the CPU if the control loop hangs for >2 seconds.
///
/// On ESP32-C3, the RTC Watchdog Timer (RWDT) is fed in the control loop.
/// If the Embassy executor hangs, the RWDT triggers a full system reset
/// independently of CPU state. On host builds, the hardware WDT is a no-op.
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

// ── Software watchdog (telemetry, runs on all targets) ──────────────

#[cfg(any(target_arch = "riscv32", feature = "test"))]
mod software_watchdog {
    use super::WatchdogError;
    use core::sync::atomic::Ordering;
    use portable_atomic::AtomicU64;

    /// Timestamp of last successful feed in milliseconds
    static LAST_FEED_MS: AtomicU64 = AtomicU64::new(0);
    const WATCHDOG_TIMEOUT_MS: u64 = 500; // 3 missed ticks = 500ms at ~160ms loop cadence

    pub struct WatchdogFeeder {
        last_failure: Option<&'static str>,
    }

    impl WatchdogFeeder {
        pub fn initialize() -> Result<Self, WatchdogError> {
            LAST_FEED_MS.store(0, Ordering::SeqCst);
            Ok(Self { last_failure: None })
        }

        pub fn feed_async(&mut self, _bean_temp: f32) -> Result<(), WatchdogError> {
            let now = embassy_time::Instant::now().as_millis();
            let last = LAST_FEED_MS.swap(now, Ordering::SeqCst);
            if last > 0 && now - last > WATCHDOG_TIMEOUT_MS {
                self.last_failure = Some("watchdog_timeout");
                return Err(WatchdogError::FeedFailed("watchdog_timeout"));
            }
            self.last_failure = None;
            // Also feed the hardware WDT on ESP32
            super::hw_watchdog::feed();
            Ok(())
        }

        pub fn last_failure_reason(&self) -> Option<&'static str> {
            self.last_failure
        }

        pub fn is_alive(&self) -> bool {
            let now = embassy_time::Instant::now().as_millis();
            let last = LAST_FEED_MS.load(Ordering::SeqCst);
            last == 0 || now - last <= WATCHDOG_TIMEOUT_MS
        }
    }
}

#[cfg(not(any(target_arch = "riscv32", feature = "test")))]
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
        pub fn is_alive(&self) -> bool {
            true
        }
    }
}

#[cfg(any(target_arch = "riscv32", feature = "test"))]
pub use software_watchdog::WatchdogFeeder;

#[cfg(not(any(target_arch = "riscv32", feature = "test")))]
pub use stub::WatchdogFeeder;

// ── Hardware RTC Watchdog (ESP32-C3 only, true CPU reset on hang) ───

#[cfg(target_arch = "riscv32")]
mod hw_watchdog {
    /// Feed the ESP32-C3 RTC Watchdog Timer (RWDT).
    ///
    /// The bootloader enables the RWDT before jumping to the app. We must
    /// feed it in the control loop to prevent a CPU reset. The unlock
    /// sequence writes the magic key to WDTWPROTECT before feeding.
    ///
    /// Uses the `esp32c3` PAC for register access instead of hardcoded MMIO
    /// addresses. Register offsets are resolved from the SVD at compile time.
    pub fn feed() {
        const WDT_UNLOCK_KEY: u32 = 0x50D8_3AA1;

        let rtc_cntl = unsafe { &*esp32c3::RTC_CNTL::ptr() };
        rtc_cntl
            .wdtwprotect()
            .write(|w| unsafe { w.wdt_wkey().bits(WDT_UNLOCK_KEY) });
        rtc_cntl.wdtfeed().write(|w| w.wdt_feed().set_bit());
    }

    /// Configures the RTC Watchdog Timer with a ~2 s timeout.
    ///
    /// The RWDT runs off the internal 150 kHz RTC slow clock and resets
    /// the CPU if the control loop stops feeding it.  This init is
    /// explicit (not relying on bootloader defaults) so the safety net
    /// is always active regardless of the flash toolchain used.
    pub fn init() {
        const WDT_UNLOCK_KEY: u32 = 0x50D8_3AA1;
        // RTC_SLOW_CLK ≈ 150 kHz  →  ~2 s = 300 000 cycles
        const WDT_STAGE0_HOLD: u32 = 300_000;

        let rtc_cntl = unsafe { &*esp32c3::RTC_CNTL::ptr() };

        rtc_cntl
            .wdtwprotect()
            .write(|w| unsafe { w.wdt_wkey().bits(WDT_UNLOCK_KEY) });

        rtc_cntl
            .wdtconfig1()
            .write(|w| unsafe { w.hold().bits(WDT_STAGE0_HOLD) });

        rtc_cntl.wdtconfig0().modify(|_, w| unsafe {
            w.wdt_en()
                .set_bit()
                .wdt_stg0()
                .bits(2) // 2 = reset CPU on stage-0 timeout
                .wdt_flashboot_mod_en()
                .set_bit()
        });

        rtc_cntl
            .wdtwprotect()
            .write(|w| unsafe { w.wdt_wkey().bits(0) });
    }
}

#[cfg(not(target_arch = "riscv32"))]
mod hw_watchdog {
    pub fn feed() {}
    pub fn init() {}
}

pub use hw_watchdog::{feed as feed_hw_watchdog, init as init_hw_watchdog};

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // ── WatchdogError type tests ──────────────────────────────────────

    #[test]
    fn watchdog_error_reason_initialization() {
        assert_eq!(
            WatchdogError::InitializationFailed.reason(),
            "watchdog_init"
        );
    }

    #[test]
    fn watchdog_error_reason_not_initialized() {
        assert_eq!(
            WatchdogError::NotInitialized.reason(),
            "watchdog_unavailable"
        );
    }

    #[test]
    fn watchdog_error_reason_feed_failed() {
        assert_eq!(
            WatchdogError::FeedFailed("watchdog_timeout").reason(),
            "watchdog_timeout"
        );
    }

    #[test]
    fn watchdog_error_reason_custom() {
        assert_eq!(
            WatchdogError::FeedFailed("test_reason").reason(),
            "test_reason"
        );
    }

    #[test]
    fn watchdog_error_debug_contains_variant() {
        let err = WatchdogError::InitializationFailed;
        let debug = format!("{:?}", err);
        assert!(debug.contains("InitializationFailed"));
    }

    #[test]
    fn watchdog_error_clone_equality() {
        let err = WatchdogError::FeedFailed("test");
        assert_eq!(err, err.clone());
    }

    #[test]
    fn watchdog_error_partial_eq() {
        assert_eq!(
            WatchdogError::InitializationFailed,
            WatchdogError::InitializationFailed
        );
        assert_ne!(
            WatchdogError::InitializationFailed,
            WatchdogError::NotInitialized
        );
    }

    // ── Watchdog feed logic tests ─────────────────────────────────────
    // These test the actual software_watchdog implementation on host
    // (when built with --features test), or the stub otherwise.
    // On host+test, embassy_time provides a real clock via HostTimeDriver
    // so feed timing and state transitions are validated end-to-end.

    #[test]
    fn watchdog_initialize_resets_feeder() {
        let feeder = WatchdogFeeder::initialize().unwrap();
        assert!(feeder.is_alive());
        assert!(feeder.last_failure_reason().is_none());
    }

    #[test]
    fn watchdog_first_feed_after_init_succeeds() {
        let mut feeder = WatchdogFeeder::initialize().unwrap();
        let result = feeder.feed_async(0.0);
        assert!(result.is_ok());
    }

    #[test]
    fn watchdog_consecutive_feeds_all_succeed() {
        let mut feeder = WatchdogFeeder::initialize().unwrap();
        for i in 0..10 {
            let result = feeder.feed_async(i as f32 * 10.0);
            assert!(result.is_ok(), "Feed #{} should succeed", i);
        }
    }

    #[test]
    fn watchdog_is_alive_after_successful_feed() {
        let mut feeder = WatchdogFeeder::initialize().unwrap();
        feeder.feed_async(0.0).unwrap();
        assert!(feeder.is_alive());
    }

    #[test]
    fn watchdog_last_failure_cleared_after_successful_feed() {
        let mut feeder = WatchdogFeeder::initialize().unwrap();
        feeder.feed_async(0.0).unwrap();
        assert!(feeder.last_failure_reason().is_none());
    }

    #[test]
    fn watchdog_is_alive_returns_true_when_recently_fed() {
        let mut feeder = WatchdogFeeder::initialize().unwrap();
        // Feed multiple times to ensure the timestamp stays current
        for i in 0..5 {
            feeder.feed_async(i as f32 * 5.0).unwrap();
            assert!(
                feeder.is_alive(),
                "Should be alive immediately after feed #{}",
                i
            );
        }
    }

    #[test]
    fn watchdog_initialize_resets_state_after_previous_activity() {
        let mut feeder = WatchdogFeeder::initialize().unwrap();
        feeder.feed_async(0.0).unwrap(); // establish a feed timestamp
        assert!(feeder.is_alive());

        // Re-initialize to reset the state
        let feeder = WatchdogFeeder::initialize().unwrap();
        assert!(feeder.is_alive());
        assert!(feeder.last_failure_reason().is_none());
    }

    #[test]
    fn watchdog_feed_accepts_varying_temperatures() {
        let mut feeder = WatchdogFeeder::initialize().unwrap();
        // The bean_temp parameter is for future use but should not cause
        // failures even with edge-case values
        assert!(feeder.feed_async(-1.0).is_ok());
        assert!(feeder.feed_async(0.0).is_ok());
        assert!(feeder.feed_async(100.0).is_ok());
        assert!(feeder.feed_async(300.0).is_ok()); // above cutoff
    }

    #[test]
    fn watchdog_feeder_lifecycle() {
        // Full lifecycle: init → feed → verify alive → no failure → consistent
        let mut feeder = WatchdogFeeder::initialize().unwrap();

        // Initial state
        assert!(feeder.is_alive());
        assert!(feeder.last_failure_reason().is_none());

        // Feed cycle
        feeder.feed_async(25.0).unwrap();
        assert!(feeder.is_alive());
        assert!(feeder.last_failure_reason().is_none());

        // Second feed cycle
        feeder.feed_async(50.0).unwrap();
        assert!(feeder.is_alive());

        // Third feed cycle
        feeder.feed_async(75.0).unwrap();
        assert!(feeder.last_failure_reason().is_none());
    }
}
