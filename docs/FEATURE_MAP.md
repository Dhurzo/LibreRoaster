# Feature → File Map

*Updated 2026-09-09. Single source of truth for "where is X implemented?"*

---

## Core Control Loop

| Feature | Primary File(s) | Key Types/Functions |
|---------|----------------|---------------------|
| **Main control loop tick** | `src/application/tasks.rs` | `control_loop_task()` |
| **RoasterControl orchestrator** | `src/control/roaster_control.rs` | `process_artisan_command()` / `update_control()` |
| **SensorController** | `src/control/controllers/sensor.rs` | `SensorController::update_temperatures()` |
| **ActuatorController** (heater + fan) | `src/control/controllers/actuator.rs` | `apply_guarded_heater()` / `set_fan_speed()` |
| **SafetyController** | `src/control/controllers/safety.rs` | `SafetyController::evaluate()` |
| **CommandDispatcher** | `src/control/controllers/dispatch.rs` | `CommandDispatcher::process_command()` |
| **PID controller** | `src/control/pid.rs` | `CoffeeRoasterPid::update_feedback()` |
| **SSR scheduler** (5 Hz zero-cross) | `src/control/ssr_scheduler.rs` | `next_cycle_allowed()` / `mark_cycle()` |
| **SystemStatus aggregation** | `src/config/constants.rs` | `SystemStatus` struct (updated by `RoasterControl`) |

---

## Command Handling (Artisan/TC4 Protocol)

| Feature | Primary File(s) | Key Types/Functions |
|---------|----------------|---------------------|
| **Command parser** | `src/input/parser.rs` | `parse_artisan_command()` |
| **Command multiplexer** (USB+UART) | `src/input/multiplexer.rs` | `CommandMultiplexer` |
| **Command channel** | `src/application/service_container.rs` | `ARTISAN_CMD_CHANNEL` (`ARTISAN_CMD_CHANNEL_SIZE = 16`) |
| **Artisan command handlers** | `src/control/handlers/artisan.rs` | `ArtisanCommandHandler::commit_manual_heater()` / `commit_manual_fan()` |
| **Temperature commands** (`SETTARGET`, `PREHEAT`) | `src/control/handlers/temperature.rs` | `TemperatureCommandHandler::set_pid_target()` / `get_pid_output()` |
| **System commands** (`START`, `STOP`, `READ`, `STATUS`) | `src/control/handlers/system.rs` | `SystemCommandHandler` (`handle_command()` / `can_handle()`) |
| **Safety commands** | `src/control/handlers/safety.rs` | `SafetyCommandHandler::activate_emergency()` / `clear_emergency()` |
| **Display units (C/F)** | `src/config/constants.rs` | `TemperatureScale::convert_*` |

---

## Output / Telemetry

| Feature | Primary File(s) | Key Types/Functions |
|---------|----------------|---------------------|
| **Artisan formatter** | `src/output/artisan.rs` | `ArtisanFormatter::format_*()` |
| **Output channel** | `src/application/service_container.rs` | `ARTISAN_OUTPUT_CHANNEL` (`ARTISAN_OUTPUT_CHANNEL_SIZE = 16`) |
| **Dual output task** | `src/application/tasks.rs` | `dual_output_task()` |
| **Continuous telemetry** | `src/control/abstractions.rs` | `OutputController` |
| **READ response** | `src/output/artisan.rs:97` | `format_read_response_full()` |
| **STATUS response** | `src/output/artisan.rs:145` | `format_status_response()` (20 fields) |
| **CSV/RoR/Time formatters** | `src/output/artisan.rs` | `ArtisanFormatter` helpers (`format_artisan_line`, RoR, time) |

---

## Hardware Abstraction

| Feature | Primary File(s) | Key Types/Functions |
|---------|----------------|---------------------|
| **MAX31856 driver** | `src/hardware/max31856.rs` | `Max31856::read_temperature()` |
| **Sensor conversion hub** | `src/hardware/sensors/conversion.rs` | `SensorConversionHub` + `SensorController::update_temperatures` |
| **Shared SPI bus** | `src/hardware/shared_spi.rs` | `SharedSpiBus` |
| **SSR (heater)** | `src/hardware/ssr.rs` + `ssr_logic.rs` | `set_duty_raw()` / `SsrControlBase` |
| **Fan (LEDC PWM)** | `src/hardware/fan.rs` | `FanController::set_speed()` |
| **LEDC bus/guard** | `src/hardware/ledc_bus.rs`, `ledc_guard.rs` | `LedcBus`, `LedcGuard` |
| **USB CDC driver** | `src/hardware/usb_cdc/driver.rs` | `UsbCdcDriver` |
| **USB CDC tasks** | `src/hardware/usb_cdc/tasks.rs` | `usb_reader_task()` |
| **UART driver** | `src/hardware/uart/driver.rs` | `UartDriver` |
| **UART tasks** | `src/hardware/uart/tasks.rs` | `uart_reader_task()` |
| **Heat presence detection** | `src/hardware/heat_presence.rs` | `HeatPresenceDetector` |

---

## Safety & Watchdogs

| Feature | Primary File(s) | Key Types/Functions |
|---------|----------------|---------------------|
| **RTC watchdog** | `src/safety/watchdog.rs` | `WatchdogFeeder::feed_async()` |
| **Over-temp cutoff** | `src/control/controllers/sensor.rs` + `safety.rs` | `check_overtemp` / safety policy |
| **Stale temperature guard** | `src/control/controllers/sensor.rs` | stale check (`TEMP_VALIDITY_TIMEOUT_MS`) |
| **Heat source detection** | `src/hardware/heat_presence.rs` + `ssr_logic.rs` | heat-source state machine |
| **Emergency stop** | `src/control/roaster_control.rs` | `handle_emergency_stop()` (+ handlers/safety.rs) |
| **Regression task** | `src/safety/regression.rs` | `regression_task()` |
| **Safe shutdown (init failure)** | `src/main.rs:70-95` | `enter_safe_shutdown()` (GPIO8 3×200 ms) |

---

## Configuration & Constants

| Feature | Primary File(s) |
|---------|----------------|
| **Pin assignments** | `src/config/constants.rs:19-40` | plain `pub const` pins |
| **PID defaults** | `src/config/constants.rs` | PID constants |
| **Safety thresholds** | `src/config/constants.rs` | overtemp / RoR / probe-stuck / timeouts |
| **Timing constants** | `src/config/constants.rs` | `CONTROL_LOOP_*`, `WATCHDOG_*`, `HW_WATCHDOG_*` |
| **Memory layout** | `src/memory/constants.rs` |
| **Command enums** | `src/config/constants.rs` → `ArtisanCommand` |
| **SystemStatus struct** | `src/config/constants.rs` → `SystemStatus` |

---

## Application Composition

| Feature | Primary File(s) |
|---------|----------------|
| **AppBuilder** | `src/application/app_builder.rs` |
| **ServiceContainer** | `src/application/service_container.rs` |
| **Task spawning** | `src/application/tasks.rs` |
| **Queue metrics** | `src/application/queue_metrics.rs` |
| **Stage instrumentation** | `src/application/stage_instrumentation.rs` |

---

## Logging & Diagnostics

| Feature | Primary File(s) |
|---------|----------------|
| **TRACE stream** | `src/logging/traceability.rs` |
| **Roast ring buffer** | `src/logging/roast_logger.rs` |
| **Error types** | `src/error/app_error.rs` |
| **Error counters (HW)** | `src/hardware/error_counters.rs` |

---

## Testing Support (host-only)

| Feature | Primary File(s) |
|---------|----------------|
| **Host time driver** | `src/host_time_driver.rs` |
| **Test mocks (HW)** | `src/hardware/test_mocks.rs` |
| **Simulated sensors** | `src/hardware/sensors/simulated.rs` |
| **Fan host stub** | `src/hardware/fan_host.rs` |

---

## Quick Navigation Index

```
src/
├── application/        # Task graph, DI container, builder
├── control/
│   ├── controllers/    # Sensor, Actuator, Safety, Dispatch
│   ├── handlers/       # Artisan, Temperature, System, Safety
│   ├── pid.rs
│   ├── roaster_control.rs
│   └── ssr_scheduler.rs
├── hardware/
│   ├── usb_cdc/        # USB reader + driver
│   ├── uart/           # UART reader + driver
│   ├── sensors/        # MAX31856 + conversion hub
│   ├── max31856.rs
│   ├── ssr.rs / fan.rs
│   └── heat_presence.rs
├── input/              # Parser + multiplexer
├── output/             # Formatters + ArtisanFormatter
├── safety/             # Watchdog + regression
├── config/             # All constants, enums, SystemStatus
├── logging/            # TRACE + roast logger
└── memory/             # Memory strategy constants
```

---

*Update this file when adding/moving features. Run `grep -r "Feature Map" docs/` to verify it's current.*