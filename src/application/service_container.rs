//! Application service container with single async ownership.
//!
//! `RoasterControl` is stored in a single `EmbassyMutex<CriticalSectionRawMutex,
//! Option<RoasterControl>>` slot. This removes the previous dual-slot design
//! (a sync `Mutex<RefCell<Option<_>>>` mirror plus an async embassy mutex) and
//! the 3-retry init race that masked a synchronization bug (F5.2).
//!
//! Initialization is single-threaded: `AppBuilder::build()` runs before the
//! async executor starts, so `init_roaster` stores the instance into the
//! async mutex via its non-async `try_lock()` (guaranteed uncontended at that
//! point). All runtime access — sync (`with_roaster`, deprecated ISR shim)
//! and async (`with_roaster_async`) — goes through the same mutex. Concurrent
//! `.lock().await` callers queue instead of failing, which is what the
//! `concurrent_sensor_reads_verify_async_mutex` test exercises.

use crate::control::RoasterControl;
use crate::input::multiplexer::CommandMultiplexer;
use crate::input::ArtisanInput;
use crate::logging::traceability::{TracedCommand, TRACE_EVENT_MAX_LEN};
use crate::safety::watchdog::{WatchdogError, WatchdogFeeder};
use core::cell::RefCell;
use critical_section::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::mutex::Mutex as EmbassyMutex;
#[cfg(target_arch = "riscv32")]
use esp_hal::gpio::Output;
use heapless::String;

pub struct ServiceContainer {
    /// Single async-mutex ownership slot for RoasterControl, shared by sync
    /// and async paths. Sync init uses `try_lock()` (single-threaded, no
    /// contention); async runtime uses `.lock().await` (concurrent callers
    /// queue rather than fail).
    pub roaster: EmbassyMutex<CriticalSectionRawMutex, Option<RoasterControl>>,
    pub artisan_input: Mutex<RefCell<Option<ArtisanInput>>>,
    // Bug DRA-2 (2026-07-26): the `multiplexer` field was removed — it was
    // NEVER read or written (always None). The real multiplexer lives in the
    // `ARTISAN_MULTIPLEXER` static, accessed via `get_multiplexer()`.
    pub watchdog_feeder: Mutex<RefCell<Option<WatchdogFeeder>>>,
    /// BUG-06 (2026-08-21): the status LED's single long-lived owner. Stored
    /// here so the control-loop task can drive the pattern and
    /// `enter_safe_shutdown` can keep using the LED if the container was
    /// populated before the failure. Embedded-only (esp-hal `Output` does
    /// not exist on host).
    #[cfg(target_arch = "riscv32")]
    pub status_led: Mutex<RefCell<Option<Output<'static>>>>,
}

// Bug E1 (2026-08-03): was 8. The channel is drained once per control tick
// (~310 ms) and Artisan's session-open burst (UNITS/FILT/CHAN/PID/OT1/OT2/
// START — typically 10-15 lines) used to overrun it, dropping the TAIL of the
// burst (which is where START, or a mid-burst STOP/EmergencyStop, lands).
// Doubling the buffer keeps a full burst resident across one tick window.
// Bug H3 (2026-08-10): `MAX_COMMANDS_PER_TICK` now EQUALS this size, so the
// tick rate-limit branch is intentionally inert — the bounded channel itself
// caps the work per tick, and the old 8-command budget silently discarded
// the 9th..16th commands of a burst.
pub const ARTISAN_CMD_CHANNEL_SIZE: usize = 16;
pub const ARTISAN_OUTPUT_CHANNEL_SIZE: usize = 16;
static ARTISAN_CMD_CHANNEL: Channel<
    CriticalSectionRawMutex,
    TracedCommand,
    ARTISAN_CMD_CHANNEL_SIZE,
> = Channel::new();
static ARTISAN_OUTPUT_CHANNEL: Channel<
    CriticalSectionRawMutex,
    String<TRACE_EVENT_MAX_LEN>,
    ARTISAN_OUTPUT_CHANNEL_SIZE,
> = Channel::new();
static ARTISAN_MULTIPLEXER: Mutex<RefCell<Option<CommandMultiplexer>>> =
    Mutex::new(RefCell::new(None));

impl ServiceContainer {
    pub fn get_artisan_channel(
    ) -> &'static Channel<CriticalSectionRawMutex, TracedCommand, ARTISAN_CMD_CHANNEL_SIZE> {
        &ARTISAN_CMD_CHANNEL
    }

    pub fn get_output_channel() -> &'static Channel<
        CriticalSectionRawMutex,
        String<TRACE_EVENT_MAX_LEN>,
        ARTISAN_OUTPUT_CHANNEL_SIZE,
    > {
        &ARTISAN_OUTPUT_CHANNEL
    }

    pub fn get_multiplexer() -> &'static critical_section::Mutex<RefCell<Option<CommandMultiplexer>>>
    {
        &ARTISAN_MULTIPLEXER
    }

    pub fn init_multiplexer() {
        critical_section::with(|cs| {
            let multiplexer = CommandMultiplexer::new();
            ARTISAN_MULTIPLEXER.borrow(cs).replace(Some(multiplexer));
        });
    }

    pub fn init_watchdog(&self, feeder: WatchdogFeeder) {
        critical_section::with(|cs| {
            self.watchdog_feeder.borrow(cs).replace(Some(feeder));
        });
    }

    /// Inject a `RoasterControl` instance into the single async-mutex slot.
    ///
    /// Called exactly once from `AppBuilder::build()`, which runs single-
    /// threaded before the executor spawns any tasks. We use the embassy
    /// mutex's non-async `try_lock()` instead of `.lock().await` because the
    /// builder is not an async context — and because no task can contend at
    /// this point, `try_lock()` cannot fail.
    pub fn init_roaster(roaster: RoasterControl) {
        match Self::get_instance().roaster.try_lock() {
            Ok(mut guard) => {
                *guard = Some(roaster);
            }
            Err(_) => {
                // `try_lock` can only fail if the async mutex is currently held
                // by an executor task. `AppBuilder::build()` runs before the
                // executor starts, so this branch is a programmer error rather
                // than a runtime hazard. We log and drop the new instance
                // rather than panic (the clippy config denies panic/expect).
                log::error!("init_roaster called while async mutex held — instance dropped");
            }
        }
    }

    /// Inject an ArtisanInput instance
    pub fn init_artisan_input(input: ArtisanInput) {
        critical_section::with(|cs| {
            Self::get_instance()
                .artisan_input
                .borrow(cs)
                .borrow_mut()
                .replace(input);
        });
    }

    pub fn with_watchdog<R, F>(&self, f: F) -> Result<R, ContainerError>
    where
        F: FnOnce(&mut WatchdogFeeder) -> Result<R, WatchdogError>,
    {
        critical_section::with(|cs| {
            let mut guard = self.watchdog_feeder.borrow(cs).borrow_mut();
            if let Some(feeder) = guard.as_mut() {
                f(feeder).map_err(ContainerError::Watchdog)
            } else {
                Err(ContainerError::WatchdogUninitialized)
            }
        })
    }

    pub fn watchdog_available(&self) -> bool {
        critical_section::with(|cs| self.watchdog_feeder.borrow(cs).borrow().is_some())
    }

    pub fn get_instance() -> &'static ServiceContainer {
        // Compile-time initialized singleton. The EmbassyMutex starts empty
        // (None); `init_roaster` fills it before the executor runs.
        static SERVICE: ServiceContainer = ServiceContainer {
            roaster: EmbassyMutex::new(None),
            artisan_input: Mutex::new(RefCell::new(None)),
            watchdog_feeder: Mutex::new(RefCell::new(None)),
            #[cfg(target_arch = "riscv32")]
            status_led: Mutex::new(RefCell::new(None)),
        };
        &SERVICE
    }

    /// BUG-06: install the status LED handle (embedded-only, called once by
    /// `AppBuilder::build()` before the executor runs).
    #[cfg(target_arch = "riscv32")]
    pub fn init_status_led(led: Output<'static>) {
        critical_section::with(|cs| {
            Self::get_instance()
                .status_led
                .borrow(cs)
                .borrow_mut()
                .replace(led);
        });
    }

    /// BUG-06: run `f` on the status LED if installed. Returns `None` when
    /// the LED was never installed (host builds, or init failed before the
    /// builder ran). The closure runs inside the critical section so the
    /// GPIO write is not interleaved.
    #[cfg(target_arch = "riscv32")]
    pub fn with_status_led<R, F>(f: F) -> Option<R>
    where
        F: FnOnce(&mut Output<'static>) -> R,
    {
        critical_section::with(|cs| {
            let mut guard = Self::get_instance().status_led.borrow(cs).borrow_mut();
            guard.as_mut().map(f)
        })
    }

    pub fn is_initialized() -> bool {
        // Use try_lock for a non-async snapshot. Returns NotInitialized-style
        // false if the slot is empty or (transiently) contended; callers use
        // this only for diagnostics, not for synchronization.
        match Self::get_instance().roaster.try_lock() {
            Ok(guard) => {
                let roaster_set = guard.is_some();
                let artisan_set = critical_section::with(|cs| {
                    Self::get_instance()
                        .artisan_input
                        .borrow(cs)
                        .borrow()
                        .is_some()
                });
                roaster_set && artisan_set
            }
            Err(_) => false,
        }
    }

    /// Sync access to RoasterControl for ISR and critical section contexts.
    ///
    /// Deprecated: prefer `with_roaster_async()` in any context that can
    /// await. This shim uses `try_lock()`, so it fails (rather than waits)
    /// if the mutex is currently held by an async caller — exactly the right
    /// behaviour for an ISR that must never block.
    #[deprecated(note = "Use with_roaster_async() in async contexts")]
    pub fn with_roaster<R, F>(f: F) -> Result<R, ContainerError>
    where
        F: FnOnce(&mut RoasterControl) -> R,
    {
        let mut guard = Self::get_instance()
            .roaster
            .try_lock()
            .map_err(|_| ContainerError::NotInitialized)?;
        match guard.as_mut() {
            Some(roaster) => Ok(f(roaster)),
            None => Err(ContainerError::NotInitialized),
        }
    }

    /// Async access to RoasterControl - use this in async task contexts.
    ///
    /// Concurrent `.lock().await` callers queue on the embassy mutex: only
    /// one holds the guard at a time, the rest wait, all eventually succeed.
    /// The closure must not itself `.await` (it is sync); for the long sensor
    /// read use `roaster_async_sensor_read()`.
    pub async fn with_roaster_async<R, F>(f: F) -> Result<R, ContainerError>
    where
        F: FnOnce(&mut RoasterControl) -> R,
    {
        let mut guard = Self::get_instance().roaster.lock().await;
        match guard.as_mut() {
            Some(roaster) => Ok(f(roaster)),
            None => Err(ContainerError::NotInitialized),
        }
    }

    pub async fn read_bean_temperature() -> Result<f32, ContainerError> {
        Self::with_roaster_async(|roaster| roaster.get_status().bean_temp).await
    }

    pub async fn read_env_temperature() -> Result<f32, ContainerError> {
        Self::with_roaster_async(|roaster| roaster.get_status().env_temp).await
    }

    /// Read sensors async.
    ///
    /// Holds the async mutex for the full sensor read duration (~160 ms on
    /// embedded). This is the documented trade-off: concurrent callers
    /// `.lock().await` and queue rather than fail (which is what
    /// `concurrent_sensor_reads_verify_async_mutex` verifies). Mitigations:
    /// - Priority-drain in tasks.rs: STOP/EmergencyStop commands process
    ///   regardless of rate limit.
    /// - Future optimization: split into trigger→unlock→wait→lock→read
    ///   phases if 160 ms latency becomes a problem.
    pub async fn roaster_async_sensor_read() -> Result<(), ContainerError> {
        #[cfg(any(test, feature = "async-lock-depth-metrics"))]
        let _async_lock_depth_guard = async_lock_depth::AsyncLockDepthGuard::enter();

        let mut guard = Self::get_instance().roaster.lock().await;
        let roaster = guard.as_mut().ok_or(ContainerError::NotInitialized)?;

        // Call async sensor reading method. Preserve the actual error
        // instead of masking it as NotInitialized.
        roaster.read_sensors().await.map_err(|e| {
            let reason = match e {
                crate::control::RoasterError::TemperatureOutOfRange { source } => {
                    source.unwrap_or("temperature_out_of_range")
                }
                crate::control::RoasterError::SensorFault { source } => {
                    source.unwrap_or("sensor_fault")
                }
                crate::control::RoasterError::InvalidState { source } => {
                    source.unwrap_or("invalid_state")
                }
                crate::control::RoasterError::PidError { source } => source.unwrap_or("pid_error"),
                crate::control::RoasterError::HardwareError { source } => {
                    source.unwrap_or("hardware_error")
                }
                crate::control::RoasterError::EmergencyShutdown { source } => {
                    source.unwrap_or("emergency_shutdown")
                }
            };
            ContainerError::SensorError { reason }
        })
    }

    pub fn with_artisan_input<R, F>(f: F) -> Result<R, ContainerError>
    where
        F: FnOnce(&mut ArtisanInput) -> R,
    {
        critical_section::with(|cs| {
            let container = Self::get_instance();
            match container.artisan_input.borrow(cs).borrow_mut().as_mut() {
                Some(artisan_input) => Ok(f(artisan_input)),
                None => Err(ContainerError::NotInitialized),
            }
        })
    }

    pub fn get_command_sender() -> embassy_sync::channel::Sender<
        'static,
        CriticalSectionRawMutex,
        TracedCommand,
        ARTISAN_CMD_CHANNEL_SIZE,
    > {
        ARTISAN_CMD_CHANNEL.sender()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ContainerError {
    NotInitialized,
    WatchdogUninitialized,
    Watchdog(WatchdogError),
    SensorError { reason: &'static str },
}

impl core::fmt::Display for ContainerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ContainerError::NotInitialized => write!(f, "Service container not initialized"),
            ContainerError::WatchdogUninitialized => write!(f, "Watchdog feeder not initialized"),
            ContainerError::Watchdog(err) => {
                write!(f, "Watchdog error: {}", err.reason())
            }
            ContainerError::SensorError { reason } => {
                write!(f, "Sensor error: {}", reason)
            }
        }
    }
}

#[cfg(any(test, feature = "async-lock-depth-metrics"))]
mod async_lock_depth {
    // Bug H4 (2026-08-10): `core::sync::atomic` RMW ops (fetch_add/max/sub)
    // require the A extension — absent on riscv32imc (ESP32-C3), so the
    // `--features embedded,async-lock-depth-metrics` build failed with E0599
    // and CI never caught it. Use `portable_atomic` like the rest of the
    // firmware (watchdog.rs, traceability.rs).
    use portable_atomic::{AtomicUsize, Ordering};

    static ASYNC_LOCK_DEPTH_CURRENT: AtomicUsize = AtomicUsize::new(0);
    static ASYNC_LOCK_DEPTH_MAX: AtomicUsize = AtomicUsize::new(0);

    pub(crate) struct AsyncLockDepthGuard;

    impl AsyncLockDepthGuard {
        pub(crate) fn enter() -> Self {
            let depth = ASYNC_LOCK_DEPTH_CURRENT.fetch_add(1, Ordering::SeqCst) + 1;
            ASYNC_LOCK_DEPTH_MAX.fetch_max(depth, Ordering::SeqCst);
            AsyncLockDepthGuard
        }
    }

    impl Drop for AsyncLockDepthGuard {
        fn drop(&mut self) {
            ASYNC_LOCK_DEPTH_CURRENT.fetch_sub(1, Ordering::SeqCst);
        }
    }

    pub fn async_lock_depth_max_for_tests() -> usize {
        ASYNC_LOCK_DEPTH_MAX.load(Ordering::SeqCst)
    }

    pub fn reset_async_lock_metrics_for_tests() {
        ASYNC_LOCK_DEPTH_CURRENT.store(0, Ordering::SeqCst);
        ASYNC_LOCK_DEPTH_MAX.store(0, Ordering::SeqCst);
    }
}

#[cfg(not(any(test, feature = "async-lock-depth-metrics")))]
mod async_lock_depth {
    #[allow(dead_code)]
    pub(crate) struct AsyncLockDepthGuard;

    impl AsyncLockDepthGuard {
        #[allow(dead_code)]
        pub(crate) fn enter() -> Self {
            AsyncLockDepthGuard
        }
    }

    pub fn async_lock_depth_max_for_tests() -> usize {
        0
    }

    pub fn reset_async_lock_metrics_for_tests() {}
}

pub use async_lock_depth::{async_lock_depth_max_for_tests, reset_async_lock_metrics_for_tests};

#[cfg(test)]
impl ServiceContainer {
    /// Full state reset for test isolation.
    ///
    /// Uses `try_lock()` to drain the async slot. If the lock is currently
    /// held (e.g. by a previously panicked test), the slot is left untouched
    /// rather than blocking test teardown.
    pub fn reset_for_test() {
        if let Ok(mut guard) = Self::get_instance().roaster.try_lock() {
            *guard = None;
        }
        critical_section::with(|cs| {
            let container = Self::get_instance();
            let _ = container.artisan_input.borrow(cs).borrow_mut().take();
            let _ = container.watchdog_feeder.borrow(cs).borrow_mut().take();
        });
        while Self::get_artisan_channel().try_receive().is_ok() {}
        while Self::get_output_channel().try_receive().is_ok() {}
    }
}
