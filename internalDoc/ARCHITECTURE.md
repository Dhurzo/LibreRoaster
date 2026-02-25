# Architecture Guide

**Last Updated:** 2026-02-07 (v2.2)

## System Overview

LibreRoaster is ESP32-C3 firmware for coffee roaster control with **Artisan+ serial protocol compatibility**. The system enables Artisan software to read temperatures and control heater/fan during a roast session.

### Core Capabilities

| Capability | Description |
|------------|-------------|
| Temperature Reading | Dual MAX31856 thermocouple reading (BT + ET) |
| Heater Control | SSR PWM control (0-100%) |
| Fan Control | LEDC PWM fan control (25kHz) |
| Artisan Integration | USB CDC + UART0 dual-channel serial communication |

## Task Structure

### Embassy Async Tasks

| Task | Responsibility | Trigger |
|------|----------------|---------|
| `usb_reader_task` | Read commands from USB CDC | Every 10ms |
| `usb_writer_task` | Write responses to USB CDC | On data available |
| `uart_reader_task` | Read commands from UART0 | Every 10ms |
| `uart_writer_task` | Write responses to UART0 | On data available |
| `dual_output_task` | Route output to active channel | Every 5ms |
| `control_loop_task` | Process commands, update sensors, control outputs | Every 100ms |

### Task Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                    Artisan Software                               │
└───────────────────────────┬─────────────────────────────────────┘
                            │ Serial Commands
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│              usb_reader_task / uart_reader_task                   │
│                    (parse commands)                               │
└───────────────────────────┬─────────────────────────────────────┘
                            │ ArtisanCommand
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│                    control_loop_task                              │
│    ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐   │
│    │ CommandHandler │  │ Temperature   │  │ OutputManager   │   │
│    └──────────────┘  │ Handler       │  └──────────────────┘   │
│                       └──────────────┘                            │
└───────────────────────────┬─────────────────────────────────────┘
                            │ Formatted Output
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│                    dual_output_task                                │
│              (route to active channel)                            │
└───────────────────────────┬─────────────────────────────────────┘
                            │ Serial Response
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│              usb_writer_task / uart_writer_task                   │
│                    (transmit response)                            │
└─────────────────────────────────────────────────────────────────┘
```

## Async Model

LibreRoaster uses **Embassy async** framework with the following characteristics:

### Concurrency Model

- **Single-threaded**: All tasks run on the ESP32-C3 RISC-V core
- **Event-driven**: Tasks wake on async operations (timers, UART RX)
- **Non-blocking**: All I/O operations use async/await

### Critical Sections

```rust
critical_section::with(|cs| {
    // Access shared resources safely
    let value = multiplexer.borrow(cs).borrow_mut();
})
```

### Timing Characteristics

| Operation | Frequency | Period |
|-----------|-----------|--------|
| Control Loop | 10 Hz | 100ms |
| USB/UART Read | 100 Hz | 10ms |
| USB/UART Write | 200 Hz | 5ms |

## Service Container Pattern

The `ServiceContainer` provides dependency injection for shared services:

```rust
ServiceContainer::with_roaster(|roaster| {
    roaster.get_status()
})
```

### Available Services

| Service | Purpose |
|---------|---------|
| `RoasterControl` | Main roaster state machine |
| `ArtisanChannel` | Command queue from Artisan |
| `OutputChannel` | Response queue to Artisan |
| `CommandMultiplexer` | USB/UART channel routing |

## Data Flow

### Command Processing

```
1. Raw bytes arrive via USB CDC or UART0
2. Reader task accumulates until CR (0x0D)
3. process_usb_command_data() parses command
4. Command sent to ArtisanChannel
5. Control loop receives and processes
6. Response formatted and sent to OutputChannel
7. Dual output task routes to active channel
8. Writer task transmits response
```

### Temperature Sampling

```
1. MAX31856 sensors read via SPI
2. TemperatureHandler processes raw readings
3. Status updated with BT, ET values
4. ArtisanFormatter generates CSV response
```

## Code Organization

```
src/
├── application/
│   ├── app_builder.rs      # Service container initialization
│   ├── service_container.rs # Dependency injection
│   └── tasks.rs            # Embassy task definitions
├── hardware/
│   ├── max31856.rs        # Thermocouple driver
│   ├── ssr.rs            # Solid State Relay control
│   └── fan.rs            # Fan PWM control
├── control/
│   ├── roaster_refactored.rs  # State machine
│   └── handlers.rs            # Command handlers
├── input/
│   ├── parser.rs          # Command parsing
│   └── multiplexer.rs     # Channel routing
├── output/
│   └── artisan.rs        # CSV formatting
└── config/
    └── constants.rs       # Pin assignments, limits
```

## Error Handling

### Error Types

| Error | Description | Recovery |
|-------|-------------|----------|
| `ParseError` | Invalid command syntax | Discard, send ERR |
| `RoasterError` | State machine error | Log, continue |
| `HardwareError` | Sensor/actuator fault | Emergency stop |

### Safety Systems

- **Heat source detection**: GPIO1 monitors connected SSR
- **Temperature limits**: Hard limit at 250°C
- **Fault detection**: MAX31856 fault detection (open circuit, short, etc.)

## Command Handler Details

### OT2 Command Flow (Fan Speed Control)

The OT2 command sets fan speed with decimal value support and safety clamping.

**Flow:**
```
Artisan sends: "OT2 75.5"
    ↓
parse_artisan_command() [parser.rs:78-83]
    ↓
parse_ot2_value() [parser.rs:115-131]
    - Parses f32 decimal value
    - Rounds to nearest integer (0.5 rounds up)
    - Clamps to 0-100 range
    - Returns (clamped_value, was_clamped)
    ↓
ArtisanCommand::SetFanSpeed(value, was_clamped)
    ↓
process_artisan_command() [roaster_refactored.rs:374-385]
    ↓
RoasterCommand::SetFanManual(value)
    ↓
apply_manual_fan() [roaster_refactored.rs:203-228]
    - Sets status.fan_output
    - Calls fan.set_speed() hardware trait
    - Sets status.artisan_control = true
    ↓
Fan PWM output updated
```

**Safety Behavior:**
- If `was_clamped` is true (value was out of 0-100 range):
  - Fan is set to clamped value (0 or 100)
  - Heater is immediately stopped (safety measure)
  - Logged: "OT2 out of range - heater stopped, fan set to X%"

**Parser Details:**
- Accepts: "OT2 75", "OT2 75.5", "ot2 50" (case insensitive)
- Decimal rounding: 50.4 → 50, 50.5 → 51
- Clamping: -5 → 0 (was_clamped=true), 150 → 100 (was_clamped=true)

### READ Command Response Format

The READ command returns a 4-value CSV format (changed from 7-value in v2.2):

```
ET,BT,HEATER,FAN
```

**Fields:**
| Position | Field | Source | Description |
|----------|-------|--------|-------------|
| 1 | ET | status.env_temp | Environment temperature (°C) |
| 2 | BT | status.bean_temp | Bean temperature (°C) |
| 3 | HEATER | status.ssr_output | Heater output percentage (0-100) |
| 4 | FAN | status.fan_output | Fan speed percentage (0-100) |

**Implementation:** [artisan.rs:111-119]
```rust
pub fn format_read_response_full(status: &SystemStatus) -> String {
    format!(
        "{:.1},{:.1},{:.1},{:.1}",
        status.env_temp,   // ET
        status.bean_temp,  // BT
        status.ssr_output, // Heater
        status.fan_output  // Fan
    )
}
```

**Note:** ET2, BT2, and ambient temperature fields were removed in v2.2
to match the actual hardware configuration (2 thermocouples only).

### UNITS Command State Management

The UNITS command allows Artisan to specify temperature scale preference.

**Important:** The UNITS command only stores the preference - no actual
temperature conversion is applied. All internal temperatures remain in Celsius.

**Flow:**
```
Artisan sends: "UNITS;C" or "UNITS;F"
    ↓
parse_artisan_command() [parser.rs:46-50]
    - Accepts: C, c, F, f (case insensitive)
    - Returns: ArtisanCommand::Units(is_fahrenheit)
    ↓
process_artisan_command() [roaster_refactored.rs:426-434]
    - Creates TemperatureScale::Celsius or TemperatureScale::Fahrenheit
    - Stores in temp_settings via set_scale()
    - Logs: "Units command received - scale set to X"
```

**Current Behavior:**
- Preference is stored in `RoasterControl.temp_settings`
- No automatic temperature conversion occurs
- All sensor readings and calculations remain in Celsius
- Future enhancement: Apply conversion when formatting output for Artisan

**Parser Details:**
- Syntax: "UNITS;C" or "UNITS;F" (semicolon delimiter)
- Case insensitive: "units;c" and "UNITS;F" both valid
- Error: "UNITS;K" → ParseError::InvalidValue

### Command Handler Chain

**Handler Implementations:**
- `TemperatureCommandHandler`: PID control, temperature setpoints, Start/Stop roast
- `SafetyCommandHandler`: Emergency shutdown, fault conditions
- `ArtisanCommandHandler`: Manual heater/fan override, UP/DOWN adjustments
- `SystemCommandHandler`: Reset functionality

**Direct Handlers (in RoasterControl):**
- `SetFanSpeed` (OT2): Direct fan control with safety clamping [roaster_refactored.rs:374-385]
- `ReadStatus` (READ): Telemetry response formatting [roaster_refactored.rs:404-421]
- `Units` (UNITS): Temperature scale preference storage [roaster_refactored.rs:426-434]
- `SetHeater` (OT1): Manual heater control [roaster_refactored.rs:361-365]
- `IncreaseHeater/DecreaseHeater` (UP/DOWN): Heater adjustment via ArtisanCommandHandler

## Async Task Implementation Details

### Task 1: `control_loop_task` [tasks.rs:12-101]

- **Period:** 100ms (10 Hz) via `Timer::after(Duration::from_millis(100)).await`
- **Responsibilities:**
  - Process Artisan commands from channel
  - Read temperature sensors
  - Update PID control
  - Format and send continuous output when enabled
- **Key Operations:**
  - Calls `roaster.process_artisan_command()` for each command
  - Calls `roaster.read_sensors()` every iteration
  - Calls `roaster.update_control()` for PID/heater updates
  - Formats output via `MutableArtisanFormatter` when continuous output enabled

### Task 2: `dual_output_task` [tasks.rs:120-160]

- **Period:** 5ms (200 Hz) via `Timer::after(Duration::from_millis(5)).await`
- **Responsibilities:**
  - Route output to active communication channel (USB or UART)
  - Add CRLF line endings to output
- **Implementation:**
  - Reads from `output_channel`
  - Checks `CommandMultiplexer` for active channel
  - Appends `"\r\n"` to each message [tasks.rs:133]
  - Writes to USB CDC or UART0 based on active channel
