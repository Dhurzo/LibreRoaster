use crate::application::queue_metrics::record_queue_depth;
use crate::application::service_container::ServiceContainer;
use crate::input::multiplexer::CommChannel;
use crate::input::parser::ParseError;
use crate::hardware::static_sync::SyncCell;
use crate::input::{CommandQueue, QueueError, COMMAND_QUEUE_SIZE};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pipe::Pipe;
use embassy_time::Duration;
use embassy_time::Timer;
use heapless::Deque;
use heapless::{String, Vec};
use log::debug;

use super::buffer::CircularBuffer;
use super::driver::get_uart_driver;

pub const COMMAND_PIPE_SIZE: usize = 256;

/// Size of the UART event queue for buffering incoming bytes
pub const EVENT_QUEUE_SIZE: usize = 256;

static COMMAND_PIPE: SyncCell<Option<Pipe<CriticalSectionRawMutex, COMMAND_PIPE_SIZE>>> =
    SyncCell::new(None);
static RX_BUFFER: SyncCell<Option<CircularBuffer>> = SyncCell::new(None);
/// Buffered event queue for UART input - separates I/O from parsing
static EVENT_QUEUE: SyncCell<Option<Deque<u8, EVENT_QUEUE_SIZE>>> = SyncCell::new(None);
/// Command queue for FIFO processing - reject-on-full behavior
static COMMAND_QUEUE: SyncCell<
    Option<CommandQueue<crate::config::ArtisanCommand, COMMAND_QUEUE_SIZE>>,
> = SyncCell::new(None);

#[cfg_attr(target_arch = "riscv32", embassy_executor::task)]
pub async fn uart_reader_task() {
    let mut rbuf: [u8; 64] = [0u8; 64];

    critical_section::with(|_| unsafe {
        *COMMAND_PIPE.get() = Some(Pipe::new());
        *RX_BUFFER.get() = Some(CircularBuffer::new());
        // Initialize the event queue for buffering UART input
        *EVENT_QUEUE.get() = Some(Deque::new());
        // Initialize the command queue for FIFO processing
        *COMMAND_QUEUE.get() = Some(CommandQueue::new());
    });

    Timer::after(Duration::from_millis(10)).await;

    loop {
        // Read from UART into buffer (async, non-blocking)
        if let Some(uart) = get_uart_driver() {
            match uart.read_bytes(&mut rbuf).await {
                Ok(len) if len > 0 => {
                    // Push received bytes to event queue instead of processing directly
                    // This separates I/O from parsing concerns
                    push_to_event_queue(&rbuf[..len]);
                }
                _ => {}
            }
        }

        // Process complete lines from event queue
        process_event_queue();

        Timer::after(Duration::from_millis(10)).await;
    }
}

/// Push received bytes to the event queue
/// Uses heapless Deque for no-std compatible buffering
fn push_to_event_queue(data: &[u8]) {
    critical_section::with(|_| unsafe {
        if let Some(queue) = (*EVENT_QUEUE.get()).as_mut() {
            for &byte in data {
                // Drop oldest if queue is full (ring buffer behavior)
                if queue.len() >= EVENT_QUEUE_SIZE {
                    let _ = queue.pop_front();
                }
                let _ = queue.push_back(byte);
            }
        }
    });
}

/// Process complete lines (0x0D terminated) from the event queue
fn process_event_queue() {
    // We need to find a complete line (0x0D) and extract it
    // First, check if there's a terminator in the queue
    let has_terminator = critical_section::with(|_| unsafe {
        if let Some(queue) = (*EVENT_QUEUE.get()).as_ref() {
            queue.iter().any(|&b| b == 0x0D)
        } else {
            false
        }
    });

    if has_terminator {
        // Extract the complete line including terminator
        let mut command_data: Vec<u8, 64> = Vec::new();
        let mut extracted = false;

        critical_section::with(|_| unsafe {
            if let Some(queue) = (*EVENT_QUEUE.get()).as_mut() {
                // Extract bytes up to and including the terminator
                while let Some(byte) = queue.pop_front() {
                    let _ = command_data.push(byte);
                    if byte == 0x0D {
                        break; // Stop at terminator
                    }
                }
                extracted = true;
            }
        });

        if extracted && !command_data.is_empty() {
            // Remove the terminator (last byte if it's 0x0D)
            let last_idx = command_data.len() - 1;
            if command_data[last_idx] == 0x0D {
                // Truncate to remove terminator
                command_data.truncate(last_idx);
            }

            // Process the complete command
            if !command_data.is_empty() {
                handle_command_data_internal(&command_data);
            }
        }
    }
}

/// Internal command handler - pushes parsed commands to FIFO queue
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
            let mut depth = 0;
            let mut should_process = true;

            critical_section::with(|cs| {
                let multiplexer = ServiceContainer::get_multiplexer();
                let mut guard = multiplexer.borrow(cs).borrow_mut();
                if let Some(mux) = guard.as_mut() {
                    should_process = mux.should_process_command(CommChannel::Uart);
                }

                if should_process {
                    // Push to command queue for FIFO processing
                    // On queue full: silently drop command (no response sent - Artisan times out)
                    if let Some(queue) = unsafe { (*COMMAND_QUEUE.get()).as_mut() } {
                        match queue.try_push(cmd) {
                            Ok(()) => {
                                // Command queued successfully - will be processed by queue processor
                            }
                            Err(QueueError::Full) => {
                                // Queue full - reject silently (no response sent)
                                debug!("UART command queue full, rejecting command");
                            }
                        }
                        depth = queue.len();
                    } else {
                        // Compatibility path (tests / host helpers): if queue is not initialized,
                        // forward directly to the command channel.
                        let _ = ServiceContainer::get_artisan_channel().try_send(cmd);
                    }
                }
            });
            record_queue_depth(depth);
        }
        Err(error) => {
            send_parse_error_internal(error);
        }
    }
}

/// Send parse error (called with critical section not held)
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
            let mut message = String::<128>::new();
            let _ = message.push_str("ERR ");
            let _ = message.push_str(error.code());
            let _ = message.push_str(" ");
            let _ = message.push_str(error.message());
            let _ = output_channel.try_send(message);
        }
    });
}

#[cfg_attr(target_arch = "riscv32", embassy_executor::task)]
pub async fn uart_writer_task() {
    let mut wbuf: [u8; COMMAND_PIPE_SIZE] = [0u8; COMMAND_PIPE_SIZE];

    Timer::after(Duration::from_millis(20)).await;

    loop {
        if let Some(pipe) = unsafe { (*COMMAND_PIPE.get()).as_ref() } {
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
    let output_channel = ServiceContainer::get_output_channel();
    let line =
        String::<128>::try_from(response).map_err(|_| crate::input::InputError::BufferFull)?;
    let _ = output_channel.try_send(line);
    Ok(())
}

pub async fn send_stream(data: &str) -> Result<(), crate::input::InputError> {
    let output_channel = ServiceContainer::get_output_channel();
    let line = String::<128>::try_from(data).map_err(|_| crate::input::InputError::BufferFull)?;
    let _ = output_channel.try_send(line);
    Ok(())
}

/// Queue processor task - consumes commands from COMMAND_QUEUE and sends to artisan_channel
/// This task bridges the command queue to the control loop, ensuring commands are processed
#[cfg_attr(target_arch = "riscv32", embassy_executor::task)]
pub async fn queue_processor_task() {
    // Small delay to allow other tasks to initialize
    Timer::after(Duration::from_millis(50)).await;

    loop {
        // Try to pop a command from the queue and send to artisan_channel
        let (cmd_opt, queue_depth) = critical_section::with(|_| unsafe {
            if let Some(queue) = (*COMMAND_QUEUE.get()).as_mut() {
                let cmd = queue.pop();
                let depth = queue.len();
                (cmd, depth)
            } else {
                (None, 0)
            }
        });
        record_queue_depth(queue_depth);

        if let Some(cmd) = cmd_opt {
            let channel = ServiceContainer::get_artisan_channel();
            channel.send(cmd).await;
        }

        // Small delay to yield to other tasks and prevent tight looping
        Timer::after(Duration::from_millis(5)).await;
    }
}

// Keep legacy function for compatibility - now delegates to queue-based processing
pub fn process_command_data(data: &[u8]) {
    let event_queue_initialized =
        critical_section::with(|_| unsafe { (*EVENT_QUEUE.get()).as_ref().is_some() });

    if event_queue_initialized {
        // Standard path: enqueue bytes and process complete frames.
        push_to_event_queue(data);
        process_event_queue();
        return;
    }

    // Compatibility path (mainly tests): process a single frame directly.
    let mut command = Vec::<u8, 64>::new();
    for &byte in data {
        if byte == 0x0D {
            handle_command_data_internal(&command);
            return;
        }

        if command.push(byte).is_err() {
            send_parse_error_internal(ParseError::InvalidValue);
            return;
        }
    }
}
