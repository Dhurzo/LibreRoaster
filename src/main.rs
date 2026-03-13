#![cfg_attr(target_arch = "riscv32", no_std)]
#![cfg_attr(target_arch = "riscv32", no_main)]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for duration of a data transfer."
)]

#[cfg(target_arch = "riscv32")]
use esp_backtrace as _;

#[cfg(not(target_arch = "riscv32"))]
fn main() {}

#[cfg(target_arch = "riscv32")]
use embassy_executor::Spawner;

#[cfg(target_arch = "riscv32")]
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull};

#[cfg(target_arch = "riscv32")]
use esp_hal::spi::master::{Spi, Config as SpiConfig};

#[cfg(target_arch = "riscv32")]
use log::info;

#[cfg(target_arch = "riscv32")]
use esp_bootloader_esp_idf;

#[cfg(target_arch = "riscv32")]
esp_bootloader_esp_idf::esp_app_desc!();

#[cfg(target_arch = "riscv32")]
use static_cell::StaticCell;

#[cfg(target_arch = "riscv32")]
use core::cell::RefCell;

#[cfg(target_arch = "riscv32")]
use critical_section;

#[cfg(target_arch = "riscv32")]
use libreroaster::application::AppBuilder;
#[cfg(target_arch = "riscv32")]
use libreroaster::hardware::fan::FanController;
#[cfg(target_arch = "riscv32")]
use libreroaster::hardware::max31856::Max31856;
#[cfg(target_arch = "riscv32")]
use libreroaster::hardware::ssr::SsrControlSimple;
#[cfg(target_arch = "riscv32")]
use libreroaster::output::artisan::ArtisanFormatter;
#[cfg(target_arch = "riscv32")]
use libreroaster::hardware::shared_spi::SpiDeviceWithCs;
#[cfg(target_arch = "riscv32")]
use libreroaster::hardware::ledc_bus::LedcChannelHandle;

#[cfg(target_arch = "riscv32")]
static FAN_CELL: StaticCell<FanController<'static>> = StaticCell::new();

#[cfg(target_arch = "riscv32")]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    info!("LibreRoaster v5.1 starting...");

    // Initialize with default config
    let config = esp_hal::Config::default();
    let peripherals = esp_hal::init(config);

    info!("Hardware initialized");

    // Initialize SPI
    let spi = Spi::new(peripherals.SPI2, SpiConfig::default().with_frequency(esp_hal::time::Rate::from_khz(1000)))
        .expect("Failed to initialize SPI");

    static SPI_BUS: StaticCell<critical_section::Mutex<RefCell<Spi<esp_hal::Blocking>>>> =
        StaticCell::new();
    let spi_mutex = SPI_BUS.init(critical_section::Mutex::new(RefCell::new(spi)));
    info!("SPI initialized");

    // Create GPIO pins for SPI chip selects
    let bt_cs = Output::new(peripherals.GPIO4, Level::High, OutputConfig::default());
    let bt_spi = SpiDeviceWithCs::new(spi_mutex, bt_cs);

    let et_cs = Output::new(peripherals.GPIO3, Level::High, OutputConfig::default());
    let et_spi = SpiDeviceWithCs::new(spi_mutex, et_cs);

    // Initialize temperature sensors
    let bean_sensor = match Max31856::new(bt_spi) {
        Ok(sensor) => sensor,
        Err(e) => {
            panic!("Failed to init BT sensor: {:?}", e);
        }
    };
    let env_sensor = match Max31856::new(et_spi) {
        Ok(sensor) => sensor,
        Err(e) => {
            panic!("Failed to init ET sensor: {:?}", e);
        }
    };
    info!("Temperature sensors initialized");

    // Heat detection pin (GPIO1)
    let heat_detection_pin = Input::new(peripherals.GPIO1, InputConfig::default().with_pull(Pull::Up));
    let heat_detected = heat_detection_pin.is_low();
    info!("Heat source detection (GPIO1): {}", if heat_detected { "DETECTED" } else { "NOT DETECTED" });

    // NOTE: LEDC initialization needs updating for esp-hal 1.0 API
    info!("WARNING: LEDC not fully initialized - needs API update for esp-hal 1.0");

    // Create a minimal fan controller (will work without actual PWM for now)
    let fan_controller = match FanController::new() {
        Ok(fan) => fan,
        Err(e) => {
            panic!("Failed to initialize fan: {:?}", e);
        }
    };

    // NOTE: SSR control needs proper LEDC channel handle - using placeholder
    // In production, this needs proper LEDC initialization
    info!("NOTE: SSR control needs proper LEDC initialization - using safe defaults");

    // Move fan to static memory
    let static_fan = FAN_CELL.init(fan_controller);

    // Initialize USB CDC
    let _ = libreroaster::hardware::usb_cdc::initialize_usb_cdc_system(peripherals.USB_DEVICE);
    info!("USB CDC initialized");

    info!("Wake the f*** up samurai we have beans to burn!");

    // Build application without SSR (needs LEDC) - fan only for now
    let app = match AppBuilder::new()
        .with_uart(peripherals.UART0)
        .with_fan_control(static_fan)
        .with_temperature_sensors(bean_sensor, env_sensor)
        .with_formatter(ArtisanFormatter::new())
        .build()
    {
        Ok(app) => app,
        Err(e) => {
            panic!("Failed to build application: {:?}", e);
        }
    };

    // Start tasks - this should never return
    app.start_tasks(spawner).await;
    
    // If we somehow get here, panic
    panic!("Application tasks returned unexpectedly");
}
