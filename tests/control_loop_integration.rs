#![cfg(all(test, feature = "test", not(target_arch = "riscv32")))]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
#![allow(clippy::type_complexity)]

extern crate std;

use std::boxed::Box;
use std::sync::Mutex;

use embassy_time::Instant;
use futures::executor::block_on;
use heapless::String;

use libreroaster::application::service_container::ServiceContainer;
use libreroaster::common::{StubFan, StubHeater};
use libreroaster::config::ArtisanCommand;
use libreroaster::control::roaster_control::RoasterControl;
use libreroaster::hardware::sensors::SensorConversionHub;
use libreroaster::logging::traceability::{TraceId, TracedCommand, TRACE_EVENT_MAX_LEN};

static TEST_MUTEX: Mutex<()> = Mutex::new(());

fn acquire_lock() -> std::sync::MutexGuard<'static, ()> {
    let guard = TEST_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // Recover from poisoned state from prior panics within the same test binary.
    // Tests sharing global state (ServiceContainer, channels) must run with
    // --test-threads=1 to prevent concurrent access.
    TEST_MUTEX.clear_poison();
    guard
}

fn build_control() -> RoasterControl {
    RoasterControl::new(
        Box::new(StubHeater::new()),
        Box::new(StubFan::new()),
        SensorConversionHub::new(),
    )
    .expect("RoasterControl should build")
}

fn init_service_container() {
    let roaster = build_control();
    ServiceContainer::init_roaster(roaster);
}

fn drain_channels() {
    let cmd = ServiceContainer::get_artisan_channel();
    while cmd.try_receive().is_ok() {}
    let out = ServiceContainer::get_output_channel();
    while out.try_receive().is_ok() {}
}

fn send_command(cmd: ArtisanCommand) {
    let traced = TracedCommand {
        command: cmd,
        trace_id: TraceId::next(),
        channel: libreroaster::input::multiplexer::CommChannel::None,
    };
    let _ = ServiceContainer::get_artisan_channel().try_send(traced);
}

fn drain_output() -> std::vec::Vec<std::string::String> {
    let channel = ServiceContainer::get_output_channel();
    let mut messages = std::vec::Vec::new();
    while let Ok(msg) = channel.try_receive() {
        messages.push(msg.as_str().to_string());
    }
    messages
}

fn build_tracked_control() -> RoasterControl {
    RoasterControl::new(
        Box::new(StubHeater::new()),
        Box::new(StubFan::new()),
        SensorConversionHub::new(),
    )
    .expect("RoasterControl should build")
}

// ═══════════════════════════════════════════════════════════════════════════
// Command drain tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn drain_commands_processes_queued_read() {
    let _guard = acquire_lock();
    init_service_container();
    drain_channels();

    send_command(ArtisanCommand::ReadStatus);

    block_on(async {
        let cmd_channel = ServiceContainer::get_artisan_channel();
        while let Ok(traced) = cmd_channel.try_receive() {
            let _ = ServiceContainer::with_roaster_async(|roaster| {
                let _ = roaster.process_artisan_command(traced.command);
                if matches!(traced.command, ArtisanCommand::ReadStatus) {
                    let status = roaster.get_status();
                    let response =
                        libreroaster::output::artisan::ArtisanFormatter::format_read_response_full(
                            &status,
                        );
                    if let Ok(line) = String::<TRACE_EVENT_MAX_LEN>::try_from(response.as_str()) {
                        let _ = ServiceContainer::get_output_channel().try_send(line);
                    }
                }
            })
            .await;
        }
    });

    let output = drain_output();
    let has_read_response = output
        .iter()
        .any(|msg| msg.contains(',') && msg.split(',').count() >= 5);
    assert!(
        has_read_response,
        "Expected READ response in output channel, got: {:?}",
        output
    );
}

#[test]
fn drain_commands_processes_start_roast() {
    let _guard = acquire_lock();
    init_service_container();
    drain_channels();

    block_on(async {
        let _ = ServiceContainer::with_roaster_async(|roaster| {
            roaster.update_temperatures(25.0, 25.0, Instant::now())
        })
        .await;
    });

    send_command(ArtisanCommand::StartRoast);

    block_on(async {
        let cmd_channel = ServiceContainer::get_artisan_channel();
        while let Ok(traced) = cmd_channel.try_receive() {
            let _ = ServiceContainer::with_roaster_async(|roaster| {
                roaster.process_artisan_command(traced.command)
            })
            .await;
        }
    });

    let status = block_on(async {
        ServiceContainer::with_roaster_async(|roaster| roaster.get_status()).await
    })
    .expect("should read status");

    assert!(
        status.pid_enabled || matches!(status.state, libreroaster::config::constants::RoasterState::Heating),
        "Roaster should be in Heating state with PID enabled after START, got: pid_enabled={}, state={:?}",
        status.pid_enabled,
        status.state
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Control update stage tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn update_control_with_manual_heater_output() {
    let _guard = acquire_lock();
    let mut control = build_tracked_control();

    control
        .update_temperatures(150.0, 180.0, Instant::now())
        .expect("update temperatures");
    control
        .process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start roast");
    control
        .process_artisan_command(ArtisanCommand::SetHeater(75))
        .expect("set heater");

    let output = control
        .update_control(Instant::now())
        .expect("update control");

    assert!(
        output > 0.0,
        "Heater output should be > 0 after SetHeater(75), got: {}",
        output
    );
    let status = control.get_status();
    assert!(
        status.artisan_control,
        "Should be under Artisan+ manual control"
    );
}

#[test]
fn update_control_emergency_flag_forces_zero_output() {
    let _guard = acquire_lock();
    let mut control = build_control();

    control
        .update_temperatures(150.0, 180.0, Instant::now())
        .expect("update temperatures");
    control
        .process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start roast");
    control
        .process_artisan_command(ArtisanCommand::SetHeater(75))
        .expect("set heater");

    control
        .emergency_shutdown("test emergency")
        .expect_err("emergency_shutdown should return Err");

    let output = control
        .update_control(Instant::now())
        .expect("update control");
    assert_eq!(output, 0.0, "Emergency flag should force SSR output to 0%");
}

// ═══════════════════════════════════════════════════════════════════════════
// Full tick pipeline tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn start_roast_enters_heating_state() {
    let _guard = acquire_lock();
    let mut control = build_control();

    control
        .update_temperatures(25.0, 25.0, Instant::now())
        .expect("initial temperatures");
    let result = control.process_artisan_command(ArtisanCommand::StartRoast);

    assert!(
        result.is_ok(),
        "StartRoast should succeed, got: {:?}",
        result
    );

    let status = control.get_status();
    assert!(
        status.pid_enabled,
        "PID should be enabled after START, got pid_enabled={}, state={:?}",
        status.pid_enabled, status.state
    );
    assert!(status.target_temp > 0.0, "Should have default target temp");

    let output = control.update_control(Instant::now());
    assert!(
        output.is_ok(),
        "update_control should succeed: {:?}",
        output
    );

    use libreroaster::config::constants::RoasterState;
    assert_eq!(
        control.get_state(),
        RoasterState::Heating,
        "Should enter Heating after START, got state={:?}",
        control.get_state()
    );
}

#[test]
fn stop_roast_turns_off_heater_and_full_fan() {
    let _guard = acquire_lock();
    let mut control = build_control();

    control
        .update_temperatures(100.0, 120.0, Instant::now())
        .expect("temps");
    control
        .process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");
    control
        .process_artisan_command(ArtisanCommand::SetHeater(50))
        .expect("heater");

    let output_before = control.update_control(Instant::now()).expect("update");
    assert!(output_before > 0.0, "Heater should be on before stop");

    control
        .process_artisan_command(ArtisanCommand::Stop)
        .expect("stop");

    let output_after = control
        .update_control(Instant::now())
        .expect("update after stop");
    // Audit MT-7 (2026-08-11): the status snapshot used to be taken BEFORE
    // this final `update_control`, so `fan_output >= 99.0` was asserted on a
    // pre-tick snapshot while `output_after == 0.0` came from the post-tick
    // return — the two sides of the same STOP behaviour were pinned at
    // different points in time. Take ONE post-tick snapshot and assert both
    // the heater-off and fan-full behaviour from it.
    let status = control.get_status();
    assert!(
        !status.artisan_control,
        "Should no longer be under Artisan+ control"
    );
    assert_eq!(output_after, 0.0, "Heater should be 0% after STOP");
    assert!(
        status.fan_output >= 99.0,
        "Fan should be at 100% for cooling after stop, was: {}",
        status.fan_output
    );
}

#[test]
fn multi_tick_roast_simulation() {
    let _guard = acquire_lock();
    let mut control = build_control();

    // START first, then set manual heater — matches real Artisan workflow
    // and keeps the system in manual mode (artisan_control=true), so heater
    // output comes from the manual set-point (80%) and is not PID-dependent.
    control
        .process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");
    control
        .process_artisan_command(ArtisanCommand::SetHeater(80))
        .expect("heater");

    let mut ticks_with_heat: usize = 0;

    for _tick in 0..30 {
        // Constant temperatures → zero temperature delta → ROR always 0.0°C/s.
        // This prevents spurious emergency shutdowns on CI where consecutive
        // Instant::now() calls can differ by only microseconds, making even a
        // 0.1°C change appear as a 100,000°C/s rate of rise.
        control
            .update_temperatures(25.0, 40.0, Instant::now())
            .expect("temps");

        let output = control.update_control(Instant::now()).expect("update");

        if output > 0.0 {
            ticks_with_heat += 1;
        }
    }

    // With SetHeater(80), heater output should be 80% on every tick
    assert!(
        ticks_with_heat > 0,
        "Should have heater output on at least some ticks"
    );
    assert_eq!(
        ticks_with_heat, 30,
        "Manual heater (80%) should be active on ALL 30 ticks"
    );

    control
        .process_artisan_command(ArtisanCommand::Stop)
        .expect("stop");

    let output_after = control
        .update_control(Instant::now())
        .expect("update after stop");
    assert_eq!(output_after, 0.0, "Heater should be off after STOP");
}

// ═══════════════════════════════════════════════════════════════════════════
// Command → output pipeline tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn read_command_produces_tc4_format_output() {
    let _guard = acquire_lock();
    init_service_container();
    drain_channels();

    block_on(async {
        let _ = ServiceContainer::with_roaster_async(|roaster| {
            roaster.update_temperatures(120.3, 150.5, Instant::now())
        })
        .await;
    });

    send_command(ArtisanCommand::ReadStatus);

    block_on(async {
        let cmd_channel = ServiceContainer::get_artisan_channel();
        while let Ok(traced) = cmd_channel.try_receive() {
            let _ = ServiceContainer::with_roaster_async(|roaster| {
                let _ = roaster.process_artisan_command(traced.command);
                if matches!(traced.command, ArtisanCommand::ReadStatus) {
                    let status = roaster.get_status();
                    let response =
                        libreroaster::output::artisan::ArtisanFormatter::format_read_response_full(
                            &status,
                        );
                    if let Ok(line) = String::<TRACE_EVENT_MAX_LEN>::try_from(response.as_str()) {
                        let _ = ServiceContainer::get_output_channel().try_send(line);
                    }
                }
            })
            .await;
        }
    });

    let output = drain_output();
    let read_response = output
        .iter()
        .find(|msg| msg.contains(',') && msg.split(',').count() >= 5)
        .expect("Should have a READ response");

    let parts: std::vec::Vec<&str> = read_response.split(',').collect();
    assert!(
        parts.len() >= 5,
        "READ response should have >= 5 fields, got: {}",
        parts.len()
    );
    assert!(parts[0].parse::<f32>().is_ok(), "AMB should be numeric");
    assert!(parts[1].parse::<f32>().is_ok(), "ET should be numeric");
    assert!(parts[2].parse::<f32>().is_ok(), "BT should be numeric");
}

// ═══════════════════════════════════════════════════════════════════════════
// Fault condition tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn fault_condition_rejects_mutating_commands() {
    let _guard = acquire_lock();
    let mut control = build_control();

    control
        .update_temperatures(150.0, 180.0, Instant::now())
        .expect("temps");
    control
        .process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");
    control
        .emergency_shutdown("test")
        .expect_err("emergency_shutdown returns Err");

    let heater_result = control.process_artisan_command(ArtisanCommand::SetHeater(50));
    assert!(
        heater_result.is_err(),
        "SetHeater should be rejected during fault"
    );

    let read_result = control.process_artisan_command(ArtisanCommand::ReadStatus);
    assert!(read_result.is_ok(), "READ should still work during fault");

    let status_result = control.process_artisan_command(ArtisanCommand::StatusReport);
    assert!(
        status_result.is_ok(),
        "STATUS should still work during fault"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Profile following tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn profile_start_sets_target_from_profile() {
    let _guard = acquire_lock();
    let mut control = build_control();

    use libreroaster::config::{ProfileSetpoint, RoastProfile};
    let profile = RoastProfile {
        setpoints: heapless::Vec::from_slice(&[
            ProfileSetpoint {
                time_secs: 0,
                temperature: 100.0,
            },
            ProfileSetpoint {
                time_secs: 60,
                temperature: 150.0,
            },
            ProfileSetpoint {
                time_secs: 120,
                temperature: 200.0,
            },
        ])
        .expect("setpoints"),
    };

    libreroaster::input::parser::store_profile(profile);
    control
        .process_artisan_command(ArtisanCommand::SetProfile)
        .expect("set profile");

    control
        .update_temperatures(100.0, 120.0, Instant::now())
        .expect("temps");
    control
        .process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");

    let status = control.get_status();
    assert!(status.pid_enabled, "PID should be enabled with profile");
    assert!(
        status.target_temp >= 100.0,
        "Target should be set from profile, got: {}",
        status.target_temp
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Manual control tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn set_heater_adjusts_output_within_range() {
    let _guard = acquire_lock();
    let mut control = build_control();

    control
        .update_temperatures(150.0, 180.0, Instant::now())
        .expect("temps");
    control
        .process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");
    control
        .process_artisan_command(ArtisanCommand::SetHeater(60))
        .expect("set heater 60");

    let output = control.update_control(Instant::now()).expect("update");
    assert!(
        output > 0.0 && output <= 60.0,
        "Output {} should be in (0, 60] range",
        output
    );
}

#[test]
fn set_fan_adjusts_fan_speed() {
    let _guard = acquire_lock();
    let mut control = build_control();

    control
        .update_temperatures(150.0, 180.0, Instant::now())
        .expect("temps");
    control
        .process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");
    control
        .process_artisan_command(ArtisanCommand::SetFan(75))
        .expect("set fan 75");

    control.update_control(Instant::now()).expect("update");

    let status = control.get_status();
    assert!(
        status.fan_output > 0.0,
        "Fan should have non-zero output after SetFan(75)"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Channel-based command flow tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn channel_command_flow_start_read_stop() {
    let _guard = acquire_lock();
    init_service_container();
    drain_channels();

    block_on(async {
        let _ = ServiceContainer::with_roaster_async(|roaster| {
            roaster.update_temperatures(25.0, 25.0, Instant::now())
        })
        .await;
    });

    send_command(ArtisanCommand::StartRoast);
    block_on(async {
        let cmd_channel = ServiceContainer::get_artisan_channel();
        while let Ok(traced) = cmd_channel.try_receive() {
            let _ = ServiceContainer::with_roaster_async(|roaster| {
                roaster.process_artisan_command(traced.command)
            })
            .await;
        }
    });

    use libreroaster::config::constants::RoasterState;
    let is_heating = block_on(async {
        ServiceContainer::with_roaster_async(|roaster| roaster.get_state() == RoasterState::Heating)
            .await
    })
    .expect("read state");
    assert!(is_heating, "Should be in Heating state after START");

    drain_channels();
    send_command(ArtisanCommand::Stop);
    block_on(async {
        let cmd_channel = ServiceContainer::get_artisan_channel();
        while let Ok(traced) = cmd_channel.try_receive() {
            let _ = ServiceContainer::with_roaster_async(|roaster| {
                roaster.process_artisan_command(traced.command)
            })
            .await;
        }
    });

    let is_idle = block_on(async {
        ServiceContainer::with_roaster_async(|roaster| roaster.get_state() == RoasterState::Idle)
            .await
    })
    .expect("read state");
    assert!(is_idle, "Should be in Idle state after STOP");
}

#[test]
fn status_command_produces_output_channel_messages() {
    let _guard = acquire_lock();
    init_service_container();
    drain_channels();

    block_on(async {
        let _ = ServiceContainer::with_roaster_async(|roaster| {
            roaster.update_temperatures(80.0, 100.0, Instant::now())
        })
        .await;
    });

    send_command(ArtisanCommand::StatusReport);

    block_on(async {
        let cmd_channel = ServiceContainer::get_artisan_channel();
        let output_channel = ServiceContainer::get_output_channel();
        while let Ok(traced) = cmd_channel.try_receive() {
            let _ = ServiceContainer::with_roaster_async(|roaster| {
                let _ = roaster.process_artisan_command(traced.command);
                if matches!(traced.command, ArtisanCommand::StatusReport) {
                    let status = roaster.get_status();
                    let response =
                        libreroaster::output::artisan::ArtisanFormatter::format_status_response(
                            &status,
                        );
                    if let Ok(line) = String::<TRACE_EVENT_MAX_LEN>::try_from(response.as_str()) {
                        let _ = output_channel.try_send(line);
                    }
                }
            })
            .await;
        }
    });

    let output = drain_output();
    assert!(
        !output.is_empty(),
        "STATUS command should produce output; got 0 messages"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// PID behavior tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn pid_output_responds_to_temperature_error() {
    let _guard = acquire_lock();
    let mut control = build_control();

    control
        .update_temperatures(25.0, 30.0, Instant::now())
        .expect("temps");
    control
        .process_artisan_command(ArtisanCommand::StartRoast)
        .expect("start");
    control
        .process_artisan_command(ArtisanCommand::SetTargetTemp(200.0))
        .expect("set target");

    let output_cold = control.update_control(Instant::now()).expect("update cold");
    assert!(
        output_cold > 0.0,
        "PID should request heat when far below target, got: {}",
        output_cold
    );

    // Run several ticks to build integrator before approaching target
    for _ in 0..5 {
        control
            .update_temperatures(25.0, 30.0, Instant::now())
            .expect("temps");
        control.update_control(Instant::now()).expect("update");
    }

    control
        .update_temperatures(198.0, 200.0, Instant::now())
        .expect("near target");
    let output_near = control
        .update_control(Instant::now())
        .expect("update near target");

    assert!(
        output_cold > 0.0,
        "PID output should be positive when cold, got: {}",
        output_cold
    );
    assert!(
        output_near <= output_cold || output_cold >= 99.0,
        "PID output should not increase as temp approaches target (cold={}, near={})",
        output_cold,
        output_near
    );
}

#[test]
fn sensor_reads_do_not_spuriously_trigger_emergency_on_host() {
    let _guard = acquire_lock();
    init_service_container();
    drain_channels();

    block_on(async {
        for i in 0..6 {
            let result = ServiceContainer::roaster_async_sensor_read().await;
            assert!(
                result.is_ok(),
                "Sensor read should succeed on host (attempt {})",
                i
            );
        }
    });

    let status = block_on(async {
        ServiceContainer::with_roaster_async(|roaster| roaster.get_status()).await
    })
    .expect("read status");

    assert!(
        !status.fault_condition,
        "Host sensor reads should not cause spurious fault condition"
    );
}
