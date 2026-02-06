# Architecture

**Analysis Date:** 2026-02-04

## Pattern Overview

**Overall:** Layered Architecture with Dependency Injection and Command Handler Pattern

This is an embedded Rust application for an ESP32-C3 coffee roaster controller. The architecture follows several key patterns:

- **Dependency Injection via Builder Pattern**: Hardware dependencies are injected through `AppBuilder`
- **Command Handler Pattern**: Commands are processed by specialized handlers (`RoasterCommandHandler` trait)
- **Service Container Pattern**: Global singleton for accessing core services
- **Async Task-Based Concurrency**: Uses Embassy RTOS for concurrent operations
- **Hardware Abstraction Layer**: Traits (`Thermometer`, `Heater`, `Fan`) abstract hardware details

**Key Characteristics:**
1. Clear separation between application logic and hardware specifics
2. Dynamic dispatch for hardware drivers (enables mocking/testing)
3. Non-blocking async operations throughout
4. Safety-first design with emergency shutdown capabilities

## Layers

### Application Layer

**Purpose:** Orchestrates the entire roaster system, manages tasks, and coordinates services

**Location:** `src/application/`

**Contains:**
- `app_builder.rs`: Builder for constructing the application with dependencies
- `service_container.rs`: Global singleton for accessing core services
- `tasks.rs`: Async task definitions (control loop, Artisan output)

**Depends on:**
- Control layer (`RoasterControl`)
- Hardware layer (UART, sensors)
- Input layer (ArtisanInput)
- Output layer (ArtisanFormatter)

**Used by:** `src/main.rs` calls `AppBuilder` to construct and start the application

```rust
// Main entry point flow:
AppBuilder::new()
    .with_uart(peripherals.UART0)
    .with_real_ssr(static_ssr)
    .with_fan_control(static_fan)
    .with_temperature_sensors(bean_sensor, env_sensor)
    .with_formatter(ArtisanFormatter::new())
    .build()
    .start_tasks(spawner)
```

### Control Layer

**Purpose:** Core roasting logic, PID control, command processing, safety enforcement

**Location:** `src/control/`

**Contains:**
- `roaster_refactored.rs`: Main `RoasterControl` struct orchestrating all control logic
- `handlers.rs`: Command handlers (Temperature, Safety, Artisan, System)
- `pid.rs`: PID controller implementation
- `abstractions.rs`: Trait definitions (`PidController`, `OutputManager`)
- `traits.rs`: Hardware traits (`Thermometer`, `Heater`, `Fan`)
- `command_handler.rs`: `RoasterCommandHandler` trait

**Depends on:**
- Config layer (constants, state definitions)
- Hardware layer (for hardware operations)
- Output layer (for status formatting)

**Used by:** Application layer via `ServiceContainer`

### Hardware Layer

**Purpose:** Hardware-specific implementations for sensors, actuators, and communication

**Location:** `src/hardware/`

**Contains:**
- `max31856.rs`: Thermocouple sensor driver (Bean Temp, Env Temp)
- `fan.rs`: PWM fan controller (`FanController`, `SimpleLedcFan`)
- `ssr.rs`: Solid State Relay control (`SsrControlSimple`)
- `uart/`: UART communication driver and tasks
- `shared_spi.rs`: Shared SPI bus with chip select management

**Depends on:** ESP-HAL (ESP32 Hardware Abstraction Library)

**Used by:** Control layer via hardware traits

### Input Layer

**Purpose:** Parse incoming Artisan+ commands from serial

**Location:** `src/input/`

**Contains:**
- `parser.rs`: Artisan protocol command parsing

**Depends on:** Hardware UART layer

**Used by:** Application tasks for command ingestion

### Output Layer

**Purpose:** Format and send status data to Artisan software

**Location:** `src/output/`

**Contains:**
- `artisan.rs`: `ArtisanFormatter` and `MutableArtisanFormatter` for CSV protocol
- `manager.rs`: `OutputManager` for processing status
- `uart.rs`: UART output driver
- `scheduler.rs`: Output scheduling

**Depends on:** Config layer (SystemStatus)

**Used by:** Control layer via `TemperatureCommandHandler`

### Config Layer

**Purpose:** Constants, state definitions, type definitions

**Location:** `src/config/`

**Contains:**
- `constants.rs`: All magic numbers and type definitions
- `RoasterState`, `ArtisanCommand`, `RoasterCommand`, `SystemStatus`

**Depends on:** None (pure data/types)

**Used by:** All layers

### Error Layer

**Purpose:** Centralized error types

**Location:** `src/error/`

**Contains:**
- `app_error.rs`: Application-level errors

**Depends on:** None

## Data Flow

### Command Processing Flow (Artisan → Roaster)

```
┌─────────────────────────────────────────────────────────────┐
│ UART Serial Input                                          │
└─────────────────────┬─────────────────────────────────────┘
                      ▼
┌─────────────────────────────────────────────────────────────┐
│ uart_reader_task (embassy task)                            │
│ - Reads bytes from UART0                                    │
│ - Parses Artisan commands                                   │
└─────────────────────┬─────────────────────────────────────┘
                      ▼
┌─────────────────────────────────────────────────────────────┐
│ COMMAND_PIPE (embassy_sync::Pipe)                          │
└─────────────────────┬─────────────────────────────────────┘
                      ▼
┌─────────────────────────────────────────────────────────────┐
│ control_loop_task                                          │
│ - Receives ArtisanCommand from channel                      │
│ - Calls RoasterControl.process_artisan_command()            │
└─────────────────────┬─────────────────────────────────────┘
                      ▼
┌─────────────────────────────────────────────────────────────┐
│ RoasterControl                                              │
│ - Routes command to handlers:                               │
│   → SafetyCommandHandler (emergency checks)                │
│   → TemperatureCommandHandler (PID/temp)                    │
│   → ArtisanCommandHandler (manual control)                 │
│   → SystemCommandHandler (reset)                           │
│ - Updates SystemStatus                                      │
└─────────────────────┬─────────────────────────────────────┘
                      ▼
┌─────────────────────────────────────────────────────────────┐
│ Hardware (via traits)                                       │
│ - Heater.set_power() → SSR PWM                              │
│ - Fan.set_speed() → Fan PWM                                 │
│ - Thermometer.read_temperature() → MAX31856                 │
└─────────────────────────────────────────────────────────────┘
```

### Status Reporting Flow (Roaster → Artisan)

```
┌─────────────────────────────────────────────────────────────┐
│ RoasterControl.update_control()                             │
│ - Reads sensors via Thermometer trait                       │
│ - Computes PID output if enabled                            │
│ - Applies to hardware via Heater/Fan traits                │
└─────────────────────┬─────────────────────────────────────┘
                      ▼
┌─────────────────────────────────────────────────────────────┐
│ SystemStatus (in-memory state)                             │
│ - bean_temp, env_temp, ssr_output, fan_output               │
└─────────────────────┬─────────────────────────────────────┘
                      ▼
┌─────────────────────────────────────────────────────────────┐
│ ArtisanFormatter.format()                                    │
│ - Converts status to Artisan CSV format                     │
│ - time,ET,BT,ROR,Gas                                        │
└─────────────────────┬─────────────────────────────────────┘
                      ▼
┌─────────────────────────────────────────────────────────────┐
│ OUTPUT_CHANNEL (embassy_sync::Channel)                      │
└─────────────────────┬─────────────────────────────────────┘
                      ▼
┌─────────────────────────────────────────────────────────────┐
│ artisan_output_task (embassy task)                          │
│ - Sends formatted line via UART                              │
└─────────────────────────────────────────────────────────────┘
```

**State Management:**
- `SystemStatus`: Central struct holding all runtime state
- `RoasterControl`: Owns status, handlers, and hardware references
- `ServiceContainer`: Global singleton providing access to `RoasterControl`

## Key Abstractions

### Hardware Traits

**Purpose:** Enable dependency injection and testing

**Location:** `src/control/traits.rs`

```rust
pub trait Thermometer: Send {
    fn read_temperature(&mut self) -> Result<f32, RoasterError>;
}

pub trait Heater: Send {
    fn set_power(&mut self, duty: f32) -> Result<(), RoasterError>;
    fn get_status(&self) -> SsrHardwareStatus;
}

pub trait Fan: Send {
    fn set_speed(&mut self, duty: f32) -> Result<(), RoasterError>;
}
```

**Examples:**
- `Thermometer`: `Max31856` (real sensor), mock implementations for testing
- `Heater`: `SsrControlSimple` (real SSR), mock for testing
- `Fan`: `FanController`, `SimpleLedcFan` (both real implementations)

### Command Handler Trait

**Purpose:** Decouple command processing from routing logic

**Location:** `src/control/command_handler.rs`

```rust
pub trait RoasterCommandHandler {
    fn handle_command(
        &mut self,
        command: RoasterCommand,
        current_time: Instant,
        status: &mut SystemStatus,
    ) -> Result<(), RoasterError>;

    fn can_handle(&self, command: RoasterCommand) -> bool;
}
```

**Implementations in `handlers.rs`:**
- `TemperatureCommandHandler`: PID control, temperature setpoints
- `SafetyCommandHandler`: Emergency shutdown, fault conditions
- `ArtisanCommandHandler`: Manual heater/fan override
- `SystemCommandHandler`: Reset functionality

### Output Formatter Trait

**Purpose:** Abstract output formatting for different protocols

**Location:** `src/output/traits.rs`

**Implementations:**
- `ArtisanFormatter`: Artisan+ CSV protocol

## Entry Points

### Application Entry Point

**Location:** `src/main.rs`

**Responsibilities:**
1. Initialize ESP-HAL peripherals
2. Configure GPIO pins (SSR, Fan, Sensors, Heat Detection)
3. Initialize SPI and temperature sensors (MAX31856)
4. Build application via `AppBuilder`
5. Start async tasks via Embassy spawner

**Flow:**
```rust
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // Hardware init
    let peripherals = esp_hal::init(config);
    
    // Sensor init
    let bean_sensor = Max31856::new(bt_spi)?;
    let env_sensor = Max31856::new(et_spi)?;
    
    // Driver init
    let real_ssr = SsrControlSimple::new(heat_detection_pin, ssr_channel)?;
    let fan_impl = SimpleLedcFan::new(fan_channel);
    
    // App build and start
    let app = AppBuilder::new()
        .with_uart(peripherals.UART0)
        .with_real_ssr(static_ssr)
        .with_fan_control(static_fan)
        .with_temperature_sensors(bean_sensor, env_sensor)
        .with_formatter(ArtisanFormatter::new())
        .build()?;

    app.start_tasks(spawner).await
}
```

### Async Tasks

**Location:** `src/application/tasks.rs`

**Task 1: `uart_reader_task`**
- Spawned by `uart_reader_task()` in `src/hardware/uart/tasks.rs`
- Reads serial input, parses Artisan commands, sends to command channel

**Task 2: `uart_writer_task`**
- Spawned by `uart_writer_task()` in `src/hardware/uart/tasks.rs`
- Sends responses back to Artisan

**Task 3: `control_loop_task`**
- Main control loop (100ms period)
- Processes commands, reads sensors, updates PID, writes output

**Task 4: `artisan_output_task`**
- Sends formatted status lines to UART

## Error Handling

**Strategy:** Result-based error propagation with centralized error enum

**Pattern:**
- Each layer defines its own error types
- Errors propagate up via `?` operator
- Critical errors trigger emergency shutdown

```rust
// Central error type in control layer
pub enum RoasterError {
    TemperatureOutOfRange,
    SensorFault,
    InvalidState,
    PidError,
    HardwareError,
    EmergencyShutdown,
}

// Safety enforcement
if self.status.bean_temp >= OVERTEMP_THRESHOLD {
    self.emergency_shutdown("Over-temperature detected")?;
}
```

**Emergency Shutdown:**
- Cuts power to heater via SSR
- Enables fan at 100% for cooling
- Sets system state to `Error`
- Returns `RoasterError::EmergencyShutdown`

## Cross-Cutting Concerns

**Logging:** Uses `log` crate with `esp-println` for ESP32 output

**Safety:**
- Temperature sensor timeout detection (1s validity window)
- Over-temperature threshold (260°C)
- Heat source detection (SSR feedback)

**Concurrency:**
- Embassy executor for async/await
- Critical section mutex for shared state
- Channel-based communication between tasks

---

*Architecture analysis: 2026-02-04*
