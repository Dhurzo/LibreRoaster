pub mod multiplexer;
pub mod parser;
// NOTE: init_state module is commented out (handshake disabled for Artisan Scope)
// pub mod init_state;

pub use multiplexer::{CommChannel, CommandMultiplexer};
pub use parser::parse_artisan_command;

use crate::config::ArtisanCommand;
#[cfg(target_arch = "riscv32")]
use crate::hardware::uart::{send_response, uart_reader_task, COMMAND_PIPE_SIZE};
#[cfg(not(target_arch = "riscv32"))]
use crate::hardware::uart::{send_response, COMMAND_PIPE_SIZE};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pipe::Pipe;
use embassy_time::Duration;
use embassy_time::Timer;
use heapless::Deque;

/// Default size for command queue - handles bursts of commands
pub const COMMAND_QUEUE_SIZE: usize = 32;

/// Error returned when command queue is full
#[derive(Debug, Clone, PartialEq)]
pub enum QueueError {
    Full,
}

/// FIFO command queue with reject-on-full behavior
/// - push adds to back, pop removes from front
/// - try_push returns error when full (for rejection)
pub struct CommandQueue<T, const N: usize> {
    queue: Deque<T, N>,
}

impl<T, const N: usize> CommandQueue<T, N> {
    /// Create a new empty command queue
    pub fn new() -> Self {
        Self {
            queue: Deque::new(),
        }
    }

    /// Try to push an item to the back of the queue
    /// Returns Err(QueueError::Full) if queue is at capacity
    pub fn try_push(&mut self, item: T) -> Result<(), QueueError> {
        self.queue.push_back(item).map_err(|_| QueueError::Full)
    }

    /// Pop an item from the front of the queue (if any)
    pub fn pop(&mut self) -> Option<T> {
        self.queue.pop_front()
    }

    /// Check if the queue is full
    pub fn is_full(&self) -> bool {
        self.queue.len() >= N
    }

    /// Check if the queue is empty
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Get the current number of items in the queue
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Get the maximum capacity of the queue
    pub fn capacity(&self) -> usize {
        N
    }
}

impl<T, const N: usize> Default for CommandQueue<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

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
    spawner
        .spawn(uart_reader_task())
        .map_err(|_| InputError::UartError)?;
    Ok(())
}
