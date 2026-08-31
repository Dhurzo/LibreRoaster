//! Artisan input subsystem for LibreRoaster.
//!
//! Exposes the command parser (`parser`) and the active-transport multiplexer
//! (`multiplexer`), plus the `ArtisanInput` DI handle and UART task bootstrap.

pub mod multiplexer;
/// TC4/Artisan command-line parser and PROFILE/FANPROFILE staging.
pub mod parser;
// NOTE: init_state module is commented out (handshake disabled for Artisan Scope)
// pub mod init_state;

/// Active-transport types re-exported for the application layer.
pub use multiplexer::{CommChannel, CommandMultiplexer};
/// Primary entry point: parse a command line into an `ArtisanCommand`.
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
/// Errors arising from the Artisan input subsystem.
pub enum InputError {
    /// UART transport or task failure.
    UartError,
    /// Command failed to parse.
    ParseError,
    /// An input/response buffer could not accept more data.
    BufferFull,
}

/// Placeholder for Artisan input. Command parsing happens in the UART/USB queue
/// processor tasks; this struct exists for ServiceContainer DI compatibility.
pub struct ArtisanInput;

impl ArtisanInput {
    /// Construct the (stateless) Artisan input handle.
    pub fn new() -> Result<Self, InputError> {
        Ok(Self)
    }

    /// Send a response string back to the active Artisan transport (device only).
    #[cfg(target_arch = "riscv32")]
    pub async fn send_response(&mut self, response: &str) -> Result<(), InputError> {
        send_response(response).await
    }
}

/// Spawn the UART reader task on the Embassy executor (device only).
#[cfg(target_arch = "riscv32")]
pub fn start_uart_tasks(spawner: &embassy_executor::Spawner) -> Result<(), InputError> {
    spawner
        .spawn(uart_reader_task())
        .map_err(|_| InputError::UartError)?;
    Ok(())
}
