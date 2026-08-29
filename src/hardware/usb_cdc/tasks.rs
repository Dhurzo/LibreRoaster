//! USB CDC reader task and legacy command-processing helpers.
//!
//! Implements `RxSource` for USB CDC, defines the static transport config/state,
//! and exposes `usb_reader_task` plus the legacy direct-processing path that
//! mirrors the UART task (used by integration tests).

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

/// USB CDC-specific RxSource implementation.
pub struct UsbCdcRx;

impl RxSource for UsbCdcRx {
    type Error = crate::hardware::usb_cdc::driver::UsbCdcError;

    async fn read_bytes(buffer: &mut [u8]) -> Result<usize, Self::Error> {
        driver::usb_cdc_read_bytes(buffer).await
    }
}

/// USB CDC transport configuration.
static USB_CONFIG: TransportConfig = TransportConfig {
    name: "USB",
    channel: CommChannel::Usb,
    reader_start_delay_ms: 100, // USB CDC needs more time to enumerate
    writer_start_delay_ms: 20,
    reader_poll_interval_ms: 10,
};

static USB_STATE: TransportRxState = TransportRxState::new();

/// USB CDC reader task - thin wrapper around generic implementation.
#[cfg_attr(target_arch = "riscv32", embassy_executor::task)]
pub async fn usb_reader_task() {
    run_reader_task(UsbCdcRx, &USB_STATE, &USB_CONFIG).await;
}

/// Process USB command data directly (legacy compatibility, mainly for tests).
///
/// Audit MP-4 (2026-08-11): the previous loop `return`ed after the FIRST
/// line terminator, silently dropping every later command in the same
/// buffer (the UART twin was fixed for this in L18). It now processes each
/// complete line in `data` in order; a trailing unterminated fragment is
/// dropped, and a bare terminator surfaces as an `EmptyCommand` parse error
/// exactly once per empty line — mirroring uart `process_command_data`.
pub fn process_usb_command_data(data: &[u8]) {
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
                handle_complete_usb_command(&command);
                processed_any = true;
                command.clear();
            }
            continue;
        }

        if command.push(byte).is_err() {
            send_usb_parse_error(ParseError::CommandTooLong);
            return;
        }
    }
}

/// Test-only version of process_usb_command_data for integration tests.
#[cfg(feature = "test")]
pub fn process_usb_command_data_test(data: &[u8]) {
    process_usb_command_data(data);
}

/// Internal handler for complete USB command (legacy compatibility path).
fn handle_complete_usb_command(command: &[u8]) {
    // Audit MP-1 (2026-08-11): skip parsing — and with it the parser-side
    // PROFILE/FANPROFILE FIFO side effects — for lines the multiplexer gate
    // would refuse (inactive transport). Mirrors the pre-parse gate in
    // `transport_tasks::process_event_queue`; `would_process_command` is a
    // pure predicate that never activates the channel (P8 preserved).
    let accepted = critical_section::with(|cs| {
        let multiplexer = ServiceContainer::get_multiplexer();
        let guard = multiplexer.borrow(cs).borrow();
        guard
            .as_ref()
            .is_none_or(|mux| mux.would_process_command(CommChannel::Usb))
    });
    if !accepted {
        return;
    }

    let parse_result = if command.is_empty() {
        Err(ParseError::EmptyCommand)
    } else {
        core::str::from_utf8(command)
            .map_err(|_| ParseError::InvalidValue)
            .and_then(crate::input::parse_artisan_command)
    };

    match parse_result {
        Ok(cmd) => {
            let traced = TracedCommand::new(cmd, CommChannel::Usb);
            let mut should_process = true;
            let mut sent = false;

            critical_section::with(|cs| {
                let multiplexer = ServiceContainer::get_multiplexer();
                let mut guard = multiplexer.borrow(cs).borrow_mut();
                if let Some(mux) = guard.as_mut() {
                    should_process = mux.should_process_command(CommChannel::Usb);
                }

                if should_process {
                    let artisan_channel = ServiceContainer::get_artisan_channel();
                    match artisan_channel.try_send(traced) {
                        Ok(()) => {
                            trace_command_enqueue(&traced, artisan_channel.len(), false);
                            sent = true;
                        }
                        Err(_) => {
                            debug!("USB artisan channel full, command dropped");
                        }
                    }
                }
            });

            if sent {
                crate::application::queue_metrics::record_queue_depth(
                    ServiceContainer::get_artisan_channel().len(),
                );
            }
        }
        Err(error) => {
            send_usb_parse_error(error);
        }
    }
}

/// Send parse error via USB CDC (legacy compatibility).
///
/// Audit MP-4 (2026-08-11): must NOT activate a channel from `None` — the
/// P8 fix in `transport_tasks::send_parse_error` reserved activation for
/// successfully parsed commands. Boot-time garbage on one wire can no
/// longer hijack the session before a valid command arrives.
fn send_usb_parse_error(error: ParseError) {
    let mut should_write = true;

    critical_section::with(|cs| {
        let multiplexer = ServiceContainer::get_multiplexer();
        let mut guard = multiplexer.borrow(cs).borrow_mut();
        if let Some(mux) = guard.as_mut() {
            should_write = mux.should_write_to(CommChannel::Usb);
        }

        if should_write {
            let output_channel = ServiceContainer::get_output_channel();
            let mut message = String::<TRACE_EVENT_MAX_LEN>::new();
            let _ = message.push_str("ERR ");
            let _ = message.push_str(error.code());
            let _ = message.push_str(" ");
            let _ = message.push_str(error.message());
            crate::hardware::error_counters::try_send_output(output_channel, message);
        }
    });
}
