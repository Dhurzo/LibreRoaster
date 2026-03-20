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
use crate::error::app_error::InitError;

#[cfg(target_arch = "riscv32")]
fn enter_safe_shutdown(error: InitError) -> ! {
    log::error!("Initialization failed: {:?}", error);

    // Blink GPIO8 LED to indicate error (3 short blinks, pause, repeat)
    let peripherals = esp_hal::Peripherals::take().unwrap();
    let mut led = Output::new(
        peripherals.GPIO8,
        Level::High,
        OutputConfig::default()
    );

    loop {
        for _ in 0..3 {
            led.set_low();
            embassy_time::Timer::after(embassy_time::Duration::from_millis(200)).await;
            led.set_high();
            embassy_time::Timer::after(embassy_time::Duration::from_millis(200)).await;
        }
        embassy_time::Timer::after(embassy_time::Duration::from_secs(1)).await;
    }
}

#[cfg(target_arch = "riscv32")]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    info!("LibreRoaster v5.1 starting...");

    // Initialize with default config
    let config = esp_hal::Config::default();
    let peripherals = esp_hal::init(config);

    info!("Hardware initialized");

    // Initialize all hardware (returns Result, no panics)
    let hw_handles = match libreroaster::hardware::init::init_hardware(peripherals) {
        Ok(handles) => handles,
        Err(e) => {
            enter_safe_shutdown(e);
        }
    };

    info!("Sensors initialized (BT: GPIO4, ET: GPIO3)");
    info!("SSR control initialized");
    info!("Fan controller initialized");

    // Initialize USB CDC
    let _ = libreroaster::hardware::usb_cdc::initialize_usb_cdc_system(peripherals.USB_DEVICE);
    info!("USB CDC initialized");

    info!("Wake the f*** up samurai we have beans to burn!");

    // ========== Build and Start Application ==========
    let app = match AppBuilder::new()
        .with_uart(peripherals.UART0)
        .with_real_ssr(hw_handles.ssr)
        .with_fan_control(hw_handles.fan)
        .with_temperature_sensors(hw_handles.bean_sensor, hw_handles.env_sensor)
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
