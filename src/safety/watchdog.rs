//! Dual-layer watchdog: software counter (telemetry) + hardware RTC WDT (CPU reset).
//! The software watchdog provides status telemetry via the STATUS command.
//! The hardware watchdog resets the CPU if the control loop hangs for >2 seconds.
//!
//! On ESP32-C3, the RTC Watchdog Timer (RWDT) is fed in the control loop.
//! If the Embassy executor hangs, the RWDT triggers a full system reset
//! independently of CPU state. On host builds, the hardware WDT is a no-op.

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

    /// Sentinel meaning "never fed yet". `u64::MAX` on purpose: the previous
    /// sentinel was `0`, but a feed landing in the first millisecond of the
    /// time driver's baseline (real `Instant::now()` = 0 ms — always the case
    /// in host tests, theoretically possible after a boot that feeds before
    /// the counter advances) was stored as `0` and became indistinguishable
    /// from "never fed", making `is_alive()` return `true` forever (Bug S9,
    /// 2026-08-05). A real ms timestamp can never reach `u64::MAX`, so the
    /// sentinel is unambiguous.
    const NEVER_FED: u64 = u64::MAX;

    /// Timestamp of last successful feed in milliseconds
    static LAST_FEED_MS: AtomicU64 = AtomicU64::new(NEVER_FED);
    // Bug audit 2026-08-02: the previous 500 ms assumed a ~100 ms loop
    // cadence ("5 missed ticks"). The real cadence is one tick per
    // MAX31856 conversion wait (210 ms) + 100 ms timer + overhead ≈ 330 ms,
    // leaving only ~170 ms of margin — two slightly delayed ticks tripped a
    // false emergency shutdown mid-roast. 1000 ms covers three full ticks
    // (~990 ms) plus margin, while still failing well before the 2.2 s HW
    // RWDT (which, unlike the software path, resets the chip without the
    // orderly `SAFETY WATCHDOG` escalation + shutdown).
    const WATCHDOG_TIMEOUT_MS: u64 = 1000;

    pub struct WatchdogFeeder {
        last_failure: Option<&'static str>,
    }

    impl WatchdogFeeder {
        pub fn initialize() -> Result<Self, WatchdogError> {
            LAST_FEED_MS.store(NEVER_FED, Ordering::SeqCst);
            Ok(Self { last_failure: None })
        }

        pub fn feed_async(&mut self, _bean_temp: f32) -> Result<(), WatchdogError> {
            // Audit M-T3 (2026-08-11): `bean_temp` is reserved for a future
            // overtemp-gated feed (Artisan's convention: stop feeding during
            // an overtemp crisis so the RWDT trips). Deliberately UNUSED
            // today — the feed is unconditional, per Bug B18 below.
            // Bug B18: feed the HW WDT unconditionally FIRST. The fact that we
            // are executing this at all proves the control loop is alive — even
            // a degraded-but-alive loop (gap > 500ms) must keep the RWDT fed,
            // otherwise (with B2 fixed) the chip resets at ~2.2s and skips the
            // designed `SAFETY WATCHDOG` escalation + orderly shutdown.
            super::hw_watchdog::feed();

            let now = embassy_time::Instant::now().as_millis();
            let last = LAST_FEED_MS.swap(now, Ordering::SeqCst);
            // Bug L7 (2026-07-25): saturating subtraction. With
            // `overflow-checks = true` (release builds may still opt-in for
            // `embedded`), `now - last` would underflow if the embassy-time
            // clock were to wrap or two test threads interleaved so that the
            // new `now` is older than `last`. Saturate to 0 so a transient
            // out-of-order pair NEVER panics; if `now - last` is 0 the
            // timeout branch is taken conservatively (correct: a clock that
            // wrapped is unreliable).
            // Bug S9: the `last > 0` guard (which skipped the gap check for
            // "never fed") is replaced by the unambiguous `NEVER_FED`
            // sentinel, so a genuine first-millisecond feed is checked like
            // any other.
            if last != NEVER_FED && now.saturating_sub(last) > WATCHDOG_TIMEOUT_MS {
                self.last_failure = Some("watchdog_timeout");
                return Err(WatchdogError::FeedFailed("watchdog_timeout"));
            }
            self.last_failure = None;
            Ok(())
        }

        pub fn last_failure_reason(&self) -> Option<&'static str> {
            self.last_failure
        }

        pub fn is_alive(&self) -> bool {
            let now = embassy_time::Instant::now().as_millis();
            let last = LAST_FEED_MS.load(Ordering::SeqCst);
            // Audit L-6 (2026-08-11): `NEVER_FED` counts as alive BY DESIGN —
            // before the first feed the loop is starting up, not hung. The
            // real enforcement is the gap check inside `feed_async` and the
            // HW RWDT, both of which fire on a genuinely dead loop.
            last == NEVER_FED || now.saturating_sub(last) <= WATCHDOG_TIMEOUT_MS
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
        // Bug B2: re-lock the WDT protect register after feeding. Leaving it
        // unlocked meant stray writes (or a stuck task) could reconfigure the
        // WDT silently. This pairs with the same write at the end of init().
        rtc_cntl
            .wdtwprotect()
            .write(|w| unsafe { w.wdt_wkey().bits(0) });
    }

    /// Configures the RTC Watchdog Timer with a ~2 s timeout.
    ///
    /// The RWDT runs off the internal ~136 kHz RTC slow clock (RC_SLOW_CLK on
    /// the C3 is ~136 kHz, not 150 kHz) and resets the system if the control
    /// loop stops feeding it.
    ///
    /// Bug A5 (2026-07-25): the effective HOLD written into the register is
    /// `HOLD << (1 + WDT_DELAY_SEL)` where `WDT_DELAY_SEL ∈ {0,1,2,3}` is
    /// stored in efuse `RD_REPEAT_DATA1`. With the typical value `0`, the
    /// actual timeout is 2× the value we write — so naively writing 300 000
    /// yields ~4.4 s instead of the documented ~2.2 s, and with `1..=3` it
    /// blows up to 8.8 s / 17.6 s / 35.2 s, all well past the safety budget.
    /// We compensate by shifting the requested value right by `(1 + sel)`
    /// before writing; that delivers the requested ≈ 2.2 s nominal timeout
    /// across the whole efuse range.
    ///
    /// Additionally, `esp_hal::init` leaves `WDTCONFIG0` zeroed, so
    /// `wdt_sys_reset_length` / `wdt_cpu_reset_length` are 0 — i.e. a 100 ns
    /// reset pulse that the flash chip may not even see. esp-hal programs 7
    /// (≈ 3.2 µs). We do the same so the reset reliably reaches the
    /// peripherals and the flash boot loader.
    pub fn init() {
        const WDT_UNLOCK_KEY: u32 = 0x50D8_3AA1;
        // RTC_SLOW_CLK ≈ 136 kHz → ~2.2 s nominal = 300 000 cycles.
        // Bug M6 (2026-08-10): shared with config::constants so the margin
        // assertion there bounds the value actually programmed here.
        const WDT_STAGE0_HOLD_NOMINAL: u32 = crate::config::constants::HW_WATCHDOG_STAGE0_CYCLES;
        // 7 ≈ 3.2 µs reset pulse — mirror esp-hal defaults so the WD timeout
        // reliably latches the system into reset instead of producing a
        // 100 ns blip that peripherals can ignore.
        const WDT_RESET_PULSE_LEN: u8 = 7;

        let rtc_cntl = unsafe { &*esp32c3::RTC_CNTL::ptr() };

        rtc_cntl
            .wdtwprotect()
            .write(|w| unsafe { w.wdt_wkey().bits(WDT_UNLOCK_KEY) });

        // Read the efuse-stored `wdt_delay_sel` (0..=3) before configuring the
        // RWDT so we can compensate its ×2/×4/×8/×16 shift upstream.
        let efuse = unsafe { &*esp32c3::EFUSE::ptr() };
        let wdt_delay_sel: u32 = efuse.rd_repeat_data1().read().wdt_delay_sel().bits() as u32;
        // Saturating right shift: a 0 sel → 1, a 3 sel → 4. Clamp to 6 (the
        // upper bound the report cites; saturating at 6 also guards against a
        // future efuse value we have not accounted for).
        let shift = 1u32 + wdt_delay_sel.min(3);
        let hold = WDT_STAGE0_HOLD_NOMINAL >> shift;

        rtc_cntl
            .wdtconfig1()
            .write(|w| unsafe { w.hold().bits(hold) });

        rtc_cntl.wdtconfig0().modify(|_, w| unsafe {
            w.wdt_en()
                .set_bit()
                .wdt_stg0()
                // Bug B2: 3 = ResetSystem (full chip reset, peripherals back to
                // reset state). The previous value `1` selected Interrupt —
                // and no RWDT interrupt handler exists in this firmware, so a
                // timeout did nothing: the safety net was effectively absent.
                // esp-hal `RwdtStageAction`: Off=0, Interrupt=1, ResetCpu=2,
                // ResetSystem=3, ResetRtc=4.
                .bits(3)
                .wdt_sys_reset_length()
                .bits(WDT_RESET_PULSE_LEN)
                .wdt_cpu_reset_length()
                .bits(WDT_RESET_PULSE_LEN)
                // Bug M4 (2026-08-10): the TRM (§12.2.2.4) and BOTH vendor HALs
                // clear `wdt_flashboot_mod_en` after boot, before configuring
                // the RWDT in software: esp-hal writes `.bit(false)`
                // (rtc_cntl/mod.rs) and ESP-IDF's `rtc_wdt_disable` does
                // `REG_CLR_BIT(..., RTC_CNTL_WDT_FLASHBOOT_MOD_EN)`. The
                // previous `set_bit()` kept a second enable path (flashboot
                // mode) active in the one safety net of the roast; it adds
                // nothing (`wdt_en` already arms the watchdog) and diverges
                // from the documented boot sequence.
                .wdt_flashboot_mod_en()
                .clear_bit()
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

/// Re-export of the hardware RTC watchdog feed/init for the control loop.
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
    fn watchdog_feed_ignores_bean_temp_parameter() {
        let mut feeder = WatchdogFeeder::initialize().unwrap();
        // The bean_temp parameter is reserved (see feed_async doc) and must
        // not cause failures with any value. There is deliberately NO
        // overtemp gating today — the feed is unconditional (Bug B18).
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
