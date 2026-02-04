pub mod parser;

pub use parser::parse_artisan_command;

use crate::config::ArtisanCommand;
use crate::hardware::uart::{send_response, uart_reader_task, uart_writer_task, COMMAND_PIPE_SIZE};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pipe::Pipe;
use embassy_time::Duration;
use embassy_time::Timer;

static mut COMMAND_PIPE: Option<Pipe<CriticalSectionRawMutex, COMMAND_PIPE_SIZE>> = None;

#[derive(Debug, Clone, PartialEq)]
pub enum InputError {
    UartError,
    ParseError,
    BufferFull,
}

pub struct ArtisanInput;

impl ArtisanInput {
    pub fn new() -> Result<Self, InputError> {
        Ok(Self)
    }

    pub async fn read_command(&mut self) -> Result<Option<ArtisanCommand>, InputError> {
        let mut cmd_buf: [u8; 64] = [0u8; 64];

        #[allow(static_mut_refs)]
        if let Some(pipe) = unsafe { COMMAND_PIPE.as_ref() } {
            pipe.read(&mut cmd_buf).await;
        }

        if cmd_buf[0] == 0 {
            Timer::after(Duration::from_millis(10)).await;
            return Ok(None);
        }

        let len = cmd_buf.iter().take_while(|&&b| b != 0).count();
        if len == 0 {
            return Ok(None);
        }

        let command_str =
            core::str::from_utf8(&cmd_buf[..len]).map_err(|_| InputError::ParseError)?;

        match parse_artisan_command(command_str) {
            Ok(cmd) => Ok(Some(cmd)),
            Err(_) => Ok(None),
        }
    }

    pub fn try_read_command(&mut self) -> Result<Option<ArtisanCommand>, InputError> {
        let _cmd_buf: [u8; 64] = [0u8; 64];

        #[allow(static_mut_refs)]
        if let Some(_pipe) = unsafe { COMMAND_PIPE.as_ref() } {}

        Ok(None)
    }

    pub async fn send_response(&mut self, response: &str) -> Result<(), InputError> {
        send_response(response).await
    }
}

#[cfg(target_arch = "riscv32")]
pub fn start_uart_tasks(spawner: &embassy_executor::Spawner) -> Result<(), InputError> {
    critical_section::with(|_| unsafe {
        COMMAND_PIPE = Some(Pipe::new());
    });

    spawner
        .spawn(uart_reader_task())
        .map_err(|_| InputError::UartError)?;
    spawner
        .spawn(uart_writer_task())
        .map_err(|_| InputError::UartError)?;
    Ok(())
}
