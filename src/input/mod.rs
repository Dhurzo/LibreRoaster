pub mod multiplexer;
pub mod parser;
// NOTE: init_state module is commented out (handshake disabled for Artisan Scope)
// pub mod init_state;

pub use multiplexer::{CommChannel, CommandMultiplexer};
pub use parser::parse_artisan_command;

#[cfg(target_arch = "riscv32")]
use crate::hardware::uart::{send_response, uart_reader_task};

// Bug M1 (2026-07-26): `CommandQueue` / `COMMAND_QUEUE_SIZE` / `QueueError`
// were removed — the F5.3 refactor moved the production command path to the
// Embassy channel (`ServiceContainer::get_artisan_channel`,
// ARTISAN_CMD_CHANNEL_SIZE = 8); the legacy queue was exercised ONLY by
// `tests/transport_flood_test.rs`, giving false confidence in flood
// robustness. Tests that want to exercise backpressure should target the
// real channel.

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
