# ⚙️ Runtime Architecture

**Last updated:** 2026-10-22

This page is the README-level runtime overview. [`ARCHITECTURE.md`](ARCHITECTURE.md) owns the full
system model, task graph, ownership boundaries, and timing; [`CONTROL_LOOP.md`](CONTROL_LOOP.md)
owns tick-level behavior; [`CONTROL_CORE.md`](CONTROL_CORE.md) owns the normative control rules.

## Boot and composition

On the embedded target, `src/main.rs` initializes the HAL, heap, logging, LEDC, SPI, GPIO drivers,
sensors, watchdogs, USB CDC, and UART, then builds the application through `AppBuilder`. After that
it starts `esp-rtos` and the Embassy executor.

`AppBuilder` constructs `RoasterControl` and injects it into `ServiceContainer`, the process-wide
service locator. The container owns the async-mutex-guarded `RoasterControl`, the Artisan input,
and the watchdog feeder. The command channel, output channel, and command multiplexer are
module-level statics exposed through accessors.

`RoasterControl` coordinates four focused controllers:

- **SensorController** — sampling, validation, EMA filtering, and per-channel fault debounce
- **ActuatorController** — heater and fan actuation together, including slew-rate limiting and the
  heat-source cross-check
- **SafetyController** — emergency flags and safety policy evaluation
- **CommandDispatcher** — command routing

## Task topology

The runtime starts one `async_main_task` supervisor plus five long-lived Embassy tasks:

- **USB reader** — collects native USB CDC bytes and parses commands
- **UART reader** — collects UART0 bytes and parses commands
- **Control loop** — drains commands, reads sensors, updates control, feeds watchdogs, and emits
  telemetry
- **Dual output** — routes formatted output to the active transport
- **Regression** — runs explicit over-temperature regression checks on embedded builds with the
  `regression` feature; otherwise it is a no-op task

The readers feed one shared command channel. Each transport has a byte-level event queue; there is
no intermediate queue-processor task. The control loop is the application core.

## Control loop

The loop runs on a 100 ms timer, but the real tick is approximately 310–330 ms because each tick
waits for the MAX31856 one-shot conversion (210 ms). A tick runs these stages in order:

1. drain commands from the shared channel;
2. read both thermocouple channels;
3. update control (safety, PID or manual output, profiles, charge detection);
4. write actuator state through the guarded LEDC path and log guard timeouts;
5. feed the software and hardware watchdogs;
6. emit telemetry and drain the `#DUMP` ring buffer;
7. update the status LED and finalize the tick.

The loop also records command latency, guard state, watchdog health, and PID internals so runtime
health is observable, not just roast temperature.

## Command and telemetry flow

Artisan sends text over USB CDC or UART0. A reader task frames and parses the command, then places
it on the shared command channel. The control loop drains the command, calls
`RoasterControl::process_artisan_command`, and routes any immediate response through the output
channel. Sensor and control updates publish `SystemStatus`; the formatter produces `READ`, `STATUS`,
acknowledgements, or error lines. The dual-output task writes them to the active transport.

Design boundaries worth keeping in mind:

- no persistence layer: roast state, telemetry, and profiles are RAM-only;
- the command channel is intentionally small and uses `try_send` backpressure only;
- the host/embedded split keeps parser, control, and integration tests runnable on x86_64;
- safety gates are independent backstops, not substitutes for safe hardware.

For the stage-by-stage control rules, continue with [`CONTROL_LOOP.md`](CONTROL_LOOP.md). For the
boot sequence, memory model, and state ownership, continue with [`ARCHITECTURE.md`](ARCHITECTURE.md).
