//! Core-audit regression tests (method BUG_HUNT_METHOD_2026-10-09, run 2026-10-09
//! on branch `refactor/control-core`).
//!
//! - Q1: a PROFILE/RORPROFILE dropped by a FULL command channel on the legacy
//!   UART/USB ingress paths must drain its staged payload (BUG-2c-1 discipline
//!   already holds on the event-queue path and the latch-reject path).
//! - G: CHARGE-while-latched anchors the recovered roast (N2 through an
//!   EmergencyStop; guard test, green throughout).

#![cfg(all(test, feature = "test", not(target_arch = "riscv32")))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

extern crate std;

use std::sync::Mutex;

use embassy_time::{Duration, Instant};
use libreroaster::application::service_container::{ServiceContainer, ARTISAN_CMD_CHANNEL_SIZE};
use libreroaster::common::{StubFan, StubHeater};
use libreroaster::config::ArtisanCommand;
use libreroaster::control::RoasterControl;
use libreroaster::hardware::sensors::SensorConversionHub;
use libreroaster::input::multiplexer::CommChannel;
use libreroaster::logging::traceability::{TraceId, TracedCommand};

static LOCK: Mutex<()> = Mutex::new(());

fn reset_all() {
    ServiceContainer::init_multiplexer();
    while ServiceContainer::get_artisan_channel()
        .try_receive()
        .is_ok()
    {}
    while ServiceContainer::get_output_channel().try_receive().is_ok() {}
    // Never leak a staged payload into the next test.
    let _ = libreroaster::input::parser::take_profile();
    let _ = libreroaster::input::parser::ror_profile_take();
    let _ = libreroaster::input::parser::fan_profile_take();
}

/// Fill the shared artisan channel to capacity so the next ingress send fails.
fn fill_artisan_channel() {
    let ch = ServiceContainer::get_artisan_channel();
    for _ in 0..ARTISAN_CMD_CHANNEL_SIZE {
        let traced = TracedCommand {
            command: ArtisanCommand::ReadStatus,
            trace_id: TraceId::next(),
            channel: CommChannel::None,
        };
        assert!(ch.try_send(traced).is_ok(), "channel should fill up");
    }
    assert!(
        ch.try_send(TracedCommand {
            command: ArtisanCommand::ReadStatus,
            trace_id: TraceId::next(),
            channel: CommChannel::None,
        })
        .is_err(),
        "channel must be full for the drop probe"
    );
}

#[test]
fn q1_uart_legacy_full_channel_drains_staged_profile() {
    let _g = LOCK.lock().unwrap_or_else(|p| p.into_inner());
    reset_all();
    fill_artisan_channel();
    libreroaster::hardware::uart::tasks::process_command_data(b"PROFILE;0,180;120,200\r");
    let staged = libreroaster::input::parser::take_profile();
    reset_all();
    assert!(
        staged.is_none(),
        "Q1: PROFILE dropped on a full channel must not stay staged (UART legacy path)"
    );
}

#[test]
fn q1_usb_legacy_full_channel_drains_staged_ror_profile() {
    let _g = LOCK.lock().unwrap_or_else(|p| p.into_inner());
    reset_all();
    fill_artisan_channel();
    libreroaster::hardware::usb_cdc::tasks::process_usb_command_data(b"RORPROFILE;0,15\r");
    let staged = libreroaster::input::parser::ror_profile_take();
    reset_all();
    assert!(
        staged.is_none(),
        "Q1: RORPROFILE dropped on a full channel must not stay staged (USB legacy path)"
    );
}

/// N2 through an EmergencyStop: a CHARGE marker sent while latched is kept,
/// and the START recovery anchors the roast to it. Guard test: must stay green.
#[test]
fn guard_charge_while_latched_anchors_recovered_roast() {
    let _g = LOCK.lock().unwrap_or_else(|p| p.into_inner());
    let mut c = RoasterControl::new(
        Box::new(StubHeater::new()),
        Box::new(StubFan::new()),
        SensorConversionHub::new(),
    )
    .expect("build");
    let t0 = Instant::now();
    let at = |ms: u64| t0 + Duration::from_millis(ms);
    // Latch the device, then mark the charge while latched.
    assert!(c
        .process_artisan_command_at(ArtisanCommand::EmergencyStop, at(0))
        .is_ok());
    assert!(c
        .process_artisan_command_at(ArtisanCommand::Charge(None), at(100))
        .is_ok());
    // START recovers and starts the roast; the kept marker must anchor it.
    assert!(c
        .process_artisan_command_at(ArtisanCommand::StartRoast, at(200))
        .is_ok());
    assert!(c.update_temperatures(95.0, 115.0, at(300)).is_ok());
    assert!(c
        .update_control(at(300) + Duration::from_millis(215))
        .is_ok());
    assert!(
        c.get_status().charge_detected,
        "G: CHARGE-while-latched must anchor the recovered roast (N2)"
    );
}
