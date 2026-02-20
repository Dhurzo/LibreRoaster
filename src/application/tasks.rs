extern crate alloc;

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
    let mut was_continuous = false;

    loop {
        let current_time = Instant::now();

        if let Ok(command) = cmd_channel.try_receive() {
            let output_channel = ServiceContainer::get_output_channel();

            let _ = ServiceContainer::with_roaster_async(
                |roaster: &mut crate::control::roaster_refactored::RoasterControl| {
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
                },
            );
        }

        // Control loop now uses async read_sensors() - no longer blocks executor
        // Using the roaster_async_sensor_read method that takes ownership, calls async, returns it
        let sensor_err = ServiceContainer::roaster_async_sensor_read().await.err();

        if sensor_err.is_none() {
            debug!(
                "Sensors: BT: {:.1}°C, ET: {:.1}°C",
                ServiceContainer::read_bean_temperature().await.unwrap_or(0.0),
                ServiceContainer::read_env_temperature().await.unwrap_or(0.0)
            );
        } else {
            warn!("Sensor read error: {:?}", sensor_err);
        }

        // Do sync control update separately
        let _update_result = ServiceContainer::with_roaster_async(
            |roaster: &mut crate::control::roaster_refactored::RoasterControl| -> Result<(), ()> {
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

        if let Some(e) = sensor_err {
            info!("Service container error in control loop: {:?}", e);
        }

        let mut is_continuous_now = false;
        let mut status_for_output = None;

        let _ = ServiceContainer::with_roaster_async(
            |roaster: &mut crate::control::roaster_refactored::RoasterControl| {
                is_continuous_now = roaster.get_output_manager().is_continuous_enabled();
                if is_continuous_now {
                    status_for_output = Some(roaster.get_status());
                }
            },
        );

        if is_continuous_now != was_continuous {
            formatter.reset();
            was_continuous = is_continuous_now;
        }

        if let Some(status) = status_for_output {
            let line = formatter.format(&status);

            match line {
                Ok(formatted_line) => {
                    if let Ok(s) = heapless::String::try_from(formatted_line.as_str()) {
                        let _ = output_channel.try_send(s);
                    }
                }
                Err(e) => {
                    debug!("Formatter error: {:?}", e);
                }
            }
        }

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
                    let bytes = append_crlf(data.as_str());
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
                    CommChannel::None => {}
                }
            }
        }

        Timer::after(Duration::from_millis(5)).await;
    }
}

fn append_crlf(payload: &str) -> alloc::vec::Vec<u8> {
    let mut bytes = payload.as_bytes().to_vec();
    bytes.extend_from_slice(b"\r\n");
    bytes
}

#[cfg(test)]
mod tests {
    use super::append_crlf;

    #[test]
    fn test_append_crlf_appends_single_terminator() {
        let payload = "READ,120.3,150.5,75.0,25.0";
        let bytes = append_crlf(payload);

        let output = core::str::from_utf8(&bytes).expect("Output should be valid UTF-8");
        assert_eq!(output, "READ,120.3,150.5,75.0,25.0\r\n");
    }
}
