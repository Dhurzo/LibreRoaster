//! Fault injection MID-ROAST — Fase 2 del plan BUG-CATCH-PLAN.md.
//!
//! These tests inject hardware faults into a live `RoasterControl` session
//! (after the roast started) and assert the safety escalation. They close the
//! coverage gaps found by the audit:
//!   - heater write failure mid-roast   (Bug B / EC-23 — was untested)
//!   - fan write failure mid-roast      (Bug B fan path, roaster_control.rs:898)
//!   - sensor disconnect mid-roast      (debounce → NaN → emergency)
//!   - software watchdog timeout        (watchdog.rs:78-81 — was untested)
//!   - interleaved USB/UART routing     (multiplexer, byte/command level)
//!
//! Run: cargo test --test safety_injection_midroast_tests --features test
//!       --target x86_64-unknown-linux-gnu

#![cfg(all(test, feature = "test", not(target_arch = "riscv32")))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

extern crate alloc;
extern crate std;
// Fase 3: este binario usa `libreroaster::hardware::test_mocks`, que está
// gated a `any(test, feature = "test")` (el Arc compartido de los mocks no
// compila en riscv32 sin target_has_atomic=ptr). El gate de cabecera de
// arriba evita que un `cargo test` plano (sin --features test) intente
// compilar este binario y falle E0432.

use std::boxed::Box;

use embassy_time::Instant;

use libreroaster::common::{StubFan, StubHeater};
use libreroaster::config::constants::{RoasterState, SsrHardwareStatus};
use libreroaster::config::ArtisanCommand;
use libreroaster::control::RoasterControl;
use libreroaster::hardware::sensors::{SensorConversionHub, SensorFault};
use libreroaster::hardware::test_mocks::{MockFan, MockSsr};

fn make_control_with(
    heater: Box<dyn libreroaster::control::traits::Heater + Send>,
    fan: Box<dyn libreroaster::control::traits::Fan + Send>,
) -> RoasterControl {
    RoasterControl::new(heater, fan, SensorConversionHub::new()).expect("test control should build")
}

/// Start a PID roast with stub hardware and run it to Heating state.
fn start_roast(ctrl: &mut RoasterControl) {
    let now = Instant::now();
    ctrl.update_temperatures(25.0, 25.0, now).expect("temps");
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");
    let now = Instant::now();
    ctrl.update_temperatures(180.0, 200.0, now).expect("temps");
    ctrl.update_control(now).expect("first roast tick");
    assert_eq!(ctrl.get_state(), RoasterState::Heating);
}

// ═══════════════════════════════════════════════════════════════════════════
// T1 — Heater write failure mid-roast (Bug B / EC-23 escalation path)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn heater_write_failure_mid_roast_escalates_to_latched_emergency() {
    let mut ssr = MockSsr::new();
    let mut ctrl = make_control_with(Box::new(ssr.clone()), Box::new(StubFan::new()));
    start_roast(&mut ctrl);
    assert!(
        ssr.current_power() > 0.0,
        "the roast must be driving the heater before the fault"
    );

    // Mid-roast: the NEXT 4 heater writes fail — the tick write plus the
    // EMERGENCY_HEATER_OFF_RETRIES (3) retries of force_heater_off.
    ssr.fail_next_writes(4);

    // Tick 200 ms in the future: real-time ticks are µs apart, which would
    // land inside the 100 ms SSR cycle guard and skip the write entirely
    // (guard-busy returns the held output without touching the hardware).
    let now = Instant::now() + embassy_time::Duration::from_millis(200);
    ctrl.update_temperatures(190.0, 210.0, now).expect("temps");
    let result = ctrl.update_control(now);

    assert!(
        result.is_err(),
        "a failed heater write must escalate, not be swallowed (Bug B)"
    );
    assert_eq!(ctrl.get_state(), RoasterState::Error);
    assert!(ctrl.get_status().fault_condition, "latch must be armed");
    // S7 fix (2026-08-05): `ssr_output` is only zeroed when the heater
    // physically acknowledged the off write. Here every write failed (tick +
    // EMERGENCY_HEATER_OFF_RETRIES), so the field must keep the last APPLIED
    // duty instead of claiming a cut that never happened.
    assert!(
        ctrl.get_status().ssr_output > 0.0,
        "S7: ssr_output must reflect the unknown heater state (last applied > 0), \
         not claim an off that never landed — got {}",
        ctrl.get_status().ssr_output
    );
    assert_eq!(ctrl.get_status().fan_output, 100.0, "fan must cool");
    assert_eq!(
        ctrl.get_status().ssr_hardware_status,
        SsrHardwareStatus::Error,
        "total heater-off failure must surface as hardware Error"
    );
    assert!(
        ssr.write_calls() >= 4,
        "1 tick write + EMERGENCY_HEATER_OFF_RETRIES retries expected"
    );
    // S7 evidence: the honest signal for a heater that never acknowledged the
    // off write is `ssr_hardware_status == Error`; `ssr_output` keeps the last
    // applied duty, and the mock's last successful write is non-zero.
    assert!(
        ssr.current_power() > 0.0,
        "S7: mock heater still holds its last successful (non-zero) duty"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// T2 — Fan write failure mid-roast (Bug B fan path)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn fan_write_failure_mid_roast_escalates_to_latched_emergency() {
    let mut fan = MockFan::new();
    let mut ctrl = make_control_with(Box::new(StubHeater::new()), Box::new(fan.clone()));
    start_roast(&mut ctrl);
    assert_eq!(
        fan.current_speed(),
        libreroaster::config::constants::FAN_MIN_SAFETY_PCT,
        "heater on with no OT2 → fan must sit on the safety floor"
    );

    fan.fail_next_speed_writes(1);

    let now = Instant::now();
    ctrl.update_temperatures(190.0, 210.0, now).expect("temps");
    let result = ctrl.update_control(now);

    assert!(
        result.is_err(),
        "a failed fan write with heat on must escalate (Bug B fan path)"
    );
    assert_eq!(ctrl.get_state(), RoasterState::Error);
    assert!(ctrl.get_status().fault_condition);
    assert_eq!(ctrl.get_status().ssr_output, 0.0);
    assert_eq!(
        ctrl.get_status().fan_output,
        100.0,
        "emergency must force the fan to 100 %"
    );
    assert!(
        fan.emergency_calls() >= 1,
        "emergency fan 100 % must have been attempted"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// T3 — Sensor disconnect mid-roast (debounce → NaN → emergency)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn sensor_disconnect_mid_roast_escalates_after_debounce() {
    let mut ctrl = make_control_with(Box::new(StubHeater::new()), Box::new(StubFan::new()));
    start_roast(&mut ctrl);
    assert!(!ctrl.get_status().fault_condition);

    let faulted = SensorFault {
        fault_detected: true,
        ..SensorFault::default()
    };
    let clean = SensorFault::default();

    // First 4 faulted samples: below SENSOR_FAULT_DEBOUNCE — transient glitch
    // tolerance must hold (no emergency, no NaN poisoning).
    for _ in 0..4 {
        let now = Instant::now();
        ctrl.update_temperatures_with_fault(0.0, 0.0, faulted, faulted, now)
            .expect("faulted read is not an error");
        let result = ctrl.update_control(now);
        assert!(
            result.is_ok(),
            "a transient glitch window must NOT latch an emergency (F4.11 debounce)"
        );
        assert!(!ctrl.get_status().fault_condition);
    }

    // 5th consecutive faulted sample: bean_temp is poisoned with NaN (the
    // B-Q hold keeps the timestamp frozen, so the NaN PV trap fires first).
    let now = Instant::now();
    ctrl.update_temperatures_with_fault(0.0, 0.0, faulted, faulted, now)
        .expect("faulted read");
    let result = ctrl.update_control(now);

    assert!(
        result.is_err(),
        "a persistent sensor fault must escalate after the debounce window"
    );
    assert_eq!(ctrl.get_state(), RoasterState::Error);
    assert!(ctrl.get_status().fault_condition);
    assert_eq!(ctrl.get_status().ssr_output, 0.0);
    assert_eq!(ctrl.get_status().fan_output, 100.0);

    // Recovery with a clean sample must NOT un-latch (emergency is sticky).
    let now = Instant::now();
    ctrl.update_temperatures_with_fault(150.0, 150.0, clean, clean, now)
        .expect("clean read");
    assert!(
        ctrl.get_status().fault_condition,
        "latched emergency must persist across a healthy tick"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// T4 — Software watchdog timeout branch (watchdog.rs:78-81, was untested)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn software_watchdog_times_out_after_missed_feeds() {
    use libreroaster::safety::watchdog::{WatchdogError, WatchdogFeeder};
    use std::thread;
    use std::time::Duration as StdDuration;

    let mut feeder = WatchdogFeeder::initialize().expect("watchdog init");
    // Sentinel note (Fase 2 finding): `LAST_FEED_MS == 0` is both the
    // "never fed" sentinel AND a real timestamp — a feed landing in the
    // first millisecond of the time driver's baseline is stored as 0 and
    // makes `is_alive()` report alive forever (the gap check is skipped).
    // On hardware the first feed happens ~300 ms+ after boot, so the window
    // is theoretical there; on host tests the first feed always lands at 0.
    // Prime the timestamp with a feed, wait past the 1 s window, and verify
    // the timeout branch (watchdog.rs:78-81) fires.
    feeder.feed_async(25.0).expect("first feed ok");
    thread::sleep(StdDuration::from_millis(50));
    feeder.feed_async(25.0).expect("priming feed ok");
    assert!(feeder.is_alive());

    // Miss the 1000 ms timeout window by a comfortable margin.
    thread::sleep(StdDuration::from_millis(1100));

    assert!(
        !feeder.is_alive(),
        "watchdog must report dead after the gap"
    );
    let result = feeder.feed_async(25.0);
    assert!(matches!(
        result,
        Err(WatchdogError::FeedFailed("watchdog_timeout"))
    ));
    assert_eq!(feeder.last_failure_reason(), Some("watchdog_timeout"));

    // A prompt feed recovers the software watchdog.
    feeder.feed_async(25.0).expect("recovery feed ok");
    assert!(feeder.is_alive());
    assert!(feeder.last_failure_reason().is_none());
}

// ═══════════════════════════════════════════════════════════════════════════
// T5 — Interleaved USB/UART commands route to the active channel only
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn interleaved_usb_uart_commands_route_to_active_channel_only() {
    use libreroaster::application::service_container::ServiceContainer;
    use libreroaster::hardware::uart::tasks::process_command_data;
    use libreroaster::hardware::usb_cdc::tasks::process_usb_command_data;
    use libreroaster::input::ArtisanInput;

    ServiceContainer::init_roaster(make_control_with(
        Box::new(StubHeater::new()),
        Box::new(StubFan::new()),
    ));
    ServiceContainer::init_artisan_input(ArtisanInput::new().expect("input should build"));
    // init_artisan_input does NOT register the multiplexer — a real boot goes
    // through init_multiplexer(); without it the routing gate is a no-op.
    ServiceContainer::init_multiplexer();

    let artisan_channel = ServiceContainer::get_artisan_channel();
    while artisan_channel.try_receive().is_ok() {}

    // USB activates the session with a fan command.
    process_usb_command_data(b"IO3 45\r");
    // UART commands while USB is active must be refused (multiplexer).
    process_command_data(b"OT2 60\r");
    process_command_data(b"OT1 80\r");
    // USB keeps working.
    process_usb_command_data(b"READ\r");

    let mut cmds = alloc::vec::Vec::new();
    while let Ok(traced) = artisan_channel.try_receive() {
        cmds.push(traced.command);
    }

    assert_eq!(
        cmds,
        alloc::vec![ArtisanCommand::SetFan(45), ArtisanCommand::ReadStatus],
        "only the active transport's commands may execute; UART OT2/OT1 must be dropped"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// T6 — Stuck-on SSR (status Error) must gate manual control off
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn ssr_not_available_gates_manual_and_pid_output_to_zero() {
    let mut ssr = MockSsr::new();
    let mut ctrl = make_control_with(Box::new(ssr.clone()), Box::new(StubFan::new()));

    // A stuck-on/unknown SSR is reported as Error by the hardware monitor.
    ssr.set_status(SsrHardwareStatus::Error);

    // Manual command: applied, but the next tick must zero the output.
    ctrl.process_artisan_command(ArtisanCommand::SetHeater(80))
        .expect("manual heater command");
    let now = Instant::now();
    ctrl.update_temperatures(150.0, 150.0, now).expect("temps");
    ctrl.update_control(now).expect("tick");

    assert_eq!(
        ctrl.get_status().ssr_output,
        0.0,
        "manual control with SSR not Available must output 0 %"
    );

    // PID path: same gate (roaster_control.rs:770-772).
    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");
    let now = Instant::now();
    ctrl.update_temperatures(180.0, 200.0, now).expect("temps");
    ctrl.update_control(now).expect("tick");
    assert_eq!(
        ctrl.get_status().ssr_output,
        0.0,
        "PID control with SSR not Available must output 0 %"
    );
}
