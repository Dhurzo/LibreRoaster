use crate::application::service_container::ServiceContainer;
use crate::config::FAN_PWM_PIN;
use crate::control::traits::{Fan, Heater, Thermometer};
use crate::control::RoasterControl;
use crate::hardware::fan::FanController;
use crate::hardware::uart::initialize_uart_system;
use crate::input::ArtisanInput;
use crate::output::artisan::ArtisanFormatter;
use embassy_executor::Spawner;
use esp_hal::peripherals::{GPIO9, LEDC, UART0};

use alloc::boxed::Box;
use critical_section;
use log::info;

pub struct AppBuilder<'a> {
    uart0: Option<UART0<'a>>,
    ledc: Option<LEDC<'a>>,
    gpio9: Option<GPIO9<'a>>,
    formatter: Option<ArtisanFormatter>,
    heater: Option<Box<dyn Heater + Send>>,
    fan: Option<Box<dyn Fan + Send>>,
    bean_sensor: Option<Box<dyn Thermometer + Send>>,
    env_sensor: Option<Box<dyn Thermometer + Send>>,
}

impl<'a> AppBuilder<'a> {
    pub fn new() -> Self {
        Self {
            uart0: None,
            ledc: None,
            gpio9: None,
            formatter: None,
            heater: None,
            fan: None,
            bean_sensor: None,
            env_sensor: None,
        }
    }

    pub fn with_uart(mut self, uart0: UART0<'a>) -> Self {
        self.uart0 = Some(uart0);
        self
    }

    pub fn with_ledc(mut self, ledc: LEDC<'a>, gpio9: GPIO9<'a>) -> Self {
        self.ledc = Some(ledc);
        self.gpio9 = Some(gpio9);
        self
    }

    /// Configura el elemento calefactor real.
    pub fn with_real_ssr<H>(mut self, ssr: H) -> Self
    where
        H: Heater + Send + 'static,
    {
        self.heater = Some(Box::new(ssr));
        self
    }

    pub fn with_fan_control<F>(mut self, fan: F) -> Self
    where
        F: Fan + Send + 'static,
    {
        self.fan = Some(Box::new(fan));
        self
    }

    pub fn with_temperature_sensors<B, E>(mut self, bean_sensor: B, env_sensor: E) -> Self
    where
        B: Thermometer + Send + 'static,
        E: Thermometer + Send + 'static,
    {
        self.bean_sensor = Some(Box::new(bean_sensor));
        self.env_sensor = Some(Box::new(env_sensor));
        self
    }

    pub fn with_formatter(mut self, formatter: ArtisanFormatter) -> Self {
        self.formatter = Some(formatter);
        self
    }

    pub fn build(self) -> Result<Application, BuildError> {
        if let Some(uart0) = self.uart0 {
            initialize_uart_system(uart0).map_err(|e| BuildError::UartInit(e))?;
        }

        let fan: Box<dyn Fan + Send> = if let Some(fan) = self.fan {
            fan
        } else if let (Some(ledc), Some(gpio9)) = (self.ledc, self.gpio9) {
            let fan_controller =
                FanController::with_ledc(ledc, gpio9).map_err(|e| BuildError::FanInit(e))?;
            info!(
                "Fan controller initialized with LEDC PWM on GPIO{}",
                FAN_PWM_PIN
            );
            Box::new(fan_controller)
        } else {
            let fan_controller = FanController::new().map_err(|e| BuildError::FanInit(e))?;
            info!("Fan control not available - no LEDC hardware");
            Box::new(fan_controller)
        };

        let heater = self
            .heater
            .ok_or(BuildError::MissingPeripheral("SSR Heater"))?;
        let bean_sensor = self
            .bean_sensor
            .ok_or(BuildError::MissingPeripheral("Bean Temperature Sensor"))?;
        let env_sensor = self.env_sensor.ok_or(BuildError::MissingPeripheral(
            "Environment Temperature Sensor",
        ))?;

        let roaster = RoasterControl::new(heater, fan, bean_sensor, env_sensor)
            .map_err(|e| BuildError::RoasterInit(e))?;

        let artisan_input = ArtisanInput::new().map_err(|e| BuildError::ArtisanInit(e))?;
        let formatter = self.formatter.unwrap_or_else(ArtisanFormatter::new);

        critical_section::with(|cs| {
            let container = crate::application::service_container::ServiceContainer::get_instance();
            container.roaster.borrow(cs).borrow_mut().replace(roaster);
            container
                .artisan_input
                .borrow(cs)
                .borrow_mut()
                .replace(artisan_input);
        });

        ServiceContainer::init_multiplexer();

        info!("Application components initialized successfully");

        Ok(Application {
            formatter,
            built: true,
        })
    }
}

pub struct Application {
    formatter: ArtisanFormatter,
    built: bool,
}

impl Application {
    pub fn formatter(&self) -> &ArtisanFormatter {
        &self.formatter
    }

    pub fn clone_formatter(&self) -> ArtisanFormatter {
        self.formatter.clone()
    }

    pub fn verify_initialization(&self) -> Result<(), VerificationError> {
        if !self.built {
            return Err(VerificationError::NotBuilt);
        }

        if !ServiceContainer::is_initialized() {
            return Err(VerificationError::ServicesNotInitialized);
        }

        Ok(())
    }

    pub async fn start_tasks(&self, spawner: Spawner) -> Result<(), TaskError> {
        use crate::hardware::uart::tasks::{uart_reader_task, uart_writer_task};
        use crate::hardware::usb_cdc::tasks::{usb_reader_task, usb_writer_task};

        self.verify_initialization()
            .map_err(|e| TaskError::VerificationFailed(e))?;

        spawner
            .spawn(uart_reader_task())
            .map_err(|e| TaskError::SpawnFailed(e))?;
        spawner
            .spawn(uart_writer_task())
            .map_err(|e| TaskError::SpawnFailed(e))?;

        spawner
            .spawn(usb_reader_task())
            .map_err(|e| TaskError::SpawnFailed(e))?;
        spawner
            .spawn(usb_writer_task())
            .map_err(|e| TaskError::SpawnFailed(e))?;

        spawner
            .spawn(super::dual_output_task())
            .map_err(|e| TaskError::SpawnFailed(e))?;

        spawner
            .spawn(super::control_loop_task())
            .map_err(|e| TaskError::SpawnFailed(e))?;

        info!("All application tasks started successfully");
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BuildError {
    UartInit(crate::hardware::uart::UartError),
    RoasterInit(crate::control::RoasterError),
    FanInit(crate::hardware::fan::FanError),
    SsrInit(crate::hardware::ssr::SsrError),
    ArtisanInit(crate::input::InputError),
    ContainerInit(&'static str),
    MissingPeripheral(&'static str),
}

impl core::fmt::Display for BuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            BuildError::UartInit(e) => write!(f, "UART initialization failed: {:?}", e),
            BuildError::RoasterInit(e) => write!(f, "Roaster initialization failed: {:?}", e),
            BuildError::FanInit(e) => write!(f, "Fan controller initialization failed: {:?}", e),
            BuildError::SsrInit(e) => write!(f, "SSR control initialization failed: {:?}", e),
            BuildError::ArtisanInit(e) => write!(f, "Artisan input initialization failed: {:?}", e),
            BuildError::ContainerInit(e) => {
                write!(f, "Service container initialization failed: {}", e)
            }
            BuildError::MissingPeripheral(name) => {
                write!(f, "Missing required peripheral: {}", name)
            }
        }
    }
}

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
