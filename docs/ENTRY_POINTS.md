# Entry Points — "I want to do X, where do I start?"

*Generated 2026-08-04. Task-oriented entry points for common modifications.*

---

## Adding a New Artisan/TC4 Command

| Step | File | What to Do |
|------|------|------------|
| 1. Add command enum variant | `src/config/constants.rs` | Add to `ArtisanCommand` enum |
| 2. Parse it | `src/input/parser.rs` | Add branch in `parse_artisan_command()` |
| 3. Route it | `src/control/controllers/dispatch.rs` | Add match arm in `CommandDispatcher::dispatch()` |
| 4. Handle it | `src/control/handlers/*.rs` | Create handler fn (see `artisan.rs`, `temperature.rs`, `system.rs`) |
| 5. Add test | `tests/artisan_integration_test.rs` | Add test case in `command_*` modules |

> **Pattern**: Commands are parsed → enter `ARTISAN_CMD_CHANNEL` → control loop drains → `CommandDispatcher` routes → handler mutates `RoasterControl` → optional immediate response via `OUTPUT_CHANNEL`.

---

## Modifying PID Behavior

| Target | File | Notes |
|--------|------|-------|
| PID algorithm | `src/control/pid.rs` | `PidController::update()` — pure math, no hardware |
| PID config (Kp, Ki, Kd, limits) | `src/config/constants.rs` | `PidConfig` struct |
| PID integration point | `src/control/controllers/actuator.rs` | `ActuatorController::update()` calls PID |
| PID setpoint source | `src/control/roaster_control.rs` | `RoasterControl::pid_setpoint()` |

---

## Changing Sensor Handling (MAX31856)

| Target | File | Notes |
|--------|------|-------|
| Raw SPI read | `src/hardware/max31856.rs` | `Max31856::read_temp_c()` |
| Conversion / validation / EMA | `src/hardware/sensors/conversion.rs` | `SensorHub::sample_all()` |
| SensorController logic | `src/control/controllers/sensor.rs` | Fault debounce, stale check |
| Add 3rd sensor | `src/hardware/shared_spi.rs` + `conversion.rs` | New CS pin, extend `SensorHub` |

---

## Modifying Heater (SSR) Behavior

| Target | File | Notes |
|--------|------|-------|
| Zero-cross timing (5 Hz) | `src/control/ssr_scheduler.rs` | `SsrScheduler::update()` |
| Duty cycle → hardware | `src/hardware/ssr.rs` | `SsrDriver::set_duty_cycle()` |
| Slew-rate limiting | `src/control/controllers/actuator.rs` | `ActuatorController::update_heater()` |
| Cycle guard (100 ms) | `src/control/controllers/actuator.rs` | `heater_cycle_guard` logic |

---

## Modifying Fan (LEDC PWM) Behavior

| Target | File | Notes |
|--------|------|-------|
| PWM frequency/duty | `src/hardware/fan.rs` | `FanDriver::set_speed()` |
| Fan profile / curve | `src/control/controllers/actuator.rs` | `ActuatorController::update_fan()` |
| Fan config | `src/config/constants.rs` | `FanConfig` |

---

## Adding/Changing Safety Rules

| Target | File | Notes |
|--------|------|-------|
| Over-temp thresholds | `src/config/constants.rs` | `SafetyConfig::overtemp_*` |
| Safety policy evaluation | `src/control/controllers/safety.rs` | `SafetyController::evaluate()` |
| Emergency stop behavior | `src/control/handlers/safety.rs` | `handle_emergency_stop()` |
| Stale temp timeout | `src/config/constants.rs` | `TimingConfig::stale_temp_timeout_ms` |
| Watchdog feed | `src/safety/watchdog.rs` | `RtcWatchdog::feed()` — called in control loop |

---

## Changing Pin Assignments

| Target | File | Notes |
|--------|------|-------|
| All pin constants | `src/config/constants.rs` | `PinConfig` struct |
| Hardware init (peripherals) | `src/hardware/init.rs` | `init_peripherals()` |
| SPI bus pins | `src/hardware/shared_spi.rs` | `SharedSpiBus::new()` |
| LEDC channels | `src/hardware/ledc_bus.rs` | `LedcBus::new()` |
| **⚠ Strapping pins** | `docs/HARDWARE.md` | GPIO9 (fan) is strapping — check before changing |

---

## Modifying Output Format (READ, STATUS, Telemetry)

| Target | File | Notes |
|--------|------|-------|
| READ response | `src/output/artisan.rs:230` | `format_read_response()` |
| STATUS response | `src/output/artisan.rs:150` | `format_status_response()` |
| Continuous telemetry | `src/output/artisan.rs:410` | `format_artisan_line()` |
| Display units (C/F) | `src/config/constants.rs` | `TemperatureScale` impl |
| Add new telemetry field | `src/control/roaster_control.rs` | Extend `SystemStatus` + formatter |

---

## Adding a New Embassy Task

| Step | File | Notes |
|------|------|-------|
| 1. Define task fn | `src/application/tasks.rs` | `#[embassy_executor::task] async fn my_task(...)` |
| 2. Add channel if needed | `src/application/mod.rs` | `static MY_CHANNEL: Channel<...>` |
| 3. Spawn in builder | `src/application/app_builder.rs` | `spawner.spawn(my_task(...))` |
| 4. Wire in ServiceContainer | `src/application/service_container.rs` | Add accessor if shared state needed |

---

## Host vs Embedded Code Paths

| Scenario | File | Mechanism |
|----------|------|-----------|
| Hardware driver stubs | `src/hardware/*_host.rs`, `src/hardware/test_mocks.rs` | `#[cfg(feature = "test")]` |
| Simulated sensors | `src/hardware/sensors/simulated.rs` | `#[cfg(feature = "test")]` |
| Host time driver | `src/host_time_driver.rs` | `#[cfg(feature = "test")]` |
| Regression task stub | `src/safety/regression.rs` | `#[cfg(not(feature = "regression"))]` |
| USB/UART selection | `src/hardware/transport_tasks.rs` | Feature-gated |

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
| Main entry point? | `src/main.rs` → `init_peripherals()` → `AppBuilder::build()` → `spawn_tasks()` |
| Control loop tick rate? | `src/application/tasks.rs:45` — `TICK_INTERVAL = 100.ms()` (real ≈ 310–330 ms with MAX31856) |
| Command channel capacity? | `src/application/mod.rs:15` — `ARTISAN_CMD_CHANNEL: Channel<..., 8>` |
| Output channel capacity? | `src/application/mod.rs:18` — `OUTPUT_CHANNEL: Channel<..., 16>` |
| Watchdog timeout? | `src/safety/watchdog.rs:25` — `WDT_TIMEOUT_MS = 5000` |
| Heap size? | `src/memory/constants.rs:12` — `HEAP_SIZE = 72 * 1024` |
| Max profile points? | `src/config/constants.rs` — `MAX_PROFILE_POINTS = 32` |
| USB write timeout? | `src/hardware/usb_cdc/driver.rs:65` — `50 ms + 20 ms` |

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