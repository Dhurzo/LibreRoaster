#![allow(clippy::unwrap_used)]
use super::*;
use crate::common::{StubFan, StubHeater};
use crate::config::{ArtisanCommand, RoasterCommand, RoasterState, SsrHardwareStatus};
use crate::control::traits::Fan;
use crate::hardware::sensors::SensorConversionHub;
use alloc::boxed::Box;
use alloc::sync::Arc;
use core::cell::RefCell;
use critical_section::Mutex;
use embassy_time::Instant;

fn make_control() -> RoasterControl {
    let heater = Box::new(StubHeater::new());
    let fan = Box::new(StubFan::new());
    RoasterControl::new(heater, fan, SensorConversionHub::new()).expect("test control should build")
}

fn make_control_with_stubs(heater: StubHeater, fan: StubFan) -> RoasterControl {
    RoasterControl::new(Box::new(heater), Box::new(fan), SensorConversionHub::new())
        .expect("test control should build")
}

// ── Construction and static methods ──────────

#[test]
fn construction_defaults_to_idle() {
    let ctrl = make_control();
    assert_eq!(ctrl.get_state(), RoasterState::Idle);
    assert_eq!(ctrl.get_status().state, RoasterState::Idle);
    assert!(!ctrl.get_status().fault_condition);
    assert_eq!(ctrl.get_fan_speed(), 0.0);
}

#[test]
fn is_temperature_valid_accepts_normal() {
    assert!(RoasterControl::is_temperature_valid(25.0));
    assert!(RoasterControl::is_temperature_valid(0.0));
    assert!(RoasterControl::is_temperature_valid(200.0));
}

#[test]
fn is_temperature_valid_rejects_nan() {
    assert!(!RoasterControl::is_temperature_valid(f32::NAN));
}

#[test]
fn is_temperature_valid_rejects_extreme() {
    assert!(!RoasterControl::is_temperature_valid(9999.0));
    assert!(!RoasterControl::is_temperature_valid(-9999.0));
}

// ── Getters ─────────────────────────────────

#[test]
fn get_fan_speed_returns_status_value() {
    let ctrl = make_control();
    assert_eq!(ctrl.get_fan_speed(), 0.0);
}

#[test]
fn status_mut_allows_modification() {
    let mut ctrl = make_control();
    let status = ctrl.status_mut();
    status.bean_temp = 150.0;
    status.env_temp = 200.0;
    assert_eq!(ctrl.get_status().bean_temp, 150.0);
    assert_eq!(ctrl.get_status().env_temp, 200.0);
}

// ── Emergency shutdown ──────────────────────

#[test]
fn emergency_shutdown_changes_state_and_returns_error() {
    let mut ctrl = make_control();
    let result = ctrl.emergency_shutdown("test shutdown");
    assert!(matches!(
        result,
        Err(RoasterError::EmergencyShutdown { .. })
    ));
    assert_eq!(ctrl.get_state(), RoasterState::Error);
    assert!(ctrl.get_status().fault_condition);
}

#[test]
fn emergency_shutdown_fan_goes_to_100() {
    let heater = StubHeater::new();
    let fan = StubFan::new();
    let mut ctrl = make_control_with_stubs(heater, fan);
    let _ = ctrl.emergency_shutdown("test");
    assert_eq!(ctrl.get_status().fan_output, 100.0);
}

// ── Overtemp regression ─────────────────────

#[test]
fn mark_overtemp_regression_active_sets_flag() {
    let mut ctrl = make_control();
    ctrl.mark_overtemp_regression_active(true);
    assert!(ctrl.get_status().overtemp_regression_active);
}

#[test]
fn mark_overtemp_regression_active_clears_flag() {
    let mut ctrl = make_control();
    ctrl.mark_overtemp_regression_active(true);
    ctrl.mark_overtemp_regression_active(false);
    assert!(!ctrl.get_status().overtemp_regression_active);
}

// ── Update temperatures ─────────────────────

#[test]
fn update_temperatures_normal() {
    let mut ctrl = make_control();
    let now = Instant::from_millis(1000);
    let result = ctrl.update_temperatures(150.0, 120.0, now);
    assert!(result.is_ok());
    assert_eq!(ctrl.get_status().bean_temp, 150.0);
    assert_eq!(ctrl.get_status().env_temp, 120.0);
}

#[test]
fn update_temperatures_overtemp_triggers_emergency() {
    let mut ctrl = make_control();
    let now = Instant::from_millis(1000);
    // OVERTEMP_THRESHOLD is 260°C, MAX_VALID_TEMP is 350°C
    let result = ctrl.update_temperatures(300.0, 25.0, now);
    assert!(result.is_err());
    assert_eq!(ctrl.get_state(), RoasterState::Error);
    assert!(ctrl.get_status().fault_condition);
}

// ── Process command ─────────────────────────

#[test]
fn process_stop_roast_returns_to_idle() {
    let mut ctrl = make_control();
    let now = Instant::from_millis(1000);
    let result = ctrl.process_command(RoasterCommand::StopRoast, now);
    assert!(result.is_ok());
    assert_eq!(ctrl.get_state(), RoasterState::Idle);
}

#[test]
fn process_stop_roast_clears_fault() {
    let heater = StubHeater::new();
    let fan = StubFan::new();
    let mut ctrl = make_control_with_stubs(heater, fan);
    let _ = ctrl.emergency_shutdown("fault");
    assert!(ctrl.get_status().fault_condition);
    let now = Instant::from_millis(2000);
    let result = ctrl.process_command(RoasterCommand::StopRoast, now);
    assert!(result.is_ok());
    assert!(!ctrl.get_status().fault_condition);
}

#[test]
fn process_emergency_stop_triggers_safety() {
    let heater = StubHeater::new();
    let fan = StubFan::new();
    let mut ctrl = make_control_with_stubs(heater, fan);
    let now = Instant::from_millis(1000);
    let result = ctrl.process_command(RoasterCommand::EmergencyStop, now);
    assert!(matches!(
        result,
        Err(RoasterError::TemperatureOutOfRange { .. })
    ));
    assert!(ctrl.get_status().fault_condition);
}

#[test]
fn process_set_heater_manual_triggers_manual_policy() {
    let heater = StubHeater::new();
    let fan = StubFan::new();
    let mut ctrl = make_control_with_stubs(heater, fan);
    let now = Instant::from_millis(1000);
    let result = ctrl.process_command(RoasterCommand::SetHeaterManual(50), now);
    assert!(result.is_ok());
}

#[test]
fn process_set_fan_manual_triggers_manual_policy() {
    let heater = StubHeater::new();
    let fan = StubFan::new();
    let mut ctrl = make_control_with_stubs(heater, fan);
    let now = Instant::from_millis(1000);
    let result = ctrl.process_command(RoasterCommand::SetFanManual(75), now);
    assert!(result.is_ok());
}

// ── Process Artisan command ─────────────────

#[test]
fn artisan_stop_returns_ok() {
    let mut ctrl = make_control();
    let result = ctrl.process_artisan_command(ArtisanCommand::Stop);
    assert!(result.is_ok());
}

#[test]
fn artisan_emergency_stop_triggers_emergency() {
    // Audit MT-7 (2026-08-11): the old body asserted only `is_ok()` — the
    // name promises the emergency *latch*, so pin it: Error state,
    // fault_condition, and the safety-latch flag must all be set after
    // `EmergencyStop` (mirrors stop_latches_then_off_recovers below).
    let mut ctrl = make_control();
    let result = ctrl.process_artisan_command(ArtisanCommand::EmergencyStop);
    assert!(result.is_ok());
    assert_eq!(ctrl.get_state(), RoasterState::Error);
    assert!(ctrl.get_status().fault_condition);
    assert!(
        ctrl.safety().is_emergency_active(),
        "EmergencyStop must arm the safety latch"
    );
}

#[test]
fn artisan_set_pid_gain_updates_gains() {
    let mut ctrl = make_control();
    let result = ctrl.process_artisan_command(ArtisanCommand::SetPidGain(1.0, 0.1, 0.05));
    assert!(result.is_ok());
}

#[test]
fn artisan_set_target_temp_valid() {
    let mut ctrl = make_control();
    let result = ctrl.process_artisan_command(ArtisanCommand::SetTargetTemp(200.0));
    assert!(result.is_ok());
    assert_eq!(ctrl.get_status().target_temp, 200.0);
}

#[test]
fn artisan_set_target_temp_out_of_range_rejected() {
    let mut ctrl = make_control();
    let result = ctrl.process_artisan_command(ArtisanCommand::SetTargetTemp(999.0));
    assert!(matches!(
        result,
        Err(RoasterError::InvalidState {
            source: Some("target_temp_out_of_range")
        })
    ));
}

#[test]
fn artisan_start_roast_starts_streaming() {
    let mut ctrl = make_control();
    let result = ctrl.process_artisan_command(ArtisanCommand::StartRoast);
    assert!(result.is_ok());
}

#[test]
fn artisan_set_pid_channel_switches_channel() {
    let mut ctrl = make_control();
    let result = ctrl.process_artisan_command(ArtisanCommand::SetPidChannel(1));
    assert!(result.is_ok());
    assert_eq!(ctrl.get_status().pid_channel, 1);
}

#[test]
fn artisan_set_pid_cycle_time_updates() {
    let mut ctrl = make_control();
    let result = ctrl.process_artisan_command(ArtisanCommand::SetPidCycleTime(500));
    assert!(result.is_ok());
    assert_eq!(ctrl.get_status().pid_cycle_time_ms, 500);
}

#[test]
fn artisan_set_pid_output_limits_updates() {
    let mut ctrl = make_control();
    let result = ctrl.process_artisan_command(ArtisanCommand::SetPidOutputLimits(10.0, 90.0));
    assert!(result.is_ok());
    assert_eq!(ctrl.get_status().pid_output_min, 10.0);
    assert_eq!(ctrl.get_status().pid_output_max, 90.0);
}

#[test]
fn artisan_chan_returns_ok() {
    let mut ctrl = make_control();
    let result = ctrl.process_artisan_command(ArtisanCommand::Chan(4));
    assert!(result.is_ok());
}

#[test]
fn artisan_units_returns_ok() {
    let mut ctrl = make_control();
    let result = ctrl.process_artisan_command(ArtisanCommand::Units(true));
    assert!(result.is_ok());
}

#[test]
fn artisan_filt_returns_ok() {
    let mut ctrl = make_control();
    let result = ctrl.process_artisan_command(ArtisanCommand::Filt(5));
    assert!(result.is_ok());
}

#[test]
fn artisan_status_report_returns_ok() {
    let mut ctrl = make_control();
    let result = ctrl.process_artisan_command(ArtisanCommand::StatusReport);
    assert!(result.is_ok());
}

#[test]
fn artisan_preheat_sets_target() {
    let mut ctrl = make_control();
    let result = ctrl.process_artisan_command(ArtisanCommand::Preheat(180.0));
    assert!(result.is_ok());
}

#[test]
fn artisan_set_profile_with_no_data_returns_ok() {
    let mut ctrl = make_control();
    let result = ctrl.process_artisan_command(ArtisanCommand::SetProfile);
    assert!(result.is_ok());
}

#[test]
fn artisan_set_fan_profile_with_no_data_returns_ok() {
    let mut ctrl = make_control();
    let result = ctrl.process_artisan_command(ArtisanCommand::SetFanProfile);
    assert!(result.is_ok());
}

#[test]
fn artisan_run_regression_returns_ok() {
    let mut ctrl = make_control();
    let result = ctrl.process_artisan_command(ArtisanCommand::RunRegression);
    assert!(result.is_ok());
}

#[test]
fn accessor_methods_return_references() {
    // Audit MT-7 (2026-08-11): the old body only bound the accessors to
    // `_`-prefixed locals (zero assertions). Prove the accessors return
    // LIVE references: mutate through the `_mut` pair and verify the
    // change is visible through the read accessor.
    let mut ctrl = make_control();

    // safety: activate via safety_mut, observe via safety.
    assert!(!ctrl.safety().is_emergency_active(), "starts clean");
    ctrl.safety_mut().activate_emergency();
    assert!(
        ctrl.safety().is_emergency_active(),
        "safety_mut() mutation must be visible through safety()"
    );
    ctrl.safety_mut().clear_emergency();
    assert!(!ctrl.safety().is_emergency_active(), "cleared");

    // sensor: a temperature update must be visible through sensor().
    let now = Instant::from_millis(1000);
    ctrl.update_temperatures(150.0, 120.0, now).expect("temps");
    assert_eq!(
        ctrl.sensor().last_temp_read(),
        Some(now),
        "sensor() must return the live sensor controller state"
    );

    // actuator: the default StubHeater reports Available.
    assert_eq!(
        ctrl.actuator().get_ssr_hardware_status(),
        crate::config::constants::SsrHardwareStatus::Available,
        "actuator() must return the live actuator controller state"
    );

    // dispatch: default state is not streaming.
    assert!(
        !ctrl.dispatch().is_streaming(&ctrl.get_status()),
        "dispatch() must reflect the live dispatcher state"
    );
}

// ── Read status (READ command) ──────────────

#[test]
fn artisan_read_status_returns_ok() {
    let mut ctrl = make_control();
    let result = ctrl.process_artisan_command(ArtisanCommand::ReadStatus);
    assert!(result.is_ok());
}

// ── V2-1: STOP bricks the roaster — OFF must recover ───────

#[test]
fn stop_latches_then_off_recovers() {
    // Bug V2-1: `STOP` (→ EmergencyStop) arms the latch and leaves the
    // device bricked (the only sanctioned recovery, `RoasterCommand::
    // StopRoast`, has no protocol producer). `OFF` (which parses to
    // `ArtisanCommand::Stop`, token "OFF"/"PID,OFF") must un-latch and
    // return the roaster to a controllable state.
    let mut ctrl = make_control();

    // Arm the latch the way `STOP` does.
    let r = ctrl.process_artisan_command(ArtisanCommand::EmergencyStop);
    assert!(r.is_ok(), "STOP path must succeed at arming the latch");
    assert!(ctrl.safety().is_emergency_active());
    assert!(ctrl.get_status().fault_condition);
    assert_eq!(ctrl.get_state(), RoasterState::Error);

    // Any non-whitelisted command must still be rejected while latched.
    let blocked = ctrl.process_artisan_command(ArtisanCommand::SetHeater(50));
    assert!(blocked.is_err(), "Latch must reject heater commands");

    // `OFF` must clear the latch and recover.
    let recover = ctrl.process_artisan_command(ArtisanCommand::Stop);
    assert!(recover.is_ok(), "OFF recovery path must succeed");
    assert!(
        !ctrl.safety().is_emergency_active(),
        "OFF must clear the emergency latch"
    );
    assert!(
        !ctrl.get_status().fault_condition,
        "OFF must clear fault_condition"
    );
    assert_eq!(
        ctrl.get_state(),
        RoasterState::Idle,
        "OFF recovery returns the roaster to Idle"
    );

    // After recovery a heater command must work again — i.e. the device
    // is no longer bricked.
    let after = ctrl.process_artisan_command(ArtisanCommand::SetHeater(50));
    assert!(
        after.is_ok(),
        "Post-recovery heater command must be accepted: {:?}",
        after
    );
}

#[test]
fn stop_streaming_does_not_clear_state_while_latched() {
    // Bug V2-1 (B34 consistency): while the emergency latch is armed,
    // `stop_streaming` must NOT repaint the state to `Idle`. The
    // `EmergencyStop` handler reaches `dispatch.stop_streaming` without
    // clearing the latch; the device must remain visibly `Error` so
    // telemetry does not claim "Idle" with the fan pinned and commands
    // rejected.
    let mut ctrl = make_control();
    let _ = ctrl.emergency_shutdown("test latch");
    assert_eq!(ctrl.get_state(), RoasterState::Error);

    // Driving the plain `EmergencyStop` artisan command again calls
    // `handle_emergency_stop`, which re-arms and re-stops without ever
    // clearing the latch — the state must stay `Error`.
    let r = ctrl.process_artisan_command(ArtisanCommand::EmergencyStop);
    assert!(r.is_ok());
    assert!(ctrl.safety().is_emergency_active());
    assert_eq!(
        ctrl.get_state(),
        RoasterState::Error,
        "Latched stop must keep state = Error, not Idle"
    );
    assert_eq!(ctrl.get_status().state, RoasterState::Error);
}

// ── BUG-02: SSR availability latch must be re-armed by operator recovery ─

#[test]
fn off_rearms_ssr_hardware_status_without_fault() {
    // BUG-02 (2026-08-21): heat detection latches `NotDetected`/`Error`
    // WITHOUT arming the emergency latch (periodic_health_check swallows
    // the error). A plain `OFF` then bypasses `clear_emergency_explicit`
    // (no fault) — `handle_stop` must still re-arm the heater, or the
    // build without the GPIO1 current-sense circuit dead-locks the
    // heater until a power cycle.
    let heater = StubHeater::new();
    heater.set_status(SsrHardwareStatus::NotDetected);
    let mut ctrl = make_control_with_stubs(heater, StubFan::new());
    assert_eq!(
        ctrl.get_status().ssr_hardware_status,
        SsrHardwareStatus::NotDetected
    );

    let r = ctrl.process_artisan_command(ArtisanCommand::Stop);
    assert!(r.is_ok(), "OFF must succeed: {:?}", r);
    assert_eq!(
        ctrl.get_status().ssr_hardware_status,
        SsrHardwareStatus::Available,
        "operator OFF must re-arm the SSR availability"
    );
}

#[test]
fn emergency_stop_does_not_rearm_ssr_hardware_status() {
    // The internal/emergency path must NOT re-arm: `EmergencyStop` is the
    // action that latches, not the one that recovers.
    let heater = StubHeater::new();
    heater.set_status(SsrHardwareStatus::Error);
    let mut ctrl = make_control_with_stubs(heater, StubFan::new());

    let r = ctrl.process_artisan_command(ArtisanCommand::EmergencyStop);
    assert!(r.is_ok());

    // The stub was moved into the control; verify via the published
    // status. `handle_emergency_stop` refreshes it from the driver
    // (`force_heater_off`), so it must still read the driver value
    // `NotDetected` — and crucially NOT `Available` (no re-arm).
    assert_eq!(
        ctrl.get_status().ssr_hardware_status,
        SsrHardwareStatus::NotDetected,
        "EmergencyStop must not re-arm the SSR availability"
    );
}

#[test]
fn off_with_latched_emergency_rearms_ssr_hardware_status() {
    // Latched path: OFF runs clear_emergency_explicit + handle_stop, both
    // of which re-arm (idempotent). The published status must recover.
    let heater = StubHeater::new();
    heater.set_status(SsrHardwareStatus::Error);
    let mut ctrl = make_control_with_stubs(heater, StubFan::new());

    let _ = ctrl.process_artisan_command(ArtisanCommand::EmergencyStop);
    assert!(ctrl.safety().is_emergency_active());

    let r = ctrl.process_artisan_command(ArtisanCommand::Stop);
    assert!(r.is_ok(), "OFF recovery must succeed: {:?}", r);
    assert_eq!(
        ctrl.get_status().ssr_hardware_status,
        SsrHardwareStatus::Available,
        "latched OFF recovery must re-arm the SSR availability"
    );
}

#[test]
fn start_with_latch_rearms_ssr_hardware_status() {
    // START is a documented re-energizing recovery — it must also re-arm
    // the SSR availability state machine.
    let heater = StubHeater::new();
    heater.set_status(SsrHardwareStatus::NotDetected);
    let mut ctrl = make_control_with_stubs(heater, StubFan::new());

    let _ = ctrl.process_artisan_command(ArtisanCommand::EmergencyStop);
    assert!(ctrl.safety().is_emergency_active());

    let r = ctrl.process_artisan_command(ArtisanCommand::StartRoast);
    assert!(r.is_ok(), "START recovery must succeed: {:?}", r);
    assert_eq!(
        ctrl.get_status().ssr_hardware_status,
        SsrHardwareStatus::Available,
        "START recovery must re-arm the SSR availability"
    );
}

#[test]
fn heater_command_works_after_rearm() {
    // End-to-end BUG-02: after the re-arm, a heater command is actually
    // applied (no more forced 0 % output).
    let heater = StubHeater::new();
    heater.set_status(SsrHardwareStatus::NotDetected);
    let mut ctrl = make_control_with_stubs(heater, StubFan::new());

    let r = ctrl.process_artisan_command(ArtisanCommand::Stop);
    assert!(r.is_ok());

    let heater_after = ctrl.process_artisan_command(ArtisanCommand::SetHeater(50));
    assert!(
        heater_after.is_ok(),
        "heater command must be accepted after re-arm: {:?}",
        heater_after
    );
}

// ── BUG-08: telemetry stream is opt-in (STREAM;ON/OFF) ──────────

#[test]
fn stream_command_toggles_telemetry() {
    let mut ctrl = make_control();
    assert!(!ctrl.get_output_manager().is_continuous_enabled());

    ctrl.process_artisan_command(ArtisanCommand::SetStreaming(true))
        .expect("STREAM;ON succeeds");
    assert!(ctrl.get_output_manager().is_continuous_enabled());

    ctrl.process_artisan_command(ArtisanCommand::SetStreaming(false))
        .expect("STREAM;OFF succeeds");
    assert!(!ctrl.get_output_manager().is_continuous_enabled());
}

#[test]
fn control_commands_do_not_auto_enable_telemetry() {
    let mut ctrl = make_control();

    ctrl.process_artisan_command(ArtisanCommand::SetHeater(50))
        .expect("OT1 succeeds");
    assert!(!ctrl.get_output_manager().is_continuous_enabled());

    ctrl.process_artisan_command(ArtisanCommand::SetFan(60))
        .expect("OT2 succeeds");
    assert!(!ctrl.get_output_manager().is_continuous_enabled());

    ctrl.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("START succeeds");
    assert!(!ctrl.get_output_manager().is_continuous_enabled());

    ctrl.process_artisan_command(ArtisanCommand::SetTargetTemp(200.0))
        .expect("PID;SV succeeds");
    assert!(!ctrl.get_output_manager().is_continuous_enabled());
}

#[test]
fn stop_clears_telemetry_stream() {
    let mut ctrl = make_control();
    ctrl.process_artisan_command(ArtisanCommand::SetStreaming(true))
        .expect("STREAM;ON succeeds");
    assert!(ctrl.get_output_manager().is_continuous_enabled());

    ctrl.process_artisan_command(ArtisanCommand::Stop)
        .expect("OFF succeeds");
    assert!(
        !ctrl.get_output_manager().is_continuous_enabled(),
        "STOP clears the session-scoped STREAM state"
    );
}

#[test]
fn stream_accepted_while_latched() {
    // BUG-08: STREAM has zero actuator side effects — allowed while the
    // safety latch is armed (same rationale as CHAN/UNITS/FILT).
    let mut ctrl = make_control();
    let _ = ctrl.process_artisan_command(ArtisanCommand::EmergencyStop);
    assert!(ctrl.safety().is_emergency_active());

    let r = ctrl.process_artisan_command(ArtisanCommand::SetStreaming(true));
    assert!(
        r.is_ok(),
        "STREAM;ON must be accepted while latched: {:?}",
        r
    );
    assert!(ctrl.get_output_manager().is_continuous_enabled());
}

// ── V2-16a: RoR guard must not fire during empty-drum PREHEAT ─

#[test]
fn ror_guard_skipped_in_preheat_empty_drum() {
    // Bug V2-16a: an empty drum with a low-mass BT probe heats faster
    // than MAX_BT_RATE_OF_RISE during PREHEAT; the guard used to fire
    // every tick in all states, bricking the device (via V2-1) within
    // 1-2 seconds. The guard is now gated to `Heating`/`Stable`.
    let mut ctrl = make_control();

    // Drive PREHEAT (state -> Preheating, PID enabled).
    let r = ctrl.process_artisan_command(ArtisanCommand::Preheat(180.0));
    assert!(r.is_ok());
    assert_eq!(ctrl.get_state(), RoasterState::Preheating);

    // Inject two samples ~0.8s apart with a 1.0 °C jump → 1.25 °C/s,
    // well above MAX_BT_RATE_OF_RISE (0.5 °C/s). The derivative filter
    // (α=0.3) will produce a non-zero rate that exceeds the limit. The
    // guard must NOT fire in Preheating.
    let t0 = Instant::from_millis(0);
    let t1 = Instant::from_millis(800);
    ctrl.update_temperatures(150.0, 120.0, t0).unwrap();
    // Run one control tick so the filter seeds `last_pv_sample`.
    let _ = ctrl.update_control(t0);
    ctrl.update_temperatures(151.0, 120.0, t1).unwrap();
    let out = ctrl.update_control(t1);

    assert!(
        out.is_ok(),
        "RoR guard must not trigger emergency in Preheating: {:?}",
        out
    );
    assert_ne!(
        ctrl.get_state(),
        RoasterState::Error,
        "Preheating must not flip to Error from a RoR transient"
    );
}

// ── V2-4: START swallowed after PID;SV / OT1 in Idle ───────────

#[test]
fn start_after_pid_sv_in_idle_starts_roast() {
    // Bug V2-4: `PID;SV` enables PID with state=Idle, which made
    // `is_streaming()` true. The old gate swallowed START as "ignored",
    // keeping `profile_start_time` unset so the temporal backstops stayed
    // inactive. The state-based gate (V2-4/V2-16c) must take the full
    // handoff when the state is Idle.
    let mut ctrl = make_control();

    // Pre-condition: PID enabled from Idle, state remains Idle.
    let r = ctrl.process_artisan_command(ArtisanCommand::SetTargetTemp(200.0));
    assert!(r.is_ok());
    assert!(ctrl.get_status().pid_enabled);
    assert_eq!(ctrl.get_state(), RoasterState::Idle);

    // START must now perform the handoff, not be ignored.
    let r = ctrl.process_artisan_command(ArtisanCommand::StartRoast);
    assert!(r.is_ok());
    assert_eq!(ctrl.get_state(), RoasterState::Heating);
    // `enable_pid_control` is called from the START handoff, so PID is
    // the active mode (pid_enabled=true, artisan_control=false by design
    // — see `dispatch.enable_pid` setting it false).
    assert!(
        ctrl.get_status().pid_enabled,
        "START must enable PID control"
    );
    assert!(
        ctrl.profile_start_time.is_some(),
        "START must fix profile_start_time so MAX_ROAST_TIME/comms-idle activate"
    );
}

#[test]
fn start_after_ot1_in_idle_starts_roast() {
    // Bug V2-4: `OT1` enables `artisan_control` in Idle (manual heater),
    // which also counted as "streaming" under the old gate. START must take
    // the full handoff.
    let mut ctrl = make_control();

    let r = ctrl.process_artisan_command(ArtisanCommand::SetHeater(40));
    assert!(r.is_ok());
    assert!(ctrl.get_status().artisan_control);
    assert_eq!(ctrl.get_state(), RoasterState::Idle);

    let r = ctrl.process_artisan_command(ArtisanCommand::StartRoast);
    assert!(r.is_ok());
    assert_eq!(ctrl.get_state(), RoasterState::Heating);
    assert!(ctrl.profile_start_time.is_some());
}

#[test]
fn start_during_active_roast_is_ignored() {
    // Regression guard for V2-4: the new state-based gate must still
    // ignore a second START that arrives during an active roast.
    let mut ctrl = make_control();
    let r = ctrl.process_artisan_command(ArtisanCommand::StartRoast);
    assert!(r.is_ok());
    assert_eq!(ctrl.get_state(), RoasterState::Heating);
    let first_start = ctrl.profile_start_time;

    let r = ctrl.process_artisan_command(ArtisanCommand::StartRoast);
    assert!(r.is_ok());
    assert_eq!(ctrl.get_state(), RoasterState::Heating);
    // START ignored keeps the ORIGINAL start time — a second START must
    // not silently restart the roast clock.
    assert_eq!(ctrl.profile_start_time, first_start);
}

// ── V2-16c: temporal backstops protect manual mode too ──────────

#[test]
fn comms_idle_protects_manual_mode_when_heater_energized() {
    // Bug V2-16c: in pure Artisan-manual mode (OT1 from a slider, no
    // START) the state stays Idle, so the previous state-only gate left a
    // USB disconnect with the heater at 80 % completely unprotected. The
    // physical gate (heater_energized || roast_active) must trigger the
    // comms-idle emergency even from Idle.
    let mut ctrl = make_control();

    // Energize the heater via OT1 in Idle. After the guarded heater write
    // `status.ssr_output` reflects the commanded percentage.
    let r = ctrl.process_artisan_command(ArtisanCommand::SetHeater(80));
    assert!(r.is_ok());
    assert!(
        ctrl.get_status().ssr_output > 0.0,
        "test precondition: heater must actually be energized"
    );
    assert_eq!(ctrl.get_state(), RoasterState::Idle);

    // Backdate last_command_received_at_ms so the idle window is exceeded.
    // Use a large fixed `now` (NOT `Instant::now()`): the host driver's
    // baseline starts at process boot, so early tests see `now.as_millis()`
    // well below COMMS_IDLE_TIMEOUT_MS (15 s), which would make
    // `saturating_sub` clamp to zero and the comms-idle check never trip.
    let now = Instant::from_millis(60_000);
    let backdated = Instant::from_millis(
        60_000u64.saturating_sub(crate::config::constants::COMMS_IDLE_TIMEOUT_MS + 1000),
    )
    .as_millis();
    ctrl.status_mut().last_command_received_at_ms = backdated;

    let out = ctrl.update_control(now);
    // Bug L11 (2026-07-25): `emergency_shutdown` always returns `Err` (the
    // actuator's `emergency_shutdown` ends with `Err(RoasterError::EmergencyShutdown)`),
    // so `update_control`'s `emergency_shutdown(...)?` early-returns with
    // that Err — the dead `return Ok(0.0)` that used to follow it has been
    // removed. The relevant assertion is the side-effect (latch armed),
    // not the return value, so `let _ = out;` covers both Ok and Err.
    let _ = out;
    assert!(
        ctrl.safety().is_emergency_active(),
        "Comms-idle must trigger an emergency when the heater is energized in Idle"
    );
}

#[test]
fn comms_idle_does_not_trigger_when_idle_and_heater_off() {
    // Regression guard: the physical gate must NOT over-trigger and
    // spuriously shut down an idle, cold roaster that has simply been
    // quiet for a while (the common pre-roast waiting state).
    let mut ctrl = make_control();
    let now = Instant::from_millis(60_000);
    let backdated = Instant::from_millis(
        60_000u64.saturating_sub(crate::config::constants::COMMS_IDLE_TIMEOUT_MS + 5000),
    )
    .as_millis();
    ctrl.status_mut().last_command_received_at_ms = backdated;
    ctrl.status_mut().ssr_output = 0.0;

    let _ = ctrl.update_control(now);
    assert!(
        !ctrl.safety().is_emergency_active(),
        "Idle + heater off must NOT trigger comms-idle, even after a long quiet period"
    );
}

// ── V2-7: #DUMP queue clears, survives full rings, re-pushes ───────

#[test]
fn handle_dump_log_clears_previous_dump() {
    // Bug V2-7: a second `#DUMP` request must not splice two partial
    // dumps together. `handle_dump_log` starts by clearing the deque.
    let mut ctrl = make_control();
    // Start a roast and stop it so the logger has at least one row.
    crate::logging::roast_logger::start_roast(embassy_time::Instant::now());
    crate::logging::roast_logger::log_sample(
        crate::logging::roast_logger::LogSampleData {
            bt: 100.0,
            et: 90.0,
            heater: 50.0,
            fan: 30.0,
            target: 200.0,
            ror: 0.0,
        },
        embassy_time::Instant::now(),
    );
    let r = ctrl.process_artisan_command(ArtisanCommand::DumpLog);
    assert!(r.is_ok());
    // Drain the queue fully.
    while ctrl.take_dump_row().is_some() {}
    // Request a second dump — the deque was cleared, so only this dump's
    // rows come out. If clear() had been skipped, the first dump's rows
    // would still be queued and the second call would append on top.
    // We assert the count after the second dump is small (one header row
    // in the dump string + any data rows — but the logger is still
    // active and the buffer holds 1 row, so the queue should be small,
    // not 2× the first call).
    let r = ctrl.process_artisan_command(ArtisanCommand::DumpLog);
    assert!(r.is_ok());
    let second_count = core::cell::Cell::new(0usize);
    while ctrl.take_dump_row().is_some() {
        second_count.set(second_count.get() + 1);
    }
    // The dump for a single-row buffer is "#DUMP <header>\n<row>\n" which
    // `handle_dump_log` splits into 2 non-empty lines (header + row).
    assert_eq!(
        second_count.get(),
        2,
        "second dump should have 2 rows (header + 1 data), not spliced with the first"
    );
    // Clean up the logger state so other tests are unaffected.
    crate::logging::roast_logger::stop_roast();
}

#[test]
fn start_clears_dump_pending() {
    // Bug V2-7: a START drops any in-flight dump so it does not bleed
    // into the new roast's live telemetry.
    let mut ctrl = make_control();
    // Seed the deque with a sentinel row (skip the real logger path).
    let row = heapless::String::<{ crate::logging::roast_logger::DUMP_ROW_CAPACITY }>::try_from(
        "sentinel-row",
    )
    .unwrap();
    ctrl.push_dump_row_front(row);
    assert!(ctrl.take_dump_row().is_some(), "sentinel row is queued");

    // Re-seed and start a roast.
    let row = heapless::String::<{ crate::logging::roast_logger::DUMP_ROW_CAPACITY }>::try_from(
        "sentinel-row-2",
    )
    .unwrap();
    ctrl.push_dump_row_front(row);
    let r = ctrl.process_artisan_command(ArtisanCommand::StartRoast);
    assert!(r.is_ok());
    assert!(
        ctrl.take_dump_row().is_none(),
        "START must clear the pending #DUMP queue"
    );
}

#[test]
fn push_dump_row_front_preserves_fifo_order() {
    // Bug V2-7: re-pushing a row to the front when the output channel is
    // full must keep FIFO order — the row is retried next, before any row
    // that was already behind it.
    let mut ctrl = make_control();
    ctrl.push_dump_row_front(
        heapless::String::<{ crate::logging::roast_logger::DUMP_ROW_CAPACITY }>::try_from("a")
            .unwrap(),
    );
    ctrl.push_dump_row_front(
        heapless::String::<{ crate::logging::roast_logger::DUMP_ROW_CAPACITY }>::try_from("b")
            .unwrap(),
    );
    // front = "b","a" so pop_front gives "b" first, then "a".
    assert_eq!(ctrl.take_dump_row().unwrap().as_str(), "b");
    assert_eq!(ctrl.take_dump_row().unwrap().as_str(), "a");
}

// ── V2-5: PREHEAT drops the cooldown latch ──────────────────────

#[test]
fn preheat_drops_cooling_latch() {
    // Bug V2-5 (B3 residual): `OFF` at a high BT arms the cooldown latch
    // (fan 100 %). A subsequent `PREHEAT;180` used to keep the latch armed
    // for the whole preheat — the PID heated against maximum airflow, and
    // since the heater kept BT > COOLING_RELEASE_BEAN_TEMP_C the latch
    // could never auto-release. Only START cleared it. PREHEAT is a
    // deliberate re-energize, so it must also clear the latch.
    let mut ctrl = make_control();

    // Simulate a STOP having latched cooldown: set the latch directly
    // via the field-touchable path the production STOP uses.
    // EmergencyStop arms the SAFETY latch (which we do NOT want to clear
    // in PREHEAT — that path is V2-1's OFF). Use a plain STOP via the
    // Artisan `Stop` handler so `cooling_active = true` and the safety
    // latch stays cleared.
    let r = ctrl.process_artisan_command(ArtisanCommand::Stop);
    assert!(r.is_ok());
    // The field is private; assert through the observable effect: a
    // subsequent `update_control` would force the fan to 100 % while the
    // latch is active. We instead assert the post-PREHEAT behaviour
    // directly via a status snapshot once PREHEAT clears the latch.
    // PREHEAT transitions to Preheating and must drop the latch.
    let r = ctrl.process_artisan_command(ArtisanCommand::Preheat(180.0));
    assert!(r.is_ok());
    assert_eq!(ctrl.get_state(), RoasterState::Preheating);
    // After PREHEAT we can re-arm the latch via STOP and observe that
    // PREHEAT clears it again — i.e. the test is reproducible.
    let _ = ctrl.process_artisan_command(ArtisanCommand::Stop);
    let _ = ctrl.process_artisan_command(ArtisanCommand::Preheat(180.0));
    assert_eq!(ctrl.get_state(), RoasterState::Preheating);
    // The fan selector in `update_control` is the assertion surface: if
    // the latch were still armed, the fan would be forced to 100 % and
    // append_crlf-style telemetry would show fan_output=100 after a tick.
    // Run a tick with a finite, sub-60 °C BT so the latch's BT<60 self-
    // release cannot mask the PREHEAT effect.
    ctrl.status_mut().bean_temp = 25.0;
    ctrl.status_mut().env_temp = 25.0;
    let _ = ctrl.update_control(Instant::from_millis(1_000));
    // With the latch cleared by PREHEAT and BT well below 60 °C, the fan
    // must NOT be clamped to 100 % by the cooldown path.
    assert_ne!(
        ctrl.get_status().fan_output,
        100.0,
        "PREHEAT must drop the cooldown latch (fan not forced to 100 %)"
    );
}

// ── V2-13: OFF+START preserves the fan profile ──────────────────

#[test]
fn off_start_preserves_fan_profile() {
    // Bug V2-13: `stop_streaming` used to clear `fan_profile = None`,
    // asymmetric with the temperature profile (which survived OFF). An
    // `OFF` → `START` flow silently wiped the fan profile and forced the
    // operator to re-send `FANPROFILE`. The cooldown latch already
    // takes precedence over the fan profile in the fan selector, and
    // clearing `profile_start_time` already disables interpolation during
    // cooldown — so the `fan_profile = None` line was both redundant for
    // the cooldown safety and harmful for the legitimate-profile path.
    // We thread a fan profile in via the private field (tests are inside
    // the module) and assert STOP does NOT erase it.
    use crate::config::constants::{FanProfile, FanSetpoint, MAX_PROFILE_SETPOINTS};

    let mut ctrl = make_control();

    // Seed a single-setpoint fan profile (target 33 % throughout).
    let mut setpoints = heapless::Vec::<FanSetpoint, MAX_PROFILE_SETPOINTS>::new();
    let _ = setpoints.push(FanSetpoint {
        time_secs: 0,
        fan_speed: 33,
    });
    let profile = FanProfile { setpoints };
    ctrl.fan_profile = Some(profile);
    assert!(
        ctrl.fan_profile.is_some(),
        "test precondition: profile loaded"
    );

    // STOP/OFF must NOT clear the fan profile (the V2-13 fix removed the
    // `self.fan_profile = None;` line from `stop_streaming`).
    let _ = ctrl.process_artisan_command(ArtisanCommand::Stop);
    assert!(
        ctrl.fan_profile.is_some(),
        "V2-13: STOP must NOT erase the loaded fan profile"
    );
    // Sanity: profile_start_time WAS cleared (interpolation off during
    // cooldown), but the profile itself survives.
    assert!(ctrl.profile_start_time.is_none());

    // The next START re-energizes and re-fixes profile_start_time; the
    // fan profile remains available for the fan selector.
    let _ = ctrl.process_artisan_command(ArtisanCommand::StartRoast);
    assert_eq!(ctrl.get_state(), RoasterState::Heating);
    assert!(
        ctrl.fan_profile.is_some(),
        "V2-13: fan profile must survive the OFF → START cycle"
    );
    assert!(ctrl.profile_start_time.is_some());
}

// ── P1 (2026-08-03): legacy RoR guard must not apply BT threshold to ET ──

#[test]
fn pid_channel_1_does_not_trigger_legacy_ror() {
    // Bug P1: with `PID;CHAN;1` (ET as PV), the legacy
    // `check_rate_of_rise` consumes `status.derivative_rate` — which
    // `refresh_filtered_derivative` feeds from the ACTIVE PV (ET). The
    // 0.5 °C/s threshold calibrated for the sluggish BT would abort a
    // healthy roast ~1 s into Heating. Reproduce: CHAN;1, ET climbing
    // ~1 °C/s for 5 ticks in Heating → no emergency. The BT-only
    // `check_bt_rate` guard (fed by `refresh_bt_guard_derivative`) is
    // what must protect this configuration.
    let mut ctrl = make_control();
    let r = ctrl.process_artisan_command(ArtisanCommand::SetPidChannel(1));
    assert!(r.is_ok());
    let r = ctrl.process_artisan_command(ArtisanCommand::StartRoast);
    assert!(r.is_ok());
    assert_eq!(ctrl.get_state(), RoasterState::Heating);
    assert_eq!(ctrl.get_status().pid_channel, 1);

    // Seed the sample pair, then climb ET ~0.31 °C per 310 ms tick
    // (≈ 1.0 °C/s — well above MAX_BT_RATE_OF_RISE) while BT stays flat.
    let t0 = Instant::from_millis(1000);
    ctrl.update_temperatures(150.0, 150.0, t0).unwrap();
    let _ = ctrl.update_control(t0);
    let mut et = 151.0;
    let mut now = Instant::from_millis(1310);
    for _ in 0..5 {
        ctrl.update_temperatures(150.0, et, now).unwrap();
        let out = ctrl.update_control(now);
        assert!(
            out.is_ok(),
            "P1: healthy ET heat-up under CHAN;1 must not trip the legacy RoR guard at {:?}: {:?}",
            now,
            out
        );
        assert_ne!(
            ctrl.get_state(),
            RoasterState::Error,
            "P1: CHAN;1 must not flip to Error from a healthy ET heat-up"
        );
        et += 0.31;
        now = Instant::from_millis(now.as_millis() + 310);
    }
}

// ── P3 (2026-08-03): START/PREHEAT recover from the STOP latch ─────────

#[test]
fn start_after_stop_recovers_to_heating() {
    // Bug P3: `STOP` (→ EmergencyStop) arms the emergency latch, and the
    // only previously-sanctioned recovery (`RoasterCommand::StopRoast`)
    // has no production producer — the next roast was impossible until
    // the undocumented `OFF` token. START is the operator's deliberate
    // re-energize: it must un-latch and start the roast.
    let mut ctrl = make_control();
    let r = ctrl.process_artisan_command(ArtisanCommand::EmergencyStop);
    assert!(r.is_ok(), "STOP path must arm the latch");
    assert!(ctrl.safety().is_emergency_active());
    assert!(ctrl.get_status().fault_condition);
    assert_eq!(ctrl.get_state(), RoasterState::Error);

    let r = ctrl.process_artisan_command(ArtisanCommand::StartRoast);
    assert!(r.is_ok(), "START after STOP must be accepted: {:?}", r);
    assert_eq!(
        ctrl.get_state(),
        RoasterState::Heating,
        "START after STOP must recover to a running roast"
    );
    assert!(
        !ctrl.safety().is_emergency_active(),
        "P3: START must clear the emergency latch"
    );
    assert!(
        !ctrl.get_status().fault_condition,
        "P3: START must clear fault_condition"
    );
    assert!(
        ctrl.profile_start_time.is_some(),
        "P3: recovered roast must have a profile clock"
    );
}

#[test]
fn preheat_after_stop_recovers() {
    // Bug P3 companion: PREHEAT is likewise a deliberate re-energize and
    // must recover from a latched STOP.
    let mut ctrl = make_control();
    let _ = ctrl.process_artisan_command(ArtisanCommand::EmergencyStop);
    assert!(ctrl.safety().is_emergency_active());

    let r = ctrl.process_artisan_command(ArtisanCommand::Preheat(180.0));
    assert!(r.is_ok(), "PREHEAT after STOP must be accepted: {:?}", r);
    assert_eq!(ctrl.get_state(), RoasterState::Preheating);
    assert!(!ctrl.safety().is_emergency_active());
    assert!(!ctrl.get_status().fault_condition);
}

// ── P4 (2026-08-03): RoR guard arms for PID;SV from Idle ───────────────

#[test]
fn pid_sv_in_idle_energizes_with_ror_guard() {
    // Bug P4: `PID;SV`/`SETTARGET` from Idle enables the PID (state stays
    // Idle) and the heater heats toward the setpoint with NO RoR
    // supervision — a runaway was only stopped by overtemp/comms-idle.
    // The guard must now arm on (Idle && pid_enabled && heater_energized):
    // BT climbing > 0.5 °C/s for 3 ticks → emergency shutdown.
    let mut ctrl = make_control();
    let r = ctrl.process_artisan_command(ArtisanCommand::SetTargetTemp(200.0));
    assert!(r.is_ok());
    assert!(ctrl.get_status().pid_enabled);
    assert_eq!(ctrl.get_state(), RoasterState::Idle);

    // Tick 0: seed a fresh reading; the PID (Kp=2, error=50) drives the
    // heater to saturation, so ssr_output must be > 0 afterwards.
    let t0 = Instant::from_millis(60_000);
    ctrl.status_mut().last_command_received_at_ms = t0.as_millis();
    ctrl.update_temperatures(150.0, 120.0, t0).unwrap();
    let _ = ctrl.update_control(t0);
    assert!(
        ctrl.get_status().ssr_output > 0.0,
        "test precondition: PID;SV in Idle must energize the heater"
    );

    // BT climbs ~1.6 °C/s (0.5 °C per 310 ms tick) toward the target.
    // After the EMA filter warms up, the derivative exceeds 0.5 °C/s for
    // 3 consecutive ticks → the extended guard must abort.
    let mut bt = 150.5;
    let mut now = Instant::from_millis(60_310);
    for _ in 0..6 {
        ctrl.update_temperatures(bt, 120.0, now).unwrap();
        let _ = ctrl.update_control(now);
        bt += 0.5;
        now = Instant::from_millis(now.as_millis() + 310);
    }
    assert!(
        ctrl.safety().is_emergency_active(),
        "P4: unsupervised PID;SV heater in Idle with BT rising >0.5 °C/s must abort"
    );
    assert_eq!(ctrl.get_state(), RoasterState::Error);
}

#[test]
fn pid_sv_in_idle_does_not_abort_on_healthy_bt() {
    // Regression guard for the P4 extension: a healthy BT drift
    // (< 0.5 °C/s) under PID;SV from Idle must NOT trip the guard.
    let mut ctrl = make_control();
    let _ = ctrl.process_artisan_command(ArtisanCommand::SetTargetTemp(200.0));
    let t0 = Instant::from_millis(70_000);
    ctrl.status_mut().last_command_received_at_ms = t0.as_millis();
    ctrl.update_temperatures(150.0, 120.0, t0).unwrap();
    let _ = ctrl.update_control(t0);

    let mut bt = 150.1;
    let mut now = Instant::from_millis(70_310);
    for _ in 0..8 {
        ctrl.update_temperatures(bt, 120.0, now).unwrap();
        let _ = ctrl.update_control(now);
        bt += 0.1; // ~0.32 °C/s — comfortably below the 0.5 °C/s limit
        now = Instant::from_millis(now.as_millis() + 310);
    }
    assert!(
        !ctrl.safety().is_emergency_active(),
        "P4: a healthy <0.5 °C/s drift under PID;SV in Idle must not abort"
    );
    assert_ne!(ctrl.get_state(), RoasterState::Error);
}

// ── P5 (2026-08-03): probe-stuck detector ──────────────────────────────

#[test]
fn probe_stuck_pid_mode_fires_after_flat_bt() {
    // Bug P5 + Audit A-TC4-C (2026-08-12): in firmware-PID mode the
    // detector keeps the original single-stage latch at
    // PROBE_STUCK_TIMEOUT_SECS (120 s): a flat PV FAR from the setpoint
    // while the loop is chasing it is a control hazard (a shorted TC
    // reads flat ~0 °C — a VALID temperature with no MAX31856 fault bit).
    let mut ctrl = make_control();
    let _ = ctrl.process_artisan_command(ArtisanCommand::SetTargetTemp(200.0));
    // High proportional gain with zero integral: BT far from the target
    // saturates the output, guaranteeing the arm-gate precondition.
    let _ = ctrl.process_artisan_command(ArtisanCommand::SetPidGain(300.0, 0.0, 0.0));

    let t0 = Instant::from_millis(300_000);
    // Pretend the commands arrived at t0 so the comms-idle backstop
    // does not fire instead of the probe detector.
    ctrl.status_mut().last_command_received_at_ms = t0.as_millis();
    ctrl.update_temperatures(0.0, 25.0, t0).unwrap();
    let _ = ctrl.update_control(t0); // arms the baseline on the first PID tick
    assert!(
        ctrl.get_status().ssr_output > 0.0,
        "test precondition: PID chasing a far target must energize the heater"
    );
    assert!(ctrl.get_status().pid_enabled);

    let t1 = Instant::from_millis(
        300_000 + crate::config::constants::PROBE_STUCK_TIMEOUT_SECS * 1000 + 1000,
    );
    ctrl.status_mut().last_command_received_at_ms = t1.as_millis();
    ctrl.update_temperatures(0.0, 25.0, t1).unwrap();
    let _ = ctrl.update_control(t1);

    assert!(
        ctrl.safety().is_emergency_active(),
        "P5: flat BT far from the PID target must latch at the 120 s threshold"
    );
    assert_eq!(ctrl.get_state(), RoasterState::Error);
}

#[test]
fn probe_stuck_manual_mode_two_stage_warns_then_latches() {
    // Audit A-TC4-C (2026-08-12): manual / Artisan software-PID mode is
    // two-stage. At PROBE_STUCK_TIMEOUT_SECS (120 s) the detector must
    // NOT latch — a legitimately slow finish can hold BT < 1 °C for
    // 2 min at low duty. Only after PROBE_STUCK_MANUAL_LATCH_SECS (300 s)
    // of continuous flat BT does the emergency latch fire, keeping the
    // dead-probe backstop (Bug S1) closed.
    let mut ctrl = make_control();
    let _ = ctrl.process_artisan_command(ArtisanCommand::SetHeater(80));
    assert!(
        ctrl.get_status().ssr_output > 0.0,
        "test precondition: manual heater must be energized"
    );
    assert!(!ctrl.get_status().pid_enabled);

    let t0 = Instant::from_millis(300_000);
    ctrl.status_mut().last_command_received_at_ms = t0.as_millis();
    ctrl.update_temperatures(0.0, 25.0, t0).unwrap();
    let _ = ctrl.update_control(t0); // arms the baseline

    // Stage 1: past the 120 s warning threshold — no latch.
    let t1 = Instant::from_millis(
        300_000 + crate::config::constants::PROBE_STUCK_TIMEOUT_SECS * 1000 + 1000,
    );
    ctrl.status_mut().last_command_received_at_ms = t1.as_millis();
    ctrl.update_temperatures(0.0, 25.0, t1).unwrap();
    let _ = ctrl.update_control(t1);

    assert!(
        !ctrl.safety().is_emergency_active(),
        "A-TC4-C: manual mode must NOT latch at the 120 s warning threshold"
    );
    assert_ne!(ctrl.get_state(), RoasterState::Error);

    // Stage 2: past the 300 s latch threshold — real latch.
    let t2 = Instant::from_millis(
        300_000 + crate::config::constants::PROBE_STUCK_MANUAL_LATCH_SECS * 1000 + 1000,
    );
    ctrl.status_mut().last_command_received_at_ms = t2.as_millis();
    ctrl.update_temperatures(0.0, 25.0, t2).unwrap();
    let _ = ctrl.update_control(t2);

    assert!(
        ctrl.safety().is_emergency_active(),
        "A-TC4-C: manual mode must latch at the 300 s threshold"
    );
    assert_eq!(ctrl.get_state(), RoasterState::Error);
}

#[test]
fn probe_stuck_detector_does_not_fire_on_moving_bt() {
    // A live probe that moves ≥ PROBE_STUCK_VARIATION_C within the window
    // must NOT trip the detector.
    let mut ctrl = make_control();
    let _ = ctrl.process_artisan_command(ArtisanCommand::SetHeater(80));
    let t0 = Instant::from_millis(400_000);
    ctrl.status_mut().last_command_received_at_ms = t0.as_millis();
    ctrl.update_temperatures(40.0, 25.0, t0).unwrap();
    let _ = ctrl.update_control(t0);

    let t1 = Instant::from_millis(
        400_000 + crate::config::constants::PROBE_STUCK_TIMEOUT_SECS * 1000 + 1000,
    );
    ctrl.status_mut().last_command_received_at_ms = t1.as_millis();
    ctrl.update_temperatures(42.0, 25.0, t1).unwrap(); // moved +2 °C
    let _ = ctrl.update_control(t1);

    assert!(
        !ctrl.safety().is_emergency_active(),
        "P5: a probe that moved ≥ 1 °C must not trip the detector"
    );
    assert_ne!(ctrl.get_state(), RoasterState::Error);
}

#[test]
fn probe_stuck_does_not_fire_when_regulating_near_target() {
    // A healthy roast in steady state holds BT nearly flat BY DESIGN (the
    // PID's job), and on a cold ambient / big drum the equilibrium duty
    // can sit at or above PROBE_STUCK_HEATER_MIN_PCT. The detector must
    // disarm within PROBE_STUCK_TARGET_MARGIN_C of the setpoint —
    // otherwise a stable roast at ≥50 % duty trips a FALSE "Probe stuck"
    // emergency.
    let mut ctrl = make_control();
    let _ = ctrl.process_artisan_command(ArtisanCommand::SetTargetTemp(200.0));
    // High proportional gain with zero integral: the output is purely
    // proportional, so BT parked 0.2 °C under the target yields a
    // STEADY ≥ 50 % duty with a FLAT BT — the exact steady-state regime
    // the margin exists to protect.
    let _ = ctrl.process_artisan_command(ArtisanCommand::SetPidGain(300.0, 0.0, 0.0));

    let t0 = Instant::from_millis(900_000);
    ctrl.status_mut().last_command_received_at_ms = t0.as_millis();
    ctrl.update_temperatures(199.8, 25.0, t0).unwrap();
    let _ = ctrl.update_control(t0);
    assert!(
        ctrl.get_status().ssr_output >= 50.0,
        "test precondition: steady-state duty must be ≥ PROBE_STUCK_HEATER_MIN_PCT \
             (got {:.1}%)",
        ctrl.get_status().ssr_output
    );

    let t1 = Instant::from_millis(
        900_000 + crate::config::constants::PROBE_STUCK_TIMEOUT_SECS * 1000 + 1000,
    );
    ctrl.status_mut().last_command_received_at_ms = t1.as_millis();
    ctrl.update_temperatures(199.8, 25.0, t1).unwrap(); // flat, near target
    let _ = ctrl.update_control(t1);

    assert!(
        !ctrl.safety().is_emergency_active(),
        "P5: a flat BT within the target margin while PID-regulating must not trip"
    );
    assert_ne!(ctrl.get_state(), RoasterState::Error);
}

// ── P6 (2026-08-03): MAX_ROAST_TIME must not run during PREHEAT ────────

#[test]
fn preheat_does_not_count_toward_max_roast_time() {
    // Bug P6: the 30-min cap must NOT run during Preheating — big drums
    // legitimately preheat for over half an hour. The old gate keyed on
    // `heater_energized || roast_active` (Preheating included), so a long
    // preheat hit the cap mid-preheat and aborted before loading beans.
    let mut ctrl = make_control();
    let r = ctrl.process_artisan_command(ArtisanCommand::Preheat(180.0));
    assert!(r.is_ok());
    assert_eq!(ctrl.get_state(), RoasterState::Preheating);

    let t0 = Instant::from_millis(500_000);
    ctrl.status_mut().last_command_received_at_ms = t0.as_millis();
    ctrl.update_temperatures(25.0, 25.0, t0).unwrap();
    let _ = ctrl.update_control(t0);
    // Second tick: `heater_energized` only sees the output applied on
    // tick 0, so the heat-session clock arms here.
    let t1 = Instant::from_millis(500_310);
    ctrl.status_mut().last_command_received_at_ms = t1.as_millis();
    ctrl.update_temperatures(25.5, 25.5, t1).unwrap();
    let _ = ctrl.update_control(t1);
    assert!(
        ctrl.get_status().ssr_output > 0.0,
        "test precondition: preheat heater must be energized"
    );
    assert!(
        ctrl.heat_session_start.is_some(),
        "test precondition: the heat-session clock is armed"
    );

    // Backdate the heat session past MAX_ROAST_TIME_SECS (tests are
    // in-module so the private field is reachable) and tick again at a
    // timestamp that implies ≥ 30 minutes of session time.
    ctrl.heat_session_start = Some(Instant::from_millis(100_000));
    let t_far = Instant::from_millis(2_000_000);
    ctrl.status_mut().last_command_received_at_ms = t_far.as_millis();
    // BT moves +2 °C across the gap so the P5 probe detector stays happy.
    ctrl.update_temperatures(27.0, 27.0, t_far).unwrap();
    let _ = ctrl.update_control(t_far);

    assert!(
        !ctrl.safety().is_emergency_active(),
        "P6: a preheat longer than MAX_ROAST_TIME_SECS must NOT abort"
    );
    assert_ne!(ctrl.get_state(), RoasterState::Error);
}

#[test]
fn start_resets_heat_session_clock() {
    // Bug P6 companion: START drops the manual heat-session clock so the
    // roast budget anchors to `profile_start_time` from the START.
    let mut ctrl = make_control();
    let _ = ctrl.process_artisan_command(ArtisanCommand::Preheat(180.0));
    let t0 = Instant::from_millis(1000);
    ctrl.update_temperatures(25.0, 25.0, t0).unwrap();
    let _ = ctrl.update_control(t0);
    // Second tick so the heat-session clock sees the applied heater output.
    let t1 = Instant::from_millis(1310);
    ctrl.update_temperatures(25.5, 25.5, t1).unwrap();
    let _ = ctrl.update_control(t1);
    assert!(ctrl.heat_session_start.is_some());

    let r = ctrl.process_artisan_command(ArtisanCommand::StartRoast);
    assert!(r.is_ok());
    assert_eq!(ctrl.get_state(), RoasterState::Heating);
    assert!(
        ctrl.heat_session_start.is_none(),
        "P6: START must reset the heat-session clock"
    );
    assert!(ctrl.profile_start_time.is_some());
}

// ── P10 (2026-08-03): #CHARGE fires on a realistic 2.26 °C/s drop ──────

#[test]
fn charge_detection_fires_on_low_rate_drop() {
    // Bug P10: with CHARGE_DROP_THRESHOLD_C = 6.0, a ~2.26 °C/s drop
    // (0.7 °C per 310 ms tick) spanning the 10-sample deque (~3.1 s)
    // fires #CHARGE. Under the previous 8.0 threshold the same profile
    // only accumulated 6.3 °C — the charge would have been silently
    // missed at the low end of the real 2–3 °C/s charge signature.
    let mut ctrl = make_control();
    let r = ctrl.process_artisan_command(ArtisanCommand::StartRoast);
    assert!(r.is_ok());
    assert_eq!(ctrl.get_state(), RoasterState::Heating);

    let t0 = Instant::from_millis(5_000);
    ctrl.update_temperatures(200.0, 220.0, t0).unwrap();
    let _ = ctrl.update_control(t0);
    let mut bt = 199.3;
    let mut now = Instant::from_millis(5_310);
    for _ in 1..10 {
        ctrl.update_temperatures(bt, 220.0, now).unwrap();
        let _ = ctrl.update_control(now);
        bt -= 0.7;
        now = Instant::from_millis(now.as_millis() + 310);
    }
    assert!(
        ctrl.get_status().charge_detected,
        "P10: a ~2.26 °C/s drop must fire #CHARGE with the 6.0 °C threshold"
    );
}

// ── P11 (2026-08-03): START resets the charge-detection state ──────────

#[test]
fn start_clears_charge_state() {
    // Bug P11: a batch that ends WITHOUT a STOP (e.g. PREHEAT → START
    // cadence) kept `charge_detected` latched, so the `!charge_detected`
    // gate never re-fired #CHARGE on the next batch. START must reset it
    // (idempotent with the `stop_streaming` reset on STOP/OFF).
    let mut ctrl = make_control();
    // Simulate a previous roast in which charge was detected.
    ctrl.charge_detected = true;
    ctrl.charge_time = Some(Instant::from_millis(100));
    ctrl.status_mut().charge_detected = true;

    let r = ctrl.process_artisan_command(ArtisanCommand::StartRoast);
    assert!(r.is_ok());
    assert_eq!(ctrl.get_state(), RoasterState::Heating);
    assert!(
        !ctrl.charge_detected && ctrl.charge_time.is_none() && !ctrl.get_status().charge_detected,
        "P11: START must clear the charge-detection state for the next batch"
    );
}

// ── B-L / B-H (2026-08-04): fan retry discipline in emergency paths ────

/// Shared per-instance attempt counter for `FlakyFan` (an `Arc` so the
/// test can read the count after the fan is moved into the control
/// object; a `critical_section::Mutex` keeps the fan `Send` as required
/// by `Box<dyn Fan + Send>`). Each test owns its own `Arc`, so tests
/// running in parallel never interfere.
type FanAttemptCounter = Arc<Mutex<RefCell<u8>>>;

fn new_fan_attempt_counter() -> FanAttemptCounter {
    Arc::new(Mutex::new(RefCell::new(0)))
}

fn read_fan_attempts(counter: &FanAttemptCounter) -> u8 {
    critical_section::with(|cs| *counter.borrow(cs).borrow())
}

/// Fan stub whose `emergency_set_speed` fails for the first
/// `fail_attempts` calls, then succeeds. Used to verify that the
/// emergency paths retry the fan instead of giving up after one attempt
/// (Bug B-L / B-H) and that `status.fan_output` is only published after a
/// successful write.
struct FlakyFan {
    fail_attempts: u8,
    attempts: FanAttemptCounter,
    last_speed: RefCell<f32>,
}

impl FlakyFan {
    fn new(fail_attempts: u8, attempts: FanAttemptCounter) -> Self {
        Self {
            fail_attempts,
            attempts,
            last_speed: RefCell::new(0.0),
        }
    }
}

impl Fan for FlakyFan {
    fn set_speed(&mut self, duty: f32) -> Result<(), RoasterError> {
        *self.last_speed.borrow_mut() = duty;
        Ok(())
    }

    fn emergency_set_speed(&mut self, percentage: f32) -> Result<(), RoasterError> {
        let attempt = critical_section::with(|cs| {
            let mut n = self.attempts.borrow(cs).borrow_mut();
            *n = n.saturating_add(1);
            *n
        });
        if attempt <= self.fail_attempts {
            return Err(RoasterError::HardwareError {
                source: Some("flaky_fan"),
            });
        }
        *self.last_speed.borrow_mut() = percentage;
        Ok(())
    }

    fn get_speed(&self) -> f32 {
        *self.last_speed.borrow()
    }
}

fn make_control_with_fan(fan: Box<dyn Fan + Send>) -> RoasterControl {
    let heater = Box::new(StubHeater::new());
    RoasterControl::new(heater, fan, SensorConversionHub::new()).expect("test control should build")
}

#[test]
fn emergency_shutdown_fan_retries_until_success() {
    // Bug B-L: the fan used to get a single attempt while the heater got
    // EMERGENCY_HEATER_OFF_RETRIES. A fan that fails twice and then
    // succeeds must still end at 100 %.
    let attempts = new_fan_attempt_counter();
    let fan = Box::new(FlakyFan::new(2, attempts.clone()));
    let mut ctrl = make_control_with_fan(fan);

    let result = ctrl.emergency_shutdown("test");
    assert!(matches!(
        result,
        Err(RoasterError::EmergencyShutdown { .. })
    ));
    assert_eq!(ctrl.get_state(), RoasterState::Error);
    assert_eq!(
        read_fan_attempts(&attempts),
        crate::config::constants::EMERGENCY_FAN_RETRIES,
        "B-L: the fan must be retried EMERGENCY_FAN_RETRIES times"
    );
    assert!(
        ctrl.get_status().fan_output == 100.0,
        "B-L: fan retries must land at 100 % (fan_output = {})",
        ctrl.get_status().fan_output
    );
}

#[test]
fn emergency_shutdown_fan_total_failure_keeps_fan_output_honest() {
    // Bug B-L: when the fan never accepts a write, `status.fan_output`
    // must NOT claim 100 % (the previous code wrote it unconditionally).
    // Bug S4 (2026-08-05): a total fan failure during an internal trap is
    // no longer absorbed — `emergency_shutdown` escalates as
    // `HardwareError(emergency_fan_failed)` so the control loop surfaces
    // an ERR to Artisan ("no fan means unsafe to continue").
    let attempts = new_fan_attempt_counter();
    let fan = Box::new(FlakyFan::new(u8::MAX, attempts.clone()));
    let mut ctrl = make_control_with_fan(fan);

    let result = ctrl.emergency_shutdown("test");
    assert!(
            matches!(
                result,
                Err(RoasterError::HardwareError {
                    source: Some("emergency_fan_failed")
                })
            ),
            "S4: a total fan failure must escalate as HardwareError(emergency_fan_failed), got {result:?}"
        );
    assert_eq!(ctrl.get_state(), RoasterState::Error);
    assert_eq!(
        read_fan_attempts(&attempts),
        crate::config::constants::EMERGENCY_FAN_RETRIES,
        "B-L: the fan gets exactly EMERGENCY_FAN_RETRIES attempts before giving up"
    );
    assert_ne!(
        ctrl.get_status().fan_output,
        100.0,
        "B-L: fan_output must reflect the physical state (fan never reached 100 %)"
    );
}

#[test]
fn artisan_stop_fan_failure_returns_err() {
    // Bug B-H: the Artisan STOP token path (`handle_emergency_stop`) must
    // escalate when the fan cannot reach 100 % — the control loop then
    // emits an ERR to Artisan instead of silently acknowledging.
    let attempts = new_fan_attempt_counter();
    let fan = Box::new(FlakyFan::new(u8::MAX, attempts.clone()));
    let mut ctrl = make_control_with_fan(fan);

    let result = ctrl.process_artisan_command(ArtisanCommand::EmergencyStop);
    assert!(result.is_err(), "B-H: fan failure must propagate as Err");
    assert!(ctrl.safety().is_emergency_active());
    assert_eq!(ctrl.get_state(), RoasterState::Error);
    assert_ne!(
        ctrl.get_status().fan_output,
        100.0,
        "B-H: fan_output must not claim 100 % when the fan never moved"
    );
}

#[test]
fn artisan_stop_fan_success_returns_ok() {
    // Bug B-H: with a working fan the STOP path still acknowledges.
    let fan = Box::new(StubFan::new());
    let mut ctrl = make_control_with_fan(fan);

    let result = ctrl.process_artisan_command(ArtisanCommand::EmergencyStop);
    assert!(result.is_ok());
    assert!(ctrl.safety().is_emergency_active());
    assert_eq!(ctrl.get_state(), RoasterState::Error);
    assert_eq!(
        ctrl.get_status().fan_output,
        100.0,
        "B-H: successful fan write must publish 100 %"
    );
}
