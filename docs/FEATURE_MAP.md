# Feature → File Map

*Generated 2026-08-04. Single source of truth for "where is X implemented?"*

---

## Core Control Loop

| Feature | Primary File(s) | Key Types/Functions |
|---------|----------------|---------------------|
| **Main control loop tick** | `src/application/tasks.rs` | `control_loop_task()` |
| **RoasterControl orchestrator** | `src/control/roaster_control.rs` | `RoasterControl::tick()` |
| **SensorController** | `src/control/controllers/sensor.rs` | `SensorController::sample()` |
| **ActuatorController** (heater + fan) | `src/control/controllers/actuator.rs` | `ActuatorController::update()` |
| **SafetyController** | `src/control/controllers/safety.rs` | `SafetyController::evaluate()` |
| **CommandDispatcher** | `src/control/controllers/dispatch.rs` | `CommandDispatcher::dispatch()` |
| **PID controller** | `src/control/pid.rs` | `PidController::update()` |
| **SSR scheduler** (5 Hz zero-cross) | `src/control/ssr_scheduler.rs` | `SsrScheduler::update()` |
| **SystemStatus aggregation** | `src/control/roaster_control.rs:1200+` | `RoasterControl::build_status()` |

---

## Command Handling (Artisan/TC4 Protocol)

| Feature | Primary File(s) | Key Types/Functions |
|---------|----------------|---------------------|
| **Command parser** | `src/input/parser.rs` | `parse_artisan_command()` |
| **Command multiplexer** (USB+UART) | `src/input/multiplexer.rs` | `ArtisanMultiplexer` |
| **Command channel** | `src/application/mod.rs` | `ARTISAN_CMD_CHANNEL` |
| **Artisan command handlers** | `src/control/handlers/artisan.rs` | `handle_artisan_command()` |
| **Temperature commands** (`SETTARGET`, `PREHEAT`) | `src/control/handlers/temperature.rs` | `handle_set_target()` |
| **System commands** (`START`, `STOP`, `READ`, `STATUS`) | `src/control/handlers/system.rs` | `handle_system_command()` |
| **Safety commands** (`OT`, `RESET`) | `src/control/handlers/safety.rs` | `handle_safety_command()` |
| **Display units (C/F)** | `src/config/constants.rs` | `TemperatureScale::convert_*` |

---

## Output / Telemetry

| Feature | Primary File(s) | Key Types/Functions |
|---------|----------------|---------------------|
| **Artisan formatter** | `src/output/artisan.rs` | `ArtisanFormatter::format_*()` |
| **Output channel** | `src/application/mod.rs` | `OUTPUT_CHANNEL` |
| **Dual output task** | `src/application/tasks.rs` | `dual_output_task()` |
| **Continuous telemetry** | `src/control/abstractions.rs` | `OutputController` |
| **READ response** | `src/output/artisan.rs:230` | `format_read_response()` |
| **STATUS response** | `src/output/artisan.rs:150` | `format_status_response()` |
| **CSV/RoR/Time formatters** | `src/output/formatters/*.rs` | `CsvFormatter`, `RorFormatter` |

---

## Hardware Abstraction

| Feature | Primary File(s) | Key Types/Functions |
|---------|----------------|---------------------|
| **MAX31856 driver** | `src/hardware/max31856.rs` | `Max31856::read_temp()` |
| **Sensor conversion hub** | `src/hardware/sensors/conversion.rs` | `SensorHub::sample_all()` |
| **Shared SPI bus** | `src/hardware/shared_spi.rs` | `SharedSpiBus` |
| **SSR (heater)** | `src/hardware/ssr.rs` | `SsrDriver::set_duty()` |
| **Fan (LEDC PWM)** | `src/hardware/fan.rs` | `FanDriver::set_speed()` |
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
| **RTC watchdog** | `src/safety/watchdog.rs` | `RtcWatchdog::feed()` |
| **Over-temp cutoff** | `src/control/controllers/safety.rs` | `SafetyController::check_overtemp()` |
| **Stale temperature guard** | `src/control/controllers/sensor.rs` | `SensorController::check_stale()` |
| **Heat source detection** | `src/hardware/heat_presence.rs` | `HeatPresenceDetector::check()` |
| **Emergency stop** | `src/control/handlers/safety.rs` | `handle_emergency_stop()` |
| **Regression task** | `src/safety/regression.rs` | `regression_task()` |
| **Safe shutdown (init failure)** | `src/application/app_builder.rs` | `safe_shutdown_init_error()` |

---

## Configuration & Constants

| Feature | Primary File(s) |
|---------|----------------|
| **Pin assignments** | `src/config/constants.rs` → `PinConfig` |
| **PID defaults** | `src/config/constants.rs` → `PidConfig` |
| **Safety thresholds** | `src/config/constants.rs` → `SafetyConfig` |
| **Timing constants** | `src/config/constants.rs` → `TimingConfig` |
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