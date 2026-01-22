use crate::control::RoasterControl;
use crate::hardware::fan::FanController;
use crate::input::ArtisanInput;
use core::cell::RefCell;
use critical_section::Mutex;

/// Thread-safe service container for managing global application state
/// Replaces unsafe static mut variables with proper synchronization
pub struct ServiceContainer {
    roaster: Mutex<RefCell<Option<RoasterControl>>>,
    fan: Mutex<RefCell<Option<FanController>>>,
    artisan_input: Mutex<RefCell<Option<ArtisanInput>>>,
}

impl ServiceContainer {
    /// Create a new empty service container
    pub const fn new() -> Self {
        Self {
            roaster: Mutex::new(RefCell::new(None)),
            fan: Mutex::new(RefCell::new(None)),
            artisan_input: Mutex::new(RefCell::new(None)),
        }
    }

    /// Initialize the container with all services
    pub fn initialize(
        roaster: RoasterControl,
        fan: FanController,
        artisan_input: ArtisanInput,
    ) -> Result<(), ContainerError> {
        critical_section::with(|cs| {
            let container = Self::get_instance();

            // Initialize roaster
            container.roaster.borrow(cs).borrow_mut().replace(roaster);

            // Initialize fan
            container.fan.borrow(cs).borrow_mut().replace(fan);

            // Initialize artisan input
            container
                .artisan_input
                .borrow(cs)
                .borrow_mut()
                .replace(artisan_input);

            Ok(())
        })
    }

    /// Get reference to the global service container instance
    fn get_instance() -> &'static Self {
        static INSTANCE: ServiceContainer = ServiceContainer::new();
        &INSTANCE
    }

    /// Execute operation on roaster with proper error handling
    pub fn with_roaster<R, F>(f: F) -> Result<R, ContainerError>
    where
        F: FnOnce(&mut RoasterControl) -> R,
    {
        critical_section::with(|cs| {
            let container = Self::get_instance();
            let mut roaster_ref = container.roaster.borrow(cs).borrow_mut();

            match roaster_ref.as_mut() {
                Some(roaster) => Ok(f(roaster)),
                None => Err(ContainerError::NotInitialized),
            }
        })
    }

    /// Execute operation on fan with proper error handling
    pub fn with_fan<R, F>(f: F) -> Result<R, ContainerError>
    where
        F: FnOnce(&mut FanController) -> R,
    {
        critical_section::with(|cs| {
            let container = Self::get_instance();
            let mut fan_ref = container.fan.borrow(cs).borrow_mut();

            match fan_ref.as_mut() {
                Some(fan) => Ok(f(fan)),
                None => Err(ContainerError::NotInitialized),
            }
        })
    }

    /// Execute operation on artisan input with proper error handling
    pub fn with_artisan_input<R, F>(f: F) -> Result<R, ContainerError>
    where
        F: FnOnce(&mut ArtisanInput) -> R,
    {
        critical_section::with(|cs| {
            let container = Self::get_instance();
            let mut artisan_ref = container.artisan_input.borrow(cs).borrow_mut();

            match artisan_ref.as_mut() {
                Some(artisan) => Ok(f(artisan)),
                None => Err(ContainerError::NotInitialized),
            }
        })
    }

    /// Check if all services are initialized
    pub fn is_initialized() -> bool {
        critical_section::with(|cs| {
            let container = Self::get_instance();

            let roaster_ok = container.roaster.borrow(cs).borrow().is_some();
            let fan_ok = container.fan.borrow(cs).borrow().is_some();
            let artisan_ok = container.artisan_input.borrow(cs).borrow().is_some();

            roaster_ok && fan_ok && artisan_ok
        })
    }

    /// Reset all services (for testing/emergency reset)
    pub fn reset() -> Result<(), ContainerError> {
        critical_section::with(|cs| {
            let container = Self::get_instance();

            container.roaster.borrow(cs).borrow_mut().take();
            container.fan.borrow(cs).borrow_mut().take();
            container.artisan_input.borrow(cs).borrow_mut().take();

            Ok(())
        })
    }
}

/// Error types for service container operations
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ContainerError {
    NotInitialized,
    BorrowFailed,
    InvalidState,
}

impl core::fmt::Display for ContainerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ContainerError::NotInitialized => write!(f, "Service container not initialized"),
            ContainerError::BorrowFailed => write!(f, "Failed to borrow service"),
            ContainerError::InvalidState => write!(f, "Container in invalid state"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::RoasterControl;
    use crate::hardware::fan::FanController;
    use crate::input::ArtisanInput;

    #[test]
    fn test_container_initialization() {
        // Reset container first
        let _ = ServiceContainer::reset();

        assert!(!ServiceContainer::is_initialized());

        // This would require hardware in real tests
        // For now, just test the state tracking
    }

    #[test]
    fn test_error_display() {
        let error = ContainerError::NotInitialized;
        assert_eq!(error.to_string(), "Service container not initialized");
    }
}
