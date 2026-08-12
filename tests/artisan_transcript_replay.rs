//! Artisan golden-transcript replay (Audit A-TC4, 2026-08-12).
//!
//! The wire-format assertions in the rest of the suite were pinned against
//! the firmware's own documentation — a format regression on the READ/STATUS
//! or handshake-ack side would pass every existing test. This file replays
//! byte transcripts of a real Artisan session (connect handshake, software
//! PID slider stream, firmware PID) through the PRODUCTION pipeline
//! (`process_command_data` → parser/multiplexer → artisan channel →
//! `RoasterControl::process_artisan_command` → output channel) and asserts
//! the wire contract Artisan depends on:
//!
//! - handshake acks are '#'-prefixed (`#1200` for CHAN, `#OK` for
//!   UNITS/FILT — Artisan's ArduinoTC4 driver rejects anything else and
//!   re-initialises forever),
//! - READ carries the TC4 5-field format (`AMB,ET,BT,0.0,0.0`) with PID off
//!   and the 8-field PID variant (`...,HEATER,FAN,SV`) with PID on —
//!   never the deprecated 4-field legacy format,
//! - STATUS carries exactly 20 fields,
//! - a clean session produces zero `ERR` lines and zero channel drops.
//!
//! The transcripts model what Artisan actually sends (verified against the
//! Artisan source, `artisanlib/comm.py` + `pid_control.py`): `\n`-terminated
//! lines, `;`-delimited handshake/PID commands, integer slider duties.

#![cfg(all(test, feature = "test", not(target_arch = "riscv32")))]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

extern crate std;

use std::string::String as StdString;
use std::sync::Mutex;
use std::vec::Vec;

use futures::executor::block_on;
use heapless::String;

use libreroaster::application::service_container::ServiceContainer;
use libreroaster::common::{StubFan, StubHeater};
use libreroaster::config::ArtisanCommand;
use libreroaster::control::roaster_control::RoasterControl;
use libreroaster::hardware::sensors::SensorConversionHub;
use libreroaster::hardware::uart::tasks::process_command_data;
use libreroaster::input::ArtisanInput;
use libreroaster::logging::traceability::TRACE_EVENT_MAX_LEN;
use libreroaster::output::artisan::ArtisanFormatter;

/// Serializes tests that share global ServiceContainer state.
static TEST_MUTEX: Mutex<()> = Mutex::new(());

fn acquire_lock() -> std::sync::MutexGuard<'static, ()> {
    let guard = TEST_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    TEST_MUTEX.clear_poison();
    guard
}

fn init_service_container() {
    let roaster = RoasterControl::new(
        Box::new(StubHeater::new()),
        Box::new(StubFan::new()),
        SensorConversionHub::new(),
    )
    .expect("RoasterControl should build");
    ServiceContainer::init_roaster(roaster);
    ServiceContainer::init_artisan_input(ArtisanInput::new().expect("input should build"));
    ServiceContainer::init_multiplexer();
}

fn drain_channels() {
    let cmd = ServiceContainer::get_artisan_channel();
    while cmd.try_receive().is_ok() {}
    let out = ServiceContainer::get_output_channel();
    while out.try_receive().is_ok() {}
}

/// Feed a transcript (one `\n`-terminated command per line) through the
/// UART transport entry point, exactly as a real serial session arrives.
fn feed_transcript(transcript: &str) {
    for line in transcript.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let mut bytes = line.as_bytes().to_vec();
        bytes.push(b'\n');
        process_command_data(&bytes);
    }
}

/// Drain the artisan channel and process every command through the roaster,
/// emitting READ/STATUS responses exactly like the production control loop
/// (tasks.rs `drain_commands`). Returns the non-TRACE output lines.
fn process_all_queued() -> Vec<StdString> {
    let mut collected = Vec::new();
    block_on(async {
        let cmd_channel = ServiceContainer::get_artisan_channel();
        while let Ok(traced) = cmd_channel.try_receive() {
            let _ = ServiceContainer::with_roaster_async(|roaster| {
                let _ = roaster.process_artisan_command(traced.command);
                match traced.command {
                    ArtisanCommand::ReadStatus => {
                        let status = roaster.get_status();
                        let response = ArtisanFormatter::format_read_response_full(&status);
                        if let Ok(line) = String::<TRACE_EVENT_MAX_LEN>::try_from(response.as_str())
                        {
                            let _ = ServiceContainer::get_output_channel().try_send(line);
                        }
                    }
                    ArtisanCommand::StatusReport => {
                        let status = roaster.get_status();
                        let response = ArtisanFormatter::format_status_response(&status);
                        if let Ok(line) = String::<TRACE_EVENT_MAX_LEN>::try_from(response.as_str())
                        {
                            let _ = ServiceContainer::get_output_channel().try_send(line);
                        }
                    }
                    _ => {}
                }
            })
            .await;
            // Drain the output channel while processing: it is only 16 deep
            // and a long transcript would otherwise silently drop responses
            // through the best-effort `try_send` (production
            // `dual_output_task` drains every 5 ms — mirror it here).
            drain_output_into(&mut collected);
        }
    });
    drain_output_into(&mut collected);
    collected
}

fn drain_output_into(collected: &mut Vec<StdString>) {
    let channel = ServiceContainer::get_output_channel();
    while let Ok(msg) = channel.try_receive() {
        if msg.as_str().starts_with("TRACE,") {
            continue;
        }
        collected.push(StdString::from(msg.as_str()));
    }
}

#[derive(Debug, PartialEq, Eq)]
enum WireLine {
    /// `ERR ...` — error line (any).
    Err,
    /// `#...` — handshake ack or spontaneous telemetry.
    Hash,
    /// READ with PID off: `AMB,ET,BT,0.0,0.0` (5 numeric fields).
    Read5,
    /// READ with PID on: `AMB,ET,BT,0.0,0.0,HEATER,FAN,SV` (8 numeric fields).
    Read8,
    /// STATUS deep telemetry (exactly 20 fields, mixed types).
    Status20,
    /// Anything else — including the deprecated 4-field READ, which must
    /// NEVER appear on the wire.
    Unexpected,
}

fn classify(line: &str) -> WireLine {
    if line.starts_with("ERR ") {
        return WireLine::Err;
    }
    if line.starts_with('#') {
        return WireLine::Hash;
    }
    match line.split(',').count() {
        5 => WireLine::Read5,
        8 => WireLine::Read8,
        20 => WireLine::Status20,
        _ => WireLine::Unexpected,
    }
}

/// Assert every line on the wire is well-formed and known, returning the
/// classified lines.
fn assert_all_lines_wellformed(outputs: &[StdString]) -> Vec<WireLine> {
    let mut classes = Vec::new();
    for line in outputs {
        match classify(line) {
            WireLine::Unexpected => panic!(
                "unexpected wire line (legacy 4-field READ?): '{line}'. \
                 The production pipeline must emit the TC4 5/8-field READ."
            ),
            WireLine::Read5 => {
                let fields: Vec<&str> = line.split(',').collect();
                for field in &fields {
                    assert!(
                        field.parse::<f32>().is_ok(),
                        "READ field must be numeric, got '{field}' in '{line}'"
                    );
                }
                assert_eq!(
                    fields[3], "0.0",
                    "CHAN3 placeholder must be 0.0, got '{}' in '{line}'",
                    fields[3]
                );
                assert_eq!(
                    fields[4], "0.0",
                    "CHAN4 placeholder must be 0.0, got '{}' in '{line}'",
                    fields[4]
                );
                classes.push(WireLine::Read5);
            }
            WireLine::Read8 => {
                let fields: Vec<&str> = line.split(',').collect();
                for field in &fields {
                    assert!(
                        field.parse::<f32>().is_ok(),
                        "PID READ field must be numeric, got '{field}' in '{line}'"
                    );
                }
                classes.push(WireLine::Read8);
            }
            other => classes.push(other),
        }
    }
    classes
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. Connect handshake + READ polling (Artisan session open)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn connect_handshake_transcript_replays_cleanly() {
    let _guard = acquire_lock();
    init_service_container();
    drain_channels();

    feed_transcript(include_str!(
        "fixtures/artisan_transcripts/connect_handshake_ascii.txt"
    ));
    let outputs = process_all_queued();

    assert!(
        !outputs.iter().any(|l| l.starts_with("ERR ")),
        "a clean handshake must produce no ERR lines, got: {:?}",
        outputs
    );

    // Artisan's ArduinoTC4 handshake only accepts empty or '#'-prefixed
    // responses ("Arduino could not set channels/units/filters" otherwise).
    assert!(
        outputs.iter().any(|l| l.as_str() == "#1200"),
        "CHAN ack '#1200' expected, got: {:?}",
        outputs
    );
    assert!(
        outputs.iter().filter(|l| l.as_str() == "#OK").count() >= 2,
        "UNITS and FILT acks '#OK' expected, got: {:?}",
        outputs
    );

    let classes = assert_all_lines_wellformed(&outputs);
    assert!(
        classes.contains(&WireLine::Read5),
        "READ during the handshake session must be the 5-field TC4 format"
    );
    // No drops: the channel absorbs Artisan's session-open burst.
    assert!(
        !outputs.iter().any(|l| l.starts_with("ERR channel_full")),
        "no command may be dropped during session open"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Software-PID slider session (modern Artisan default path)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn software_pid_slider_session_replays_cleanly() {
    let _guard = acquire_lock();
    init_service_container();
    drain_channels();

    feed_transcript(include_str!(
        "fixtures/artisan_transcripts/software_pid_session_ascii.txt"
    ));
    let outputs = process_all_queued();

    assert!(
        !outputs.iter().any(|l| l.starts_with("ERR ")),
        "a clean slider session must produce no ERR lines, got: {:?}",
        outputs
    );
    let classes = assert_all_lines_wellformed(&outputs);
    // Artisan's software PID drives OT1 without ever sending START/PID;ON —
    // so the wire stays in the 5-field format for the whole session.
    assert!(
        classes.iter().all(|c| *c != WireLine::Read8),
        "software-PID mode must never flip READ to the 8-field PID format"
    );
    assert!(
        classes.contains(&WireLine::Read5),
        "READ must be the 5-field TC4 format"
    );

    // Roast end = `OT1;0` (what Artisan sends): heater must land at 0.
    let heater = block_on(async {
        ServiceContainer::with_roaster_async(|roaster| roaster.get_status().ssr_output).await
    })
    .expect("read status");
    assert_eq!(heater, 0.0, "OT1;0 at roast end must cut the heater");
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Firmware-PID session (legacy TC4 PID path)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn firmware_pid_session_replays_cleanly() {
    let _guard = acquire_lock();
    init_service_container();
    drain_channels();

    feed_transcript(include_str!(
        "fixtures/artisan_transcripts/firmware_pid_session_ascii.txt"
    ));
    let outputs = process_all_queued();

    assert!(
        !outputs.iter().any(|l| l.starts_with("ERR ")),
        "a clean firmware-PID session must produce no ERR lines, got: {:?}",
        outputs
    );
    let classes = assert_all_lines_wellformed(&outputs);

    // With PID on, READ must carry the 8-field variant Artisan's
    // `rsplit(',')` parses (HEATER, FAN, SV at fields 5..7).
    assert!(
        classes.contains(&WireLine::Read8),
        "READ with PID on must be the 8-field TC4 PID format, got: {:?}",
        outputs
    );
    // After `PID;OFF` the final READ drops back to 5 fields.
    assert!(
        classes.contains(&WireLine::Read5),
        "READ after PID;OFF must return to the 5-field format"
    );
    // STATUS must always be the 20-field deep telemetry line.
    assert!(
        classes.contains(&WireLine::Status20),
        "STATUS must carry exactly 20 fields, got: {:?}",
        outputs
    );

    // `PID;OFF` is Artisan's roast-end command for the firmware-PID mode:
    // streaming stops, heater is cut, and no safety latch is armed.
    let status = block_on(async {
        ServiceContainer::with_roaster_async(|roaster| roaster.get_status()).await
    })
    .expect("read status");
    assert_eq!(status.ssr_output, 0.0, "PID;OFF must cut the heater");
    assert!(
        !status.fault_condition,
        "PID;OFF (roast end) must NOT arm the safety latch"
    );
}
