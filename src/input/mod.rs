pub mod multiplexer;
pub mod parser;
// NOTE: init_state module is commented out (handshake disabled for Artisan Scope)
// pub mod init_state;

pub use multiplexer::{CommChannel, CommandMultiplexer};
pub use parser::parse_artisan_command;

#[cfg(target_arch = "riscv32")]
use crate::hardware::uart::{send_response, uart_reader_task};
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

#[derive(Debug, Clone, PartialEq)]
pub enum InputError {
    UartError,
    ParseError,
    BufferFull,
}

/// Placeholder for Artisan input. Command parsing happens in the UART/USB queue
/// processor tasks; this struct exists for ServiceContainer DI compatibility.
pub struct ArtisanInput;

impl ArtisanInput {
    pub fn new() -> Result<Self, InputError> {
        Ok(Self)
    }

    #[cfg(target_arch = "riscv32")]
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
