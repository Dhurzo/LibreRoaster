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
    let heater = MockSsr::new();
    let fan = MockFan::new();
    let hub = SensorConversionHub::new();
    let rc = RoasterControl::new(Box::new(heater), Box::new(fan), hub)
        .expect("RoasterControl should init without profile");

    let status = rc.get_status();
    assert!(!status.pid_enabled);
    assert!(!status.charge_detected);
    assert_eq!(rc.get_state(), RoasterState::Idle);
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
    let heater = MockSsr::new();
    let fan = MockFan::new();
    let hub = SensorConversionHub::new();
    let mut rc = RoasterControl::new(Box::new(heater), Box::new(fan), hub).expect("init");

    // No fan profile loaded — update_control should use manual fan
    let now = embassy_time::Instant::now();
    rc.status_mut().ssr_output = 50.0;
    rc.status_mut().fan_output = 30.0;
    let result = rc.update_control(now);
    assert!(result.is_ok()); // Should not panic or error
}

#[test]
fn start_without_preheat_uses_default_or_profile() {
    // START without prior PREHEAT should work (backward compat)
    let result = parse_artisan_command("START");
    assert!(matches!(result, Ok(ArtisanCommand::StartRoast)));
}

#[test]
fn preheat_then_start_transitions_normally() {
    // PREHEAT sets target, START should either continue or transition
    assert!(matches!(
        parse_artisan_command("PREHEAT 180"),
        Ok(ArtisanCommand::Preheat(180.0))
    ));
    assert!(matches!(
        parse_artisan_command("START"),
        Ok(ArtisanCommand::StartRoast)
    ));
    // Both parse correctly — the transition is in RoasterControl handler
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
    assert!(matches!(
        parse_artisan_command("PREHEAT 50"),
        Ok(ArtisanCommand::Preheat(50.0))
    ));
    assert!(matches!(
        parse_artisan_command("PREHEAT 300"),
        Ok(ArtisanCommand::Preheat(300.0))
    ));
    assert!(parse_artisan_command("PREHEAT 49").is_err());
    assert!(parse_artisan_command("PREHEAT 301").is_err());
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
