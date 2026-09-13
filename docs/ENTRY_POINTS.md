# Entry Points — "I want to do X, where do I start?"

*Updated 2026-09-09. Task-oriented entry points for common modifications.*

---

## Adding a New Artisan/TC4 Command

| Step | File | What to Do |
|------|------|------------|
| 1. Add command enum variant | `src/config/constants.rs` | Add to `ArtisanCommand` enum |
| 2. Parse it | `src/input/parser.rs` | Add branch in `parse_artisan_command()` |
| 3. Route it | `src/control/controllers/dispatch.rs` | Add match arm in `CommandDispatcher::process_command()` |
| 4. Handle it | `src/control/handlers/*.rs` | Create handler fn (see `artisan.rs`, `temperature.rs`, `system.rs`) |
| 5. Add test | `tests/artisan_integration_test.rs` | Add test case in `command_*` modules |

> **Pattern**: Commands are parsed → enter `ARTISAN_CMD_CHANNEL` → control loop drains → `CommandDispatcher` routes → handler mutates `RoasterControl` → optional immediate response via `OUTPUT_CHANNEL`.

---

## Modifying PID Behavior

| Target | File | Notes |
|--------|------|-------|
| PID algorithm | `src/control/pid.rs` | `CoffeeRoasterPid::update_feedback()` — pure math, no hardware |
| PID config (Kp, Ki, Kd, limits) | `src/control/pid.rs` | `CoffeeRoasterPid` gains via `with_gains()` / `set_gains()` |
| PID integration point | `src/control/controllers/actuator.rs` | `ActuatorController::apply_guarded_heater()` consumes PID output |
| PID setpoint source | `src/control/handlers/temperature.rs` | `TemperatureCommandHandler::set_pid_target()` |

---

## Changing Sensor Handling (MAX31856)

| Target | File | Notes |
|--------|------|-------|
| Raw SPI read | `src/hardware/max31856.rs` | `Max31856::read_temperature()` |
| Conversion / validation / EMA | `src/hardware/sensors/conversion.rs` | `SensorConversionHub` + `SensorController::update_temperatures` |
| SensorController logic | `src/control/controllers/sensor.rs` | Fault debounce, stale check |
| Add 3rd sensor | `src/hardware/shared_spi.rs` + `conversion.rs` | New CS pin, extend `SensorConversionHub` |

---

## Modifying Heater (SSR) Behavior

| Target | File | Notes |
|--------|------|-------|
| Zero-cross timing (5 Hz) | `src/config/constants.rs` | `SSR_CONTROL_CYCLE_HZ = 5` (fixed; `src/hardware/init.rs:122-183` Timer0/14-bit/Ch1) |
| Duty cycle → hardware | `src/hardware/ssr.rs` + `ssr_logic.rs` | `set_duty_raw()` / `SsrControlBase` state machine |
| Slew-rate limiting | `src/control/controllers/actuator.rs` | `ActuatorController::update_heater()` |
| Cycle guard (100 ms) | `src/control/controllers/actuator.rs` | `heater_cycle_guard` logic |

---

## Modifying Fan (LEDC PWM) Behavior

| Target | File | Notes |
|--------|------|-------|
| PWM frequency/duty | `src/hardware/fan.rs` | `FanController::set_speed()` / `emergency_set_speed()` |
| Fan profile / curve | `src/control/controllers/actuator.rs` | `ActuatorController::update_fan()` |
| Fan config | `src/config/constants.rs` | `FanConfig` |

---

## Adding/Changing Safety Rules

| Target | File | Notes |
|--------|------|-------|
| Over-temp thresholds | `src/config/constants.rs` | `OVERTEMP_THRESHOLD = 260.0` |
| Safety policy evaluation | `src/control/controllers/safety.rs` | `SafetyController` |
| Emergency stop behavior | `src/control/roaster_control.rs` | `handle_emergency_stop()` (+ `src/control/handlers/safety.rs`) |
| Stale temp timeout | `src/config/constants.rs` | `TEMP_VALIDITY_TIMEOUT_MS = 1000` |
| Watchdog feed | `src/safety/watchdog.rs` | `WatchdogFeeder::feed_async()` — called once per tick via `ServiceContainer::with_watchdog` |

---

## Changing Pin Assignments

| Target | File | Notes |
|--------|------|-------|
| All pin constants | `src/config/constants.rs:19-40` | Plain `pub const` pins (no `PinConfig` struct) |
| Hardware init (peripherals) | `src/hardware/init.rs` | `init_hardware()` + boot `assert_eq!` (`:86-112`) |
| SPI bus pins | `src/hardware/shared_spi.rs` | `SharedSpiBus::new()` |
| LEDC channels | `src/hardware/ledc_bus.rs` | `LedcBus::new()` |
| **⚠ Strapping pins** | `docs/HARDWARE.md` | GPIO9 (fan) is strapping — check before changing |

---

## Modifying Output Format (READ, STATUS, Telemetry)

| Target | File | Notes |
|--------|------|-------|
| READ response | `src/output/artisan.rs:97` | `format_read_response_full()` |
| STATUS response | `src/output/artisan.rs:145` | `format_status_response()` (20 fields) |
| Continuous telemetry | `src/output/artisan.rs:38` | `format_artisan_line()` (`#<time>,ET,BT,ROR,Gas`) |
| Display units (C/F) | `src/config/constants.rs` | `TemperatureSettings` impl |
| Add new telemetry field | `src/control/roaster_control.rs` | Extend `SystemStatus` + formatter |

---

## Adding a New Embassy Task

| Step | File | Notes |
|------|------|-------|
| 1. Define task fn | `src/application/tasks.rs` (+ `src/hardware/uart/tasks.rs`, `src/hardware/usb_cdc/tasks.rs`) | `#[embassy_executor::task] async fn my_task(...)` |
| 2. Add channel if needed | `src/application/service_container.rs` | `static MY_CHANNEL: Channel<...>` (channels live here, not `application/mod.rs`) |
| 3. Spawn in builder | `src/application/app_builder.rs` | `spawner.spawn(my_task(...))` |
| 4. Wire in ServiceContainer | `src/application/service_container.rs` | Add accessor if shared state needed |

---

## Host vs Embedded Code Paths

| Scenario | File | Mechanism |
|----------|------|-----------|
| Hardware driver stubs | `src/hardware/*` host paths, `src/hardware/test_mocks.rs` | `#[cfg(feature = "test")]` / host cfgs |
| Simulated sensors | `src/hardware/sensors/simulated.rs` | `#[cfg(feature = "simulated-sensors")]` (not plain `#[cfg(test)]`) |
| Host time driver | `src/host_time_driver.rs` | `#[cfg(feature = "test")]` |
| Regression task stub | `src/safety/regression.rs` | `#[cfg(not(all(target_arch = "riscv32", feature = "regression")))]` |
| USB/UART reader tasks | `src/hardware/usb_cdc/tasks.rs`, `src/hardware/uart/tasks.rs` (+ `src/hardware/transport_tasks.rs` event queue) | Not feature-gated out; readers own parsing (F5.3) |

---

## Debugging / Instrumentation Entry Points

| Need | File | What to Add |
|------|------|-------------|
| TRACE event | `src/logging/traceability.rs` | `trace_event!()` macro |
| Roast log entry | `src/logging/roast_logger.rs` | `RoastLogger::push()` |
| Queue depth metric | `src/application/queue_metrics.rs` | `QueueMetrics::record()` |
| Stage timing | `src/application/stage_instrumentation.rs` | `StageTimer::start/stop()` |
| Error counter (HW) | `src/hardware/error_counters.rs` | `ErrorCounters::inc()` |

---

## Common "Where is X?" Quick Reference

| Question | Answer |
|----------|--------|
| Main entry point? | `src/main.rs` → `init_hardware()` → `AppBuilder::build()` → `async_main_task` → `start_tasks()` |
| Control loop tick rate? | `src/config/constants.rs:333` — `CONTROL_LOOP_PERIOD_MS = 100` (real tick `CONTROL_LOOP_TICK_MS` ≈ 310–330 ms with MAX31856); timer used in `src/application/tasks.rs:1051-1055` |
| Command channel capacity? | `src/application/service_container.rs:55` — `ARTISAN_CMD_CHANNEL_SIZE = 16` |
| Output channel capacity? | `src/application/service_container.rs:57` — `ARTISAN_OUTPUT_CHANNEL_SIZE = 16` |
| Watchdog timeout? | `src/safety/watchdog.rs:55` — software `WATCHDOG_TIMEOUT_MS = 1000`; HW nominal `HW_WATCHDOG_TIMEOUT_MS` ≈ 2206 ms (`constants.rs:225`) |
| Heap size? | `src/main.rs:143` — `heap_allocator!(size: 72 * 1024)` |
| Max profile points? | `src/config/constants.rs:318` — `MAX_PROFILE_SETPOINTS = 16` (32 is `MAX_CURVE_POINTS` for simulated curves) |
| USB write timeout? | `src/hardware/usb_cdc/driver.rs:92-113` — 50 ms write (+10 ms best-effort terminator) + 20 ms flush; UART `uart/driver.rs:79-84` 50 ms + 50 ms |

---

## Making Changes Safely (Checklist)

1. **Read** `CONTEXT.md` + `docs/ARCHITECTURE.md` + `.planning/codebase/CONVENTIONS.md`
2. **Find** entry point in this file
3. **Edit** following patterns in `CONVENTIONS.md` (no `unwrap`/`expect` in prod, `Send` bounds, etc.)
4. **Test**: `cargo test --target x86_64-unknown-linux-gnu --features test`
5. **Build embedded**: `cargo build --release --target riscv32imc-unknown-none-elf --features embedded`
6. **Clippy**: `cargo clippy --locked --all-targets -- -W clippy::unwrap_used -W clippy::expect_used -W clippy::panic`

---

*Update when new patterns emerge. This file + `FEATURE_MAP.md` = full navigation.*