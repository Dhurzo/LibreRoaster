use crate::config::ArtisanCommand;
use crate::control::RoasterControl;
use crate::input::multiplexer::CommandMultiplexer;
use crate::input::ArtisanInput;
use core::cell::RefCell;
use critical_section::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use heapless::String;

pub struct ServiceContainer {
    pub roaster: Mutex<RefCell<Option<RoasterControl>>>,
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

    pub fn is_initialized() -> bool {
        critical_section::with(|cs| {
            let container = Self::get_instance();
            container.roaster.borrow(cs).borrow().is_some()
                && container.artisan_input.borrow(cs).borrow().is_some()
        })
    }

    pub fn get_instance() -> &'static ServiceContainer {
        // Using a static mutable reference with unsafe code for singleton pattern
        // This is safe because it's only called once during initialization
        static SERVICE: ServiceContainer = ServiceContainer {
            roaster: Mutex::new(RefCell::new(None)),
            artisan_input: Mutex::new(RefCell::new(None)),
            multiplexer: Mutex::new(RefCell::new(None)),
        };
        &SERVICE
    }

    pub fn with_roaster<R, F>(f: F) -> Result<R, ContainerError>
    where
        F: FnOnce(&mut RoasterControl) -> R,
    {
        critical_section::with(|cs| {
            let container = Self::get_instance();
            match container.roaster.borrow(cs).borrow_mut().as_mut() {
                Some(roaster) => Ok(f(roaster)),
                None => Err(ContainerError::NotInitialized),
            }
        })
    }

    pub fn with_roaster_mut<R, F>(f: F) -> Result<R, ContainerError>
    where
        F: FnOnce(&mut RoasterControl) -> R,
    {
        critical_section::with(|cs| {
            let container = Self::get_instance();
            match container.roaster.borrow(cs).borrow_mut().as_mut() {
                Some(roaster) => Ok(f(roaster)),
                None => Err(ContainerError::NotInitialized),
            }
        })
    }

    pub fn read_bean_temperature() -> Result<f32, ContainerError> {
        Self::with_roaster(|roaster| Ok(roaster.get_status().bean_temp)).unwrap_or(Ok(0.0))
    }

    pub fn read_env_temperature() -> Result<f32, ContainerError> {
        Self::with_roaster(|roaster| Ok(roaster.get_status().env_temp)).unwrap_or(Ok(0.0))
    }

    /// Perform async sensor read by taking roaster out, calling async methods, then putting back
    /// This pattern allows async operations on RoasterControl which is normally behind a sync mutex
    pub async fn roaster_async_sensor_read() -> Result<(), ContainerError> {
        // Take roaster out of the container
        let mut roaster: RoasterControl = critical_section::with(|cs| {
            let container = Self::get_instance();
            container.roaster.borrow(cs).borrow_mut().take()
                .expect("Roaster not initialized")
        });

        // Now we can call async methods - this is the gap closure!
        // The concrete Max31856 type enables async temperature reading
        roaster.read_sensors().await.map_err(|_| ContainerError::NotInitialized)?;

        // Also do the control update (sync)
        let _ = roaster.update_control(embassy_time::Instant::now());

        // Put roaster back
        critical_section::with(|cs| {
            let container = Self::get_instance();
            container.roaster.borrow(cs).borrow_mut().replace(roaster);
        });

        Ok(())
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
