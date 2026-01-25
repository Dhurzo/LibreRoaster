#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::timer::timg::TimerGroup;
use log::info;

extern crate alloc;

use libreroaster::application::AppBuilder;
use libreroaster::output::ArtisanFormatter;

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 66320);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    info!(
        "LibreRoaster started - Artisan+ UART control ready:\n
            Wake the f*** up samurai we have beans to burn!"
    );

    // Extract needed peripherals before moving the struct
    let uart0 = peripherals.UART0;
    let ledc = peripherals.LEDC;
    let gpio8 = peripherals.GPIO8;
    let _gpio2 = peripherals.GPIO2;
    
    // Build and initialize application using AppBuilder
    let app = AppBuilder::new()
        .with_uart(uart0)                 // UART with GPIO21/22 pins
        .with_ledc(ledc, gpio8)           // LEDC for fan PWM control
        .with_formatter(ArtisanFormatter::new())
        .build()
        .expect("Failed to build application");

    // Start all application tasks
    app.start_tasks(spawner).await
        .expect("Failed to start application tasks");

    loop {
        Timer::after(Duration::from_secs(5)).await;
        info!("Heartbeat - LibreRoaster running with Artisan+ control");
    }
}


