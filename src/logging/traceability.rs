//! Command/event traceability for the control pipeline.
//!
//! Emits `TRACE,...` events (when the `instrumentation` or `test` feature is
//! enabled) that correlate each Artisan command with its queue, actuation,
//! and telemetry stages via a monotonic `TraceId`. Release builds compile the
//! formatting helpers to nothing (Audit M-R1).

#[cfg(any(feature = "instrumentation", feature = "test"))]
use crate::application::service_container::ServiceContainer;
use crate::config::ArtisanCommand;
use crate::error::AppError;
use crate::input::multiplexer::CommChannel;
#[cfg(any(feature = "instrumentation", feature = "test"))]
use core::fmt::Write;
#[cfg(any(feature = "instrumentation", feature = "test"))]
use heapless::String;
use portable_atomic::{AtomicU32, Ordering};

/// F5.5 (Gap #2): Maximum length of a single trace/telemetry event routed
/// through the shared output channel. Previously 192, which left the STATUS
/// line (`format_status_response`) and `#DUMP` data rows too close to their
/// formatted sizes — a slightly longer STATUS or dump row would trip the
/// `ERR status_too_long` path when a small bump in field width would have
/// sufficed. 256 gives ~25% headroom over the 20-field STATUS line and
/// keeps a single dump row (sample ~24 chars) comfortably below the cap.
/// Embedded cost: +64 bytes static per channel buffer (≈30 callsites); well
/// within the riscv32imc RAM budget.
pub const TRACE_EVENT_MAX_LEN: usize = 256;

static TRACE_ID_COUNTER: AtomicU32 = AtomicU32::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Monotonic identifier correlating traced events across the pipeline.
pub struct TraceId(u32);

impl TraceId {
    /// Allocate the next unique trace id (wraps 0 to 1 to stay non-zero).
    pub fn next() -> Self {
        let next = TRACE_ID_COUNTER
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(1);

        if next == 0 {
            TraceId(1)
        } else {
            TraceId(next)
        }
    }

    /// Construct a trace id from a raw value (e.g. from a test fixture).
    pub const fn from_u32(value: u32) -> Self {
        TraceId(value)
    }

    /// Return the underlying numeric id.
    pub const fn value(&self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Pipeline stage a traced event was emitted at.
pub enum TraceStep {
    /// Command pushed onto the shared channel.
    QueueEnqueue,
    /// Command pulled off the shared channel.
    QueueDequeue,
    /// Command pushed via the fallback (no active transport) path.
    QueueFallback,
    /// Actuator outputs applied (SSR + fan).
    Actuation,
    /// Periodic telemetry/guard snapshot emitted.
    Telemetry,
    /// Guard (LEDC/time watchdog) evaluation emitted.
    Guard,
}

impl TraceStep {
    /// Return the stable string label for the trace step.
    pub const fn as_str(&self) -> &'static str {
        match self {
            TraceStep::QueueEnqueue => "queue_enqueue",
            TraceStep::QueueDequeue => "queue_dequeue",
            TraceStep::QueueFallback => "queue_fallback",
            TraceStep::Actuation => "actuation",
            TraceStep::Telemetry => "telemetry",
            TraceStep::Guard => "guard",
        }
    }
}

#[derive(Clone, Copy, Debug)]
/// A command tagged with its trace id and originating transport.
pub struct TracedCommand {
    pub command: ArtisanCommand,
    pub trace_id: TraceId,
    pub channel: CommChannel,
}

impl TracedCommand {
    /// Build a traced command with a freshly allocated trace id.
    pub fn new(command: ArtisanCommand, channel: CommChannel) -> Self {
        Self {
            command,
            trace_id: TraceId::next(),
            channel,
        }
    }

    /// Build a traced command with a caller-supplied trace id (tests).
    pub const fn with_trace_id(
        command: ArtisanCommand,
        channel: CommChannel,
        trace_id: TraceId,
    ) -> Self {
        Self {
            command,
            trace_id,
            channel,
        }
    }
}

/// Emit a queue-enqueue (or fallback) trace event for a command.
pub fn trace_command_enqueue(traced: &TracedCommand, depth: usize, fallback: bool) {
    // Audit M-R1 (2026-08-11): in releases without instrumentation/test the
    // event was formatted (soft-float f32 Display, String<256>) and then
    // DISCARDED by emit_event on every command. Skip the formatting entirely.
    #[cfg(not(any(feature = "instrumentation", feature = "test")))]
    {
        let _ = (traced, depth, fallback);
    }
    #[cfg(any(feature = "instrumentation", feature = "test"))]
    {
        let step = if fallback {
            TraceStep::QueueFallback
        } else {
            TraceStep::QueueEnqueue
        };

        if let Some(event) = format_trace_enqueue(traced, step, depth, fallback) {
            emit_event(event);
        }
    }
}

/// Emit a queue-dequeue trace event for a command.
pub fn trace_queue_dequeue(traced: &TracedCommand, depth: usize) {
    // Audit M-R1: skip formatting when the event would be discarded.
    #[cfg(not(any(feature = "instrumentation", feature = "test")))]
    {
        let _ = (traced, depth);
    }
    #[cfg(any(feature = "instrumentation", feature = "test"))]
    {
        if let Some(event) = format_trace_dequeue(traced, depth) {
            emit_event(event);
        }
    }
}

/// Emit an actuation trace event recording SSR/fan outputs and latency.
pub fn trace_actuation(
    traced: &TracedCommand,
    ssr_output: f32,
    fan_output: f32,
    latency_us: u32,
    saturation_active: bool,
) {
    // Audit M-R1: skip formatting when the event would be discarded.
    #[cfg(not(any(feature = "instrumentation", feature = "test")))]
    {
        let _ = (
            traced,
            ssr_output,
            fan_output,
            latency_us,
            saturation_active,
        );
    }
    #[cfg(any(feature = "instrumentation", feature = "test"))]
    {
        if let Some(event) = format_trace_actuation(
            traced,
            ssr_output,
            fan_output,
            latency_us,
            saturation_active,
        ) {
            emit_event(event);
        }
    }
}

/// Emit a periodic telemetry trace event (guard/watchdog status).
pub fn trace_telemetry(
    trace_id: TraceId,
    guard_timeout: bool,
    guard_timeouts: u16,
    watchdog_feed_ok: bool,
    app_error: Option<&AppError>,
) {
    // Audit M-R1: skip formatting when the event would be discarded.
    #[cfg(not(any(feature = "instrumentation", feature = "test")))]
    {
        let _ = (
            trace_id,
            guard_timeout,
            guard_timeouts,
            watchdog_feed_ok,
            app_error,
        );
    }
    #[cfg(any(feature = "instrumentation", feature = "test"))]
    {
        if let Some(event) = format_trace_telemetry(
            trace_id,
            guard_timeout,
            guard_timeouts,
            watchdog_feed_ok,
            app_error,
        ) {
            emit_event(event);
        }
    }
}

/// Emit a guard evaluation trace event (timeout/watchdog/error status).
pub fn trace_guard(
    trace_id: TraceId,
    guard_timeout: bool,
    guard_timeouts: u16,
    watchdog_feed_ok: bool,
    watchdog_failure: Option<&'static str>,
    app_error: Option<&AppError>,
) {
    // Audit M-R1: skip formatting when the event would be discarded.
    #[cfg(not(any(feature = "instrumentation", feature = "test")))]
    {
        let _ = (
            trace_id,
            guard_timeout,
            guard_timeouts,
            watchdog_feed_ok,
            watchdog_failure,
            app_error,
        );
    }
    #[cfg(any(feature = "instrumentation", feature = "test"))]
    {
        if let Some(event) = format_trace_guard(
            trace_id,
            guard_timeout,
            guard_timeouts,
            watchdog_feed_ok,
            watchdog_failure,
            app_error,
        ) {
            emit_event(event);
        }
    }
}

#[cfg(any(feature = "instrumentation", feature = "test"))]
fn emit_event(event: String<TRACE_EVENT_MAX_LEN>) {
    #[cfg(any(feature = "instrumentation", feature = "test"))]
    let _ = crate::hardware::error_counters::try_send_output(
        ServiceContainer::get_output_channel(),
        event,
    );
    #[cfg(not(any(feature = "instrumentation", feature = "test")))]
    let _ = event;
}

#[cfg(any(feature = "instrumentation", feature = "test"))]
fn format_trace_event<F>(
    trace_id: TraceId,
    step: TraceStep,
    formatter: F,
) -> Option<String<TRACE_EVENT_MAX_LEN>>
where
    F: FnOnce(&mut String<TRACE_EVENT_MAX_LEN>) -> core::fmt::Result,
{
    let mut output = String::<TRACE_EVENT_MAX_LEN>::new();
    write!(output, "TRACE,{},{},", trace_id.value(), step.as_str()).ok()?;
    formatter(&mut output).ok()?;
    Some(output)
}

#[cfg(any(feature = "instrumentation", feature = "test"))]
fn format_trace_enqueue(
    traced: &TracedCommand,
    step: TraceStep,
    depth: usize,
    fallback: bool,
) -> Option<String<TRACE_EVENT_MAX_LEN>> {
    format_trace_event(traced.trace_id, step, |output| {
        write!(
            output,
            "cmd={:?},channel={},depth={},fallback={}",
            traced.command,
            channel_label(traced.channel),
            depth,
            bool_flag(fallback)
        )
    })
}

#[cfg(any(feature = "instrumentation", feature = "test"))]
fn format_trace_dequeue(
    traced: &TracedCommand,
    depth: usize,
) -> Option<String<TRACE_EVENT_MAX_LEN>> {
    format_trace_event(traced.trace_id, TraceStep::QueueDequeue, |output| {
        write!(
            output,
            "cmd={:?},channel={},depth={}",
            traced.command,
            channel_label(traced.channel),
            depth
        )
    })
}

#[cfg(any(feature = "instrumentation", feature = "test"))]
fn format_trace_actuation(
    traced: &TracedCommand,
    ssr_output: f32,
    fan_output: f32,
    latency_us: u32,
    saturation_active: bool,
) -> Option<String<TRACE_EVENT_MAX_LEN>> {
    format_trace_event(traced.trace_id, TraceStep::Actuation, |output| {
        write!(
            output,
            "cmd={:?},channel={},ssr={},fan={},latency_us={},saturation_active={}",
            traced.command,
            channel_label(traced.channel),
            ssr_output,
            fan_output,
            latency_us,
            bool_flag(saturation_active)
        )
    })
}

#[cfg(any(feature = "instrumentation", feature = "test"))]
fn format_trace_telemetry(
    trace_id: TraceId,
    guard_timeout: bool,
    guard_timeouts: u16,
    watchdog_feed_ok: bool,
    app_error: Option<&AppError>,
) -> Option<String<TRACE_EVENT_MAX_LEN>> {
    format_trace_event(trace_id, TraceStep::Telemetry, |output| {
        write!(
            output,
            "guard_timeout={},guard_timeouts={},watchdog={}",
            bool_flag(guard_timeout),
            guard_timeouts,
            watchdog_label(watchdog_feed_ok)
        )?;

        // Append AppError metadata if available
        if let Some(err) = app_error {
            write!(output, ",error_category={}", err.category())?;
            if let Some(source) = err.source() {
                write!(output, ",error_source={}", source)?;
            }
        }

        Ok(())
    })
}

#[cfg(any(feature = "instrumentation", feature = "test"))]
fn format_trace_guard(
    trace_id: TraceId,
    guard_timeout: bool,
    guard_timeouts: u16,
    watchdog_feed_ok: bool,
    watchdog_failure: Option<&'static str>,
    app_error: Option<&AppError>,
) -> Option<String<TRACE_EVENT_MAX_LEN>> {
    format_trace_event(trace_id, TraceStep::Guard, |output| {
        write!(
            output,
            "guard_timeout={},guard_timeouts={},watchdog={}",
            bool_flag(guard_timeout),
            guard_timeouts,
            watchdog_label(watchdog_feed_ok)
        )?;

        if let Some(reason) = watchdog_failure {
            write!(output, ",watchdog_failure={}", reason)?;
        }

        // Append AppError metadata if available
        if let Some(err) = app_error {
            write!(output, ",error_category={}", err.category())?;
            if let Some(source) = err.source() {
                write!(output, ",error_source={}", source)?;
            }
        }

        Ok(())
    })
}

#[cfg(any(feature = "instrumentation", feature = "test"))]
/// Format a safe-shutdown guard trace event (used by `trace_safe_shutdown_guard`).
pub fn format_safe_shutdown_guard(
    trace_id: TraceId,
    app_error: Option<&AppError>,
) -> Option<String<TRACE_EVENT_MAX_LEN>> {
    format_trace_guard(
        trace_id,
        true,
        1,
        false,
        Some("init_error_failure"),
        app_error,
    )
}

/// Emit a safe-shutdown guard trace event (always reports a failure).
pub fn trace_safe_shutdown_guard(trace_id: TraceId, app_error: Option<&AppError>) {
    // Audit M-R1: skip formatting when the event would be discarded.
    #[cfg(not(any(feature = "instrumentation", feature = "test")))]
    {
        let _ = (trace_id, app_error);
    }
    #[cfg(any(feature = "instrumentation", feature = "test"))]
    {
        if let Some(event) = format_safe_shutdown_guard(trace_id, app_error) {
            emit_event(event);
        }
    }
}

#[cfg(any(feature = "instrumentation", feature = "test"))]
fn channel_label(channel: CommChannel) -> &'static str {
    match channel {
        CommChannel::None => "None",
        CommChannel::Usb => "Usb",
        CommChannel::Uart => "Uart",
    }
}

#[cfg(any(feature = "instrumentation", feature = "test"))]
fn watchdog_label(ok: bool) -> &'static str {
    if ok {
        "ok"
    } else {
        "fail"
    }
}

#[cfg(any(feature = "instrumentation", feature = "test"))]
fn bool_flag(value: bool) -> u8 {
    if value {
        1
    } else {
        0
    }
}

// Audit MR-1 (2026-08-11): the `format_trace_*`/`emit_event` helpers are
// gated behind `cfg(any(feature = "instrumentation", feature = "test"))` so
// release builds compile them to nothing (no wasted formatting). The test
// module references those helpers, so it must carry the SAME gate — plain
// `cargo clippy --all-targets` (default features, test profile) otherwise
// fails to compile the module against the absent helpers.
#[cfg(all(test, any(feature = "instrumentation", feature = "test")))]
mod tests {
    use super::*;
    use crate::config::ArtisanCommand;
    use crate::input::multiplexer::CommChannel;

    fn traced_command() -> TracedCommand {
        TracedCommand::with_trace_id(
            ArtisanCommand::ReadStatus,
            CommChannel::Uart,
            TraceId::from_u32(42),
        )
    }

    #[test]
    fn format_enqueue_includes_command_and_depth() {
        let event = format_trace_enqueue(&traced_command(), TraceStep::QueueEnqueue, 5, false)
            .expect("should format enqueue event");
        assert!(event.starts_with("TRACE,42,queue_enqueue,"));
        assert!(event.contains("cmd=ReadStatus"));
        assert!(event.contains("depth=5"));
        assert!(event.contains("fallback=0"));
    }

    #[test]
    fn format_dequeue_flags_depth() {
        let event = format_trace_dequeue(&traced_command(), 2).expect("should format dequeue");
        assert!(event.starts_with("TRACE,42,queue_dequeue,"));
        assert!(event.contains("depth=2"));
    }

    #[test]
    fn format_actuation_records_outputs() {
        let traced = traced_command();
        let event = format_trace_actuation(&traced, 33.5, 12.0, 150, true)
            .expect("should format actuation");
        assert!(event.starts_with("TRACE,42,actuation,"));
        assert!(event.contains("ssr=33.5"));
        assert!(event.contains("fan=12"));
        assert!(event.contains("latency_us=150"));
        assert!(event.contains("saturation_active=1"));
    }

    #[test]
    fn format_telemetry_reports_guard_watchdog() {
        let event = format_trace_telemetry(TraceId::from_u32(7), true, 3, false, None)
            .expect("should format telemetry");
        assert!(event.starts_with("TRACE,7,telemetry,"));
        assert!(event.contains("guard_timeout=1"));
        assert!(event.contains("guard_timeouts=3"));
        assert!(event.contains("watchdog=fail"));
    }

    #[test]
    fn format_telemetry_includes_app_error_metadata() {
        let app_err = AppError::Temperature {
            message: heapless::String::<128>::try_from("Test error").unwrap(),
            source: crate::error::TemperatureError::SensorFault,
        };
        let event = format_trace_telemetry(TraceId::from_u32(8), false, 0, true, Some(&app_err))
            .expect("should format telemetry with AppError");
        assert!(event.starts_with("TRACE,8,telemetry,"));
        assert!(event.contains("guard_timeout=0"));
        assert!(event.contains("error_category=temperature"));
        assert!(event.contains("error_source=sensor_fault"));
    }

    #[test]
    fn format_guard_appends_failure_marker() {
        let event = format_trace_guard(TraceId::from_u32(9), false, 5, true, Some("timeout"), None)
            .expect("should format guard");
        assert!(event.starts_with("TRACE,9,guard,"));
        assert!(event.contains("guard_timeout=0"));
        assert!(event.contains("guard_timeouts=5"));
        assert!(event.contains("watchdog=ok"));
        assert!(event.contains("watchdog_failure=timeout"));
    }

    #[test]
    fn format_guard_includes_app_error_metadata() {
        let app_err = AppError::Control {
            source: crate::error::ControlError::PidError,
        };
        let event = format_trace_guard(TraceId::from_u32(10), false, 0, true, None, Some(&app_err))
            .expect("should format guard with AppError");
        assert!(event.starts_with("TRACE,10,guard,"));
        assert!(event.contains("guard_timeout=0"));
        assert!(event.contains("error_category=control"));
        assert!(event.contains("error_source=pid_error"));
    }

    #[test]
    fn format_safe_shutdown_guard_includes_failure_metadata() {
        let app_err = AppError::Control {
            source: crate::error::ControlError::PidError,
        };
        let event = format_safe_shutdown_guard(TraceId::from_u32(11), Some(&app_err))
            .expect("should format safe shutdown guard");
        assert!(event.starts_with("TRACE,11,guard,"));
        assert!(event.contains("guard_timeout=1"));
        assert!(event.contains("guard_timeouts=1"));
        assert!(event.contains("watchdog=fail"));
        assert!(event.contains("watchdog_failure=init_error_failure"));
        assert!(event.contains("error_category=control"));
        assert!(event.contains("error_source=pid_error"));
    }
}
