//! Generic transport tasks for UART and USB CDC.
//!
//! This module provides a unified implementation of the receive/parse/enqueue
//! logic that was previously duplicated between `uart/tasks.rs` and
//! `usb_cdc/tasks.rs`. Each transport implements the `RxSource` and `TxSink`
//! traits, and the generic functions handle the rest. Transport-specific
//! modules provide non-generic `#[embassy_executor::task]` wrappers.
//!
//! F5.3: Command path simplified — reader task pushes parsed commands directly
//! to the artisan channel via `try_send` (with multiplexer gating). The
//! intermediate `command_queue` and `run_queue_processor_task` have been
//! removed (was: 2 queues + 3 tasks per transport → 1 queue + 1 task).

use crate::application::queue_metrics::record_queue_depth;
use crate::application::service_container::ServiceContainer;
use crate::input::multiplexer::CommChannel;
use crate::input::parser::ParseError;
use crate::logging::traceability::{trace_command_enqueue, TracedCommand, TRACE_EVENT_MAX_LEN};
use core::cell::RefCell;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::Mutex as BlockingMutex;
use embassy_sync::pipe::Pipe;
use embassy_time::{Duration, Timer};
use heapless::{Deque, String, Vec};
use log::debug;

/// Size of the event queue for buffering incoming bytes before parsing.
pub const EVENT_QUEUE_SIZE: usize = 256;

/// Size of the command pipe for the writer task (UART only; USB CDC writes
/// directly via the output channel).
pub const COMMAND_PIPE_SIZE: usize = 256;

/// Trait for transport receive half.
#[allow(async_fn_in_trait)]
pub trait RxSource {
    type Error: core::fmt::Debug;

    /// Read bytes into the provided buffer.
    async fn read_bytes(buffer: &mut [u8]) -> Result<usize, Self::Error>;
}

/// Trait for transport transmit half.
#[allow(async_fn_in_trait)]
pub trait TxSink {
    type Error: core::fmt::Debug;

    /// Write bytes to the transport.
    async fn write_bytes(data: &[u8]) -> Result<(), Self::Error>;
}

/// Configuration for a transport task set.
#[derive(Clone, Copy)]
pub struct TransportConfig {
    /// Unique name for this transport (used in logs).
    pub name: &'static str,
    /// Channel identifier for multiplexer routing.
    pub channel: CommChannel,
    /// Size of the event queue (bytes).
    pub event_queue_size: usize,
    /// Size of the command pipe for writer task.
    pub command_pipe_size: usize,
    /// Initial delay before reader task starts polling (ms).
    pub reader_start_delay_ms: u64,
    /// Initial delay before writer task starts (ms).
    pub writer_start_delay_ms: u64,
    /// Poll interval for reader task (ms).
    pub reader_poll_interval_ms: u64,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            name: "transport",
            channel: CommChannel::None,
            event_queue_size: EVENT_QUEUE_SIZE,
            command_pipe_size: COMMAND_PIPE_SIZE,
            reader_start_delay_ms: 10,
            writer_start_delay_ms: 20,
            reader_poll_interval_ms: 10,
        }
    }
}

/// Internal state for a transport's receive path.
pub struct TransportRxState {
    pub event_queue:
        BlockingMutex<CriticalSectionRawMutex, RefCell<Option<Deque<u8, EVENT_QUEUE_SIZE>>>>,
    pub command_pipe: BlockingMutex<
        CriticalSectionRawMutex,
        RefCell<Option<Pipe<CriticalSectionRawMutex, COMMAND_PIPE_SIZE>>>,
    >,
}

impl TransportRxState {
    pub const fn new() -> Self {
        Self {
            event_queue: BlockingMutex::new(RefCell::new(None)),
            command_pipe: BlockingMutex::new(RefCell::new(None)),
        }
    }

    pub fn init(&self) {
        self.event_queue
            .lock(|cell| *cell.borrow_mut() = Some(Deque::new()));
        self.command_pipe
            .lock(|cell| *cell.borrow_mut() = Some(Pipe::new()));
    }

    fn take_pipe(&self) -> Option<Pipe<CriticalSectionRawMutex, COMMAND_PIPE_SIZE>> {
        self.command_pipe.lock(|cell| cell.borrow_mut().take())
    }
}

impl Default for TransportRxState {
    fn default() -> Self {
        Self::new()
    }
}

/// Push received bytes to the event queue.
///
/// Bug #2 fix: if the queue is full when a new byte arrives, the entire
/// pending partial command is flushed rather than dropping just the oldest
/// byte. Dropping one byte from the middle of an in-progress command would
/// corrupt it silently (e.g. "SETTAR" losing its leading 'S' becomes
/// "ETTAR"), producing nonsense when the terminator finally arrives.
/// Flushing the whole queue guarantees the host sees a clean error
/// (`ERR buffer_overflow`) on the next terminator instead of a corrupted
/// command. The error itself is emitted by `process_event_queue` when it
/// detects the empty line caused by the flush.
pub(crate) fn push_to_event_queue(
    event_queue: &BlockingMutex<
        CriticalSectionRawMutex,
        RefCell<Option<Deque<u8, EVENT_QUEUE_SIZE>>>,
    >,
    data: &[u8],
    overflow: &mut EventQueueOverflow,
) {
    event_queue.lock(|cell| {
        if let Some(queue) = cell.borrow_mut().as_mut() {
            for &byte in data {
                if queue.len() >= EVENT_QUEUE_SIZE {
                    // Drop the entire pending partial command.
                    queue.clear();
                    overflow.triggered = true;
                }
                let _ = queue.push_back(byte);
            }
        }
    });
}

/// Tracks whether the event queue has overflowed since the last line was
/// extracted. Used by `process_event_queue` to emit an `ERR buffer_overflow`
/// instead of silently corrupting the next command.
#[derive(Default)]
pub struct EventQueueOverflow {
    pub triggered: bool,
}

/// Check if the event queue has a line terminator (CR or LF).
pub(crate) fn event_queue_has_terminator(
    event_queue: &BlockingMutex<
        CriticalSectionRawMutex,
        RefCell<Option<Deque<u8, EVENT_QUEUE_SIZE>>>,
    >,
) -> bool {
    event_queue.lock(|cell| {
        if let Some(queue) = cell.borrow().as_ref() {
            queue.iter().any(|&b| b == 0x0D || b == 0x0A)
        } else {
            false
        }
    })
}

/// Extract one complete line from the event queue.
pub(crate) fn extract_line_from_event_queue(
    event_queue: &BlockingMutex<
        CriticalSectionRawMutex,
        RefCell<Option<Deque<u8, EVENT_QUEUE_SIZE>>>,
    >,
) -> Option<Vec<u8, 256>> {
    let mut command_data = Vec::<u8, 256>::new();
    let mut extracted = false;

    event_queue.lock(|cell| {
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
        Some(command_data)
    } else {
        None
    }
}

/// Handle a parsed command: check multiplexer, push to artisan channel via try_send.
async fn handle_parsed_command(
    cmd: crate::config::ArtisanCommand,
    channel: CommChannel,
    config: &TransportConfig,
) {
    let traced = TracedCommand::new(cmd, channel);
    let mut should_process = true;
    let mut sent = false;
    let mut channel_full = false;

    critical_section::with(|cs| {
        let multiplexer = ServiceContainer::get_multiplexer();
        let mut guard = multiplexer.borrow(cs).borrow_mut();
        if let Some(mux) = guard.as_mut() {
            should_process = mux.should_process_command(channel);
        }

        if should_process {
            let artisan_channel = ServiceContainer::get_artisan_channel();
            match artisan_channel.try_send(traced) {
                Ok(()) => {
                    trace_command_enqueue(&traced, artisan_channel.len(), false);
                    sent = true;
                }
                Err(_) => {
                    debug!("{} artisan channel full, command dropped", config.name);
                    channel_full = true;
                }
            }
        }
    });

    // Bug V2-15: record the queue depth on EVERY dispatch decision — the
    // drop path (channel full) is the one that motivated this metric in the
    // first place, but the previous code only recorded it inside `if sent`.
    // Dropped commands left zero telemetry footprint, hiding back-pressure.
    record_queue_depth(ServiceContainer::get_artisan_channel().len());

    // Bug #1: notify the host that the command was dropped because the
    // artisan channel was full. Without this, Artisan would keep sending
    // commands that disappear silently, leaving the roaster in an
    // unexpected state. Emitting ERR lets the host decide to retry.
    if channel_full {
        send_channel_full_error(channel, config).await;
    }
}

/// Send an `ERR channel_full` response through the output channel so the
/// host knows its command was dropped due to backpressure. Multiplexer-aware
/// (only writes if this channel is the active TX).
async fn send_channel_full_error(channel: CommChannel, _config: &TransportConfig) {
    let mut should_write = true;
    critical_section::with(|cs| {
        let multiplexer = ServiceContainer::get_multiplexer();
        let mut guard = multiplexer.borrow(cs).borrow_mut();
        if let Some(mux) = guard.as_mut() {
            if matches!(mux.get_active_channel(), CommChannel::None) {
                let _ = mux.on_command_received(channel);
            }
            should_write = mux.should_write_to(channel);
        }

        if should_write {
            let output_channel = ServiceContainer::get_output_channel();
            let mut message = String::<TRACE_EVENT_MAX_LEN>::new();
            let _ = message.push_str("ERR channel_full command_dropped");
            let _ = output_channel.try_send(message);
        }
    });
}

/// Send a parse error response via the output channel (multiplexer-aware).
async fn send_parse_error(error: ParseError, channel: CommChannel, _config: &TransportConfig) {
    let mut should_write = true;

    critical_section::with(|cs| {
        let multiplexer = ServiceContainer::get_multiplexer();
        let mut guard = multiplexer.borrow(cs).borrow_mut();
        if let Some(mux) = guard.as_mut() {
            if matches!(mux.get_active_channel(), CommChannel::None) {
                let _ = mux.on_command_received(channel);
            }
            should_write = mux.should_write_to(channel);
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

/// Process the event queue: drain *all* complete lines in this iteration.
///
/// Bug B11: the previous implementation did a single `if
/// event_queue_has_terminator` per call. With CRLF terminators the first
/// pass extracted the command and left the trailing LF in the queue; on
/// the next arrival the only extraction consumed that bare LF and returned
/// `None` (terminator-only → empty buffer), silently discarding the turn
/// even though a complete command sat in the queue. Net effect: one
/// command per two arrivals, half of the Artisan `READ` polls unanswered,
/// and a ramp-up burst (`CHAN;\nUNITS;\nFILT;\n` in one chunk) was
/// serialized at one-command-per-poll.
///
/// Fix: (1) loop while any terminator is present, (2) on a terminator-only
/// extraction (`None`) `continue` to keep draining rather than exit, so a
/// bare LF (the trailing byte of CRLF) does not consume a turn.
pub(crate) async fn process_event_queue(
    event_queue: &BlockingMutex<
        CriticalSectionRawMutex,
        RefCell<Option<Deque<u8, EVENT_QUEUE_SIZE>>>,
    >,
    channel: CommChannel,
    config: &TransportConfig,
    overflow: &mut EventQueueOverflow,
) {
    while event_queue_has_terminator(event_queue) {
        let Some(command_data) = extract_line_from_event_queue(event_queue) else {
            // A terminator was present but the extracted line was empty
            // (e.g. a bare LF left over from a CRLF). The terminator has
            // been consumed; keep draining the rest of the queue rather
            // than waiting for another byte to arrive.
            // Bug V2-10: but if an overflow was latched, the trailing
            // fragment (here, only terminators) carried the overflow flag
            // and must still produce the buffer_overflow error — otherwise
            // the flag survives into the next VALID command and gets
            // wrongly attributed to it. Consume the flag here and surface
            // the error for THIS extraction turn rather than the next
            // command.
            if overflow.triggered {
                overflow.triggered = false;
                send_parse_error(ParseError::BufferOverflow, channel, config).await;
                return;
            }
            continue;
        };

        // Bug #2: if the event queue overflowed since the last line was
        // extracted, the current line is the trailing fragment of a
        // command whose leading bytes were dropped. Emit an explicit
        // buffer_overflow error so the host knows its command was
        // discarded, rather than chasing the (truncated) remaining bytes
        // through the parser.
        if overflow.triggered {
            overflow.triggered = false;
            send_parse_error(ParseError::BufferOverflow, channel, config).await;
            return;
        }

        // If the command buffer is at capacity (256 bytes), the command
        // was truncated — emit an explicit error rather than parse the
        // truncated junk.
        if command_data.len() >= 256 {
            send_parse_error(ParseError::CommandTooLong, channel, config).await;
            continue;
        }

        let parse_result = if command_data.is_empty() {
            Err(ParseError::EmptyCommand)
        } else {
            core::str::from_utf8(&command_data)
                .map_err(|_| ParseError::InvalidValue)
                .and_then(crate::input::parse_artisan_command)
        };

        match parse_result {
            Ok(cmd) => {
                handle_parsed_command(cmd, channel, config).await;
            }
            Err(error) => {
                send_parse_error(error, channel, config).await;
            }
        }
    }
}

/// Generic reader task implementation.
///
/// Reads bytes from the transport, buffers them in an event queue,
/// extracts complete lines (CR/LF terminated), parses them as Artisan
/// commands, and pushes them directly to the artisan channel via try_send
/// (or drops with debug log when full). No intermediate command queue.
pub async fn run_reader_task<RX: RxSource>(
    _rx: RX,
    state: &'static TransportRxState,
    config: &'static TransportConfig,
) {
    state.init();

    let mut rbuf = [0u8; 64];
    let mut overflow = EventQueueOverflow::default();
    let reader_poll_interval = Duration::from_millis(config.reader_poll_interval_ms);
    let reader_start_delay = Duration::from_millis(config.reader_start_delay_ms);

    Timer::after(reader_start_delay).await;

    loop {
        // L6: track whether the most recent read filled the buffer so we
        // can skip the 10 ms poll sleep when a burst is in flight and the
        // UART FIFO would otherwise overflow waiting for the next tick.
        let mut buffer_was_full = false;
        // Read from the transport using the trait's static method
        match RX::read_bytes(&mut rbuf).await {
            Ok(len) if len > 0 => {
                crate::hardware::error_counters::reset_error_count(config.name);
                push_to_event_queue(&state.event_queue, &rbuf[..len], &mut overflow);
                if len == rbuf.len() {
                    buffer_was_full = true;
                }
            }
            Ok(0) => { /* no data — idle poll */ }
            Ok(_) => { /* should not happen */ }
            Err(e) => {
                crate::hardware::error_counters::increment_error_count(config.name);
                log::warn!("{} read error: {:?}", config.name, e);
            }
        }

        process_event_queue(&state.event_queue, config.channel, config, &mut overflow).await;

        if !buffer_was_full {
            Timer::after(reader_poll_interval).await;
        }
    }
}

/// Generic writer task implementation (for transports that use a command pipe, like UART).
///
/// Reads from the command pipe and writes to the transport.
pub async fn run_writer_task<TX: TxSink>(
    _tx: TX,
    state: &'static TransportRxState,
    config: &'static TransportConfig,
) {
    let writer_start_delay = Duration::from_millis(config.writer_start_delay_ms);
    let pipe = state.take_pipe();

    let Some(pipe) = pipe else {
        log::warn!("{} writer task: command pipe not initialized", config.name);
        return;
    };

    let mut wbuf = [0u8; COMMAND_PIPE_SIZE];

    Timer::after(writer_start_delay).await;

    loop {
        let len = pipe.read(&mut wbuf).await;
        if len > 0 {
            if let Err(e) = TX::write_bytes(&wbuf[..len]).await {
                log::warn!("{} write error: {:?}", config.name, e);
            }
        }
    }
}
