# Code Map and Change Recipes

*Updated 2026-10-22. Canonical source of truth for “where is X implemented?” and “where do I start
when changing X?”.*

Use this document to find the owning module and the symbols involved. Prefer symbol names over line
numbers; line numbers drift with every refactor. Architecture and behavior belong in
[`ARCHITECTURE.md`](ARCHITECTURE.md), [`CONTROL-LOOP.en.md`](CONTROL-LOOP.en.md), and
[`CONTROL_CORE.md`](CONTROL_CORE.md).

---

## Start Here

| Need | Entry point |
|---|---|
| Binary entry point and boot | `src/main.rs` → `init_hardware()` → `AppBuilder::build()` → `async_main_task` → `start_tasks()` |
| Application composition | `src/application/app_builder.rs`, `src/application/service_container.rs` |
| Long-lived tasks | `src/application/tasks.rs`, `src/hardware/usb_cdc/tasks.rs`, `src/hardware/uart/tasks.rs` |
| Central control orchestration | `src/control/roaster_control.rs` → `RoasterControl::process_artisan_command()` / `update_control()` |
| Command parsing | `src/input/parser.rs` → `parse_artisan_command()` |
| Wire formatting | `src/output/artisan.rs` → `ArtisanFormatter` |
| Safety evidence | [`SAFETY_CASE.md`](SAFETY_CASE.md) |

---

## Core Control Loop

| Feature | Primary files | Key symbols |
|---|---|---|
| Control-loop tick | `src/application/tasks.rs` | `control_loop_task()`, `control_loop_tick()` |
| Roaster orchestrator | `src/control/roaster_control.rs` | `RoasterControl::process_artisan_command()`, `update_control()` |
| Sensor controller | `src/control/controllers/sensor.rs` | `SensorController::update_temperatures()` |
| Actuator controller | `src/control/controllers/actuator.rs` | `apply_guarded_heater()`, fan output selection |
| Safety controller | `src/control/controllers/safety.rs` | `SafetyController::evaluate()` |
| Command dispatcher | `src/control/controllers/dispatch.rs` | `CommandDispatcher::process_command()` |
| PID controller | `src/control/pid.rs` | `CoffeeRoasterPid::update_feedback()` |
| SSR scheduler | `src/control/ssr_scheduler.rs` | `next_cycle_allowed()`, `mark_cycle()` |
| Status aggregation | `src/config/constants.rs` | `SystemStatus` |

## Command Handling (Artisan/TC4)

| Feature | Primary files | Key symbols |
|---|---|---|
| Parser | `src/input/parser.rs` | `parse_artisan_command()` |
| USB/UART multiplexer | `src/input/multiplexer.rs` | `CommandMultiplexer` |
| Shared command channel | `src/application/service_container.rs` | `ARTISAN_CMD_CHANNEL` |
| Artisan command handlers | `src/control/handlers/artisan.rs` | `ArtisanCommandHandler` |
| Temperature commands | `src/control/roaster_control.rs`, `src/control/handlers/temperature.rs` | `handle_set_target_temp()`, `handle_preheat()` |
| System commands | `src/control/roaster_control.rs` | `handle_start_roast()`, `handle_emergency_stop()`, `handle_read_status()`, `handle_status_report()` |
| Safety commands | `src/control/handlers/safety.rs` | `SafetyCommandHandler` |
| Display units | `src/config/constants.rs` | `TemperatureScale` |

## Output and Telemetry

| Feature | Primary files | Key symbols |
|---|---|---|
| Artisan formatter | `src/output/artisan.rs` | `ArtisanFormatter`, `MutableArtisanFormatter` |
| Output channel | `src/application/service_container.rs` | `ARTISAN_OUTPUT_CHANNEL` |
| Dual output task | `src/application/tasks.rs` | `dual_output_task()` |
| Telemetry output | `src/control/abstractions.rs` | `OutputController` |
| READ response | `src/output/artisan.rs` | `format_read_response_full()` |
| STATUS response | `src/output/artisan.rs` | `format_status_response()` |
| CSV, RoR, and time formatting | `src/output/artisan.rs` | `ArtisanFormatter` helpers |

STATUS field semantics and diagnostics are owned by [`INSTRUMENTATION.md`](INSTRUMENTATION.md).
Wire grammar is owned by [`PROTOCOL.md`](PROTOCOL.md).

## Hardware Abstraction

| Feature | Primary files | Key symbols |
|---|---|---|
| MAX31856 driver | `src/hardware/max31856.rs` | `Max31856::read_temperature()` |
| Sensor conversion | `src/hardware/sensors/conversion.rs` | `SensorConversionHub`, `SensorController::update_temperatures()` |
| Shared SPI | `src/hardware/shared_spi.rs` | `SharedSpiDevice`, `SpiDeviceWithCs` |
| SSR heater | `src/hardware/ssr.rs`, `src/hardware/ssr_logic.rs` | `set_duty_raw()`, `SsrControlBase` |
| Fan PWM | `src/hardware/fan.rs` | `FanController::set_speed()`, `emergency_set_speed()` |
| LEDC bus and guard | `src/hardware/ledc_bus.rs`, `src/hardware/ledc_guard.rs` | `LedcBus`, `LedcGuard` |
| USB CDC driver | `src/hardware/usb_cdc/driver.rs` | `UsbCdcTxDriver`, `UsbCdcRxDriver`; host stub `UsbCdcDriver` |
| USB reader task | `src/hardware/usb_cdc/tasks.rs` | `usb_reader_task()` |
| UART driver | `src/hardware/uart/driver.rs`; host stub `uart/driver_host.rs` | `UartTxDriver`, `UartRxDriver` |
| UART reader task | `src/hardware/uart/tasks.rs` | `uart_reader_task()` |
| Heat-source detection | `src/hardware/heat_presence.rs` | `debounce_heat_absent()`, `HeatPresenceOutcome` |
| Status LED | `src/hardware/status_led.rs` | LED pattern logic and per-tick driver |

## Safety and Watchdogs

| Feature | Primary files | Key symbols |
|---|---|---|
| RTC watchdog | `src/safety/watchdog.rs` | `WatchdogFeeder::feed_async()` |
| Over-temperature cutoff | `src/control/controllers/sensor.rs`, `safety.rs` | `SensorController::update_temperatures()`, `SafetyController::evaluate()` |
| Stale-temperature guard | `src/control/controllers/sensor.rs` | `TEMP_VALIDITY_TIMEOUT_MS` check |
| Heat-source detection | `src/hardware/heat_presence.rs`, `src/hardware/ssr_logic.rs` | `SsrControlBase::detect_heat_source()`, `cross_check_heat_detection()` |
| Emergency stop | `src/control/roaster_control.rs`, `src/control/handlers/safety.rs` | `handle_emergency_stop()` |
| Regression task | `src/safety/regression.rs` | `regression_task()` |
| Safe shutdown on init failure | `src/main.rs` | `enter_safe_shutdown()` |

Hazard-to-evidence mapping is owned by [`SAFETY_CASE.md`](SAFETY_CASE.md). Control invariants are
summarized in [`CONTROL_CORE.md`](CONTROL_CORE.md).

## Configuration and Constants

| Feature | Primary file |
|---|---|
| Pin assignments | `src/config/constants.rs` |
| PID defaults | `src/config/constants.rs` |
| Safety thresholds | `src/config/constants.rs` |
| Timing constants | `src/config/constants.rs` |
| Memory layout | `src/memory/constants.rs` |
| Command enums | `src/config/constants.rs` → `ArtisanCommand` |
| Status struct | `src/config/constants.rs` → `SystemStatus` |

The canonical pin map and electrical constraints are in [`HARDWARE.md`](HARDWARE.md).

## Application Composition

| Feature | Primary file(s) |
|---|---|
| `AppBuilder` | `src/application/app_builder.rs` |
| `ServiceContainer` | `src/application/service_container.rs` |
| Task spawning | `src/application/tasks.rs` |
| Queue metrics | `src/application/queue_metrics.rs` |
| Stage instrumentation | `src/application/stage_instrumentation.rs` |

## Logging and Diagnostics

| Feature | Primary file(s) |
|---|---|
| TRACE stream | `src/logging/traceability.rs` |
| Roast ring buffer | `src/logging/roast_logger.rs` |
| Edge log gating | `src/logging/edge_log_gate.rs` |
| Error types | `src/error/app_error.rs` |
| Hardware error counters | `src/hardware/error_counters.rs` |

## Testing Support (Host Only)

| Feature | Primary file(s) |
|---|---|
| Host time driver | `src/host_time_driver.rs` |
| Hardware mocks | `src/hardware/test_mocks.rs` |
| Simulated sensors | `src/hardware/sensors/simulated.rs` |
| Fan host stub | `src/hardware/fan_host.rs` |

---

## Change Recipes

### Add or Change an Artisan/TC4 Command

1. Add the command variant in `ArtisanCommand` (`src/config/constants.rs`).
2. Parse it in `parse_artisan_command()` (`src/input/parser.rs`).
3. Route it in `CommandDispatcher::process_command()` (`src/control/controllers/dispatch.rs`).
4. Handle it in `src/control/handlers/*.rs` or `RoasterControl` as appropriate.
5. Add coverage in `tests/artisan_integration_test.rs` and, for wire-visible behavior, the golden
   transcript suite (`tests/artisan_transcript_replay.rs`).
6. Update [`PROTOCOL.md`](PROTOCOL.md); do not duplicate the grammar elsewhere.

### Modify PID Behavior

| Target | File | Notes |
|---|---|---|
| PID algorithm | `src/control/pid.rs` | Pure math; no hardware access |
| Gains and limits | `src/control/pid.rs`, `src/config/constants.rs` | `CoffeeRoasterPid`, `with_gains()`, `set_gains()` |
| Guarded heater application | `src/control/controllers/actuator.rs` | `ActuatorController::apply_guarded_heater()` |
| Setpoint source | `src/control/handlers/temperature.rs` | `TemperatureCommandHandler::set_pid_target()` |

### Change Sensor Handling (MAX31856)

| Target | File | Notes |
|---|---|---|
| Raw SPI read | `src/hardware/max31856.rs` | `Max31856::read_temperature()` |
| Conversion, validation, EMA | `src/hardware/sensors/conversion.rs` | `SensorConversionHub` |
| Fault debounce and stale checks | `src/control/controllers/sensor.rs` | `SensorController` |
| Add a third sensor | `src/hardware/shared_spi.rs`, `src/hardware/sensors/conversion.rs` | New CS pin and conversion-hub extension |

### Modify Heater (SSR) Behavior

| Target | File | Notes |
|---|---|---|
| Zero-cross timing | `src/config/constants.rs` | `SSR_CONTROL_CYCLE_HZ` |
| Duty cycle to hardware | `src/hardware/ssr.rs`, `src/hardware/ssr_logic.rs` | `set_duty_raw()`, `SsrControlBase` |
| Slew-rate limiting | `src/control/controllers/actuator.rs` | `apply_guarded_heater()` |
| Cycle guard | `src/control/ssr_scheduler.rs` | `SsrCycleGuard` |
| Heat-sense interpretation | `src/hardware/ssr_logic.rs` | `heat-sense` feature only |

### Modify Fan (LEDC PWM) Behavior

| Target | File | Notes |
|---|---|---|
| PWM frequency and duty | `src/hardware/fan.rs` | `FanController::set_speed()`, `emergency_set_speed()` |
| Fan profile | `src/control/roaster_control.rs` | Fan selector in `update_control()` |
| Fan configuration | `src/config/constants.rs` | `FanProfile`, `FanSetpoint` |

### Add or Change a Safety Rule

| Target | File | Notes |
|---|---|---|
| Thresholds | `src/config/constants.rs` | Over-temperature, RoR, probe-stuck, timeouts |
| Policy evaluation | `src/control/controllers/safety.rs` | `SafetyController::evaluate()` |
| Emergency behavior | `src/control/roaster_control.rs`, `src/control/handlers/safety.rs` | `handle_emergency_stop()` |
| Watchdog feed | `src/safety/watchdog.rs` | `WatchdogFeeder::feed_async()` |
| Evidence mapping | [`SAFETY_CASE.md`](SAFETY_CASE.md) | Update hazard, guard, and test columns |

### Change Pin Assignments

| Target | File | Notes |
|---|---|---|
| Pin constants | `src/config/constants.rs` | Plain `pub const` values |
| Hardware init | `src/hardware/init.rs` | `init_hardware()` and boot asserts |
| SPI pins | `src/hardware/shared_spi.rs` | Shared bus construction |
| LEDC channels | `src/hardware/ledc_bus.rs` | `LedcBus::new()` |
| Strapping-pin constraints | [`HARDWARE.md`](HARDWARE.md), [`CONNECTION_TYPES.md`](CONNECTION_TYPES.md) | GPIO9 is a boot strap |

### Change Output Format (READ, STATUS, Telemetry)

| Target | File | Notes |
|---|---|---|
| READ response | `src/output/artisan.rs` | `format_read_response_full()` |
| STATUS response | `src/output/artisan.rs` | `format_status_response()`; field semantics in [`INSTRUMENTATION.md`](INSTRUMENTATION.md) |
| Continuous telemetry | `src/output/artisan.rs` | `format_artisan_line()` |
| Display units | `src/config/constants.rs` | `TemperatureSettings`, `TemperatureScale` |
| Add a telemetry field | `src/control/roaster_control.rs`, `src/output/artisan.rs` | Extend `SystemStatus` and the formatter |

### Add a New Embassy Task

1. Define the task in `src/application/tasks.rs` or the relevant hardware module.
2. Add any channel or shared state in `src/application/service_container.rs`.
3. Spawn it in `src/application/app_builder.rs`.
4. Wire accessors through `ServiceContainer` if other tasks need it.
5. Document architectural impact in [`ARCHITECTURE.md`](ARCHITECTURE.md), not here.

### Work With Host and Embedded Code Paths

| Scenario | Mechanism |
|---|---|
| Hardware driver stubs | `src/hardware/*_host.rs`, `src/hardware/test_mocks.rs`, `#[cfg(feature = "test")]` |
| Simulated sensors | `#[cfg(feature = "simulated-sensors")]` in `src/hardware/sensors/simulated.rs` |
| Host time driver | `src/host_time_driver.rs` with `test`/host cfg |
| Regression task stub | `#[cfg(not(all(target_arch = "riscv32", feature = "regression")))]` |
| USB/UART reader tasks | Not feature-gated out; readers parse and feed the shared channel |

### Debugging and Instrumentation

| Need | File |
|---|---|
| TRACE event | `src/logging/traceability.rs` |
| Roast log entry | `src/logging/roast_logger.rs` |
| Queue-depth metric | `src/application/queue_metrics.rs` |
| Stage timing | `src/application/stage_instrumentation.rs` |
| Hardware error counter | `src/hardware/error_counters.rs` |
| STATUS interpretation | [`INSTRUMENTATION.md`](INSTRUMENTATION.md) |

---

## Common “Where Is X?” Reference

| Question | Answer |
|---|---|
| Main entry point? | `src/main.rs` → `init_hardware()` → `AppBuilder::build()` → `async_main_task` → `start_tasks()` |
| Control-loop period? | `CONTROL_LOOP_PERIOD_MS` in `src/config/constants.rs`; real tick includes the MAX31856 conversion wait |
| Command channel capacity? | `ARTISAN_CMD_CHANNEL_SIZE` in `src/application/service_container.rs` |
| Output channel capacity? | `ARTISAN_OUTPUT_CHANNEL_SIZE` in `src/application/service_container.rs` |
| Watchdog timeouts? | `src/safety/watchdog.rs` and `src/config/constants.rs` |
| Heap size? | `heap_allocator!` in `src/main.rs` |
| Maximum profile points? | `MAX_PROFILE_SETPOINTS` in `src/config/constants.rs` |
| USB/UART write timeouts? | `src/hardware/usb_cdc/driver.rs`, `src/hardware/uart/driver.rs` |
| Why GPIO5 for MISO? | Avoids the FSPIQ strapping conflict; see [`HARDWARE.md`](HARDWARE.md) |

## Before Changing Code

1. Read [`../CONTEXT.md`](../CONTEXT.md), [`ARCHITECTURE.md`](ARCHITECTURE.md), and
   [`.planning/codebase/CONVENTIONS.md`](../.planning/codebase/CONVENTIONS.md).
2. Find the owning module in this document.
3. Follow the existing patterns: no `unwrap`/`expect`/`panic` in production, respect `Send` bounds,
   and keep blocking I/O out of async tasks.
4. Run the verification workflow in [`DEVELOPMENT.md`](DEVELOPMENT.md).
5. Update the owning document and this map when code moves.

---

*This file is intentionally a map, not a tutorial. If information conflicts with source code, the
source wins; update this map.*
