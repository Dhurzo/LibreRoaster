use crate::application::service_container::{ServiceContainer, ContainerError};
use crate::config::{SSR_CONTROL_PIN, FAN_PWM_PIN};
use crate::control::RoasterControl;
use crate::hardware::fan::FanController;
use crate::hardware::ssr::{SsrControl, SsrPlaceholder};
use crate::hardware::uart::initialize_uart_system;
use crate::input::ArtisanInput;
use crate::output::artisan::ArtisanFormatter;
use embassy_executor::Spawner;
use esp_hal::peripherals::{UART0, LEDC, GPIO8};
use log::info;

/// Application builder for safe and organized initialization
/// Extracts initialization logic from main.rs for better maintainability
pub struct AppBuilder<'a> {
    uart0: Option<UART0<'a>>,
    ledc: Option<LEDC<'a>>,
    gpio8: Option<GPIO8<'a>>,
    formatter: Option<ArtisanFormatter>,
}

impl<'a> AppBuilder<'a> {
    /// Create a new application builder
    pub fn new() -> Self {
        Self {
            uart0: None,
            ledc: None,
            gpio8: None,
            formatter: None,
        }
    }

    /// Configure UART system
    pub fn with_uart(mut self, uart0: UART0<'a>) -> Self {
        self.uart0 = Some(uart0);
        self
    }

    /// Configure LEDC peripherals for fan control
    pub fn with_ledc(mut self, ledc: LEDC<'a>, gpio8: GPIO8<'a>) -> Self {
        self.ledc = Some(ledc);
        self.gpio8 = Some(gpio8);
        self
    }

    /// Configure Artisan formatter
    pub fn with_formatter(mut self, formatter: ArtisanFormatter) -> Self {
        self.formatter = Some(formatter);
        self
    }

    /// Build and initialize the complete application
    pub fn build(mut self) -> Result<Application, BuildError> {
        // Initialize UART system
        if let Some(uart0) = self.uart0 {
            initialize_uart_system(uart0).map_err(|e| BuildError::UartInit(e))?;
        }

        // Initialize fan controller with real LEDC if available, otherwise placeholder
        let fan = if let (Some(ledc), Some(gpio8)) = (self.ledc.take(), self.gpio8.take()) {
            let fan_controller = FanController::with_ledc(ledc, gpio8)
                .map_err(|e| BuildError::FanInit(e))?;
            info!("Fan controller initialized with LEDC PWM on GPIO{}", FAN_PWM_PIN);
            fan_controller
        } else {
            let fan_controller = FanController::new()
                .map_err(|e| BuildError::FanInit(e))?;
            info!("Fan control not available - no LEDC hardware");
            fan_controller
        };

        // Initialize SSR with placeholder implementation
        let _ssr_placeholder = SsrPlaceholder::default();
        let _ssr = SsrControl::new(_ssr_placeholder)
            .map_err(|e| BuildError::SsrInit(e))?;
        info!("SSR control initialized on GPIO{} (placeholder)", SSR_CONTROL_PIN);

        // Initialize core components
        let roaster = RoasterControl::new()
            .map_err(|e| BuildError::RoasterInit(e))?;
        let artisan_input = ArtisanInput::new()
            .map_err(|e| BuildError::ArtisanInit(e))?;
        let formatter = self.formatter.unwrap_or_else(ArtisanFormatter::new);

        // Initialize service container
        ServiceContainer::initialize_with_ssr(roaster, fan, artisan_input)
            .map_err(|e| BuildError::ContainerInit(e))?;

        info!("Application components initialized successfully (placeholder hardware)");

        Ok(Application {
            formatter,
            built: true,
        })
    }
}

/// Represents the initialized application
pub struct Application {
    formatter: ArtisanFormatter,
    built: bool,
}

impl Application {
    /// Get Artisan formatter
    pub fn formatter(&self) -> &ArtisanFormatter {
        &self.formatter
    }

    /// Clone formatter for use in tasks
    pub fn clone_formatter(&self) -> ArtisanFormatter {
        self.formatter.clone()
    }

    /// Verify all services are properly initialized
    pub fn verify_initialization(&self) -> Result<(), VerificationError> {
        if !self.built {
            return Err(VerificationError::NotBuilt);
        }

        if !ServiceContainer::is_initialized() {
            return Err(VerificationError::ServicesNotInitialized);
        }

        Ok(())
    }

    /// Start all application tasks
    pub async fn start_tasks(&self, spawner: Spawner) -> Result<(), TaskError> {
        use crate::hardware::uart::tasks::{uart_reader_task, uart_writer_task};

        // Verify initialization before starting tasks
        self.verify_initialization()
            .map_err(|e| TaskError::VerificationFailed(e))?;

        // Clone formatter for tasks
        let formatter2 = self.clone_formatter();
        let formatter3 = self.clone_formatter();

        // Start UART communication tasks
        spawner.spawn(uart_reader_task()).map_err(|e| TaskError::SpawnFailed(e))?;
        spawner.spawn(uart_writer_task()).map_err(|e| TaskError::SpawnFailed(e))?;

        // Spawn control tasks
        spawner
            .spawn(super::control_loop_task(
                formatter2,
            ))
            .map_err(|e| TaskError::SpawnFailed(e))?;
            
        spawner
            .spawn(super::artisan_uart_handler_task(
                formatter3,
            ))
            .map_err(|e| TaskError::SpawnFailed(e))?;

        info!("All application tasks started successfully");
        Ok(())
    }
}

/// Application build errors
#[derive(Debug, Clone, PartialEq)]
pub enum BuildError {
    UartInit(crate::hardware::uart::UartError),
    RoasterInit(crate::control::RoasterError),
    FanInit(crate::hardware::fan::FanError),
    SsrInit(crate::hardware::ssr::SsrError),
    ArtisanInit(crate::input::InputError),
    ContainerInit(ContainerError),
}

impl core::fmt::Display for BuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            BuildError::UartInit(e) => write!(f, "UART initialization failed: {:?}", e),
            BuildError::RoasterInit(e) => write!(f, "Roaster initialization failed: {:?}", e),
            BuildError::FanInit(e) => write!(f, "Fan controller initialization failed: {:?}", e),
            BuildError::SsrInit(e) => write!(f, "SSR control initialization failed: {:?}", e),
            BuildError::ArtisanInit(e) => write!(f, "Artisan input initialization failed: {:?}", e),
            BuildError::ContainerInit(e) => write!(f, "Service container initialization failed: {}", e),
        }
    }
}

/// Application verification errors
#[derive(Debug, Clone, PartialEq)]
pub enum VerificationError {
    NotBuilt,
    ServicesNotInitialized,
}

impl core::fmt::Display for VerificationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            VerificationError::NotBuilt => write!(f, "Application not built"),
            VerificationError::ServicesNotInitialized => write!(f, "Services not initialized"),
        }
    }
}

/// Task spawning errors
#[derive(Debug)]
pub enum TaskError {
    VerificationFailed(VerificationError),
    SpawnFailed(embassy_executor::SpawnError),
}

impl core::fmt::Display for TaskError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TaskError::VerificationFailed(e) => write!(f, "Verification failed: {}", e),
            TaskError::SpawnFailed(e) => write!(f, "Failed to spawn task: {:?}", e),
        }
    }
}

impl Default for AppBuilder<'_> {
    fn default() -> Self {
        Self::new()
    }
}