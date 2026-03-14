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
use libreroaster::hardware::ledc_bus::LedcBus;

#[cfg(target_arch = "riscv32")]
use esp_hal::ledc::{Ledc, LowSpeed, LSGlobalClkSource};
#[cfg(target_arch = "riscv32")]
use esp_hal::ledc::channel;
#[cfg(target_arch = "riscv32")]
use esp_hal::ledc::timer::{self, TimerIFace};
#[cfg(target_arch = "riscv32")]
use esp_hal::time::Rate;

#[cfg(target_arch = "riscv32")]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    info!("LibreRoaster v5.1 starting...");

    // Initialize with default config
    let config = esp_hal::Config::default();
    let peripherals = esp_hal::init(config);

    info!("Hardware initialized");

    // ========== Initialize LEDC ==========
    let mut ledc = Ledc::new(peripherals.LEDC);
    ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);
    info!("LEDC peripheral acquired");

    // Timer 0: SSR (1 Hz - slow PWM for heater control)
    let mut timer0 = ledc.timer::<LowSpeed>(timer::Number::Timer0);
    timer0.configure(timer::config::Config {
        duty: timer::config::Duty::Duty8Bit,
        clock_source: timer::LSClockSource::APBClk,
        frequency: Rate::from_hz(1),
    }).unwrap();
    info!("Timer 0 configured (SSR, 1 Hz)");

    // Timer 1: Fan (25 kHz - fast PWM for fan speed)
    let mut timer1 = ledc.timer::<LowSpeed>(timer::Number::Timer1);
    timer1.configure(timer::config::Config {
        duty: timer::config::Duty::Duty8Bit,
        clock_source: timer::LSClockSource::APBClk,
        frequency: Rate::from_hz(25000),
    }).unwrap();
    info!("Timer 1 configured (Fan, 25 kHz)");

    // ========== Configure Channels ==========
    // Fan channel on GPIO9
    let fan_pin = Output::new(peripherals.GPIO9, Level::Low, OutputConfig::default());
    let fan_channel = ledc.channel(channel::Number::Channel0, fan_pin);
    
    // SSR channel on GPIO10
    let ssr_pin = Output::new(peripherals.GPIO10, Level::Low, OutputConfig::default());
    let ssr_channel = ledc.channel(channel::Number::Channel1, ssr_pin);
    
    // Create LEDC bus with safety wrapper
    let ledc_bus = LedcBus::new(
        fan_channel,
        channel::Number::Channel0,
        ssr_channel,
        channel::Number::Channel1,
    );
    
    static LEDC_BUS: StaticCell<LedcBus<'static>> = StaticCell::new();
    let ledc_bus = LEDC_BUS.init(ledc_bus);
    info!("LEDC Bus initialized (Fan: GPIO9, SSR: GPIO10)");

    // ========== Initialize SPI ==========
    let spi = Spi::new(peripherals.SPI2, SpiConfig::default().with_frequency(Rate::from_khz(1000)))
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

    // ========== Initialize Temperature Sensors ==========
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
    info!("Temperature sensors initialized (BT: GPIO4, ET: GPIO3)");

    // ========== Initialize Heat Detection (GPIO1) ==========
    let heat_detection_pin = Input::new(peripherals.GPIO1, InputConfig::default().with_pull(Pull::Up));
    let heat_detected = heat_detection_pin.is_low();
    info!("Heat source detection (GPIO1): {}", if heat_detected { "DETECTED" } else { "NOT DETECTED" });

    // Get handles from LedcBus
    let ssr_handle = ledc_bus.ssr_handle();
    let fan_handle = ledc_bus.fan_handle();

    // ========== Initialize SSR Control ==========
    let real_ssr = match SsrControlSimple::new(heat_detection_pin, ssr_handle) {
        Ok(ssr) => ssr,
        Err(e) => {
            panic!("Failed to initialize SSR: {:?}", e);
        }
    };
    info!("SSR control initialized");

    // ========== Initialize Fan Controller ==========
    let fan_controller = match FanController::with_handle(fan_handle) {
        Ok(fan) => fan,
        Err(e) => {
            panic!("Failed to initialize fan: {:?}", e);
        }
    };
    info!("Fan controller initialized");

    // Initialize USB CDC
    let _ = libreroaster::hardware::usb_cdc::initialize_usb_cdc_system(peripherals.USB_DEVICE);
    info!("USB CDC initialized");

    info!("Wake the f*** up samurai we have beans to burn!");

    // ========== Build and Start Application ==========
    let app = match AppBuilder::new()
        .with_uart(peripherals.UART0)
        .with_real_ssr(real_ssr)
        .with_fan_control(fan_controller)
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
    let _ = app.start_tasks(spawner).await;
    
    // If we somehow get here, panic
    panic!("Application tasks returned unexpectedly");
}
