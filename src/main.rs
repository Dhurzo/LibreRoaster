//! Firmware entry point for the LibreRoaster ESP32-C3 coffee-roaster controller.
//!
//! Boots the `no_std` binary on the ESP32-C3 (`riscv32`), initialises all hardware via
//! `libreroaster::hardware::init`, builds the application with `AppBuilder`, starts the
//! embassy executor under the ESP RTOS scheduler, and parks init failures into a
//! safe-shutdown blink loop. On non-embedded targets this file is a no-op stub so the
//! crate still compiles for the host test suite.

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
        // Feed the RWDT (already armed by init_hw_watchdog) so the
        // safe-shutdown blink pattern stays observable instead of the
        // ~2.2 s watchdog resetting the chip.
        libreroaster::safety::watchdog::feed_hw_watchdog();
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
            // The RWDT is armed in init_hw_watchdog() (called before
            // builder.build()). Feed every iteration so the operator can
            // read the error blink pattern instead of the ~2.2 s watchdog
            // resetting the chip.
            loop {
                libreroaster::safety::watchdog::feed_hw_watchdog();
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

    // Initialize the esp-println logger before any info!() calls.
    //
    // esp_println writes to the same physical channel as the Artisan
    // protocol (USB-Serial-JTAG on the C3 by default, or UART0). In
    // production we set the level filter to Warn so that the per-tick
    // info!/debug! chatter (a ~6/s MAX31856 read dump, plus control loop
    // telemetry) does NOT corrupt READ responses or continuous telemetry on
    // the wire. The `instrumentation` feature on a debug build raises the
    // filter to Debug — disable it for production flashes.
    //
    // The long-term direction is a custom `log::Log` that writes to a
    // *separate* UART1 on GPIO2, so logs and protocol never share a wire.
    // That change requires HW validation on the bench and is left until the
    // board is physically wired; reducing the level here is the safe interim.
    #[cfg(not(feature = "instrumentation"))]
    esp_println::logger::init_logger(log::LevelFilter::Warn);
    #[cfg(feature = "instrumentation")]
    esp_println::logger::init_logger(log::LevelFilter::Debug);

    info!("LibreRoaster v0.0.1 Alpha starting...");

    let init_peripherals = InitPeripherals {
        ledc: peripherals.LEDC,
        spi2: peripherals.SPI2,
        gpio9: peripherals.GPIO9,
        gpio10: peripherals.GPIO10,
        gpio8: peripherals.GPIO8,
        gpio7: peripherals.GPIO7,
        gpio6: peripherals.GPIO6,
        gpio5: peripherals.GPIO5,
        #[cfg(not(feature = "simulated-sensors"))]
        gpio4: peripherals.GPIO4,
        #[cfg(not(feature = "simulated-sensors"))]
        gpio3: peripherals.GPIO3,
        gpio1: peripherals.GPIO1,
    };

    let hw_handles = run_init_or_panic(libreroaster::hardware::init::init_hardware(
        init_peripherals,
    ));
    info!("Hardware initialized");

    #[cfg(not(feature = "simulated-sensors"))]
    info!("Sensors initialized (BT: GPIO4, ET: GPIO3)");
    #[cfg(feature = "simulated-sensors")]
    info!("Simulated sensors active (no real thermocouples required)");
    info!("SSR control initialized");
    info!("Fan controller initialized");

    libreroaster::safety::watchdog::init_hw_watchdog();
    info!("Hardware watchdog initialized (RTC WDT)");

    // The success log is emitted ONLY on Ok. On Err we log loudly but do NOT
    // halt: UART0 remains a fully functional transport, and on riscv32
    // `init_usb_cdc` is effectively infallible (StaticCell::init cannot
    // fail), so this branch is defensive — halting here would brick a
    // working UART session over an unreachable error.
    match libreroaster::hardware::usb_cdc::initialize_usb_cdc_system(peripherals.USB_DEVICE) {
        Ok(()) => info!("USB CDC initialized"),
        Err(e) => log::error!(
            "USB CDC initialization FAILED: {:?} — USB transport unavailable, UART0 remains active",
            e
        ),
    }

    info!("Wake the f*** up samurai we have beans to burn!");

    let app = {
        #[cfg(not(feature = "simulated-sensors"))]
        let builder = AppBuilder::new()
            .with_uart(peripherals.UART0)
            .with_uart_pins(peripherals.GPIO20, peripherals.GPIO21)
            .with_real_ssr(hw_handles.ssr)
            .with_fan_control(hw_handles.fan)
            .with_temperature_sensors(hw_handles.bean_sensor, hw_handles.env_sensor)
            .with_status_led(hw_handles.status_led);

        #[cfg(feature = "simulated-sensors")]
        let builder = AppBuilder::new()
            .with_uart(peripherals.UART0)
            .with_uart_pins(peripherals.GPIO20, peripherals.GPIO21)
            .with_real_ssr(hw_handles.ssr)
            .with_fan_control(hw_handles.fan)
            .with_simulated_sensors()
            .with_status_led(hw_handles.status_led);

        match builder.build() {
            Ok(app) => app,
            Err(e) => {
                log::error!("AppBuilder failed: {:?}", e);
                // Keep the RWDT fed while we halt so the error stays
                // observable instead of the watchdog resetting the system
                // every ~2.2 s.
                loop {
                    libreroaster::safety::watchdog::feed_hw_watchdog();
                    esp_hal::rom::ets_delay_us(1_000_000);
                }
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

    static APPLICATION: static_cell::StaticCell<libreroaster::application::Application> =
        static_cell::StaticCell::new();
    let app = APPLICATION.init(app);

    executor.run(|spawner| {
        spawner.must_spawn(async_main_task(app, spawner));
    })
}
