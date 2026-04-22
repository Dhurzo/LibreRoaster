#![cfg_attr(target_arch = "riscv32", no_std)]
#![cfg_attr(target_arch = "riscv32", no_main)]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for duration of a data transfer."
)]
extern crate alloc;
#[cfg(target_arch = "riscv32")]
use esp_backtrace as _;

#[cfg(not(target_arch = "riscv32"))]
fn main() {}

#[cfg(target_arch = "riscv32")]
use embassy_executor::Spawner;

#[cfg(target_arch = "riscv32")]
use log::info;

#[cfg(target_arch = "riscv32")]
use esp_hal::gpio::{Level, Output, OutputConfig};

#[cfg(target_arch = "riscv32")]
use esp_bootloader_esp_idf;

#[cfg(target_arch = "riscv32")]
esp_bootloader_esp_idf::esp_app_desc!();

#[cfg(target_arch = "riscv32")]
use libreroaster::application::AppBuilder;
#[cfg(target_arch = "riscv32")]
use libreroaster::output::artisan::ArtisanFormatter;

#[cfg(target_arch = "riscv32")]
use esp_hal::peripherals::Peripherals;

#[cfg(target_arch = "riscv32")]
use libreroaster::error::app_error::{AppError, InitError};
#[cfg(target_arch = "riscv32")]
use libreroaster::hardware::init::InitPeripherals;
#[cfg(target_arch = "riscv32")]
use libreroaster::logging::traceability::{trace_safe_shutdown_guard, TraceId};

#[cfg(target_arch = "riscv32")]
use core::fmt::Write;

#[cfg(target_arch = "riscv32")]
fn format_init_error(error: &InitError) -> heapless::String<256> {
    let mut buf = heapless::String::<256>::new();
    let (what, reason) = match error {
        InitError::ServiceContainer { what, reason } => (what, reason.as_str()),
        InitError::HardwareInit { what, reason } => (what, reason.as_str()),
        InitError::TaskSpawn { what, reason } => (what, reason.as_str()),
        InitError::MemoryAllocation { what, reason } => (what, reason.as_str()),
    };
    let _ = core::write!(&mut buf, "safe_shutdown: {} - {}", what, reason);
    buf
}

#[cfg(target_arch = "riscv32")]
async fn enter_safe_shutdown(error: InitError) -> ! {
    // Log the InitError diagnostics for telemetry/TRACE correlation
    let error_msg = format_init_error(&error);
    log::error!("safe_shutdown: {} - entering error loop", error_msg);

    // Emit host-facing error event using Artisan protocol format
    let artisan_err = ArtisanFormatter::format_err(99, &error_msg);
    log::error!("{}", artisan_err);

    let app_error = AppError::Initialization { source: error };
    trace_safe_shutdown_guard(TraceId::next(), Some(&app_error));

    // Blink GPIO8 LED to indicate error (3 short blinks, pause, repeat)
    let peripherals = unsafe { Peripherals::steal() };
    let mut led = Output::new(peripherals.GPIO8, Level::High, OutputConfig::default());

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

    // Prepare peripherals for init_hardware
    let init_peripherals = InitPeripherals {
        ledc: peripherals.LEDC,
        spi2: peripherals.SPI2,
        gpio9: peripherals.GPIO9,
        gpio10: peripherals.GPIO10,
        gpio4: peripherals.GPIO4,
        gpio3: peripherals.GPIO3,
        gpio1: peripherals.GPIO1,
    };

    // Initialize all hardware (returns Result, no panics)
    let hw_handles = match libreroaster::hardware::init::init_hardware(init_peripherals) {
        Ok(handles) => handles,
        Err(e) => enter_safe_shutdown(e).await,
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
            enter_safe_shutdown(e.into()).await;
        }
    };

    // Start tasks - this should never return
    let _ = app.start_tasks(spawner).await;

    // If we somehow get here, panic
    enter_safe_shutdown(InitError::TaskSpawn {
        what: "main",
        reason: alloc::string::String::from("Application tasks returned unexpectedly"),
    }).await;
}
