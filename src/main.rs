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
use esp_alloc as _;

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
    let error_msg = format_init_error(&error);
    log::error!("safe_shutdown: {} - entering error loop", error_msg);

    let artisan_err = ArtisanFormatter::format_err(99, &error_msg);
    log::error!("{}", artisan_err);

    let app_error = AppError::Initialization { source: error };
    trace_safe_shutdown_guard(TraceId::next(), Some(&app_error));

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
#[embassy_executor::task]
async fn async_main_task(
    app: &'static mut libreroaster::application::Application,
    spawner: Spawner,
) -> ! {
    if let Err(e) = app.start_tasks(spawner).await {
        enter_safe_shutdown(InitError::TaskSpawn {
            what: "main",
            reason: alloc::format!("{:?}", e),
        })
        .await;
    }

    // All tasks spawned successfully — this task sleeps forever
    loop {
        embassy_time::Timer::after(embassy_time::Duration::from_secs(86400)).await;
    }
}

#[cfg(target_arch = "riscv32")]
fn run_init_or_panic<T>(result: Result<T, InitError>) -> T {
    match result {
        Ok(v) => v,
        Err(e) => {
            let error_msg = format_init_error(&e);
            log::error!("safe_shutdown: {} - halting", error_msg);
            loop {
                esp_hal::rom::ets_delay_us(1_000_000);
            }
        }
    }
}

#[cfg(target_arch = "riscv32")]
#[esp_hal::main]
fn main() -> ! {
    let config = esp_hal::Config::default();
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 72 * 1024);

    // Initialize the esp-println logger before any info!() calls
    esp_println::logger::init_logger(log::LevelFilter::Info);

    info!("LibreRoaster v5.1 starting...");
    info!("Hardware initialized");

    let init_peripherals = InitPeripherals {
        ledc: peripherals.LEDC,
        spi2: peripherals.SPI2,
        gpio9: peripherals.GPIO9,
        gpio10: peripherals.GPIO10,
        gpio8: peripherals.GPIO8,
        gpio7: peripherals.GPIO7,
        gpio6: peripherals.GPIO6,
        gpio5: peripherals.GPIO5,
        gpio4: peripherals.GPIO4,
        gpio3: peripherals.GPIO3,
        gpio1: peripherals.GPIO1,
    };

    let hw_handles = run_init_or_panic(libreroaster::hardware::init::init_hardware(
        init_peripherals,
    ));

    info!("Sensors initialized (BT: GPIO4, ET: GPIO3)");
    info!("SSR control initialized");
    info!("Fan controller initialized");

    libreroaster::safety::watchdog::init_hw_watchdog();
    info!("Hardware watchdog initialized (RTC WDT)");

    let _ = libreroaster::hardware::usb_cdc::initialize_usb_cdc_system(peripherals.USB_DEVICE);
    info!("USB CDC initialized");

    info!("Wake the f*** up samurai we have beans to burn!");

    let mut app = match AppBuilder::new()
        .with_uart(peripherals.UART0)
        .with_uart_pins(peripherals.GPIO20, peripherals.GPIO21)
        .with_real_ssr(hw_handles.ssr)
        .with_fan_control(hw_handles.fan)
        .with_temperature_sensors(hw_handles.bean_sensor, hw_handles.env_sensor)
        .with_formatter(ArtisanFormatter::new())
        .build()
    {
        Ok(app) => app,
        Err(e) => {
            log::error!("AppBuilder failed: {:?}", e);
            loop {
                esp_hal::rom::ets_delay_us(1_000_000);
            }
        }
    };

    // Start RTOS scheduler (must precede embassy executor)
    let timg0 = esp_hal::timer::timg::TimerGroup::new(peripherals.TIMG0);
    let sw_int =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_int.software_interrupt0);

    // Create and run embassy executor inside the RTOS main task
    static EXECUTOR: static_cell::StaticCell<esp_rtos::embassy::Executor> =
        static_cell::StaticCell::new();
    let executor = EXECUTOR.init(esp_rtos::embassy::Executor::new());

    // SAFETY: executor.run() never returns, so &mut app lives forever
    let app = unsafe { core::mem::transmute::<&mut _, &'static mut _>(&mut app) };

    executor.run(|spawner| {
        spawner.must_spawn(async_main_task(app, spawner));
    })
}
