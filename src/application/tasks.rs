use crate::application::service_container::ServiceContainer;
use crate::input::multiplexer::CommChannel;
use crate::output::artisan::ArtisanFormatter;
use crate::output::artisan::MutableArtisanFormatter;
use embassy_executor::task;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, Instant, Timer};
use heapless::String;
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

        if let Ok(command) = cmd_channel.try_receive() {
            let output_channel = ServiceContainer::get_output_channel();

            let _ = ServiceContainer::with_roaster(|roaster| {
                match roaster.process_artisan_command(command) {
                    Ok(()) => {
                        debug!("Processed Artisan command successfully");

                        if let crate::config::ArtisanCommand::ReadStatus = command {
                            let status = roaster.get_status();
                            // Use full READ response with 7 values per Artisan spec
                            let response = ArtisanFormatter::format_read_response_full(&status);

                            if let Ok(line) = String::<128>::try_from(response.as_str()) {
                                let _ = output_channel.try_send(line);
                            }
                        }
                    }
                    Err(err) => {
                        warn!("Failed to process Artisan command: {:?}", err);
                        send_handler_error(output_channel, &err);
                    }
                }
            });
        }

        let control_result = ServiceContainer::with_roaster(
            |roaster: &mut crate::control::RoasterControl| -> Result<(), ()> {
                match roaster.read_sensors() {
                    Ok(()) => {
                        debug!(
                            "Sensors: BT: {:.1}°C, ET: {:.1}°C",
                            roaster.get_status().bean_temp,
                            roaster.get_status().env_temp
                        );
                    }
                    Err(e) => {
                        warn!("Sensor read error: {:?}", e);
                    }
                }
                match roaster.update_control(current_time) {
                    Ok(output) => {
                        debug!(
                            "Control: SSR {:.1}%, Fan {:.1}%",
                            output,
                            roaster.get_fan_speed()
                        );
                    }
                    Err(e) => {
                        warn!("Control update error: {:?}", e);
                    }
                }
                Ok(())
            },
        );

        if let Err(e) = control_result {
            info!("Service container error in control loop: {:?}", e);
        }

        let _ = ServiceContainer::with_roaster(|roaster: &mut crate::control::RoasterControl| {
            if roaster.get_output_manager().is_continuous_enabled() {
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
        });

        Timer::after(Duration::from_millis(100)).await;
    }
}

fn send_handler_error(
    output_channel: &Channel<
        embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
        String<128>,
        { crate::application::service_container::ARTISAN_OUTPUT_CHANNEL_SIZE },
    >,
    error: &crate::control::RoasterError,
) {
    let mut message = String::<128>::new();
    let _ = message.push_str("ERR handler_failed ");
    let _ = message.push_str(error.message_token());

    let _ = output_channel.try_send(message);
}

#[task]
pub async fn dual_output_task() {
    info!("Dual output task started - USB CDC + UART");

    let output_channel = ServiceContainer::get_output_channel();

    loop {
        if let Ok(data) = output_channel.try_receive() {
            let (channel, data_to_write) = critical_section::with(|cs| {
                let multiplexer = ServiceContainer::get_multiplexer();
                let mut guard = multiplexer.borrow(cs).borrow_mut();
                if let Some(mux) = guard.as_mut() {
                    let active_channel = mux.get_active_channel();
                    let mut bytes = data.as_bytes().to_vec();
                    bytes.extend_from_slice(b"\r\n");
                    (active_channel, Some(bytes))
                } else {
                    (CommChannel::None, None)
                }
            });

            if let Some(bytes) = data_to_write {
                match channel {
                    CommChannel::Usb => {
                        if let Some(usb) = crate::hardware::usb_cdc::driver::get_usb_cdc_driver() {
                            let _ = usb.write_bytes(&bytes).await;
                        }
                    }
                    CommChannel::Uart => {
                        if let Some(uart) = crate::hardware::uart::driver::get_uart_driver() {
                            let _ = uart.write_bytes(&bytes).await;
                        }
                    }
                    CommChannel::None => {
                    }
                }
            }
        }

        Timer::after(Duration::from_millis(5)).await;
    }
}
