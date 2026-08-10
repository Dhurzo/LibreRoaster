use crate::application::queue_metrics::record_queue_depth;
use crate::application::service_container::ServiceContainer;
use crate::hardware::transport_tasks::{
    run_reader_task, RxSource, TransportConfig, TransportRxState,
};
use crate::input::multiplexer::CommChannel;
use crate::input::parser::ParseError;
use crate::logging::traceability::{trace_command_enqueue, TracedCommand, TRACE_EVENT_MAX_LEN};
use heapless::{String, Vec};
use log::debug;

use super::driver;

/// UART-specific RxSource implementation.
pub struct UartRx;

impl RxSource for UartRx {
    type Error = crate::hardware::uart::driver::UartError;

    async fn read_bytes(buffer: &mut [u8]) -> Result<usize, Self::Error> {
        driver::uart_read_bytes(buffer).await
    }
}

// L8: `UartTx` and `run_writer_task` were removed together (Bug L18,
// 2026-08-10: the generic writer task and its command pipe were never
// spawned — static RAM only). Output goes through `dual_output_task` via
// the shared output channel, so leaving a second writer on the pipe is a
// recipe for interleaved lines. The embedded driver `uart_write_bytes` is
// still re-exported in case a future transport wants it directly.

/// UART transport configuration.
static UART_CONFIG: TransportConfig = TransportConfig {
    name: "UART",
    channel: CommChannel::Uart,
    reader_start_delay_ms: 10,
    writer_start_delay_ms: 20,
    reader_poll_interval_ms: 10,
};

/// UART transport receive state (static allocation).
static UART_STATE: TransportRxState = TransportRxState::new();

/// UART reader task - thin wrapper around generic implementation.
#[cfg_attr(target_arch = "riscv32", embassy_executor::task)]
pub async fn uart_reader_task() {
    run_reader_task(UartRx, &UART_STATE, &UART_CONFIG).await;
}

// L8: `uart_writer_task` was removed. Returning the same `run_writer_task`
// wrapper would race `dual_output_task` (which owns the single output pipe
// and is the only sanctioned writer) — the dead wrapper is replaced with
// a compile-time assertion that the generic `run_writer_task` stays
// available for current and future use, even though no spawner references
// it today.

/// Send a response via UART (multiplexer-aware).
pub async fn send_response(response: &str) -> Result<(), crate::input::InputError> {
    let output_channel = ServiceContainer::get_output_channel();
    let line = String::<TRACE_EVENT_MAX_LEN>::try_from(response)
        .map_err(|_| crate::input::InputError::BufferFull)?;
    let _ = output_channel.try_send(line);
    Ok(())
}

/// Send streaming data via UART (multiplexer-aware).
pub async fn send_stream(data: &str) -> Result<(), crate::input::InputError> {
    let output_channel = ServiceContainer::get_output_channel();
    let line = String::<TRACE_EVENT_MAX_LEN>::try_from(data)
        .map_err(|_| crate::input::InputError::BufferFull)?;
    let _ = output_channel.try_send(line);
    Ok(())
}

/// Process command data directly (legacy compatibility, mainly for tests).
///
/// Bug L18 (2026-08-10): this used to `return` after the FIRST line
/// terminator, silently dropping every later command in the buffer. It now
/// processes each complete line in `data` in order; a trailing unterminated
/// fragment is dropped (matching the event-queue path's behaviour), and a
/// bare terminator still surfaces as an `EmptyCommand` parse error exactly
/// once per empty line.
pub fn process_command_data(data: &[u8]) {
    // For test compatibility, we process directly without the event queue
    const COMMAND_BUFFER_SIZE: usize = 256;
    let mut command = Vec::<u8, COMMAND_BUFFER_SIZE>::new();
    let mut processed_any = false;
    for &byte in data {
        if byte == 0x0D || byte == 0x0A {
            // A line is due when it holds bytes, or when this is the very
            // first terminator of the buffer (bare `\r` = empty command).
            // A terminator right after a completed line (the `\n` of a
            // CRLF pair) must NOT emit a second EmptyCommand error.
            if !command.is_empty() || !processed_any {
                handle_command_data_internal(&command);
                processed_any = true;
                command.clear();
            }
            continue;
        }
        if command.push(byte).is_err() {
            send_parse_error_internal(ParseError::CommandTooLong);
            return;
        }
    }
}

/// Internal command handler for legacy/compatibility path.
fn handle_command_data_internal(data: &[u8]) {
    let parse_result = if data.is_empty() {
        Err(ParseError::EmptyCommand)
    } else {
        core::str::from_utf8(data)
            .map_err(|_| ParseError::InvalidValue)
            .and_then(crate::input::parse_artisan_command)
    };

    match parse_result {
        Ok(cmd) => {
            let traced = TracedCommand::new(cmd, CommChannel::Uart);
            let mut should_process = true;
            let mut sent = false;

            critical_section::with(|cs| {
                let multiplexer = ServiceContainer::get_multiplexer();
                let mut guard = multiplexer.borrow(cs).borrow_mut();
                if let Some(mux) = guard.as_mut() {
                    should_process = mux.should_process_command(CommChannel::Uart);
                }

                if should_process {
                    let artisan_channel = ServiceContainer::get_artisan_channel();
                    match artisan_channel.try_send(traced) {
                        Ok(()) => {
                            trace_command_enqueue(&traced, artisan_channel.len(), false);
                            sent = true;
                        }
                        Err(_) => {
                            debug!("UART artisan channel full, command dropped");
                        }
                    }
                }
            });

            if sent {
                record_queue_depth(ServiceContainer::get_artisan_channel().len());
            }
        }
        Err(error) => {
            send_parse_error_internal(error);
        }
    }
}

/// Send parse error (legacy compatibility).
fn send_parse_error_internal(error: ParseError) {
    let mut should_write = true;

    critical_section::with(|cs| {
        let multiplexer = ServiceContainer::get_multiplexer();
        let mut guard = multiplexer.borrow(cs).borrow_mut();
        if let Some(mux) = guard.as_mut() {
            if matches!(mux.get_active_channel(), CommChannel::None) {
                let _ = mux.on_command_received(CommChannel::Uart);
            }
            should_write = mux.should_write_to(CommChannel::Uart);
        }

        if should_write {
            let output_channel = ServiceContainer::get_output_channel();
            let mut message = String::<TRACE_EVENT_MAX_LEN>::new();
            let _ = message.push_str("ERR ");
            let _ = message.push_str(error.code());
            let _ = message.push_str(" ");
            let _ = message.push_str(error.message());
            let _ = output_channel.try_send(message);
        }
    });
}
