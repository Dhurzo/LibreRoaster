#![no_std]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::timer::timg::TimerGroup;
use log::info;

extern crate alloc;

use libreroaster::control::RoasterControl;

fn main() -> ! {
    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 66320);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let sw_interrupt =
        esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);

    info!("LibreRoaster started - Artisan+ control ready");

    let executor = embassy_executor::Executor::new();
    let spawner = executor.spawner();

    executor.run(|spawner| async {
        let _roaster = RoasterControl::new().unwrap();

        loop {
            Timer::after(Duration::from_secs(5)).await;
            info!("Heartbeat - LibreRoaster running");
        }
    })
}
