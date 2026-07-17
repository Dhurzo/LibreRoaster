# LibreRoaster Architecture Guide

**Last updated:** 2026-05-02

This document describes the current firmware architecture of LibreRoaster from the implementation outward. It is written for engineers who need to reason about runtime behavior, ownership boundaries, timing, and the points where protocol handling meets real hardware.

## 1. System model

LibreRoaster is an ESP32-C3 firmware application with a host-testable Rust library around it.

The design splits into two worlds:

- **embedded runtime**: the actual device, built with `--features embedded` for `riscv32imc-unknown-none-elf`
- **host runtime**: x86_64 builds used for parser, control, concurrency, and integration tests

The embedded binary is `no_std`. That shapes almost every design choice: fixed-capacity strings, heapless buffers, explicit channels, and careful control over blocking behavior in the hot path.

## 2. Boot sequence

On the embedded target, startup proceeds in this order:

1. initialize ESP HAL,
2. allocate a 72 KiB heap,
3. initialize logging,
4. initialize LEDC, SPI, GPIO-backed hardware drivers, and sensors,
5. initialize the hardware watchdog,
6. initialize USB CDC,
7. build the application through `AppBuilder`,
8. start `esp-rtos`,
9. start the Embassy executor and spawn the firmware task graph.

The binary also contains a safe-shutdown path for initialization failures. That path formats an Artisan-style error, emits traceability data, steals peripherals to gain LED access, and then blinks GPIO8 indefinitely.

## 3. Application composition

### 3.1 AppBuilder

`AppBuilder` is the construction boundary between raw peripherals and runtime services.

It wires together:

- UART transport,
- heater abstraction,
- fan abstraction,
- sensor conversion hub,
- the formatter,
- the watchdog feeder,
- the shared service container.

The important architectural choice is that the builder creates a `RoasterControl` instance and then injects it into global runtime storage rather than returning a tree of independently owned services. That keeps task spawning simple, but it also means the `ServiceContainer` becomes the central ownership hub.

### 3.2 ServiceContainer

`ServiceContainer` is the process-wide service locator. It owns:

- `roaster`: async-mutex guarded control state,
- `roaster_sync`: sync mirror for critical-section paths,
- `artisan_input`,
- `multiplexer`,
- `watchdog_feeder`,
- shared command and output channels.

This dual-storage model is one of the architecture’s most important constraints. The firmware lazily migrates the control object from sync storage into async storage on startup so async tasks can become the steady-state owners. That keeps compatibility with older sync call sites, but it also creates a subtle “initialization handoff” phase that developers must understand before changing startup behavior.

## 4. Task graph

The embedded system is built around a fixed task graph.

### Input side

The input path was simplified in F5.3: reader tasks now own both byte collection and command parsing directly — the separate queue-processor tasks were removed.

- **USB reader task**: gathers bytes from native USB CDC and parses commands
- **UART reader task**: gathers bytes from UART0 and parses commands
- **control loop task**: drains commands, updates control, emits telemetry

### Core runtime

- **control loop task**: the real application core

Its responsibilities are:

1. drain and process pending commands,
2. update command latency metrics,
3. run sensor acquisition,
4. run control update logic,
5. write actuator state,
6. feed watchdogs,
7. emit live telemetry and instrumentation.

### Output side

- **dual output task**: routes formatted lines to the active communication channel

This keeps formatting and transport output decoupled from the control loop itself.

### Auxiliary runtime

- **regression task**: handles explicit over-temperature regression execution when built for the embedded target

## 5. Command and telemetry data flow

The runtime data path is:

1. Artisan sends text over USB CDC or UART.
2. Reader tasks collect bytes until a command boundary is reached.
3. Queue processors parse text into `ArtisanCommand` values.
4. Parsed commands enter the shared command channel.
5. The control loop drains commands and calls `RoasterControl::process_artisan_command`.
6. Some commands generate immediate formatted output.
7. Sensor and control stages update `SystemStatus`.
8. Formatter code produces `READ`, `STATUS`, acknowledgements, or error lines.
9. Output lines enter the output channel and are emitted by the output task.

Two consequences matter:

- the protocol layer is not transport-owned once parsing finishes,
- and telemetry is emitted from both command-triggered and control-loop-triggered paths.

That is why duplicate-response bugs have historically appeared around `STATUS`: there is more than one place where “response emission” can be accidentally introduced.

## 6. Control core

`RoasterControl` is the domain core. It owns and coordinates:

- sensor sampling,
- actuator control,
- PID state,
- profile following,
- preheat behavior,
- charge detection,
- safety transitions,
- runtime status publication.

The refactoring split responsibilities into controller submodules, but the architectural truth remains the same: `RoasterControl` is the single object where protocol intent becomes hardware behavior.

### State model

The high-level firmware states are:

- `Idle`
- `Preheating`
- `Heating`
- `Stable`
- `Cooling`
- `Fault`
- `EmergencyStop`
- `Error`

These are not UI-only states. They influence how commands are interpreted, whether PID is active, and whether the heater is allowed to drive power.

### Profiles

Temperature and fan profiles are represented as fixed-capacity vectors of setpoints. Both support linear interpolation and a terminal hold at the last point.

That matters for compatibility: LibreRoaster understands profile curves as runtime control inputs, not as Artisan profile files.

## 7. Hardware architecture

### Sensor path

LibreRoaster uses two MAX31856 devices on a shared SPI bus with separate chip selects.

- shared SPI bus
- ET chip select
- BT chip select
- sensor conversion hub above the raw drivers

The sensor layer is intentionally abstracted so host tests can exercise the control stack without real peripherals.

### Actuator path

The device controls:

- **heater** through an SSR-backed LEDC channel,
- **fan** through a separate LEDC-backed PWM path,
- **heat detection** through a GPIO input used as a hardware sanity signal.

The LEDC subsystem is wrapped by a guard layer to catch long or stalled access paths.

### Transport path

LibreRoaster exposes two simultaneous ingress points:

- native USB CDC
- UART0 at 115200 baud

The command multiplexer keeps the shared protocol layer transport-agnostic after ingress.

## 8. Memory and allocation strategy

LibreRoaster is not “heap free,” but it is careful about where allocation is allowed.

### Allowed dynamic behavior

- one-time initialization can allocate
- builder wiring can allocate boxed trait objects
- startup error reporting can allocate formatted strings

### Hot-path constraints

- telemetry formatting uses fixed-capacity heapless strings
- logs and channels have explicit capacities
- profile storage is fixed capacity
- roast history is fixed capacity

This mixed model is intentional: it keeps startup ergonomic without allowing uncontrolled allocation inside the real-time control loop.

## 9. Timing model and real constraints

Several timing constants define the system, but the implementation has important practical pressure points.

### Nominal cadences

- control loop target: 100 ms
- watchdog feed cadence: 100 ms
- output interval: 1000 ms default
- stale-reading timeout: 1000 ms

### Important reality

The thermocouple read path is slower than the nominal PID cadence. The code compensates with stale-data protection and instrumentation, but this means developers must not assume the control loop is operating on fresh sensor data every 100 ms.

That timing mismatch is one of the defining technical constraints of the firmware.

## 10. Safety architecture

LibreRoaster’s safety story is layered, not singular.

### Control and thermal safeguards

- over-temperature shutdown threshold
- stale-temperature invalidation
- emergency stop path
- heat-source detection

### Runtime safeguards

- software watchdog reporting
- hardware RTC watchdog reset
- LEDC timeout guard
- command-rate limiting

### Diagnostic visibility

The `STATUS` line exports watchdog health, guard timeout counts, PID internals, command latency, regression state, and current display scale. The firmware is built so external automation can detect not only roast conditions but also operational degradation.

## 11. Protocol boundary with Artisan

LibreRoaster is compatible with the official Artisan application at the serial-command level, but it is not a full reimplementation of Artisan’s broader ecosystem.

It implements:

- TC4-style command/response flows,
- temperature scale negotiation,
- PID command aliases,
- profile-following commands,
- live telemetry.

It does not natively implement:

- Artisan profile file interchange,
- WebSocket device mode,
- Modbus device mode,
- artisan.plus cloud synchronization,
- the broader vendor-driver matrix that Artisan supports.

That boundary is intentional. LibreRoaster is a device firmware endpoint, not a desktop roasting suite.

## 12. Architectural pressure points

The most important areas to treat carefully during future changes are:

1. **sync/async dual ownership in `ServiceContainer`**
2. **sensor-read duration relative to control cadence**
3. **response emission split across control and output paths**
4. **fixed-capacity output channels and buffers**
5. **transport framing differences between USB CDC and UART**

If a future change affects one of these areas, it is architectural work, not just a local refactor.

## 13. Reading order for new contributors

For a cold technical reader, the best order is:

1. this architecture guide,
2. the protocol reference (`docs/PROTOCOL.md`),
3. the hardware guide (`docs/HARDWARE.md`),
4. the development guide (`docs/DEVELOPMENT.md`),
5. the instrumentation guide (`docs/INSTRUMENTATION.md`),
6. check source code for current bug/risk information (see `docs/CONTROL_BUG_AUDIT.md` if available).
