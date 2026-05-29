use crate::application::queue_metrics::record_queue_depth;
use crate::application::service_container::ServiceContainer;
use crate::input::multiplexer::CommChannel;
use crate::input::parser::ParseError;
use crate::input::{CommandQueue, QueueError, COMMAND_QUEUE_SIZE};
use crate::logging::traceability::{
    trace_command_enqueue, trace_queue_dequeue, TracedCommand, TRACE_EVENT_MAX_LEN,
};
use core::cell::RefCell;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::Mutex as BlockingMutex;
use embassy_sync::pipe::Pipe;
use embassy_time::Duration;
use embassy_time::Timer;
use heapless::Deque;
use heapless::{String, Vec};
use log::debug;

use super::buffer::CircularBuffer;
use super::driver;

pub const COMMAND_PIPE_SIZE: usize = 256;

/// Size of the UART event queue for buffering incoming bytes
pub const EVENT_QUEUE_SIZE: usize = 256;

static COMMAND_PIPE: BlockingMutex<
    CriticalSectionRawMutex,
    RefCell<Option<Pipe<CriticalSectionRawMutex, COMMAND_PIPE_SIZE>>>,
> = BlockingMutex::new(RefCell::new(None));
static RX_BUFFER: BlockingMutex<CriticalSectionRawMutex, RefCell<Option<CircularBuffer>>> =
    BlockingMutex::new(RefCell::new(None));
/// Buffered event queue for UART input - separates I/O from parsing
static EVENT_QUEUE: BlockingMutex<
    CriticalSectionRawMutex,
    RefCell<Option<Deque<u8, EVENT_QUEUE_SIZE>>>,
> = BlockingMutex::new(RefCell::new(None));
/// Command queue for FIFO processing - reject-on-full behavior
static COMMAND_QUEUE: BlockingMutex<
    CriticalSectionRawMutex,
    RefCell<Option<CommandQueue<TracedCommand, COMMAND_QUEUE_SIZE>>>,
> = BlockingMutex::new(RefCell::new(None));

fn take_pipe() -> Option<Pipe<CriticalSectionRawMutex, COMMAND_PIPE_SIZE>> {
    COMMAND_PIPE.lock(|cell| cell.borrow_mut().take())
}

#[cfg_attr(target_arch = "riscv32", embassy_executor::task)]
pub async fn uart_reader_task() {
    let mut rbuf: [u8; 64] = [0u8; 64];

    COMMAND_PIPE.lock(|cell| *cell.borrow_mut() = Some(Pipe::new()));
    RX_BUFFER.lock(|cell| *cell.borrow_mut() = Some(CircularBuffer::new()));
    EVENT_QUEUE.lock(|cell| *cell.borrow_mut() = Some(Deque::new()));
    COMMAND_QUEUE.lock(|cell| *cell.borrow_mut() = Some(CommandQueue::new()));

    Timer::after(Duration::from_millis(10)).await;

    loop {
        // Read from UART into buffer (async, non-blocking)
        match driver::uart_read_bytes(&mut rbuf).await {
            Ok(len) if len > 0 => {
                crate::hardware::error_counters::reset_uart_error_count();
                push_to_event_queue(&rbuf[..len]);
            }
            Ok(0) => { /* no data — idle poll */ }
            Ok(_) => { /* should not happen based on pattern match above */ }
            Err(e) => {
                crate::hardware::error_counters::increment_uart_error_count();
                log::warn!("UART read error: {:?}", e);
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
    EVENT_QUEUE.lock(|cell| {
        if let Some(queue) = cell.borrow_mut().as_mut() {
            for &byte in data {
                if queue.len() >= EVENT_QUEUE_SIZE {
                    let _ = queue.pop_front();
                }
                let _ = queue.push_back(byte);
            }
        }
    });
}

/// Process complete lines (CR or LF terminated) from the event queue
fn process_event_queue() {
    let has_terminator = EVENT_QUEUE.lock(|cell| {
        if let Some(queue) = cell.borrow().as_ref() {
            queue.iter().any(|&b| b == 0x0D || b == 0x0A)
        } else {
            false
        }
    });

    if has_terminator {
        let mut command_data: Vec<u8, 64> = Vec::new();
        let mut extracted = false;

        EVENT_QUEUE.lock(|cell| {
            if let Some(queue) = cell.borrow_mut().as_mut() {
                while let Some(byte) = queue.pop_front() {
                    if byte == 0x0D || byte == 0x0A {
                        break;
                    }
                    let _ = command_data.push(byte);
                }
                extracted = true;
            }
        });

        if extracted && !command_data.is_empty() {
            // Command data already excludes terminators — process directly
            handle_command_data_internal(&command_data);
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
            let traced = TracedCommand::new(cmd, CommChannel::Uart);
            let mut depth = 0;
            let mut should_process = true;
            let mut use_channel = false;
            let mut queued = false;

            critical_section::with(|cs| {
                let multiplexer = ServiceContainer::get_multiplexer();
                let mut guard = multiplexer.borrow(cs).borrow_mut();
                if let Some(mux) = guard.as_mut() {
                    should_process = mux.should_process_command(CommChannel::Uart);
                }

                if should_process {
                    COMMAND_QUEUE.lock(|cell| {
                        if let Some(queue) = cell.borrow_mut().as_mut() {
                            match queue.try_push(traced) {
                                Ok(()) => {
                                    depth = queue.len();
                                    queued = true;
                                }
                                Err(QueueError::Full) => {
                                    debug!("UART command queue full, rejecting command");
                                    use_channel = true;
                                }
                            }
                        } else {
                            use_channel = true;
                        }
                    });
                }
            });

            if should_process {
                if queued {
                    trace_command_enqueue(&traced, depth, false);
                } else if use_channel {
                    trace_command_enqueue(&traced, depth, true);
                    let _ = ServiceContainer::get_artisan_channel().try_send(traced);
                }
            }
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
            let mut message = String::<TRACE_EVENT_MAX_LEN>::new();
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

    let pipe = match take_pipe() {
        Some(p) => p,
        None => return,
    };

    loop {
        pipe.read(&mut wbuf).await;

        let len = wbuf.iter().take_while(|&&b| b != 0).count();
        if len > 0 {
            let _ = driver::uart_write_bytes(&wbuf[..len]).await;
        }
    }
}

pub async fn send_response(response: &str) -> Result<(), crate::input::InputError> {
    let output_channel = ServiceContainer::get_output_channel();
    let line = String::<TRACE_EVENT_MAX_LEN>::try_from(response)
        .map_err(|_| crate::input::InputError::BufferFull)?;
    let _ = output_channel.try_send(line);
    Ok(())
}

pub async fn send_stream(data: &str) -> Result<(), crate::input::InputError> {
    let output_channel = ServiceContainer::get_output_channel();
    let line = String::<TRACE_EVENT_MAX_LEN>::try_from(data)
        .map_err(|_| crate::input::InputError::BufferFull)?;
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
        let (cmd_opt, queue_depth) = COMMAND_QUEUE.lock(|cell| {
            if let Some(queue) = cell.borrow_mut().as_mut() {
                let cmd = queue.pop();
                let depth = queue.len();
                (cmd, depth)
            } else {
                (None, 0)
            }
        });
        record_queue_depth(queue_depth);

        if let Some(cmd) = cmd_opt {
            trace_queue_dequeue(&cmd, queue_depth);
            let channel = ServiceContainer::get_artisan_channel();
            channel.send(cmd).await;
        }

        // Small delay to yield to other tasks and prevent tight looping
        Timer::after(Duration::from_millis(5)).await;
    }
}

// Keep legacy function for compatibility - now delegates to queue-based processing
pub fn process_command_data(data: &[u8]) {
    let event_queue_initialized = EVENT_QUEUE.lock(|cell| cell.borrow().as_ref().is_some());

    if event_queue_initialized {
        // Standard path: enqueue bytes and process complete frames.
        push_to_event_queue(data);
        process_event_queue();
        return;
    }

    // Compatibility path (mainly tests): process a single frame directly.
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
