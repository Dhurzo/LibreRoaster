#![cfg(all(test, feature = "test", not(target_arch = "riscv32")))]
/// Edge case tests for roast flow resilience.
/// Verifies firmware doesn't crash or break when hardware is missing,
/// charge detection fails, or invalid commands are received.
use libreroaster::config::*;
use libreroaster::control::roaster_control::RoasterControl;
use libreroaster::hardware::sensors::SensorConversionHub;
use libreroaster::hardware::test_mocks::{MockFan, MockSsr};
use libreroaster::input::parser::{parse_artisan_command, store_profile, take_profile};

#[test]
fn no_profile_start_falls_back_to_default_target() {
    // Audit MT-7 (2026-08-11): the test name claims the *fallback* behaviour
    // — that START without a PROFILE applies DEFAULT_TARGET_TEMP — but the
    // old body only asserted boot-time defaults. Drive a START and verify
    // the target lands on DEFAULT_TARGET_TEMP, which is the contract the
    // name promises and the audit (F2-MT7) was missing.
    let heater = MockSsr::new();
    let fan = MockFan::new();
    let hub = SensorConversionHub::new();
    let mut rc = RoasterControl::new(Box::new(heater), Box::new(fan), hub)
        .expect("RoasterControl should init without profile");

    let status = rc.get_status();
    assert!(!status.pid_enabled);
    assert!(!status.charge_detected);
    assert_eq!(rc.get_state(), RoasterState::Idle);

    // START with no PROFILE must fall back to DEFAULT_TARGET_TEMP (constants.rs).
    rc.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("START without profile should succeed");
    assert_eq!(
        rc.get_status().target_temp,
        libreroaster::config::constants::DEFAULT_TARGET_TEMP,
        "START without PROFILE should fall back to DEFAULT_TARGET_TEMP"
    );
}

#[test]
fn charge_not_detected_roast_continues_normally() {
    // Simulate: beans loaded but BT drop < threshold (slow load).
    // Roast should continue without crash and charge flag stays false.
    let heater = MockSsr::new();
    let fan = MockFan::new();
    let hub = SensorConversionHub::new();
    let mut rc = RoasterControl::new(Box::new(heater), Box::new(fan), hub).expect("init");

    // Use proper API to start the roast
    rc.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("START should work");
    rc.process_artisan_command(ArtisanCommand::SetTargetTemp(200.0))
        .expect("SETTARGET should work");

    // Simulate a gentle BT decline (5°C over several samples — below threshold)
    for bt in [180.0f32, 179.0, 178.5, 178.0, 177.8, 177.5] {
        let current_time = embassy_time::Instant::now();
        rc.update_temperatures(bt, bt + 50.0, current_time)
            .expect("update should not fail");
        let _ = rc.update_control(current_time);
    }

    // Charge should NOT have been detected (drop only ~2.5°C, threshold is 20°C)
    assert!(!rc.status_mut().charge_detected);
    // Roast state should still be Heating (no crash, no fault)
    assert_eq!(rc.get_state(), RoasterState::Heating);
}

#[test]
fn bt_below_fifty_no_charge_check() {
    let heater = MockSsr::new();
    let fan = MockFan::new();
    let hub = SensorConversionHub::new();
    let mut rc = RoasterControl::new(Box::new(heater), Box::new(fan), hub).expect("init");

    rc.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("START should work");
    rc.process_artisan_command(ArtisanCommand::SetTargetTemp(200.0))
        .expect("SETTARGET should work");

    // BT below 50°C — charge detection is inactive
    for _ in 0..10 {
        let now = embassy_time::Instant::now();
        rc.update_temperatures(35.0, 32.0, now)
            .expect("update should not fail");
        let _ = rc.update_control(now);
    }
    assert!(!rc.status_mut().charge_detected);
}

#[test]
fn fan_profile_empty_does_not_break_control() {
    // Audit MT-7 (2026-08-11): the old body poked `status.fan_output = 30.0`
    // directly and only asserted `is_ok()`. That was vacuous: with no roast
    // running, `update_control` recomputes the fan from the selector
    // (roaster_control.rs:936-943), so the stale poke was overwritten with
    // the honest 0.0. The real contract: with NO FANPROFILE loaded and a
    // genuine operator `SetFan(30)`, the selector must fall through to the
    // manual value and preserve it across ticks (the FAN-FLOOR interlock at
    // FAN_MIN_SAFETY_PCT = 20% lets 30 pass through untouched).
    let heater = MockSsr::new();
    let fan = MockFan::new();
    let hub = SensorConversionHub::new();
    let mut rc = RoasterControl::new(Box::new(heater), Box::new(fan), hub).expect("init");

    rc.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("START");
    rc.process_artisan_command(ArtisanCommand::SetFan(30))
        .expect("operator fan command");

    let now = embassy_time::Instant::now();
    rc.update_temperatures(150.0, 180.0, now).expect("temps");
    let result = rc.update_control(now);

    // (1) Did not panic or error.
    assert!(result.is_ok(), "update_control returned {:?}", result);

    // (2) Fan stayed at the operator-set manual value (no FANPROFILE =>
    //     no automatic override), proving the empty-profile branch is not
    //     silently resetting fan_output to 0 or 100.
    assert_eq!(
        rc.get_status().fan_output,
        30.0,
        "empty FANPROFILE branch must preserve the operator-set manual fan"
    );
}

#[test]
fn start_without_preheat_parses_and_runs_end_to_end() {
    // Audit MT-7 (2026-08-11): renamed from `start_without_preheat_uses_default_or_profile`
    // (overclaiming — old body only called the parser). Now also exercises the
    // control layer to assert START in the absence of PREHEAT actually enters
    // the Heating state with a target, which is the contract the name implied.
    let result = parse_artisan_command("START");
    assert!(
        matches!(result, Ok(ArtisanCommand::StartRoast)),
        "START parses"
    );

    let heater = MockSsr::new();
    let fan = MockFan::new();
    let hub = SensorConversionHub::new();
    let mut rc = RoasterControl::new(Box::new(heater), Box::new(fan), hub).expect("init");
    rc.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("START runs without preheat");
    assert_eq!(
        rc.get_state(),
        RoasterState::Heating,
        "START must enter Heating even without a preceding PREHEAT"
    );
}

#[test]
fn preheat_then_start_transitions_normally() {
    // Audit MT-7 (2026-08-11): the old body only parsed two strings and a
    // comment admitted "the transition is in RoasterControl handler". Drive
    // PREHEAT → START through RoasterControl and assert the real transition
    // (Preheating → Heating). Note the target contract: with NO profile
    // loaded, `handle_start_roast` (roaster_control.rs:1261-1268) falls back
    // to `enable_pid_control(DEFAULT_TARGET_TEMP)`, so the PREHEAT target is
    // intentionally NOT preserved across START — the test pins that too.
    assert!(matches!(
        parse_artisan_command("PREHEAT 180"),
        Ok(ArtisanCommand::Preheat(180.0))
    ));
    assert!(matches!(
        parse_artisan_command("START"),
        Ok(ArtisanCommand::StartRoast)
    ));

    let heater = MockSsr::new();
    let fan = MockFan::new();
    let hub = SensorConversionHub::new();
    let mut rc = RoasterControl::new(Box::new(heater), Box::new(fan), hub).expect("init");
    rc.process_artisan_command(ArtisanCommand::Preheat(180.0))
        .expect("PREHEAT runs");
    assert_eq!(rc.get_state(), RoasterState::Preheating);
    assert_eq!(rc.get_status().target_temp, 180.0);

    rc.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("START runs after PREHEAT");
    assert_eq!(
        rc.get_state(),
        RoasterState::Heating,
        "START after PREHEAT must transition into Heating"
    );
    // No profile loaded → START falls back to the default target (the
    // PREHEAT target is a preheat-only setpoint, not carried into the roast).
    assert_eq!(
        rc.get_status().target_temp,
        libreroaster::config::constants::DEFAULT_TARGET_TEMP,
        "START without PROFILE must fall back to DEFAULT_TARGET_TEMP"
    );
}

#[test]
fn double_start_should_not_break() {
    // Two consecutive START commands: first enables, second is ignored
    let heater = MockSsr::new();
    let fan = MockFan::new();
    let hub = SensorConversionHub::new();
    let mut rc = RoasterControl::new(Box::new(heater), Box::new(fan), hub).expect("init");

    rc.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("first START should succeed");
    // Second START should be ignored (already streaming)
    rc.process_artisan_command(ArtisanCommand::StartRoast)
        .expect("second START should not error");
    assert_eq!(rc.get_state(), RoasterState::Heating);
}

#[test]
fn stop_during_idle_does_not_crash() {
    let heater = MockSsr::new();
    let fan = MockFan::new();
    let hub = SensorConversionHub::new();
    let mut rc = RoasterControl::new(Box::new(heater), Box::new(fan), hub).expect("init");

    // Bug #3 regression: EmergencyStop from Idle now LATCHES the emergency
    // (state = Error) rather than no-op'ing back to Idle, and recovery is
    // reserved for the explicit `StopRoast` command. The previous test
    // expected `Idle` here, which was the un-latching bug: pressing STOP
    // when nothing was running should *not* allow the heater to re-energize
    // unattended if a fault had latched.
    let result = rc.process_artisan_command(ArtisanCommand::EmergencyStop);
    assert!(result.is_ok());
    assert_eq!(
        rc.get_state(),
        RoasterState::Error,
        "EmergencyStop must latch even from Idle"
    );

    // Only the explicit recovery path un-latches back to Idle.
    let now = embassy_time::Instant::now();
    rc.process_command(RoasterCommand::StopRoast, now)
        .expect("recovery");
    assert_eq!(rc.get_state(), RoasterState::Idle);
}

#[test]
fn preheat_parser_edge_values() {
    // Bug B9: parser accepts any finite temperature; the handler validates
    // after the display→°C conversion. So PREHEAT 49 is parsed fine, and
    // PREHEAT 301 °F (~149 °C) is a normal preheat that was previously
    // rejected by the parser's incorrect °C check.
    assert!(matches!(
        parse_artisan_command("PREHEAT 50"),
        Ok(ArtisanCommand::Preheat(50.0))
    ));
    assert!(matches!(
        parse_artisan_command("PREHEAT 300"),
        Ok(ArtisanCommand::Preheat(300.0))
    ));
    assert!(matches!(
        parse_artisan_command("PREHEAT 49"),
        Ok(ArtisanCommand::Preheat(49.0))
    ));
    assert!(matches!(
        parse_artisan_command("PREHEAT 301"),
        Ok(ArtisanCommand::Preheat(301.0))
    ));
    // Sanity: still rejects invalid numerics.
    assert!(parse_artisan_command("PREHEAT abc").is_err());
}

#[test]
fn fan_profile_out_of_range_returns_error() {
    assert!(parse_artisan_command("FANPROFILE;0,20;10,150").is_err());
}

#[test]
fn fan_profile_too_many_setpoints() {
    // 16 is the max — should be OK
    let segments: Vec<String> = (0..16).map(|i| format!("{},{}", i * 10, i * 6)).collect();
    let cmd = format!("FANPROFILE;{}", segments.join(";"));
    assert!(parse_artisan_command(&cmd).is_ok());
}

#[test]
fn profile_before_start_does_not_activate_until_start() {
    let heater = MockSsr::new();
    let fan = MockFan::new();
    let hub = SensorConversionHub::new();
    let mut rc = RoasterControl::new(Box::new(heater), Box::new(fan), hub).expect("init");

    // Load a profile
    let mut profile = RoastProfile::new();
    profile
        .setpoints
        .push(ProfileSetpoint {
            time_secs: 0,
            temperature: 50.0,
        })
        .unwrap();
    profile
        .setpoints
        .push(ProfileSetpoint {
            time_secs: 120,
            temperature: 200.0,
        })
        .unwrap();
    store_profile(profile);
    rc.process_artisan_command(ArtisanCommand::SetProfile)
        .expect("profile load should work");

    // Before START, PID should not be enabled
    assert!(!rc.get_status().pid_enabled);
    assert_eq!(rc.get_state(), RoasterState::Idle);

    // Clean up
    let _ = take_profile();
}

#[test]
fn charge_detection_reset_on_stop() {
    let heater = MockSsr::new();
    let fan = MockFan::new();
    let hub = SensorConversionHub::new();
    let mut rc = RoasterControl::new(Box::new(heater), Box::new(fan), hub).expect("init");

    rc.status_mut().charge_detected = true;
    rc.status_mut().state = RoasterState::Heating;
    rc.process_artisan_command(ArtisanCommand::EmergencyStop)
        .expect("emergency stop should work");

    // Bug #3 regression: EmergencyStop LATCHES (state = Error) and does not
    // clear `charge_detected` itself (charge reset happens on the next START
    // via the recovery path). Recovering requires an explicit `StopRoast`.
    assert_eq!(
        rc.get_state(),
        RoasterState::Error,
        "EmergencyStop must latch the Error state"
    );

    let now = embassy_time::Instant::now();
    rc.process_command(RoasterCommand::StopRoast, now)
        .expect("recovery");
    assert_eq!(rc.get_state(), RoasterState::Idle);
    // charge_detected was reset by stop_streaming's charge_reset on recovery.
    assert!(!rc.status_mut().charge_detected);
}

#[test]
fn preheat_parser_case_insensitive() {
    assert!(matches!(
        parse_artisan_command("preheat 200"),
        Ok(ArtisanCommand::Preheat(200.0))
    ));
    assert!(matches!(
        parse_artisan_command("PREHEAT 200"),
        Ok(ArtisanCommand::Preheat(200.0))
    ));
    assert!(matches!(
        parse_artisan_command("Preheat 200"),
        Ok(ArtisanCommand::Preheat(200.0))
    ));
}

#[test]
fn fanprofile_parser_case_insensitive() {
    assert!(matches!(
        parse_artisan_command("fanprofile;0,20"),
        Ok(ArtisanCommand::SetFanProfile)
    ));
}

#[test]
fn empty_commands_handled_gracefully() {
    assert!(parse_artisan_command("").is_err());
    assert!(parse_artisan_command("   ").is_err());
}

// ─────────────────────────────────────────────────────────────────────────
// Bug B9 — display-unit (°F) setpoints must pass the parser
// ─────────────────────────────────────────────────────────────────────────
// A U.S. user running Artisan in Fahrenheit routinely issues setpoints
// above 300 °F (e.g. PID;SV;385 ≈ 196 °C) and below 50 °F (cold-start
// preheats). The previous parser applied a 50..=300 *°C* range check
// before the handler converted display units to °C, so every °F roast
// was effectively unstartable. The handler does the real validation
// after the conversion, so the parser must accept any finite value.

#[test]
fn b9_pid_sv_fahrenheit_normal_setpoint_parses() {
    // 385 °F ≈ 196 °C — normal medium-roast setpoint, must NOT be rejected.
    assert!(matches!(
        parse_artisan_command("PID;SV;385"),
        Ok(ArtisanCommand::SetTargetTemp(v)) if (v - 385.0).abs() < f32::EPSILON
    ));
    assert!(matches!(
        parse_artisan_command("PID,SV,385"),
        Ok(ArtisanCommand::SetTargetTemp(v)) if (v - 385.0).abs() < f32::EPSILON
    ));
}

#[test]
fn b9_settarget_fahrenheit_normal_setpoint_parses() {
    assert!(matches!(
        parse_artisan_command("SETTARGET 385"),
        Ok(ArtisanCommand::SetTargetTemp(v)) if (v - 385.0).abs() < f32::EPSILON
    ));
    // Below 50 °F also parses.
    assert!(matches!(
        parse_artisan_command("SETTARGET 32"),
        Ok(ArtisanCommand::SetTargetTemp(v)) if (v - 32.0).abs() < f32::EPSILON
    ));
}

#[test]
fn b9_preheat_fahrenheit_normal_setpoint_parses() {
    assert!(matches!(
        parse_artisan_command("PREHEAT 385"),
        Ok(ArtisanCommand::Preheat(385.0))
    ));
}

#[test]
fn b9_parser_still_rejects_non_finite() {
    assert!(parse_artisan_command("SETTARGET NaN").is_err());
    assert!(parse_artisan_command("PID,SV,inf").is_err());
    assert!(parse_artisan_command("PREHEAT abc").is_err());
}

// ─────────────────────────────────────────────────────────────────────────
// Bug B8 — long PROFILE / FANPROFILE (>128 bytes) must parse, not truncate
// ─────────────────────────────────────────────────────────────────────────
// PROFILE/FANPROFILE routinely reach ~170 bytes with 16 setpoints. The
// previous `String<128>` normaliser dropped the overflow silently with
// `let _ = s.push(ch)`, either rejecting a valid command with `out_of_range`
// or accepting a truncated profile that pinned the roast at an early
// setpoint for the entire session.

#[test]
fn b8_long_profile_above_128_bytes_parses() {
    // 16 setpoints × ~9-10 chars each + "PROFILE;" prefix ≈ 170 bytes.
    let segments: Vec<String> = (0..16)
        .map(|i| format!("{},{}", i * 60, 200 + i * 2))
        .collect();
    let cmd = format!("PROFILE;{}", segments.join(";"));
    assert!(
        cmd.len() > 128,
        "test harness: PROFILE must exceed 128 bytes, got {}",
        cmd.len()
    );
    assert!(matches!(
        parse_artisan_command(&cmd),
        Ok(ArtisanCommand::SetProfile)
    ));
}

#[test]
fn b8_long_fanprofile_above_128_bytes_parses() {
    // Pad each segment with a longer time field so total > 128 bytes.
    let segments: Vec<String> = (0..16).map(|i| format!("{:04},{}", i * 1000, 30)).collect();
    let cmd = format!("FANPROFILE;{}", segments.join(";"));
    assert!(
        cmd.len() > 128,
        "test harness: FANPROFILE must exceed 128 bytes, got {}",
        cmd.len()
    );
    assert!(matches!(
        parse_artisan_command(&cmd),
        Ok(ArtisanCommand::SetFanProfile)
    ));
}
