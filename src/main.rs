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
use libreroaster::hardware::uart::initialize_uart_system;
use libreroaster::hardware::uart::tasks::{uart_reader_task, uart_writer_task};
use libreroaster::input::ArtisanInput;
use libreroaster::output::artisan::ArtisanFormatter;

static mut ROASTER: Option<RoasterControl> = None;
static mut FAN: Option<FanController> = None;
static mut ARTISAN_INPUT: Option<ArtisanInput> = None;

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

    initialize_uart_system(peripherals.UART0).expect("Failed to initialize UART system");

    info!(
        "LibreRoaster started - Artisan+ UART control ready:\n
            Wake the f*** up samurai we have beans to burn!"
    );

    // Initialize components
    let roaster = RoasterControl::new().unwrap();
    let fan = FanController::new(()).unwrap();
    let artisan_input = ArtisanInput::new().unwrap();
    let formatter = ArtisanFormatter::new();

    // Store in static for shared access
    unsafe {
        ROASTER = Some(roaster);
        FAN = Some(fan);
        ARTISAN_INPUT = Some(artisan_input);
    }

    // Clone formatter for tasks
    let formatter2 = formatter.clone();
    let formatter3 = formatter.clone();

    // Start UART communication tasks
    spawner.spawn(uart_reader_task()).unwrap();
    spawner.spawn(uart_writer_task()).unwrap();

    // Spawn Artisan+ control loop
    spawner
        .spawn(control_loop(
            #[allow(static_mut_refs)]
            unsafe {
                ROASTER.as_mut().unwrap()
            },
            #[allow(static_mut_refs)]
            unsafe {
                FAN.as_mut().unwrap()
            },
            formatter2,
        ))
        .unwrap();
    spawner
        .spawn(artisan_uart_handler(
            #[allow(static_mut_refs)]
            unsafe {
                ROASTER.as_mut().unwrap()
            },
            #[allow(static_mut_refs)]
            unsafe {
                FAN.as_mut().unwrap()
            },
            formatter3,
        ))
        .unwrap();

    loop {
        Timer::after(Duration::from_secs(5)).await;
        info!("Heartbeat - LibreRoaster running with Artisan+ control");
    }
}

#[embassy_executor::task]
async fn control_loop(
    roaster: &'static mut RoasterControl,
    _fan: &'static mut FanController,
    _formatter: ArtisanFormatter,
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
async fn artisan_uart_handler(
    roaster: &'static mut RoasterControl,
    fan: &'static mut FanController,
    _formatter: ArtisanFormatter,
) {
    info!("Artisan+ UART Handler Started");

    let artisan_input = unsafe { ARTISAN_INPUT.as_mut().unwrap() };

    loop {
        Timer::after(Duration::from_millis(50)).await;

        match artisan_input.read_command().await {
            Ok(Some(command)) => {
                info!("Received Artisan+ command: {:?}", command);

                let result = match command {
                    libreroaster::config::ArtisanCommand::ReadStatus => {
                        let status = roaster.get_status();
                        let response =
                            ArtisanFormatter::format_read_response(&status, fan.get_speed());
                        artisan_input.send_response(&response).await
                    }
                    libreroaster::config::ArtisanCommand::StartRoast => {
                        let _ = roaster.process_artisan_command(command);
                        Ok(())
                    }
                    libreroaster::config::ArtisanCommand::SetHeater(value) => {
                        let _ = roaster.process_command(
                            libreroaster::config::RoasterCommand::SetHeaterManual(value),
                            embassy_time::Instant::now(),
                        );
                        Ok(())
                    }
                    libreroaster::config::ArtisanCommand::SetFan(value) => fan
                        .set_speed(value as f32)
                        .map_err(|_| libreroaster::input::InputError::UartError),
                    libreroaster::config::ArtisanCommand::EmergencyStop => {
                        let _ = roaster.process_command(
                            libreroaster::config::RoasterCommand::ArtisanEmergencyStop,
                            embassy_time::Instant::now(),
                        );
                        let _ = fan.disable();
                        Ok(())
                    }
                };

                if let Err(e) = result {
                    info!("Error processing command: {:?}", e);
                }
            }
            Ok(None) => {}
            Err(e) => {
                info!("UART command error: {:?}", e);
            }
        }
    }
}
