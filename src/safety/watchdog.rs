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

#[cfg(target_arch = "riscv32")]
mod software_watchdog {
    use super::WatchdogError;
    use core::sync::atomic::Ordering;
    use portable_atomic::AtomicU64;

    /// Timestamp of last successful feed in milliseconds
    static LAST_FEED_MS: AtomicU64 = AtomicU64::new(0);
    const WATCHDOG_TIMEOUT_MS: u64 = 300; // 3 missed ticks = 300ms at 100ms interval

    pub struct WatchdogFeeder {
        last_failure: Option<&'static str>,
    }

    impl WatchdogFeeder {
        pub fn initialize() -> Result<Self, WatchdogError> {
            LAST_FEED_MS.store(0, Ordering::SeqCst);
            Ok(Self { last_failure: None })
        }

        pub fn feed_async(&mut self, _bean_temp: f32) -> Result<(), WatchdogError> {
            let now = embassy_time::Instant::now().as_millis() as u64;
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
            let now = embassy_time::Instant::now().as_millis() as u64;
            let last = LAST_FEED_MS.load(Ordering::SeqCst);
            last == 0 || now - last <= WATCHDOG_TIMEOUT_MS
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
        pub fn is_alive(&self) -> bool {
            true
        }
    }
}

#[cfg(target_arch = "riscv32")]
pub use software_watchdog::WatchdogFeeder;

#[cfg(not(target_arch = "riscv32"))]
pub use stub::WatchdogFeeder;

// ── Hardware RTC Watchdog (ESP32-C3 only, true CPU reset on hang) ───

#[cfg(target_arch = "riscv32")]
mod hw_watchdog {
    pub fn feed() {
        // Feed the ESP32-C3 RTC Watchdog Timer that is enabled by the
        // IDF bootloader. The RWDT is independent of the CPU — if the
        // Embassy executor hangs, the RWDT resets the system after the
        // configured timeout (~2s by default).
        //
        // The bootloader configures the RWDT before jumping to the app.
        // We must feed it in the control loop to prevent reset.
        // Register addresses are documented in the ESP32-C3 TRM §15.4.
        const RTC_CNTL_WDTWPROTECT: u32 = 0x6000_80A4;
        const RTC_CNTL_WDTFEED: u32 = 0x6000_8098;
        const WDT_UNLOCK_KEY: u32 = 0x50D8_3AA1;

        unsafe {
            core::ptr::write_volatile(RTC_CNTL_WDTWPROTECT as *mut u32, WDT_UNLOCK_KEY);
            core::ptr::write_volatile(RTC_CNTL_WDTFEED as *mut u32, 1);
        }
    }

    /// Initialize the hardware WDT timeout.
    /// Must be called once during startup before the control loop begins.
    pub fn init() {
        // HW WDT timeout is set by the ESP IDF bootloader, not configured here.
        // The IDF bootloader already enables the RWDT with a default
        // timeout. This is a placeholder for future explicit configuration
        // when moving away from the IDF bootloader.
    }
}

#[cfg(not(target_arch = "riscv32"))]
mod hw_watchdog {
    pub fn feed() {}
    pub fn init() {}
}

pub use hw_watchdog::{feed as feed_hw_watchdog, init as init_hw_watchdog};
