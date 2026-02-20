use crate::config::ArtisanCommand;
use crate::control::RoasterControl;
use crate::input::multiplexer::CommandMultiplexer;
use crate::input::ArtisanInput;
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
}

pub const ARTISAN_CMD_CHANNEL_SIZE: usize = 8;
pub const ARTISAN_OUTPUT_CHANNEL_SIZE: usize = 16;
static ARTISAN_CMD_CHANNEL: Channel<
    CriticalSectionRawMutex,
    ArtisanCommand,
    ARTISAN_CMD_CHANNEL_SIZE,
> = Channel::new();
static ARTISAN_OUTPUT_CHANNEL: Channel<
    CriticalSectionRawMutex,
    String<128>,
    ARTISAN_OUTPUT_CHANNEL_SIZE,
> = Channel::new();
static ARTISAN_MULTIPLEXER: Mutex<RefCell<Option<CommandMultiplexer>>> =
    Mutex::new(RefCell::new(None));

impl ServiceContainer {
    pub fn get_artisan_channel(
    ) -> &'static Channel<CriticalSectionRawMutex, ArtisanCommand, ARTISAN_CMD_CHANNEL_SIZE> {
        &ARTISAN_CMD_CHANNEL
    }

    pub fn get_output_channel(
    ) -> &'static Channel<CriticalSectionRawMutex, String<128>, ARTISAN_OUTPUT_CHANNEL_SIZE> {
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

    pub fn get_instance() -> &'static ServiceContainer {
        // Using a static mutable reference with unsafe code for singleton pattern
        // This is safe because it's only called once during initialization
        static SERVICE: ServiceContainer = ServiceContainer {
            roaster: EmbassyMutex::new(None),
            roaster_sync: Mutex::new(RefCell::new(None)),
            artisan_input: Mutex::new(RefCell::new(None)),
            multiplexer: Mutex::new(RefCell::new(None)),
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
        Self::with_roaster_async(|roaster| Ok(roaster.get_status().bean_temp)).await.unwrap_or(Ok(0.0))
    }

    pub async fn read_env_temperature() -> Result<f32, ContainerError> {
        Self::with_roaster_async(|roaster| Ok(roaster.get_status().env_temp)).await.unwrap_or(Ok(0.0))
    }

    /// Perform async sensor read using the async lock
    /// This uses EmbassyMutex for safe concurrent async access
    pub async fn roaster_async_sensor_read() -> Result<(), ContainerError> {
        #[cfg(any(test, feature = "async-lock-depth-metrics"))]
        let _async_lock_depth_guard = async_lock_depth::AsyncLockDepthGuard::enter();

        let result = {
            // Use async lock for safe concurrent access
            let mut guard = Self::get_instance().roaster.lock().await;

            let roaster = guard.as_mut().ok_or(ContainerError::NotInitialized)?;

            // Call async sensor reading method
            roaster
                .read_sensors()
                .await
                .map_err(|_| ContainerError::NotInitialized)?;

            // Also do the control update (sync)
            let _ = roaster.update_control(embassy_time::Instant::now());

            // Guard is automatically released when dropped
            Ok(())
        };

        result
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
    ) -> embassy_sync::channel::Sender<'static, CriticalSectionRawMutex, ArtisanCommand, 8> {
        ARTISAN_CMD_CHANNEL.sender()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ContainerError {
    NotInitialized,
}

impl core::fmt::Display for ContainerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ContainerError::NotInitialized => write!(f, "Service container not initialized"),
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
    pub(crate) struct AsyncLockDepthGuard;

    impl AsyncLockDepthGuard {
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
