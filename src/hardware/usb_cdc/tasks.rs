use crate::application::service_container::ServiceContainer;
use crate::input::parser::ParseError;
use crate::input::multiplexer::CommChannel;
use crate::log_channel;
use crate::logging::channel::Channel;
use embassy_time::Duration;
use embassy_time::Timer;
use heapless::{String, Vec};
use log::warn;

use super::driver::get_usb_cdc_driver;

pub const USB_COMMAND_PIPE_SIZE: usize = 256;

#[cfg_attr(target_arch = "riscv32", embassy_executor::task)]
pub async fn usb_reader_task() {
    let mut rbuf: [u8; 64] = [0u8; 64];

    Timer::after(Duration::from_millis(100)).await;

    loop {
        if let Some(usb) = get_usb_cdc_driver() {
            match usb.read_bytes(&mut rbuf).await {
                Ok(len) if len > 0 => {
                    let raw_cmd = core::str::from_utf8(&rbuf[..len]).unwrap_or("[binary]");
                    log_channel!(Channel::Usb, "RX: {}", raw_cmd.trim_end());
                    process_usb_command_data(&rbuf[..len]);
                }
                _ => {}
            }
        }

        Timer::after(Duration::from_millis(10)).await;
    }
}

#[cfg_attr(target_arch = "riscv32", embassy_executor::task)]
pub async fn usb_writer_task() {
    let output_channel = ServiceContainer::get_output_channel();

    loop {
        if let Ok(data) = output_channel.try_receive() {
            if let Some(usb) = get_usb_cdc_driver() {
                let mut bytes = data.as_bytes().to_vec();
                bytes.extend_from_slice(b"\r\n");
                log_channel!(Channel::Usb, "TX: {}", data);
                if let Err(e) = usb.write_bytes(&bytes).await {
                    warn!("USB CDC write error: {:?}", e);
                }
            }
        }

        Timer::after(Duration::from_millis(5)).await;
    }
}

#[cfg(feature = "test")]
pub fn process_usb_command_data(data: &[u8]) {
    let mut command = Vec::<u8, 64>::new();

    for &byte in data {
        if byte == 0x0D {
            handle_complete_usb_command(&command);
            return;
        }

        if command.push(byte).is_err() {
            send_usb_parse_error(ParseError::InvalidValue);
            return;
        }
    }
}

#[cfg(not(feature = "test"))]
pub(crate) fn process_usb_command_data(data: &[u8]) {
    let mut command = Vec::<u8, 64>::new();

    for &byte in data {
        if byte == 0x0D {
            handle_complete_usb_command(&command);
            return;
        }

        if command.push(byte).is_err() {
            send_usb_parse_error(ParseError::InvalidValue);
            return;
        }
    }
}

fn handle_complete_usb_command(command: &[u8]) {
    let parse_result = if command.is_empty() {
        Err(ParseError::EmptyCommand)
    } else {
        core::str::from_utf8(command)
            .map_err(|_| ParseError::InvalidValue)
            .and_then(crate::input::parse_artisan_command)
    };

    match parse_result {
        Ok(cmd) => {
            critical_section::with(|cs| {
                let multiplexer = ServiceContainer::get_multiplexer();
                let mut guard = multiplexer.borrow(cs).borrow_mut();
                if let Some(mux) = guard.as_mut() {
                    let should_process = mux.should_process_command(CommChannel::Usb);

                    if should_process {
                        let channel = ServiceContainer::get_artisan_channel();
                        if let Err(err) = channel.try_send(cmd) {
                            warn!("Failed to enqueue Artisan command from USB: {:?}", err);
                            send_usb_parse_error(ParseError::InvalidValue);
                        }
                    }
                }
            });
        }
        Err(error) => {
            send_usb_parse_error(error);
        }
    }
}

fn send_usb_parse_error(error: ParseError) {
    critical_section::with(|cs| {
        let multiplexer = ServiceContainer::get_multiplexer();
        let mut guard = multiplexer.borrow(cs).borrow_mut();
        if let Some(mux) = guard.as_mut() {
            let should_write = mux.should_write_to(CommChannel::Usb);

            if should_write {
                let output_channel = ServiceContainer::get_output_channel();
                let mut message = String::<128>::new();
                let _ = message.push_str("ERR ");
                let _ = message.push_str(error.code());
                let _ = message.push_str(" ");
                let _ = message.push_str(error.message());
                let _ = output_channel.try_send(message);
            }
        }
    });
}
