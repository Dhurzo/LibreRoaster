use crate::config::ArtisanCommand;
use crate::control::RoasterControl;
use crate::input::multiplexer::CommandMultiplexer;
use crate::input::ArtisanInput;
use core::cell::RefCell;
use critical_section::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use heapless::String;
use static_cell::ConstStaticCell;

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

// ConstStaticCell for singleton ServiceContainer - initialized at compile time
// SAFETY: take() can only be called once during application initialization.
// The returned reference lives for the duration of the program.
static INSTANCE: ConstStaticCell<ServiceContainer> = ConstStaticCell::new(ServiceContainer::new());

impl ServiceContainer {
    pub const fn new() -> Self {
        Self {
            roaster: Mutex::new(RefCell::new(None)),
            artisan_input: Mutex::new(RefCell::new(None)),
            multiplexer: Mutex::new(RefCell::new(None)),
        }
    }

    pub fn get_instance() -> &'static mut Self {
        // SAFETY: ConstStaticCell::take() can only be called once.
        // This is called during application initialization before any async tasks start.
        // The returned reference lives for the duration of the program.
        INSTANCE.take()
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
        Self::with_roaster(f)
    }

    pub fn read_bean_temperature() -> Result<f32, ContainerError> {
        Self::with_roaster(|roaster| Ok(roaster.get_status().bean_temp)).unwrap_or(Ok(0.0))
    }

    pub fn read_env_temperature() -> Result<f32, ContainerError> {
        Self::with_roaster(|roaster| Ok(roaster.get_status().env_temp)).unwrap_or(Ok(0.0))
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

    pub fn is_initialized() -> bool {
        critical_section::with(|cs| {
            let container = Self::get_instance();
            container.roaster.borrow(cs).borrow().is_some()
                && container.artisan_input.borrow(cs).borrow().is_some()
        })
    }

    pub fn get_artisan_channel(
    ) -> &'static Channel<CriticalSectionRawMutex, ArtisanCommand, ARTISAN_CMD_CHANNEL_SIZE> {
        &ARTISAN_CMD_CHANNEL
    }

    pub fn get_output_channel(
    ) -> &'static Channel<CriticalSectionRawMutex, String<128>, ARTISAN_OUTPUT_CHANNEL_SIZE> {
        &ARTISAN_OUTPUT_CHANNEL
    }

    pub fn get_multiplexer() -> &'static Mutex<RefCell<Option<CommandMultiplexer>>> {
        &ARTISAN_MULTIPLEXER
    }

    pub fn init_multiplexer() {
        critical_section::with(|cs| {
            let multiplexer = Self::get_multiplexer();
            let mut guard = multiplexer.borrow(cs).borrow_mut();
            if guard.is_none() {
                guard.replace(CommandMultiplexer::new());
            }
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ContainerError {
    NotInitialized,
    AlreadyInitialized,
    BorrowFailed,
    InvalidState,
    SensorError,
    HardwareInit(&'static str),
}

impl core::fmt::Display for ContainerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ContainerError::NotInitialized => write!(f, "Service container not initialized"),
            ContainerError::AlreadyInitialized => {
                write!(f, "Service container already initialized")
            }
            ContainerError::BorrowFailed => write!(f, "Container borrow failed"),
            ContainerError::InvalidState => write!(f, "Container in invalid state"),
            ContainerError::HardwareInit(msg) => {
                write!(f, "Hardware initialization failed: {}", msg)
            }
            ContainerError::SensorError => write!(f, "Sensor error"),
        }
    }
}
