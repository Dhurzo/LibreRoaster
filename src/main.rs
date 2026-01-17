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

use libreroaster::control::RoasterControl;
use libreroaster::hardware::fan::FanController;
use libreroaster::output::artisan::ArtisanFormatter;

// This creates a default app-descriptor required by esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

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

    info!("LibreRoaster started - Artisan+ control ready");

    // Initialize components
    let mut roaster = RoasterControl::new().unwrap();
    let mut fan = FanController::new(()).unwrap();
    let formatter = ArtisanFormatter::new();

    info!("LibreRoaster Artisan+ Control System");
    info!("=====================================");
    info!("Supported Commands:");
    info!("  READ  -> Returns ET,BT,Power,Fan");
    info!("  OT1 x -> Set heater to x% (0-100)");
    info!("  IO3 x -> Set fan to x% (0-100)");
    info!("  STOP  -> Emergency stop");
    info!("=====================================");

    // Spawn Artisan+ tasks
    spawner
        .spawn(control_loop(&mut roaster, &mut fan, &formatter))
        .unwrap();
    spawner
        .spawn(artisan_demo(&mut roaster, &mut fan, &formatter))
        .unwrap();

    loop {
        Timer::after(Duration::from_secs(5)).await;
        info!("Heartbeat - LibreRoaster running with Artisan+ control");
    }
}

#[embassy_executor::task]
async fn control_loop(
    roaster: &mut RoasterControl,
    fan: &mut FanController,
    formatter: &ArtisanFormatter,
) {
    info!("Roaster control loop started");

    loop {
        let current_time = embassy_time::Instant::now();

        // For testing: simulate temperatures
        let bean_temp = 25.0; // Room temp
        let env_temp = 22.0; // Room temp

        // Update temperatures
        if let Err(e) = roaster.update_temperatures(bean_temp, env_temp, current_time) {
            info!("Temperature update error: {:?}", e);
        }

        // Update control logic
        if let Err(e) = roaster.update_control(current_time) {
            info!("Control update error: {:?}", e);
        }

        // Process output (Artisan+ data streaming)
        if let Err(e) = roaster.process_output().await {
            info!("Output processing error: {:?}", e);
        }

        Timer::after(Duration::from_millis(100)).await;
    }
}

#[embassy_executor::task]
async fn artisan_demo(roaster: &mut RoasterControl, fan: &mut FanController) {
    info!("Artisan+ Command Demo Started");

    // Wait for system to stabilize
    Timer::after(Duration::from_secs(3)).await;

    // Simulate Artisan+ commands
    info!("=== Artisan+ Command Demo ===");

    // Initial status (READ command simulation)
    let status = roaster.get_status();
    let response = formatter.format_read_response(&status, fan.get_speed());
    info!("READ Response: {}", response);

    // OT1 command - Set heater to 75%
    info!("Command: OT1 75 -> Setting heater to 75%");
    let _ = roaster.process_command(
        libreroaster::config::RoasterCommand::SetHeaterManual(75),
        embassy_time::Instant::now(),
    );

    Timer::after(Duration::from_secs(2)).await;

    // IO3 command - Set fan to 50%
    info!("Command: IO3 50 -> Setting fan to 50%");
    let _ = fan.set_speed(50.0);

    Timer::after(Duration::from_secs(2)).await;

    // Updated status
    let status = roaster.get_status();
    let response = formatter.format_read_response(&status, fan.get_speed());
    info!("Updated READ Response: {}", response);

    // OT1 command - Set heater to 25%
    info!("Command: OT1 25 -> Setting heater to 25%");
    let _ = roaster.process_command(
        libreroaster::config::RoasterCommand::SetHeaterManual(25),
        embassy_time::Instant::now(),
    );

    Timer::after(Duration::from_secs(2)).await;

    // STOP command - Emergency stop
    info!("Command: STOP -> Emergency stop");
    let _ = roaster.process_command(
        libreroaster::config::RoasterCommand::ArtisanEmergencyStop,
        embassy_time::Instant::now(),
    );
    let _ = fan.disable();

    Timer::after(Duration::from_secs(2)).await;

    // Final status
    let status = roaster.get_status();
    let response = formatter.format_read_response(&status, fan.get_speed());
    info!("Final READ Response: {}", response);

    info!("=== Demo Complete ===");
    info!("Artisan+ system fully operational.");

    // Keep demo task alive
    loop {
        Timer::after(Duration::from_secs(30)).await;
        info!("Artisan+ demo complete - monitoring active");
    }
}
