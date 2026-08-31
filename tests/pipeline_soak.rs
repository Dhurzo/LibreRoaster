//! Bounded pipeline soak (Audit A-TC4, 2026-08-12).
//!
//! The longest deterministic run in the suite was ~5800 ticks at the
//! `RoasterControl` level; nothing stressed the FULL pipeline (transport
//! entry → parser → multiplexer → artisan channel → command processing →
//! formatter → output channel) with a long random command stream. This soak
//! drives ~700 mixed commands — valid Artisan traffic, garbage, latch and
//! recovery cycles — through both transport entry points and asserts:
//!
//! - the artisan channel never drops a command (`ERR channel_full` absent),
//! - every non-TRACE output line is well-formed and known (the wire never
//!   carries a truncated or legacy-format line),
//! - queues drain back to empty at the end.
//!
//! No `update_control` ticks are run, so no time-based safety trap can fire
//! — this soaks the protocol/formatting layer, not the thermal state machine
//! (that layer is covered by `safety_invariant_harness` and
//! `full_roast_verification`).

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
use libreroaster::hardware::usb_cdc::tasks::process_usb_command_data;
use libreroaster::input::ArtisanInput;
use libreroaster::logging::traceability::TRACE_EVENT_MAX_LEN;
use libreroaster::output::artisan::ArtisanFormatter;

static TEST_MUTEX: Mutex<()> = Mutex::new(());

fn acquire_lock() -> std::sync::MutexGuard<'static, ()> {
    let guard = TEST_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    TEST_MUTEX.clear_poison();
    guard
}

/// Register a fresh stub roaster + artisan input and the command multiplexer.
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

/// xorshift64* — deterministic PRNG, no external deps.
struct XorShift64(u64);

impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// The soak command table. Weights: mostly healthy Artisan traffic, some
/// garbage, occasional latch/recovery cycles.
const COMMANDS: &[&str] = &[
    "READ",
    "READ",
    "READ",
    "STATUS",
    "OT1 30",
    "OT1 55",
    "OT1 80",
    "IO3 40",
    "IO3 60",
    "UP",
    "DOWN",
    "PID;SV;200",
    "PID;SV;210",
    "CHAN;1200",
    "UNITS;C",
    "UNITS;F",
    "PID;OFF",
    // BUG-08 (2026-08-21): telemetry is opt-in — keep the stream enabled in
    // the soak so the '#'-line wire validation keeps being exercised.
    "STREAM;ON",
    "STREAM;OFF",
    "STREAM;ON",
    "BOGUS",
    "OT1 150",
    "OT1;0",
];

/// Feed one command line through a transport entry point.
///
/// The output channel is drained after every command (each enqueue emits a
/// TRACE event into the 16-deep shared channel); without the per-feed drain
/// the channel fills with TRACE lines and responses drop through the
/// best-effort `try_send` (production `dual_output_task` drains every 5 ms —
/// mirror it here). Audit A-TC4-D (2026-08-12).
fn feed(command: &str, via_usb: bool, collected: &mut Vec<StdString>) {
    let mut bytes = command.as_bytes().to_vec();
    bytes.push(b'\r');
    if via_usb {
        process_usb_command_data(&bytes);
    } else {
        process_command_data(&bytes);
    }
    drain_output_into(collected);
}

/// Drain the artisan channel and process commands exactly like the
/// production control loop, emitting READ/STATUS responses.
fn process_all_queued(collected: &mut Vec<StdString>) {
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
            // and a heavy round would otherwise silently drop responses
            // through the best-effort `try_send` (production
            // `dual_output_task` drains every 5 ms — mirror it here).
            drain_output_into(collected);
        }
    });
    drain_output_into(collected);
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

/// Every non-TRACE wire line must be well-formed: an ERR line, a '#'
/// handshake/telemetry line, the 5/8-field TC4 READ, or the 20-field STATUS.
/// Anything else — a truncated heapless buffer, or the deprecated 4-field
/// legacy READ — is a protocol regression.
fn assert_line_wellformed(line: &str) {
    if line.starts_with("ERR ") || line.starts_with('#') {
        return;
    }
    let fields: Vec<&str> = line.split(',').collect();
    match fields.len() {
        5 => {
            for field in &fields {
                assert!(
                    field.parse::<f32>().is_ok(),
                    "READ field must be numeric, got '{field}' in '{line}'"
                );
            }
            assert_eq!(fields[3], "0.0", "CHAN3 placeholder in '{line}'");
            assert_eq!(fields[4], "0.0", "CHAN4 placeholder in '{line}'");
        }
        8 => {
            for field in &fields {
                assert!(
                    field.parse::<f32>().is_ok(),
                    "PID READ field must be numeric, got '{field}' in '{line}'"
                );
            }
        }
        20 => {}
        _ => panic!("unexpected wire line (truncated or legacy format?): '{line}'"),
    }
}

#[test]
fn random_command_soak_keeps_queues_bounded_and_wire_wellformed() {
    let _guard = acquire_lock();
    init_service_container();
    drain_channels();

    let mut rng = XorShift64::new(0xA11C_EA0C_4FEED);
    let rounds = 70;
    let commands_per_round = 10;
    let mut total_commands = 0u64;
    let mut total_output = 0u64;

    for _round in 0..rounds {
        let mut outputs = Vec::new();
        for _i in 0..commands_per_round {
            let pick = rng.below(COMMANDS.len() as u64) as usize;
            let via_usb = rng.below(2) == 1;
            feed(COMMANDS[pick], via_usb, &mut outputs);
            total_commands += 1;
        }

        process_all_queued(&mut outputs);
        assert!(
            !outputs.iter().any(|l| l.starts_with("ERR channel_full")),
            "the 16-slot command channel must absorb the soak load, got drops: {:?}",
            outputs
        );
        for line in &outputs {
            assert_line_wellformed(line);
        }
        total_output += outputs.len() as u64;

        // Queue lengths must stay within their fixed capacities at all times.
        assert!(
            ServiceContainer::get_artisan_channel().len() <= 16,
            "artisan channel exceeded capacity"
        );
        assert!(
            ServiceContainer::get_output_channel().len() <= 16,
            "output channel exceeded capacity"
        );
    }

    // Final drain: nothing may remain queued after processing.
    let mut leftovers = Vec::new();
    process_all_queued(&mut leftovers);
    for line in &leftovers {
        assert_line_wellformed(line);
    }
    assert!(
        ServiceContainer::get_artisan_channel()
            .try_receive()
            .is_err(),
        "artisan channel must be empty at soak end"
    );
    assert!(
        ServiceContainer::get_output_channel()
            .try_receive()
            .is_err(),
        "output channel must be empty at soak end"
    );

    // Sanity on the soak itself: it must have actually moved traffic.
    assert!(
        total_commands >= 500,
        "soak must drive at least 500 commands, got {total_commands}"
    );
    assert!(
        total_output > 0,
        "soak must produce wire output, got {total_output} lines"
    );
}
