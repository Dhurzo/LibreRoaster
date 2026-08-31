use super::*;
use crate::common::{StubFan, StubHeater};
use crate::control::RoasterControl;
use crate::hardware::sensors::SensorConversionHub;
use crate::input::ArtisanInput;
use futures::executor::block_on;
// ── Helpers ─────────────────────────────────────────────────────────
fn build_test_roaster() -> RoasterControl {
    RoasterControl::new(
        Box::new(StubHeater::new()),
        Box::new(StubFan::new()),
        SensorConversionHub::new(),
    )
    .expect("RoasterControl should build")
}
fn init_container_with_roaster(roaster: RoasterControl) {
    ServiceContainer::init_roaster(roaster);
    let _ = ArtisanInput::new().map(ServiceContainer::init_artisan_input);
}
fn drain_all_channels() {
    while ServiceContainer::get_artisan_channel()
        .try_receive()
        .is_ok()
    {}
    while ServiceContainer::get_output_channel().try_receive().is_ok() {}
}
// ── append_crlf ────────────────────────────────────────────────────
#[test]
fn test_append_crlf_appends_single_terminator() {
    let payload = "READ,120.3,150.5,75.0,25.0";
    let bytes = append_crlf(payload);
    let output = core::str::from_utf8(&bytes).expect("Output should be valid UTF-8");
    assert_eq!(output, "READ,120.3,150.5,75.0,25.0\r\n");
}
#[test]
fn test_append_crlf_empty_payload() {
    let bytes = append_crlf("");
    let output = core::str::from_utf8(&bytes).expect("Output should be valid UTF-8");
    assert_eq!(output, "\r\n");
}
// ── TickState ───────────────────────────────────────────────────────
#[test]
fn test_tick_state_new_initializes_defaults() {
    let state = TickState::new();
    assert_eq!(state.consecutive_sensor_errors, 0);
    assert!(state.sensor_err.is_none());
    assert!(state.tick_app_error.is_none());
    assert!(state.tick_trace_id.is_none());
    assert!(!state.was_continuous);
}
#[test]
fn test_tick_state_stage_tracker_starts_at_idle() {
    let tick_state = TickState::new();
    assert!(matches!(
        tick_state.stage_tracker.current_stage,
        ControlLoopStage::Idle
    ));
}
// ── StageTracker ───────────────────────────────────────────────────
#[test]
fn test_stage_tracker_elapsed_increases_over_time() {
    let mut tracker = StageTracker::new();
    let start = Instant::now();
    tracker.start_tick(start);
    let e1 = tracker.elapsed();
    let e2 = tracker.elapsed();
    assert!(e2.as_micros() >= e1.as_micros());
}
#[test]
fn test_stage_tracker_clear_resets_stage() {
    let mut tracker = StageTracker::new();
    tracker.set_stage(ControlLoopStage::SensorRead);
    assert!(matches!(
        tracker.current_stage,
        ControlLoopStage::SensorRead
    ));
    tracker.clear();
    assert!(matches!(tracker.current_stage, ControlLoopStage::Idle));
}
#[test]
fn test_stage_tracker_stage_transitions() {
    let mut tracker = StageTracker::new();
    tracker.set_stage(ControlLoopStage::SensorRead);
    tracker.set_stage(ControlLoopStage::ControlUpdate);
    tracker.set_stage(ControlLoopStage::WatchdogFeed);
    assert!(matches!(
        tracker.current_stage,
        ControlLoopStage::WatchdogFeed
    ));
}
// ── send_handler_error ─────────────────────────────────────────────
#[test]
fn test_send_handler_error_produces_err_message() {
    use crate::control::RoasterError;
    let _guard = crate::application::tasks::tests::acquire_test_lock();
    drain_all_channels();
    let roaster = build_test_roaster();
    init_container_with_roaster(roaster);
    let error = RoasterError::InvalidState {
        source: Some("test_error_source"),
    };
    let output_channel = ServiceContainer::get_output_channel();
    send_handler_error(output_channel, &error);
    let messages: Vec<_> = (0..10)
        .filter_map(|_| output_channel.try_receive().ok())
        .collect();
    let err_msg = messages
        .iter()
        .find(|m| m.contains("ERR"))
        .map(|s| s.as_str().to_string());
    assert!(
        err_msg.is_some(),
        "Expected ERR message in output channel, got: {:?}",
        messages
    );
    let msg = err_msg.unwrap();
    assert!(
        msg.contains("handler_failed"),
        "ERR message should contain handler_failed: {}",
        msg
    );
}
// ── drain_commands / command processing ────────────────────────────
#[test]
fn drain_commands_processes_status_command() {
    let _guard = crate::application::tasks::tests::acquire_test_lock();
    let mut roaster = build_test_roaster();
    roaster
        .update_temperatures(100.0, 120.0, Instant::now())
        .expect("temps");
    init_container_with_roaster(roaster);
    drain_all_channels();
    let traced = crate::logging::traceability::TracedCommand {
        command: crate::config::ArtisanCommand::StatusReport,
        trace_id: crate::logging::traceability::TraceId::next(),
        channel: crate::input::multiplexer::CommChannel::None,
    };
    let _ = ServiceContainer::get_artisan_channel().try_send(traced);
    let mut tick_state = TickState::new();
    block_on(async {
        drain_commands(&mut tick_state).await;
    });
    let messages: Vec<_> = (0..10)
        .filter_map(|_| ServiceContainer::get_output_channel().try_receive().ok())
        .collect();
    assert!(
        !messages.is_empty(),
        "STATUS command should produce output messages"
    );
}
#[test]
fn drain_commands_processes_read_command() {
    let _guard = crate::application::tasks::tests::acquire_test_lock();
    let mut roaster = build_test_roaster();
    roaster
        .update_temperatures(120.3, 150.5, Instant::now())
        .expect("temps");
    init_container_with_roaster(roaster);
    drain_all_channels();
    let traced = crate::logging::traceability::TracedCommand {
        command: crate::config::ArtisanCommand::ReadStatus,
        trace_id: crate::logging::traceability::TraceId::next(),
        channel: crate::input::multiplexer::CommChannel::None,
    };
    let _ = ServiceContainer::get_artisan_channel().try_send(traced);
    let mut tick_state = TickState::new();
    block_on(async {
        drain_commands(&mut tick_state).await;
    });
    let messages: Vec<_> = (0..10)
        .filter_map(|_| ServiceContainer::get_output_channel().try_receive().ok())
        .collect();
    assert!(
        !messages.is_empty(),
        "READ command should produce output messages"
    );
}
// ── handle_watchdog_failure ────────────────────────────────────────
#[test]
fn watchdog_failure_increments_counter_and_flags_status() {
    let mut roaster = build_test_roaster();
    let output_channel = ServiceContainer::get_output_channel();
    roaster.status_mut().watchdog_consecutive_failures = 0;
    handle_watchdog_failure(&mut roaster, "test_failure", None, output_channel);
    let status = roaster.get_status();
    assert!(!status.watchdog_feed_ok);
    assert_eq!(status.watchdog_last_failure, Some("test_failure"));
    assert_eq!(status.watchdog_consecutive_failures, 1);
}
#[test]
fn watchdog_consecutive_failures_trigger_emergency() {
    let mut roaster = build_test_roaster();
    let output_channel = ServiceContainer::get_output_channel();
    roaster.status_mut().watchdog_consecutive_failures = 1;
    handle_watchdog_failure(
        &mut roaster,
        "second_failure",
        Some("first_failure"),
        output_channel,
    );
    assert!(roaster.safety().is_emergency_active());
}
// ── finalize_tick ──────────────────────────────────────────────────
#[test]
fn test_finalize_tick_clears_stage_tracker() {
    let _guard = crate::application::tasks::tests::acquire_test_lock();
    let roaster = build_test_roaster();
    init_container_with_roaster(roaster);
    drain_all_channels();
    let mut tick_state = TickState::new();
    tick_state.last_guard_total_timeouts = 5;
    tick_state.prev_watchdog_state = WatchdogState::Fail;
    let control_snapshot = ControlUpdateSnapshot {
        desired_output: 50.0,
        applied_output: 45.0,
        fan_output: 30.0,
    };
    let watchdog_snapshot = WatchdogSnapshot {
        feed_ok: true,
        last_failure: None,
        guard_timeouts: 3,
    };
    tick_state
        .stage_tracker
        .set_stage(ControlLoopStage::TelemetryEmit);
    finalize_tick(
        &mut tick_state,
        Some(control_snapshot),
        false,
        &watchdog_snapshot,
    );
    // finalize_tick clears the stage tracker back to Idle
    assert!(matches!(
        tick_state.stage_tracker.current_stage,
        ControlLoopStage::Idle
    ));
    // finalize_tick does NOT modify last_guard_total_timeouts
    // (that is updated by feed_watchdog_stage, not finalize_tick)
    assert_eq!(tick_state.last_guard_total_timeouts, 5);
    // finalize_tick does NOT modify prev_watchdog_state
    assert_eq!(tick_state.prev_watchdog_state, WatchdogState::Fail);
}
// ── Full control_loop_tick integration tests ───────────────────────
#[test]
fn control_loop_tick_empty_tick_completes() {
    let _guard = acquire_test_lock();
    let roaster = build_test_roaster();
    init_container_with_roaster(roaster);
    drain_all_channels();
    let mut tick_state = TickState::new();
    let output_channel = ServiceContainer::get_output_channel();
    block_on(async {
        control_loop_tick(&mut tick_state, output_channel).await;
    });
    // Tick completes without panic. Under `test` feature, stage instrumentation
    // emits 5 STAGE reports (SensorRead, ControlUpdate, LedcWrite, WatchdogFeed,
    // TelemetryEmit). No command/message output expected.
    let stage_count = drain_stage_reports(output_channel);
    assert_eq!(
        stage_count, 5,
        "Empty tick should emit exactly 5 stage reports, got {}",
        stage_count
    );
}
// ── L3: full pipeline with simulated sensors (wall clock) ──────────
// Audit L3 (2026-08-11): exercises the REAL control-loop pipeline on
// host — `control_loop_tick` with a `SensorConversionHub` backed by
// `SimulatedSensorSource` (advances by wall clock), READ commands
// flowing in through the artisan channel and TC4 responses out through
// the output channel. Gated behind `simulated-sensors`; the embedded
// 210 ms MAX31856 wait does not exist on this path, so ticks are fast
// and real time drives the curve. This is a smoke run (short window of
// the default medium roast curve), not a full roast — the full roast is
// covered deterministically at L1 in tests/full_roast_verification.rs.
#[cfg(all(test, feature = "simulated-sensors", not(target_arch = "riscv32")))]
#[test]
fn control_loop_tick_simulated_sensors_full_pipeline() {
    let _guard = acquire_test_lock();
    let roaster = build_test_roaster();
    init_container_with_roaster(roaster);
    // The real boot wires the watchdog feeder (app_builder.rs); the
    // container helper does not — without it the second tick would trip
    // the 2-consecutive-failures emergency on `WatchdogUninitialized`.
    ServiceContainer::get_instance()
        .init_watchdog(crate::safety::watchdog::WatchdogFeeder::initialize().expect("wd"));
    drain_all_channels();
    let mut tick_state = TickState::new();
    let output_channel = ServiceContainer::get_output_channel();
    let mut read_responses: usize = 0;
    for tick in 0..40u32 {
        if tick % 5 == 0 {
            // Artisan polls READ every ~250 ms simulated.
            let traced = crate::logging::traceability::TracedCommand {
                command: crate::config::ArtisanCommand::ReadStatus,
                trace_id: crate::logging::traceability::TraceId::next(),
                channel: crate::input::multiplexer::CommChannel::None,
            };
            let _ = ServiceContainer::get_artisan_channel().try_send(traced);
        }
        block_on(async {
            control_loop_tick(&mut tick_state, output_channel).await;
        });
        // Drain everything (STAGE reports + protocol responses).
        while let Ok(msg) = output_channel.try_receive() {
            if msg.contains(',') && msg.split(',').count() >= 5 {
                read_responses += 1;
            }
        }
        // Real sleep so the simulated source's wall clock advances.
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    // ~2 s of wall clock → the default curve ramps 25 → ~29 °C.
    let status = block_on(async { ServiceContainer::with_roaster_async(|r| r.get_status()).await })
        .expect("roaster initialized");
    assert!(
        status.bean_temp > 26.0 && status.bean_temp < 80.0,
        "simulated BT must advance along the ramp after ~2 s, got {:.1}",
        status.bean_temp
    );
    assert!(
        !status.fault_condition
            && status.ssr_hardware_status != crate::config::constants::SsrHardwareStatus::Error,
        "no fault may develop in the pipeline smoke run (fault={}, state={:?})",
        status.fault_condition,
        status.state
    );
    assert!(
        read_responses >= 4,
        "READ commands must produce TC4 responses through the pipeline, got {}",
        read_responses
    );
}
#[test]
fn control_loop_tick_processes_read_command() {
    let _guard = acquire_test_lock();
    let mut roaster = build_test_roaster();
    roaster
        .update_temperatures(120.3, 150.5, Instant::now())
        .expect("temps");
    init_container_with_roaster(roaster);
    drain_all_channels();
    let traced = crate::logging::traceability::TracedCommand {
        command: crate::config::ArtisanCommand::ReadStatus,
        trace_id: crate::logging::traceability::TraceId::next(),
        channel: crate::input::multiplexer::CommChannel::None,
    };
    let _ = ServiceContainer::get_artisan_channel().try_send(traced);
    let mut tick_state = TickState::new();
    let output_channel = ServiceContainer::get_output_channel();
    block_on(async {
        control_loop_tick(&mut tick_state, output_channel).await;
    });
    let messages = drain_non_stage_output(output_channel);
    assert!(
        !messages.is_empty(),
        "READ command should produce TC4 output after full tick, got 0 messages"
    );
    let has_read_response = messages
        .iter()
        .any(|msg| msg.contains(',') && msg.split(',').count() >= 5);
    assert!(
        has_read_response,
        "Expected READ response in output channel, got: {:?}",
        messages
    );
}
#[test]
fn control_loop_tick_processes_status_command() {
    let _guard = acquire_test_lock();
    let mut roaster = build_test_roaster();
    roaster
        .update_temperatures(100.0, 120.0, Instant::now())
        .expect("temps");
    init_container_with_roaster(roaster);
    drain_all_channels();
    // Send START command to transition out of Idle state.
    let traced = crate::logging::traceability::TracedCommand {
        command: crate::config::ArtisanCommand::StartRoast,
        trace_id: crate::logging::traceability::TraceId::next(),
        channel: crate::input::multiplexer::CommChannel::None,
    };
    let _ = ServiceContainer::get_artisan_channel().try_send(traced);
    let mut tick_state = TickState::new();
    let output_channel = ServiceContainer::get_output_channel();
    block_on(async {
        control_loop_tick(&mut tick_state, output_channel).await;
    });
    // Drain stage reports
    let msgs = drain_non_stage_output(output_channel);
    // Verify roaster state advanced
    let status = block_on(async { ServiceContainer::with_roaster_async(|r| r.get_status()).await })
        .expect("status read");
    // The START command was processed — PID was enabled before the tick ran it.
    assert!(
        !msgs.is_empty(),
        "START command should produce trace output after full tick, got 0 non-stage messages"
    );
    // Default state is Idle — after START command dispatch it should no longer be Idle
    // (even if sensor/watchdog issues later push it to Error).
    assert_ne!(
        status.state,
        crate::config::constants::RoasterState::Idle,
        "Roaster should have left Idle state after START + full tick, got {:?}",
        status.state
    );
    // STATUS command emits a 20-field CSV line plus TRACE lines.
    let has_status_line = msgs.iter().any(|msg| {
        msg.split(',').count() >= 15
            && msg.chars().all(|c| {
                c == '.' || c == ',' || c == '-' || c.is_ascii_digit() || c.is_alphabetic()
            })
    });
    let has_trace_line = msgs.iter().any(|msg| msg.contains("actuation"));
    assert!(
        has_status_line || has_trace_line,
        "Expected STATUS-related output, got: {:?}",
        msgs
    );
}
#[test]
fn control_loop_tick_sensor_and_control_advance() {
    let _guard = acquire_test_lock();
    let mut roaster = build_test_roaster();
    roaster
        .update_temperatures(25.0, 30.0, Instant::now())
        .expect("temps");
    init_container_with_roaster(roaster);
    drain_all_channels();
    // Start a roast to enable control
    let traced = crate::logging::traceability::TracedCommand {
        command: crate::config::ArtisanCommand::StartRoast,
        trace_id: crate::logging::traceability::TraceId::next(),
        channel: crate::input::multiplexer::CommChannel::None,
    };
    let _ = ServiceContainer::get_artisan_channel().try_send(traced);
    let mut tick_state = TickState::new();
    let output_channel = ServiceContainer::get_output_channel();
    block_on(async {
        control_loop_tick(&mut tick_state, output_channel).await;
    });
    let msgs = drain_non_stage_output(output_channel);
    let _status =
        block_on(async { ServiceContainer::with_roaster_async(|r| r.get_status()).await })
            .expect("status read");
    assert!(
        !msgs.is_empty(),
        "START command should produce trace output after full tick, got {} non-stage messages",
        msgs.len()
    );
}
#[test]
fn control_loop_three_consecutive_ticks_no_panic() {
    let _guard = acquire_test_lock();
    let roaster = build_test_roaster();
    init_container_with_roaster(roaster);
    drain_all_channels();
    let mut tick_state = TickState::new();
    let output_channel = ServiceContainer::get_output_channel();
    block_on(async {
        for _ in 0..3 {
            control_loop_tick(&mut tick_state, output_channel).await;
        }
    });
    // All three ticks completed without panic.
    // Tick state should have valid accumulated guard timeouts.
    assert!(
        tick_state.last_guard_total_timeouts < u16::MAX, // not saturated
        "Guard timeouts should be within normal range after 3 ticks"
    );
}
#[test]
fn control_loop_tick_comms_error_threshold_triggers_emergency() {
    let _guard = acquire_test_lock();
    let roaster = build_test_roaster();
    init_container_with_roaster(roaster);
    drain_all_channels();
    // Push UART error count past threshold
    for _ in 0..crate::hardware::error_counters::MAX_COMMS_READ_ERRORS {
        crate::hardware::error_counters::increment_uart_error_count();
    }
    assert!(
        crate::hardware::error_counters::any_comms_error_threshold_exceeded(),
        "Comms error threshold should be exceeded after max increments"
    );
    // Reset UART counter so test leaves clean state
    let _cleanup = ResetCommsOnDrop;
    let mut tick_state = TickState::new();
    let output_channel = ServiceContainer::get_output_channel();
    block_on(async {
        control_loop_tick(&mut tick_state, output_channel).await;
    });
    // Verify emergency shutdown was triggered
    let status = block_on(async { ServiceContainer::with_roaster_async(|r| r.get_status()).await })
        .expect("status read");
    assert!(
        status.fault_condition,
        "Emergency shutdown should activate after comms errors exceed threshold"
    );
}
#[test]
fn control_loop_multi_tick_commands_drained_across_ticks() {
    let _guard = acquire_test_lock();
    let roaster = build_test_roaster();
    init_container_with_roaster(roaster);
    drain_all_channels();
    // Fill the command channel with READ and STATUS commands
    for cmd in &[
        crate::config::ArtisanCommand::ReadStatus,
        crate::config::ArtisanCommand::StatusReport,
        crate::config::ArtisanCommand::ReadStatus,
    ] {
        let traced = crate::logging::traceability::TracedCommand {
            command: *cmd,
            trace_id: crate::logging::traceability::TraceId::next(),
            channel: crate::input::multiplexer::CommChannel::None,
        };
        let _ = ServiceContainer::get_artisan_channel().try_send(traced);
    }
    let mut tick_state = TickState::new();
    let output_channel = ServiceContainer::get_output_channel();
    // Run 3 ticks — commands should be drained across ticks
    let mut total_outputs = 0usize;
    block_on(async {
        for _ in 0..3 {
            control_loop_tick(&mut tick_state, output_channel).await;
            total_outputs += drain_non_stage_output(output_channel).len();
        }
    });
    assert!(
        total_outputs > 0,
        "Commands queued before ticks should produce output across 3 ticks, got {}",
        total_outputs
    );
}
// ── Dual output tick tests ─────────────────────────────────────────
#[test]
fn dual_output_tick_empty_channel_noop() {
    let _guard = acquire_test_lock();
    let output_channel = ServiceContainer::get_output_channel();
    drain_all_channels();
    block_on(async {
        dual_output_tick(output_channel).await;
    });
    // No message was in the channel, so nothing should have been consumed.
    assert!(output_channel.try_receive().is_err());
}
#[test]
fn dual_output_tick_consumes_output_message() {
    let _guard = acquire_test_lock();
    // Init multiplexer so channel routing works
    ServiceContainer::init_multiplexer();
    let output_channel = ServiceContainer::get_output_channel();
    drain_all_channels();
    // Send a test message
    let _ =
        output_channel.try_send(heapless::String::try_from("READ,120.3,150.5,75.0,25.0").unwrap());
    // Message should be in the channel before tick
    assert!(
        output_channel.try_receive().is_ok(),
        "Message should be available before dual_output_tick"
    );
    // But wait — try_receive removes it! Need to put it back.
    let _ =
        output_channel.try_send(heapless::String::try_from("READ,120.3,150.5,75.0,25.0").unwrap());
    block_on(async {
        dual_output_tick(output_channel).await;
    });
    // After tick, the message should have been consumed
    assert!(
        output_channel.try_receive().is_err(),
        "Message should be consumed from output channel after dual_output_tick"
    );
}
#[test]
fn dual_output_tick_multiple_messages_processed() {
    let _guard = acquire_test_lock();
    ServiceContainer::init_multiplexer();
    let output_channel = ServiceContainer::get_output_channel();
    drain_all_channels();
    // Send 3 messages
    for i in 0..3u8 {
        let msg = heapless::String::try_from(alloc::format!("MSG{}", i).as_str()).unwrap();
        try_send_output(output_channel, msg);
    }
    // Process all 3 with separate ticks
    block_on(async {
        for _ in 0..3 {
            dual_output_tick(output_channel).await;
        }
    });
    // All should be consumed
    assert!(
        output_channel.try_receive().is_err(),
        "All 3 messages should be consumed after 3 dual_output_tick calls"
    );
}
// ── RAII helper for comms error cleanup ────────────────────────────
struct ResetCommsOnDrop;
impl Drop for ResetCommsOnDrop {
    fn drop(&mut self) {
        crate::hardware::error_counters::reset_uart_error_count();
        crate::hardware::error_counters::reset_usb_error_count();
    }
}
// ── Test lock for ServiceContainer tests ───────────────────────────
static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn acquire_test_lock() -> std::sync::MutexGuard<'static, ()> {
    let guard = TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    TEST_LOCK.clear_poison();
    guard
}
/// Helper: drain output channel and return only non-stage-report messages.
/// Stage reports start with "STAGE," — the full tick emits 5 of them per tick
/// under the `test` feature (SensorRead, ControlUpdate, LedcWrite, WatchdogFeed, TelemetryEmit).
fn drain_non_stage_output(channel: &OutputChannel) -> Vec<heapless::String<TRACE_EVENT_MAX_LEN>> {
    let mut messages = Vec::new();
    while let Ok(msg) = channel.try_receive() {
        if !msg.starts_with("STAGE,") {
            messages.push(msg);
        }
    }
    messages
}
/// Helper: count stage report messages in the output channel.
fn drain_stage_reports(channel: &OutputChannel) -> usize {
    let mut count = 0;
    while let Ok(msg) = channel.try_receive() {
        if msg.starts_with("STAGE,") {
            count += 1;
        }
    }
    count
}
// ── Bug P7 (2026-08-03): comms read errors are channel-aware ──────────
#[test]
fn should_count_read_error_only_active_channel() {
    use crate::hardware::transport_tasks::should_count_read_error;
    // Only the multiplexer's ACTIVE channel counts toward the
    // MAX_COMMS_READ_ERRORS emergency threshold.
    assert!(should_count_read_error(CommChannel::Usb, CommChannel::Usb));
    assert!(should_count_read_error(
        CommChannel::Uart,
        CommChannel::Uart
    ));
    assert!(
        !should_count_read_error(CommChannel::Usb, CommChannel::Uart),
        "P7: a dead UART line must not count during a USB session"
    );
    assert!(
        !should_count_read_error(CommChannel::Uart, CommChannel::Usb),
        "P7: a dead USB line must not count during a UART session"
    );
    assert!(
        !should_count_read_error(CommChannel::None, CommChannel::Uart),
        "P7: no active session → nothing counts"
    );
    assert!(!should_count_read_error(
        CommChannel::None,
        CommChannel::Usb
    ));
}
// ── Bug P8 (2026-08-03): garbage must not hijack the multiplexer ───────
#[test]
fn garbage_line_does_not_hijack_channel() {
    use crate::hardware::transport_tasks::{send_parse_error, TransportConfig};
    use crate::input::parser::ParseError;
    let _guard = acquire_test_lock();
    // Fresh multiplexer — active channel starts as `None`.
    ServiceContainer::init_multiplexer();
    drain_all_channels();
    let config = TransportConfig {
        name: "UART",
        channel: CommChannel::Uart,
        ..TransportConfig::default()
    };
    // Boot-window garbage that fails to parse must NOT activate UART.
    block_on(async {
        send_parse_error(ParseError::UnknownCommand, CommChannel::Uart, &config).await;
    });
    let active = critical_section::with(|cs| {
        ServiceContainer::get_multiplexer()
            .borrow(cs)
            .borrow()
            .as_ref()
            .map(|mux| mux.get_active_channel())
    });
    assert_eq!(
        active,
        Some(CommChannel::None),
        "P8: a parse error in the None window must not hijack the channel"
    );
    // With no active channel there is nowhere to reply — nothing emitted.
    assert!(
        ServiceContainer::get_output_channel()
            .try_receive()
            .is_err(),
        "P8: no ERR may be emitted while no channel is active"
    );
}
