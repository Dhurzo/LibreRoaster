//! Application builder and orchestration entry point.
//!
//! `AppBuilder` collects hardware handles (heater, fan, sensors, UART, status
//! LED) and assembles them into a `RoasterControl` plus the `ServiceContainer`
//! singletons, all before the Embassy executor spawns. `Application` then
//! verifies initialization and (on embedded) starts the five long-lived tasks.

use crate::application::service_container::ServiceContainer;
use crate::control::traits::{Fan, Heater};
use crate::control::RoasterControl;
#[cfg(all(target_arch = "riscv32", not(feature = "simulated-sensors")))]
use crate::hardware::max31856::{bt_spi::BtSpi, et_spi::EtSpi, Max31856};
use crate::hardware::sensors::SensorConversionHub;
#[cfg(target_arch = "riscv32")]
use crate::hardware::uart::initialize_uart_system;
use crate::input::ArtisanInput;
#[cfg(target_arch = "riscv32")]
use embassy_executor::Spawner;
#[cfg(target_arch = "riscv32")]
use esp_hal::gpio::Output;
#[cfg(target_arch = "riscv32")]
use esp_hal::peripherals::UART0;

use crate::safety::watchdog::{WatchdogError, WatchdogFeeder};
use alloc::boxed::Box;
use log::info;

#[cfg(feature = "simulated-sensors")]
use crate::hardware::sensors::SimulatedSensorSource;

/// Fluent builder collecting hardware handles and producing an `Application`.
pub struct AppBuilder {
    /// UART0 peripheral — `None` on host or before `with_uart`; `Some` on embedded after.
    #[cfg(target_arch = "riscv32")]
    uart0: Option<UART0<'static>>,
    /// UART RX pin (GPIO20) — `None` until `with_uart_pins`.
    #[cfg(target_arch = "riscv32")]
    uart_rx: Option<esp_hal::peripherals::GPIO20<'static>>,
    /// UART TX pin (GPIO21) — `None` until `with_uart_pins`.
    #[cfg(target_arch = "riscv32")]
    uart_tx: Option<esp_hal::peripherals::GPIO21<'static>>,
    heater: Option<Box<dyn Heater + Send>>,
    fan: Option<Box<dyn Fan + Send>>,
    sensor_hub: Option<SensorConversionHub>,
    /// BUG-06: status LED handle (embedded-only) — `None` until `with_status_led`.
    #[cfg(target_arch = "riscv32")]
    status_led: Option<Output<'static>>,
}

impl Default for AppBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl AppBuilder {
    /// Returns an empty builder; call `with_*` setters before `build`.
    pub fn new() -> Self {
        Self {
            #[cfg(target_arch = "riscv32")]
            uart0: None,
            #[cfg(target_arch = "riscv32")]
            uart_rx: None,
            #[cfg(target_arch = "riscv32")]
            uart_tx: None,
            heater: None,
            fan: None,
            sensor_hub: None,
            #[cfg(target_arch = "riscv32")]
            status_led: None,
        }
    }

    /// BUG-06: hand the status LED handle to the service container so the
    /// control-loop task can drive it (embedded-only).
    #[cfg(target_arch = "riscv32")]
    pub fn with_status_led(mut self, led: Output<'static>) -> Self {
        self.status_led = Some(led);
        self
    }

    /// Attaches the UART0 peripheral used by the UART reader task.
    #[cfg(target_arch = "riscv32")]
    pub fn with_uart(mut self, uart0: UART0<'static>) -> Self {
        self.uart0 = Some(uart0);
        self
    }

    /// Attaches the UART RX/TX GPIO pins (GPIO20/GPIO21) for the UART transport.
    #[cfg(target_arch = "riscv32")]
    pub fn with_uart_pins(
        mut self,
        rx: esp_hal::peripherals::GPIO20<'static>,
        tx: esp_hal::peripherals::GPIO21<'static>,
    ) -> Self {
        self.uart_rx = Some(rx);
        self.uart_tx = Some(tx);
        self
    }

    /// Installs the real SSR heater actuator (boxed as a `Heater`).
    pub fn with_real_ssr<H>(mut self, ssr: H) -> Self
    where
        H: Heater + Send + 'static,
    {
        self.heater = Some(Box::new(ssr));
        self
    }

    /// Installs the PWM fan actuator (boxed as a `Fan`).
    pub fn with_fan_control<F>(mut self, fan: F) -> Self
    where
        F: Fan + Send + 'static,
    {
        self.fan = Some(Box::new(fan));
        self
    }

    /// Installs the two MAX31856 thermocouples (bean + environment) on real SPI.
    #[cfg(all(target_arch = "riscv32", not(feature = "simulated-sensors")))]
    pub fn with_temperature_sensors(
        self,
        bean_sensor: Max31856<BtSpi>,
        env_sensor: Max31856<EtSpi>,
    ) -> Self {
        let hub = SensorConversionHub::new(bean_sensor, env_sensor);
        self.with_sensor_conversion_hub(hub)
    }

    /// Installs synthetic temperature curves for host-side L3 pipeline tests.
    #[cfg(feature = "simulated-sensors")]
    pub fn with_simulated_sensors(self) -> Self {
        let source = SimulatedSensorSource::default_curve();
        let hub = SensorConversionHub::new_simulated(source);
        self.with_sensor_conversion_hub(hub)
    }

    /// Installs a pre-built `SensorConversionHub`.
    pub fn with_sensor_conversion_hub(mut self, hub: SensorConversionHub) -> Self {
        self.sensor_hub = Some(hub);
        self
    }

    /// Assembles `RoasterControl` + container singletons and returns an `Application`.
    pub fn build(self) -> Result<Application, BuildError> {
        #[cfg(target_arch = "riscv32")]
        if let (Some(uart0), Some(rx), Some(tx)) = (self.uart0, self.uart_rx, self.uart_tx) {
            initialize_uart_system(uart0, rx, tx).map_err(BuildError::UartInit)?;
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

        ServiceContainer::init_roaster(roaster);
        ServiceContainer::init_artisan_input(artisan_input);

        // BUG-06: install the status LED into the container (embedded-only).
        #[cfg(target_arch = "riscv32")]
        if let Some(led) = self.status_led {
            ServiceContainer::init_status_led(led);
        }

        ServiceContainer::init_multiplexer();
        let watchdog = WatchdogFeeder::initialize().map_err(BuildError::WatchdogInit)?;
        ServiceContainer::get_instance().init_watchdog(watchdog);

        info!("Application components initialized successfully");

        Ok(Application { built: true })
    }
}

/// Built application: holds the verified init flag used by `start_tasks`.
pub struct Application {
    built: bool,
}

impl Application {
    /// Confirms the builder ran and the service container is fully initialized.
    pub fn verify_initialization(&self) -> Result<(), VerificationError> {
        if !self.built {
            return Err(VerificationError::NotBuilt);
        }

        if !ServiceContainer::is_initialized() {
            return Err(VerificationError::ServicesNotInitialized);
        }

        Ok(())
    }

    /// Spawns the USB/UART readers, dual-output, control loop, and regression tasks.
    #[cfg(target_arch = "riscv32")]
    pub async fn start_tasks(&self, spawner: Spawner) -> Result<(), TaskError> {
        use crate::hardware::uart::tasks::uart_reader_task;
        use crate::hardware::usb_cdc::tasks::usb_reader_task;

        self.verify_initialization()
            .map_err(TaskError::VerificationFailed)?;

        spawner
            .spawn(uart_reader_task())
            .map_err(TaskError::SpawnFailed)?;
        spawner
            .spawn(usb_reader_task())
            .map_err(TaskError::SpawnFailed)?;

        spawner
            .spawn(super::dual_output_task())
            .map_err(TaskError::SpawnFailed)?;

        spawner
            .spawn(super::control_loop_task())
            .map_err(TaskError::SpawnFailed)?;

        spawner
            .spawn(crate::safety::regression::regression_task())
            .map_err(TaskError::SpawnFailed)?;

        info!("All application tasks started successfully");
        Ok(())
    }
}

/// Failure kinds returned by `AppBuilder::build`.
#[derive(Debug, Clone, PartialEq)]
pub enum BuildError {
    /// UART subsystem initialization failed.
    UartInit(crate::hardware::uart::UartError),
    /// `RoasterControl::new` failed.
    RoasterInit(crate::control::RoasterError),
    /// Artisan input parser initialization failed.
    ArtisanInit(crate::input::InputError),
    /// RTC watchdog initialization failed.
    WatchdogInit(WatchdogError),
    /// A container install step failed (reason token).
    ContainerInit(&'static str),
    /// A required peripheral/actuator was not supplied via `with_*`.
    MissingPeripheral(&'static str),
}

impl core::fmt::Display for BuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            BuildError::UartInit(e) => write!(f, "UART initialization failed: {:?}", e),
            BuildError::RoasterInit(e) => write!(f, "Roaster initialization failed: {:?}", e),
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

/// Failure kinds returned by `Application::verify_initialization`.
#[derive(Debug, Clone, PartialEq)]
pub enum VerificationError {
    /// `build` was never called (no `Application` produced).
    NotBuilt,
    /// The service container is missing a required component.
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

/// Failure kinds returned by `Application::start_tasks`.
#[derive(Debug)]
pub enum TaskError {
    /// `verify_initialization` rejected the start request.
    VerificationFailed(VerificationError),
    /// An Embassy `spawn` call returned an error.
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // ── BuildError Display ──────────────────────

    #[test]
    fn build_error_uart_init_display() {
        let err = BuildError::UartInit(crate::hardware::uart::UartError::InitError);
        let msg = format!("{}", err);
        assert!(msg.contains("UART"));
        assert!(msg.contains("InitError"));
    }

    #[test]
    fn build_error_roaster_init_display() {
        let err = BuildError::RoasterInit(crate::control::RoasterError::SensorFault {
            source: Some("test"),
        });
        let msg = format!("{}", err);
        assert!(msg.contains("Roaster"));
        assert!(msg.contains("SensorFault"));
    }

    #[test]
    fn build_error_artisan_init_display() {
        let err = BuildError::ArtisanInit(crate::input::InputError::ParseError);
        let msg = format!("{}", err);
        assert!(msg.contains("Artisan"));
    }

    #[test]
    fn build_error_watchdog_init_display() {
        let err = BuildError::WatchdogInit(WatchdogError::InitializationFailed);
        let msg = format!("{}", err);
        assert!(msg.contains("Watchdog"));
    }

    #[test]
    fn build_error_container_init_display() {
        let err = BuildError::ContainerInit("test component");
        let msg = format!("{}", err);
        assert!(msg.contains("container"));
        assert!(msg.contains("test component"));
    }

    #[test]
    fn build_error_missing_peripheral_display() {
        let err = BuildError::MissingPeripheral("SSR Heater");
        let msg = format!("{}", err);
        assert!(msg.contains("Missing"));
        assert!(msg.contains("SSR Heater"));
    }

    // ── BuildError PartialEq ────────────────────

    #[test]
    fn build_error_equality() {
        let a = BuildError::MissingPeripheral("fan");
        let b = BuildError::MissingPeripheral("fan");
        let c = BuildError::MissingPeripheral("heater");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    // ── VerificationError Display ───────────────

    #[test]
    fn verification_error_not_built_display() {
        let err = VerificationError::NotBuilt;
        let msg = format!("{}", err);
        assert!(msg.contains("not built"));
    }

    #[test]
    fn verification_error_services_not_initialized_display() {
        let err = VerificationError::ServicesNotInitialized;
        let msg = format!("{}", err);
        assert!(msg.contains("not initialized"));
    }

    #[test]
    fn verification_error_equality() {
        assert_eq!(VerificationError::NotBuilt, VerificationError::NotBuilt);
        assert_ne!(
            VerificationError::NotBuilt,
            VerificationError::ServicesNotInitialized
        );
    }

    // ── TaskError Display ───────────────────────

    #[test]
    fn task_error_verification_failed_display() {
        let err = TaskError::VerificationFailed(VerificationError::NotBuilt);
        let msg = format!("{}", err);
        assert!(msg.contains("Verification"));
    }

    #[test]
    fn task_error_spawn_failed_display() {
        use embassy_executor::SpawnError;
        let err = TaskError::SpawnFailed(SpawnError::Busy);
        let msg = format!("{}", err);
        assert!(msg.contains("spawn"));
    }

    #[test]
    fn task_error_debug_output() {
        use embassy_executor::SpawnError;
        let err = TaskError::SpawnFailed(SpawnError::Busy);
        let msg = format!("{:?}", err);
        assert!(msg.contains("SpawnFailed") || msg.contains("Busy"));
    }
}
