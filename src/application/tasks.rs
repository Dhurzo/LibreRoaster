use crate::application::service_container::ServiceContainer;
use crate::output::artisan::MutableArtisanFormatter;
use embassy_executor::task;
use embassy_time::{Duration, Instant, Timer};
use log::{debug, info, warn};

#[task]
pub async fn control_loop_task() {
    info!("Roaster control loop started - Artisan+ integration ACTIVE");

    let mut formatter = MutableArtisanFormatter::new();
    let _start_time = Instant::now();
    let cmd_channel = ServiceContainer::get_artisan_channel();
    let output_channel = ServiceContainer::get_output_channel();

    loop {
        let current_time = Instant::now();

        // 1. Process any pending Artisan commands from UART
        if let Ok(command) = cmd_channel.try_receive() {
            let _ = ServiceContainer::with_roaster(|roaster| {
                if let Err(e) = roaster.process_artisan_command(command) {
                    warn!("Failed to process Artisan command: {:?}", e);
                } else {
                    debug!("Processed Artisan command successfully");
                }
            });
        }

        // 2. Execute control logic
        let control_result = ServiceContainer::with_roaster(
            |roaster: &mut crate::control::RoasterControl| -> Result<(), ()> {
                match roaster.read_sensors() {
                    Ok(()) => {
                        debug!("Sensors: BT: {:.1}°C, ET: {:.1}°C",
                            roaster.get_status().bean_temp,
                            roaster.get_status().env_temp);
                    }
                    Err(e) => {
                        warn!("Sensor read error: {:?}", e);
                    }
                }
                match roaster.update_control(current_time) {
                    Ok(output) => {
                        debug!("Control: SSR {:.1}%, Fan {:.1}%",
                            output, roaster.get_fan_speed());
                    }
                    Err(e) => {
                        warn!("Control update error: {:?}", e);
                    }
                }
                Ok(())
            }
        );

        if let Err(e) = control_result {
            info!("Service container error in control loop: {:?}", e);
        }


        let _ = ServiceContainer::with_roaster(
            |roaster: &mut crate::control::RoasterControl| {
                let status = roaster.get_status();
                let line = formatter.format(&status);

                match line {
                    Ok(formatted_line) => {
                        let _ = heapless::String::try_from(formatted_line.as_str())
                            .and_then(|s| output_channel.try_send(s).map_err(|_| ()));
                    }
                    Err(e) => {
                        debug!("Formatter error: {:?}", e);
                    }
                }
            }
        );

        Timer::after(Duration::from_millis(100)).await;
    }
}

#[task]
pub async fn artisan_output_task() {
    info!("Artisan output task started");

    let output_channel = ServiceContainer::get_output_channel();
    let mut driver = crate::hardware::uart::get_uart_driver();

    loop {
        if let Ok(data) = output_channel.try_receive() {
            if let Some(ref mut uart) = driver {
                let mut bytes = data.as_bytes().to_vec();
                bytes.extend_from_slice(b"\r\n");
                if let Err(e) = uart.write_bytes(&bytes).await {
                    warn!("UART write error: {:?}", e);
                }
            }
        }

        Timer::after(Duration::from_millis(5)).await;
    }
}

#[task]
pub async fn artisan_uart_handler_task() {
    loop {
        Timer::after(Duration::from_secs(60)).await;
    }
}
