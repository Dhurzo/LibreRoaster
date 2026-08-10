//! Generic transport tasks for UART and USB CDC.
//!
//! This module provides a unified implementation of the receive/parse/enqueue
//! logic that was previously duplicated between `uart/tasks.rs` and
//! `usb_cdc/tasks.rs`. Each transport implements the `RxSource` trait, and
//! the generic functions handle the rest. Transport-specific
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
use embassy_time::{Duration, Timer};
use heapless::{Deque, String, Vec};
use log::debug;

/// Size of the event queue for buffering incoming bytes before parsing.
pub const EVENT_QUEUE_SIZE: usize = 256;

/// Trait for transport receive half.
#[allow(async_fn_in_trait)]
pub trait RxSource {
    type Error: core::fmt::Debug;

    /// Read bytes into the provided buffer.
    async fn read_bytes(buffer: &mut [u8]) -> Result<usize, Self::Error>;
}

/// Configuration for a transport task set.
#[derive(Clone, Copy)]
pub struct TransportConfig {
    /// Unique name for this transport (used in logs).
    pub name: &'static str,
    /// Channel identifier for multiplexer routing.
    pub channel: CommChannel,
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
}

impl TransportRxState {
    pub const fn new() -> Self {
        Self {
            event_queue: BlockingMutex::new(RefCell::new(None)),
        }
    }

    pub fn init(&self) {
        self.event_queue
            .lock(|cell| *cell.borrow_mut() = Some(Deque::new()));
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
                // Bug M3 (2026-08-10): while discarding, consume the bytes
                // of the corrupted line WITHOUT enqueueing them. When its
                // terminator arrives, push it so `process_event_queue`'s
                // terminator-only branch emits the `buffer_overflow` ERR and
                // consumes the latch — a subsequent CLEAN command is then
                // accepted instead of being wrongly attributed to the
                // overflow and dropped (previously a `STOP`/`EmergencyStop`
                // right after a garbage burst was silently lost).
                if overflow.discarding {
                    if byte == 0x0D || byte == 0x0A {
                        overflow.discarding = false;
                        let _ = queue.push_back(byte);
                    }
                    continue;
                }
                if queue.len() >= EVENT_QUEUE_SIZE {
                    // Drop the entire pending partial command.
                    queue.clear();
                    overflow.triggered = true;
                    if byte == 0x0D || byte == 0x0A {
                        // The overflow byte closes a line: keep it so the
                        // terminator-only extraction reports the overflow.
                        let _ = queue.push_back(byte);
                    } else {
                        // Mid-line overflow: discard until the terminator.
                        overflow.discarding = true;
                    }
                    continue;
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
    /// Bug M3 (2026-08-10): while true, `push_to_event_queue` consumes
    /// incoming bytes without enqueueing them until the corrupted line's
    /// terminator arrives, so the first clean command after a flush is NOT
    /// discarded along with the garbage.
    pub discarding: bool,
}

/// Bug P7 (2026-08-03): decide whether a read error on `channel` should be
/// counted toward the comms-error emergency threshold. ONLY the multiplexer's
/// ACTIVE channel counts: 10 consecutive read failures on a transport that is
/// not in use (e.g. a broken UART line while Artisan runs over USB) must not
/// abort the session with `emergency_shutdown`. Errors on an inactive channel
/// are still logged for diagnostics.
pub fn should_count_read_error(active: CommChannel, channel: CommChannel) -> bool {
    active == channel
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

    // Bug D (2026-08-03): a command on the INACTIVE transport was silently
    // discarded by the multiplexer — this path had NO feedback at all (the
    // ERR paths only exist for channel_full / parse errors, and both write
    // to the active transport). An EmergencyStop or STOP sent over the wrong
    // wire (e.g. UART while USB is the active session) was lost forever with
    // only an `info!` in the log. Emit an explicit ERR through the output
    // channel; the dual-output task routes it to the active session, so the
    // operator at least sees that a command was refused, not processed.
    if !should_process {
        let output_channel = ServiceContainer::get_output_channel();
        let mut msg = String::<TRACE_EVENT_MAX_LEN>::new();
        let _ = msg.push_str("ERR command_ignored_inactive_channel");
        let _ = output_channel.try_send(msg);
    }
}

/// Send an `ERR channel_full` response through the output channel so the
/// host knows its command was dropped due to backpressure. Multiplexer-aware
/// (only writes if this channel is the active TX).
///
/// Bug P8 (2026-08-03): a dropped/parse-error command must NOT activate a
/// channel from `None` — previously both error paths called
/// `mux.on_command_received(channel)`, so boot-time garbage on UART could
/// hijack the multiplexer (making UART the active/response route) before any
/// VALID command had been seen. Channel activation is reserved for
/// `handle_parsed_command` (a successfully parsed command only).
async fn send_channel_full_error(channel: CommChannel, _config: &TransportConfig) {
    let mut should_write = true;
    critical_section::with(|cs| {
        let multiplexer = ServiceContainer::get_multiplexer();
        let mut guard = multiplexer.borrow(cs).borrow_mut();
        if let Some(mux) = guard.as_mut() {
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
///
/// Bug P8 (2026-08-03): must NOT activate a channel from `None` — see
/// `send_channel_full_error`. A garbage line in the boot window is silently
/// dropped (no active channel to reply to); a real session is unaffected.
pub(crate) async fn send_parse_error(
    error: ParseError,
    channel: CommChannel,
    _config: &TransportConfig,
) {
    let mut should_write = true;

    critical_section::with(|cs| {
        let multiplexer = ServiceContainer::get_multiplexer();
        let mut guard = multiplexer.borrow(cs).borrow_mut();
        if let Some(mux) = guard.as_mut() {
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
                // Bug P7 (2026-08-03): count a read error only when this
                // transport is the multiplexer's ACTIVE channel. The control
                // loop trips a global emergency at 10 consecutive errors
                // (`MAX_COMMS_READ_ERRORS`); counting every transport
                // regardless of the active session meant a dead UART line
                // could abort a healthy USB roast (~1 s of failure). Errors on
                // an inactive transport are still logged for diagnostics.
                let active_channel = critical_section::with(|cs| {
                    ServiceContainer::get_multiplexer()
                        .borrow(cs)
                        .borrow()
                        .as_ref()
                        .map(|mux| mux.get_active_channel())
                        .unwrap_or(CommChannel::None)
                });
                if should_count_read_error(active_channel, config.channel) {
                    crate::hardware::error_counters::increment_error_count(config.name);
                }
                log::warn!("{} read error: {:?}", config.name, e);
            }
        }

        process_event_queue(&state.event_queue, config.channel, config, &mut overflow).await;

        if !buffer_was_full {
            Timer::after(reader_poll_interval).await;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::application::service_container::ServiceContainer;
    use crate::common::{StubFan, StubHeater};
    use crate::config::ArtisanCommand;
    use crate::control::RoasterControl;
    use crate::hardware::sensors::SensorConversionHub;
    use crate::input::ArtisanInput;
    use futures::executor::block_on;
    use std::sync::Mutex;

    /// Serializes the tests that touch the global `ServiceContainer` so
    /// parallel execution cannot interleave channels across tests.
    static CONTAINER_MUTEX: Mutex<()> = Mutex::new(());

    fn init_container() {
        let _guard = CONTAINER_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
        let roaster = RoasterControl::new(
            Box::new(StubHeater::new()),
            Box::new(StubFan::new()),
            SensorConversionHub::new(),
        )
        .expect("RoasterControl should build");
        ServiceContainer::init_roaster(roaster);
        ServiceContainer::init_artisan_input(ArtisanInput::new().expect("input should build"));
        ServiceContainer::init_multiplexer();
        while ServiceContainer::get_artisan_channel()
            .try_receive()
            .is_ok()
        {}
        while ServiceContainer::get_output_channel().try_receive().is_ok() {}
    }

    fn test_config() -> TransportConfig {
        TransportConfig {
            name: "test",
            channel: CommChannel::Uart,
            ..TransportConfig::default()
        }
    }

    /// Bug-hunt T-B1: byte-level accumulation across pushes (the production
    /// event queue, which the legacy `process_command_data` helpers do NOT
    /// provide — they drop unterminated fragments between calls).
    #[test]
    fn event_queue_accumulates_byte_drip_and_handles_crlf() {
        let state = TransportRxState::new();
        state.init();
        let mut overflow = EventQueueOverflow::default();

        // Drip "OT1 75\r\n" one byte at a time, as a slow host would.
        for &b in b"OT1 75\r\n" {
            push_to_event_queue(&state.event_queue, &[b], &mut overflow);
        }
        assert!(event_queue_has_terminator(&state.event_queue));

        let first = extract_line_from_event_queue(&state.event_queue);
        assert_eq!(
            first.as_deref(),
            Some(b"OT1 75".as_slice()),
            "bytes must accumulate across pushes into one command"
        );
        // The bare LF (trailing byte of CRLF) is consumed, not parsed
        // (Bug B11 semantics).
        assert!(
            extract_line_from_event_queue(&state.event_queue).is_none(),
            "bare LF must extract as None"
        );
        assert!(!event_queue_has_terminator(&state.event_queue));
        assert!(!overflow.triggered);
    }

    /// Bug-hunt T-B2: a byte-dripped command parses and dispatches through
    /// the PRODUCTION pipeline (event queue → extract → parse → multiplexer
    /// → artisan channel).
    #[test]
    fn byte_drip_command_parsed_end_to_end() {
        init_container();

        let state = TransportRxState::new();
        state.init();
        let config = test_config();
        let mut overflow = EventQueueOverflow::default();

        // Partial command without terminator across pushes.
        for &b in b"OT1 7" {
            push_to_event_queue(&state.event_queue, &[b], &mut overflow);
        }
        assert!(!event_queue_has_terminator(&state.event_queue));
        for &b in b"5\r" {
            push_to_event_queue(&state.event_queue, &[b], &mut overflow);
        }

        block_on(process_event_queue(
            &state.event_queue,
            CommChannel::Uart,
            &config,
            &mut overflow,
        ));

        let channel = ServiceContainer::get_artisan_channel();
        let mut cmds = alloc::vec::Vec::new();
        while let Ok(traced) = channel.try_receive() {
            cmds.push(traced.command);
        }
        assert_eq!(
            cmds,
            alloc::vec![ArtisanCommand::SetHeater(75)],
            "dripped command must arrive intact"
        );
    }

    /// Bug-hunt T-B3: a queue overflow must flush the partial command and
    /// emit `ERR buffer_overflow`; a valid command arriving AFTER the
    /// overflow is the trailing fragment and must NOT execute (EC-01, in the
    /// production path).
    #[test]
    fn queue_overflow_flushes_and_blocks_stale_command() {
        init_container();

        let state = TransportRxState::new();
        state.init();
        let config = test_config();
        let mut overflow = EventQueueOverflow::default();

        // Activate the UART session with a valid command first so parse
        // errors have a route to the output channel.
        push_to_event_queue(&state.event_queue, b"READ\r", &mut overflow);
        block_on(process_event_queue(
            &state.event_queue,
            CommChannel::Uart,
            &config,
            &mut overflow,
        ));
        while ServiceContainer::get_artisan_channel()
            .try_receive()
            .is_ok()
        {}

        // Flood 300 bytes without a terminator: queue clears + overflow latch.
        let junk = [b'X'; 300];
        push_to_event_queue(&state.event_queue, &junk, &mut overflow);
        assert!(overflow.triggered, "overflow must be latched");
        assert!(!event_queue_has_terminator(&state.event_queue));

        // A valid command after the flood is the trailing fragment.
        push_to_event_queue(&state.event_queue, b"OT1 50\r", &mut overflow);
        block_on(process_event_queue(
            &state.event_queue,
            CommChannel::Uart,
            &config,
            &mut overflow,
        ));

        let output = ServiceContainer::get_output_channel();
        let mut lines = alloc::vec::Vec::new();
        while let Ok(line) = output.try_receive() {
            lines.push(line.as_str().to_string());
        }
        assert!(
            lines.iter().any(|l| l.starts_with("ERR buffer_overflow")),
            "overflow must surface as ERR buffer_overflow, got {:?}",
            lines
        );

        assert!(
            ServiceContainer::get_artisan_channel()
                .try_receive()
                .is_err(),
            "the post-overflow fragment must never execute"
        );
    }
}
