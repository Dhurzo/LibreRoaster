use crate::application::service_container::ServiceContainer;
use crate::input::parser::ParseError;
use crate::input::multiplexer::CommChannel;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pipe::Pipe;
use embassy_time::Duration;
use embassy_time::Timer;
use heapless::{String, Vec};
use log::warn;

use super::buffer::CircularBuffer;
use super::driver::get_uart_driver;

pub const COMMAND_PIPE_SIZE: usize = 256;

static mut COMMAND_PIPE: Option<Pipe<CriticalSectionRawMutex, COMMAND_PIPE_SIZE>> = None;
static mut RX_BUFFER: Option<CircularBuffer> = None;

#[cfg_attr(target_arch = "riscv32", embassy_executor::task)]
pub async fn uart_reader_task() {
    let mut rbuf: [u8; 64] = [0u8; 64];

    critical_section::with(|_| unsafe {
        COMMAND_PIPE = Some(Pipe::new());
        RX_BUFFER = Some(CircularBuffer::new());
    });

    Timer::after(Duration::from_millis(10)).await;

    loop {
        Timer::after(Duration::from_millis(10)).await;

        if let Some(uart) = get_uart_driver() {
            match uart.read_bytes(&mut rbuf).await {
                Ok(len) if len > 0 => {
                    process_command_data(&rbuf[..len]);
                }
                _ => {}
            }
        }

        Timer::after(Duration::from_millis(50)).await;
    }
}

#[cfg_attr(target_arch = "riscv32", embassy_executor::task)]
pub async fn uart_writer_task() {
    let mut wbuf: [u8; COMMAND_PIPE_SIZE] = [0u8; COMMAND_PIPE_SIZE];

    Timer::after(Duration::from_millis(20)).await;

    loop {
        #[allow(static_mut_refs)]
        if let Some(pipe) = unsafe { COMMAND_PIPE.as_ref() } {
            pipe.read(&mut wbuf).await;
        }

        if let Some(uart) = get_uart_driver() {
            let len = wbuf.iter().take_while(|&&b| b != 0).count();
            if len > 0 {
                let _ = uart.write_bytes(&wbuf[..len]).await;
            }
        }
    }
}

pub async fn send_response(response: &str) -> Result<(), crate::input::InputError> {
    if let Some(uart) = get_uart_driver() {
        let mut bytes = response.as_bytes().to_vec();
        bytes.extend_from_slice(b"\r\n");

        uart.write_bytes(&bytes)
            .await
            .map_err(|_| crate::input::InputError::UartError)?;
    }

    Ok(())
}

pub async fn send_stream(data: &str) -> Result<(), crate::input::InputError> {
    #[allow(static_mut_refs)]
    if let Some(pipe) = unsafe { COMMAND_PIPE.as_ref() } {
        let mut bytes = data.as_bytes().to_vec();
        bytes.extend_from_slice(b"\r\n");
        pipe.write_all(&bytes).await;
    }
    Ok(())
}

pub fn process_command_data(data: &[u8]) {
    let mut command = Vec::<u8, 64>::new();

    for &byte in data {
        if byte == 0x0D {
            handle_complete_command(&command);
            return;
        }

        if command.push(byte).is_err() {
            send_parse_error(ParseError::InvalidValue);
            return;
        }
    }
}

fn handle_complete_command(command: &[u8]) {
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
                    let should_process = mux.should_process_command(CommChannel::Uart);

                    if should_process {
                        let channel = ServiceContainer::get_artisan_channel();
                        if let Err(err) = channel.try_send(cmd) {
                            warn!("Failed to enqueue Artisan command from UART: {:?}", err);
                            send_parse_error(ParseError::InvalidValue);
                        }
                    }
                }
            });
        }
        Err(error) => {
            send_parse_error(error);
        }
    }
}

fn send_parse_error(error: ParseError) {
    critical_section::with(|cs| {
        let multiplexer = ServiceContainer::get_multiplexer();
        let mut guard = multiplexer.borrow(cs).borrow_mut();
        if let Some(mux) = guard.as_mut() {
            let should_write = mux.should_write_to(CommChannel::Uart);

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
