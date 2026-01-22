use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pipe::Pipe;
use embassy_time::Duration;
use embassy_time::Timer;
use heapless::Vec;

use super::buffer::CircularBuffer;
use super::driver::get_uart_driver;

pub const COMMAND_PIPE_SIZE: usize = 256;

static mut COMMAND_PIPE: Option<Pipe<CriticalSectionRawMutex, COMMAND_PIPE_SIZE>> = None;
static mut RX_BUFFER: Option<CircularBuffer> = None;

#[embassy_executor::task]
pub async fn uart_reader_task() {
    let mut rbuf: [u8; 64] = [0u8; 64];

    critical_section::with(|_| unsafe {
        COMMAND_PIPE = Some(Pipe::new());
        RX_BUFFER = Some(CircularBuffer::new());
    });

    loop {
        Timer::after(Duration::from_millis(10)).await;

        if let Some(uart) = get_uart_driver() {
            match uart.read_bytes(&mut rbuf).await {
                Ok(len) if len > 0 => {
                    process_command_data(&rbuf[..len]).await;
                }
                _ => {}
            }
        }

        Timer::after(Duration::from_millis(50)).await;
    }
}

#[embassy_executor::task]
pub async fn uart_writer_task() {
    let mut wbuf: [u8; COMMAND_PIPE_SIZE] = [0u8; COMMAND_PIPE_SIZE];

    loop {
        // Allow this static_mut_ref warning as it's necessary for embedded systems
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
    // Allow this static_mut_ref warning as it's necessary for embedded systems
    #[allow(static_mut_refs)]
    if let Some(pipe) = unsafe { COMMAND_PIPE.as_ref() } {
        let mut bytes = data.as_bytes().to_vec();
        bytes.extend_from_slice(b"\r\n");
        pipe.write_all(&bytes).await;
    }
    Ok(())
}

async fn process_command_data(data: &[u8]) {
    let mut command = Vec::<u8, 64>::new();

    for &byte in data {
        if byte == 0x0D {
            // Allow this static_mut_ref warning as it's necessary for embedded systems
            #[allow(static_mut_refs)]
            if let Some(pipe) = unsafe { COMMAND_PIPE.as_ref() } {
                let _ = pipe.write_all(&command).await;
            }
            return;
        }

        if command.push(byte).is_err() {
            return;
        }
    }
}