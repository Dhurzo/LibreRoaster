use crate::application::service_container::ServiceContainer;
use crate::config::ArtisanCommand;
use crate::error::AppError;
use crate::input::multiplexer::CommChannel;
use core::fmt::Write;
use heapless::String;
use portable_atomic::{AtomicU32, Ordering};

pub const TRACE_EVENT_MAX_LEN: usize = 128;

static TRACE_ID_COUNTER: AtomicU32 = AtomicU32::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TraceId(u32);

impl TraceId {
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

    pub const fn from_u32(value: u32) -> Self {
        TraceId(value)
    }

    pub const fn value(&self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceStep {
    QueueEnqueue,
    QueueDequeue,
    QueueFallback,
    Actuation,
    Telemetry,
    Guard,
}

impl TraceStep {
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
pub struct TracedCommand {
    pub command: ArtisanCommand,
    pub trace_id: TraceId,
    pub channel: CommChannel,
}

impl TracedCommand {
    pub fn new(command: ArtisanCommand, channel: CommChannel) -> Self {
        Self {
            command,
            trace_id: TraceId::next(),
            channel,
        }
    }

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

pub fn trace_command_enqueue(traced: &TracedCommand, depth: usize, fallback: bool) {
    let step = if fallback {
        TraceStep::QueueFallback
    } else {
        TraceStep::QueueEnqueue
    };

    if let Some(event) = format_trace_enqueue(traced, step, depth, fallback) {
        emit_event(event);
    }
}

pub fn trace_queue_dequeue(traced: &TracedCommand, depth: usize) {
    if let Some(event) = format_trace_dequeue(traced, depth) {
        emit_event(event);
    }
}

pub fn trace_actuation(
    traced: &TracedCommand,
    ssr_output: f32,
    fan_output: f32,
    latency_us: u32,
    saturation_active: bool,
) {
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

pub fn trace_telemetry(
    trace_id: TraceId,
    guard_timeout: bool,
    guard_timeouts: u16,
    watchdog_feed_ok: bool,
    app_error: Option<&AppError>,
) {
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

pub fn trace_guard(
    trace_id: TraceId,
    guard_timeout: bool,
    guard_timeouts: u16,
    watchdog_feed_ok: bool,
    watchdog_failure: Option<&'static str>,
    app_error: Option<&AppError>,
) {
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

fn emit_event(event: String<TRACE_EVENT_MAX_LEN>) {
    let _ = ServiceContainer::get_output_channel().try_send(event);
}

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

fn channel_label(channel: CommChannel) -> &'static str {
    match channel {
        CommChannel::None => "None",
        CommChannel::Usb => "Usb",
        CommChannel::Uart => "Uart",
    }
}

fn watchdog_label(ok: bool) -> &'static str {
    if ok {
        "ok"
    } else {
        "fail"
    }
}

fn bool_flag(value: bool) -> u8 {
    if value {
        1
    } else {
        0
    }
}

#[cfg(test)]
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
        let event = format_trace_telemetry(TraceId::from_u32(7), true, 3, false)
            .expect("should format telemetry");
        assert!(event.starts_with("TRACE,7,telemetry,"));
        assert!(event.contains("guard_timeout=1"));
        assert!(event.contains("guard_timeouts=3"));
        assert!(event.contains("watchdog=fail"));
    }

    #[test]
    fn format_guard_appends_failure_marker() {
        let event = format_trace_guard(TraceId::from_u32(9), false, 5, true, Some("timeout"))
            .expect("should format guard");
        assert!(event.starts_with("TRACE,9,guard,"));
        assert!(event.contains("guard_timeout=0"));
        assert!(event.contains("guard_timeouts=5"));
        assert!(event.contains("watchdog=ok"));
        assert!(event.contains("watchdog_failure=timeout"));
    }
}
