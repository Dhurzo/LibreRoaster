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
use heapless::String;

pub struct ServiceContainer {
    /// Async-safe mutex for use in async task contexts
    pub roaster: EmbassyMutex<CriticalSectionRawMutex, Option<RoasterControl>>,
    /// Sync-safe mutex for use in ISR and critical sections
    pub roaster_sync: Mutex<RefCell<Option<RoasterControl>>>,
    pub artisan_input: Mutex<RefCell<Option<ArtisanInput>>>,
    pub multiplexer: Mutex<RefCell<Option<CommandMultiplexer>>>,
    pub watchdog_feeder: Mutex<RefCell<Option<WatchdogFeeder>>>,
}

pub const ARTISAN_CMD_CHANNEL_SIZE: usize = 8;
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

    /// Inject a RoasterControl instance into the sync storage slot
    pub fn init_roaster(roaster: RoasterControl) {
        critical_section::with(|cs| {
            Self::get_instance()
                .roaster_sync
                .borrow(cs)
                .borrow_mut()
                .replace(roaster);
        });
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
        // Using a static mutable reference with unsafe code for singleton pattern
        // This is safe because it's only called once during initialization
        static SERVICE: ServiceContainer = ServiceContainer {
            roaster: EmbassyMutex::new(None),
            roaster_sync: Mutex::new(RefCell::new(None)),
            artisan_input: Mutex::new(RefCell::new(None)),
            multiplexer: Mutex::new(RefCell::new(None)),
            watchdog_feeder: Mutex::new(RefCell::new(None)),
        };
        &SERVICE
    }

    pub fn is_initialized() -> bool {
        critical_section::with(|cs| {
            let container = Self::get_instance();
            container.roaster_sync.borrow(cs).borrow().is_some()
                && container.artisan_input.borrow(cs).borrow().is_some()
        })
    }

    /// Sync access to RoasterControl for ISR and critical section contexts
    /// This is the deprecated API kept for backward compatibility with ISR code
    #[deprecated(note = "Use with_roaster_async() in async contexts")]
    pub fn with_roaster<R, F>(f: F) -> Result<R, ContainerError>
    where
        F: FnOnce(&mut RoasterControl) -> R,
    {
        critical_section::with(|cs| {
            let container = Self::get_instance();
            match container.roaster_sync.borrow(cs).borrow_mut().as_mut() {
                Some(roaster) => Ok(f(roaster)),
                None => Err(ContainerError::NotInitialized),
            }
        })
    }

    /// Sync mutable access to RoasterControl for ISR and critical section contexts
    /// This is the deprecated API kept for backward compatibility with ISR code
    #[deprecated(note = "Use with_roaster_async() in async contexts")]
    pub fn with_roaster_mut<R, F>(f: F) -> Result<R, ContainerError>
    where
        F: FnOnce(&mut RoasterControl) -> R,
    {
        critical_section::with(|cs| {
            let container = Self::get_instance();
            match container.roaster_sync.borrow(cs).borrow_mut().as_mut() {
                Some(roaster) => Ok(f(roaster)),
                None => Err(ContainerError::NotInitialized),
            }
        })
    }

    /// Async access to RoasterControl - use this in async task contexts
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
        Self::with_roaster_async(|roaster| roaster.get_status().bean_temp)
            .await
    }

    pub async fn read_env_temperature() -> Result<f32, ContainerError> {
        Self::with_roaster_async(|roaster| roaster.get_status().env_temp)
            .await
    }

    // This holds the async mutex for the entire sensor read duration (~160ms),
    // potentially delaying command processing. Mitigated by:
    // - Priority-drain in tasks.rs: STOP/EmergencyStop commands process regardless of rate limit
    // - Future optimization: split into trigger→unlock→wait→lock→read phases
    pub async fn roaster_async_sensor_read() -> Result<(), ContainerError> {
        #[cfg(any(test, feature = "async-lock-depth-metrics"))]
        let _async_lock_depth_guard = async_lock_depth::AsyncLockDepthGuard::enter();

        // Retry up to 3 times if the async roaster storage is empty.
        // Handles the init race between control_loop_task and queue_processor_task:
        // both call ensure_async_roaster_initialized_from_sync() but one may
        // arrive at the None branch while the other is mid-move.
        for _ in 0..3 {
            let mut guard = Self::get_instance().roaster.lock().await;

            let roaster = match guard.as_mut() {
                Some(r) => r,
                None => {
                    // Async mutex empty – try to move from sync storage
                    drop(guard);
                    Self::ensure_async_roaster_initialized_from_sync().await?;
                    continue;
                }
            };

            // Call async sensor reading method. Preserve the actual error
            // instead of masking it as NotInitialized.
            return roaster.read_sensors().await.map_err(|e| {
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
                    crate::control::RoasterError::PidError { source } => {
                        source.unwrap_or("pid_error")
                    }
                    crate::control::RoasterError::HardwareError { source } => {
                        source.unwrap_or("hardware_error")
                    }
                    crate::control::RoasterError::EmergencyShutdown { source } => {
                        source.unwrap_or("emergency_shutdown")
                    }
                };
                ContainerError::SensorError { reason }
            });
        }

        Err(ContainerError::NotInitialized)
    }

    /// Ensure async roaster storage is initialized.
    ///
    /// On embedded startup the builder initializes `roaster_sync` first.
    /// The control loop uses async access, so we move the instance lazily
    /// into the async mutex the first time tasks run.
    pub async fn ensure_async_roaster_initialized_from_sync() -> Result<(), ContainerError> {
        let async_empty = {
            let guard = Self::get_instance().roaster.lock().await;
            guard.is_none()
        };

        if !async_empty {
            return Ok(());
        }

        let moved = critical_section::with(|cs| {
            let container = Self::get_instance();
            container.roaster_sync.borrow(cs).borrow_mut().take()
        });

        match moved {
            Some(roaster) => {
                let mut guard = Self::get_instance().roaster.lock().await;
                if guard.is_none() {
                    *guard = Some(roaster);
                }
                Ok(())
            }
            None => {
                let guard = Self::get_instance().roaster.lock().await;
                if guard.is_some() {
                    Ok(())
                } else {
                    Err(ContainerError::NotInitialized)
                }
            }
        }
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

    pub fn get_command_sender(
    ) -> embassy_sync::channel::Sender<'static, CriticalSectionRawMutex, TracedCommand, 8> {
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
    use core::sync::atomic::{AtomicUsize, Ordering};

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
    /// Full state reset for test isolation
    pub fn reset_for_test() {
        critical_section::with(|cs| {
            let _ = Self::get_instance()
                .roaster_sync
                .borrow(cs)
                .borrow_mut()
                .take();
            let _ = Self::get_instance()
                .artisan_input
                .borrow(cs)
                .borrow_mut()
                .take();
            let _ = Self::get_instance()
                .watchdog_feeder
                .borrow(cs)
                .borrow_mut()
                .take();
        });
        while Self::get_artisan_channel().try_receive().is_ok() {}
        while Self::get_output_channel().try_receive().is_ok() {}
    }
}
