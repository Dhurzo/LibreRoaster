use crate::application::service_container::ServiceContainer;
use crate::control::traits::{Fan, Heater};
use crate::control::RoasterControl;
#[cfg(target_arch = "riscv32")]
use crate::hardware::max31856::{bt_spi::BtSpi, et_spi::EtSpi, Max31856};
use crate::hardware::sensors::SensorConversionHub;
use crate::hardware::uart::initialize_uart_system;
use crate::input::ArtisanInput;
use crate::output::artisan::ArtisanFormatter;
use embassy_executor::Spawner;
use esp_hal::peripherals::UART0;

use crate::safety::regression;
use crate::safety::watchdog::{WatchdogError, WatchdogFeeder};
use alloc::boxed::Box;
use critical_section;
use log::info;

pub struct AppBuilder<'a> {
    uart0: Option<UART0<'a>>,
    formatter: Option<ArtisanFormatter>,
    heater: Option<Box<dyn Heater + Send>>,
    fan: Option<Box<dyn Fan + Send>>,
    sensor_hub: Option<SensorConversionHub>,
}

impl<'a> Default for AppBuilder<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> AppBuilder<'a> {
    pub fn new() -> Self {
        Self {
            uart0: None,
            formatter: None,
            heater: None,
            fan: None,
            sensor_hub: None,
        }
    }

    pub fn with_uart(mut self, uart0: UART0<'a>) -> Self {
        self.uart0 = Some(uart0);
        self
    }

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

    #[cfg(target_arch = "riscv32")]
    pub fn with_temperature_sensors(
        self,
        bean_sensor: Max31856<BtSpi>,
        env_sensor: Max31856<EtSpi>,
    ) -> Self {
        let hub = SensorConversionHub::new(bean_sensor, env_sensor);
        self.with_sensor_conversion_hub(hub)
    }

    pub fn with_sensor_conversion_hub(mut self, hub: SensorConversionHub) -> Self {
        self.sensor_hub = Some(hub);
        self
    }

    pub fn with_formatter(mut self, formatter: ArtisanFormatter) -> Self {
        self.formatter = Some(formatter);
        self
    }

    pub fn build(self) -> Result<Application, BuildError> {
        if let Some(uart0) = self.uart0 {
            initialize_uart_system(uart0).map_err(BuildError::UartInit)?;
        }

        let fan: Box<dyn Fan + Send> = self
            .fan
            .ok_or(BuildError::MissingPeripheral("Fan Controller"))?;

        let heater = self
            .heater
            .ok_or(BuildError::MissingPeripheral("SSR Heater"))?;
        let sensor_hub = self
            .sensor_hub
            .ok_or(BuildError::MissingPeripheral("Sensor Conversion Hub"))?;

        let roaster =
            RoasterControl::new(heater, fan, sensor_hub).map_err(BuildError::RoasterInit)?;

        let artisan_input = ArtisanInput::new().map_err(BuildError::ArtisanInit)?;
        let formatter = self.formatter.unwrap_or_default();

        critical_section::with(|cs| {
            let container = crate::application::service_container::ServiceContainer::get_instance();
            container
                .roaster_sync
                .borrow(cs)
                .borrow_mut()
                .replace(roaster);
            container
                .artisan_input
                .borrow(cs)
                .borrow_mut()
                .replace(artisan_input);
        });

        ServiceContainer::init_multiplexer();
        let watchdog = WatchdogFeeder::initialize().map_err(BuildError::WatchdogInit)?;
        ServiceContainer::get_instance().init_watchdog(watchdog);

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
        use crate::hardware::uart::tasks::{queue_processor_task, uart_reader_task};
        use crate::hardware::usb_cdc::tasks::{usb_queue_processor_task, usb_reader_task};

        self.verify_initialization()
            .map_err(TaskError::VerificationFailed)?;

        spawner
            .spawn(uart_reader_task())
            .map_err(TaskError::SpawnFailed)?;
        spawner
            .spawn(usb_reader_task())
            .map_err(TaskError::SpawnFailed)?;

        // Spawn queue processor tasks to consume commands from queues
        spawner
            .spawn(queue_processor_task())
            .map_err(TaskError::SpawnFailed)?;
        spawner
            .spawn(usb_queue_processor_task())
            .map_err(TaskError::SpawnFailed)?;

        spawner
            .spawn(super::dual_output_task())
            .map_err(TaskError::SpawnFailed)?;

        spawner
            .spawn(super::control_loop_task())
            .map_err(TaskError::SpawnFailed)?;

        #[cfg(target_arch = "riscv32")]
        {
            spawner
                .spawn(regression::regression_task())
                .map_err(TaskError::SpawnFailed)?;
        }

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
    WatchdogInit(WatchdogError),
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
            BuildError::WatchdogInit(e) => write!(f, "Watchdog initialization failed: {:?}", e),
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
            TaskError::VerificationFailed(e) => write!(f, "Verification failed: {:?}", e),
            TaskError::SpawnFailed(e) => write!(f, "Failed to spawn task: {:?}", e),
        }
    }
}
