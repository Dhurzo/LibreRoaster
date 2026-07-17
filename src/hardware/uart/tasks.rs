use crate::application::queue_metrics::record_queue_depth;
use crate::application::service_container::ServiceContainer;
use crate::hardware::transport_tasks::{
    run_reader_task, run_writer_task, RxSource, TransportConfig, TransportRxState, TxSink,
    COMMAND_PIPE_SIZE, EVENT_QUEUE_SIZE,
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

/// UART-specific TxSink implementation.
pub struct UartTx;

impl TxSink for UartTx {
    type Error = crate::hardware::uart::driver::UartError;

    async fn write_bytes(data: &[u8]) -> Result<(), Self::Error> {
        driver::uart_write_bytes(data).await
    }
}

/// UART transport configuration.
static UART_CONFIG: TransportConfig = TransportConfig {
    name: "UART",
    channel: CommChannel::Uart,
    event_queue_size: EVENT_QUEUE_SIZE,
    command_pipe_size: COMMAND_PIPE_SIZE,
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

/// UART writer task - thin wrapper around generic implementation.
#[cfg_attr(target_arch = "riscv32", embassy_executor::task)]
pub async fn uart_writer_task() {
    run_writer_task(UartTx, &UART_STATE, &UART_CONFIG).await;
}

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
pub fn process_command_data(data: &[u8]) {
    // For test compatibility, we process directly without the event queue
    let mut command = Vec::<u8, 64>::new();
    for &byte in data {
        if byte == 0x0D || byte == 0x0A {
            handle_command_data_internal(&command);
            return;
        }
        if command.push(byte).is_err() {
            send_parse_error_internal(ParseError::InvalidValue);
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
