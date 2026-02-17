# Architecture Research

**Domain:** Embedded firmware reliability for LibreRoaster
**Researched:** 2026-02-17
**Confidence:** MEDIUM

## Standard Architecture

### System Overview

```
┌─────────────────────────────────────────────────────────────┐
│                       Execution Layer                        │
├─────────────────────────────────────────────────────────────┤
│  ┌────────────┐  ┌────────────┐  ┌──────────────┐  ┌──────┐│
│  │ Control    │  │ Command    │  │ Output       │  │ Fans ││
│  │ Tasks      │  │ Multiplexer│  │ Formatter    │  │ /Heats││
│  └────┬───────┘  └────┬───────┘  └────┬────────┘  └──┬───┘│
│       │              │              │              │      │
├───────┴──────────────┴──────────────┴──────────────┴──────┤
│                       Async Runtime Layer                   │
├─────────────────────────────────────────────────────────────┤
│  ┌───────────────────────────┐  ┌────────────────────────┐ │
│  │ Embassy Executor (async) │  │ Shared State Snapshots │ │
│  └───────────────────────────┘  └────────────────────────┘ │
├─────────────────────────────────────────────────────────────┤
│                       Hardware Layer                        │
│  ┌────────────┐  ┌────────────┐  ┌──────────────┐         │
│  │ SSR PWM    │  │ LEDC Timer │  │ USB/UART DMA │         │
│  └────────────┘  └────────────┘  └──────────────┘         │
└─────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| Embassy Executor / CONTROL Tasks | Drive asynchronous sequences (SSR ramp, fan/ heater loops, telemetry) | `embassy::executor::Spawner` launching future-based tasks per controller with soft priorities |
| Command Multiplexer | Buffer and serialize command outputs (USB/UART) | Shared queue + `Arc<Mutex<_>>` guard, sends to formatter trait implementations |
| Formatter + Output Manager | Enforce CRLF, detect blocking I/O, emit status to USB/UART | trait `ArtisanFormatter` with `fmt::Write`-like API wrapping DMA transfers and `FormatterState` |
| FanController / HeaterController | Interface hardware (LEDC, SSR) and publish telemetry snapshots | Controllers hold `Arc<Mutex<ControllerState>>` taking ingress from runtime tasks |
| Shared State Snapshots | Provide consistent views for logging/telemetry without blocking (via `Arc<Mutex<_>>` clones) | Periodic `Arc::clone` with `Mutex` to share sensor/command states safely |
| Hardware Drivers (LEDC, PWM, DMA) | Physical actuation and non-blocking I/O | `esp-hal` peripherals configured via embassy timers and DMA channels |

## Recommended Project Structure

```
src/
├── control/              # CONTROL task definitions and mission logic
│   ├── ssr/              # SSR duty scheduler + reliability guards
│   ├── fan/              # FanController task (LEDC updates)
│   └── io/               # USB/UART output coordination + non-blocking loops
├── state/                # Shared controller snapshots and telemetry buffers
├── hardware/             # HAL wrappers: LEDC, PWM, DMA, analog sensors
├── output/               # Formatter + command multiplexer abstractions
└── main.rs               # Entrypoint wiring embassy executor and controllers
```

### Structure Rationale

- **control/**: keeps hardware orchestration separate per subsystem (SSR, fan, output) so dependencies (timers, DMA) can be mocked or swapped independently.
- **state/**: isolating snapshot management reduces contention and makes it explicit how non-blocking tasks read/write controller state.
- **hardware/**: central place for HAL-specific wiring ensures new reliability fixes (timer precision, LEDC reconfiguration) stay close to peripheral configuration.
- **output/**: keeps USB/UART formatting, multiplexing, and non-blocking DMA loops isolated for easier verification when enforcing CRLF or concurrency guarantees.

## Architectural Patterns

### Pattern 1: Embassy-driven control loops

**What:** Each subsystem (SSR, fan, heater) runs as an async loop spawned by Embassy, scheduling hardware updates while yielding to others through `embassy::time::Timer`.
**When to use:** when the system needs soft-real-time coordination across peripherals without blocking the executor.
**Trade-offs:** non-blocking but still monotonic, requires careful state synchronization via `Arc<Mutex<_>>`.

**Example:**
```rust
embassy::executor::Spawner::new().spawn(async move {
    let mut interval = Timer::interval(Duration::from_millis(50));
    loop {
        interval.next().await;
        let duty = ssr_scheduler.next_duty();
        pwm.set_duty(duty);
        // snapshot is landed for telemetry
    }
})?;
```

### Pattern 2: Command multiplexer + formatter trait

**What:** Encapsulate USB and UART endpoints behind a trait that enforces identical output format (CRLF) and buffers to avoid blocking.
**When to use:** when output channels share logical commands but have different physical constraints.
**Trade-offs:** adds indirection but isolates DMA setup from business logic and allows injecting non-blocking wrappers for reliability fixes.

**Example:**
```rust
trait ArtisanFormatter {
    fn write(&mut self, cmd: &str);
}

struct UsbFormatter { dma: UsbDma, buffer: String }
// ensures CLI command + telemetry happen via DMA transfers when idle
```

### Pattern 3: Snapshot-based shared state

**What:** Controllers expose `Arc<Mutex<ControllerState>>`, tasks clone handles for reads/writes, so output or telemetry tasks operate on consistent views without long-held locks.
**When to use:** when multiple async tasks need to read sensor values or actuator states simultaneously.
**Trade-offs:** adds cloning overhead and stale reads if not refreshed, but avoids deadlocks.

## Data Flow

### Request Flow

```
[Hardware sensor / scheduler input]
    ↓
[Control task] → [State snapshot] → [Hardware driver (SSR/LEDC)]
    ↓                               ↓
[Command multiplexer] ←────────────┘
    ↓
[USB/UART DMA] → user / logging
```

### State Management

```
[ControllerState Arc<Mutex<_>>]
     ↓ (lock per tick, clone handles to tasks)
[Control Tasks (SSR, fan, heater)]
     ↕
[Output Task (formatter handles USB/UART concurrently)]
```

### Key Data Flows

1. **SSR duty control:** CONTROL task updates duty scheduler → persists new duty in shared snapshot → PWM driver consumes duty on timer tick → telemetry watchdog revalidates actual SSR state for reliability.
2. **Fan LEDC updates:** FanController obtains LEDC timer handle from hardware module, applies new brightness while also writing into snapshot so formatter can expose accurate state via logs.
3. **Non-blocking I/O:** Command multiplexer aggregates strings, passes through `ArtisanFormatter` implementations that issue DMA writes; completion futures feed back readiness and optionally unblock CONTROL tasks waiting on ack.

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| 0-1 controllers | Current monolithic async executor with shared state is adequate; focus on reliability fixes rather than partitioning. |
| 2-4 controllers | Ensure additional controllers (fans, heaters) reuse the shared snapshot pattern and keep new tasks low priority to avoid starving telemetry. |
| 5+ controllers | Introduce executor priorities/affinity so reliability-critical loops (SSR duty, I/O) outrank new features; consider offloading telemetry formatting to dedicated low-priority tasks. |

### Scaling Priorities

1. **First bottleneck:** Blocking USB/UART writes — mitigate by confirming DMA-based non-blocking wrappers before adding more telemetry.
2. **Second bottleneck:** SSR duty accuracy under load — guard with watchdog verifying duty vs hardware register after each update.

## Anti-Patterns

### Anti-Pattern 1: Blocking write in CONTROL task

**What people do:** Directly drive `usb.write()` from CONTROL task, waiting for completion before proceeding.
**Why it's wrong:** A stalled USB endpoint blocks the entire executor, sacrificing SSR duty accuracy and fan updates.
**Do this instead:** Push formatted strings into the command multiplexer and use DMA-backed output futures; only yield to CONTROL after non-blocking confirmation.

### Anti-Pattern 2: Updating hardware and state in separate locks

**What people do:** Write to controller state snapshot and hardware driver under different mutexes without order.
**Why it's wrong:** Can lead to telemetry reporting stale SSR duty or LEDC brightness, undermining reliability.
**Do this instead:** Bundle hardware write + snapshot update inside the CONTROL task’s tick loop, holding a single mutex briefly before releasing.

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| USB CDC, UART | DMA-backed non-blocking writer | Formatter enforces CRLF and exposes ready state so CONTROL tasks do not block while command multiplexer drains queues. |
| LEDC/SSR hardware | `esp-hal` PWM via embassy timers | Controllers reconfigure timers through `FanController` and new SSR duty scheduler ensuring accurate duty cycle transitions. |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| Control tasks ↔ Shared snapshots | `Arc<Mutex<ControllerState>>` clones | New reliability fixes keep SSR duty + LEDC settings synchronized before releasing lock. |
| Command multiplexer ↔ Formatter trait | Async queue + DMA futures | Non-blocking I/O changes require the formatter to acknowledge completion before CONTROL tasks assume the message cleared. |
| FanController ↔ LEDC hardware module | Interface exposing `set_brightness(duty)` | LEDC update now includes notification back to the controller state to confirm hardware acknowledgement. |

## New Components Needed

- **SSR Duty Reliability Scheduler:** abstraction that tracks target vs actual duty, exposes a simple API for CONTROL tasks to request updates, and signals when hardware register interplay requires retries.
- **Non-blocking Output Driver:** wraps DMA/USB/UART writes in futures, exposes readiness to the command multiplexer so new hardware reliability fixes never block on slow endpoints.
- **FanController LEDC Monitor:** extends the existing controller to verify LEDC updates succeed (via timer compare match) and publishes fail-safe status to shared snapshots.

## Data Flow Changes

- SSR duty updates now pass through the reliability scheduler before reaching PWM hardware, ensuring the multiplier is clamped and retried if lost.
- LEDC modifications include a confirmation step: after issuing a write, the monitor reads back timer state or uses callback to confirm the duty was accepted before publishing to telemetry.
- Output flow now includes readiness futures so CONTROL tasks can enqueue messages and continue logic while DMA drains them asynchronously.

## Suggested Build Order

1. **Stabilize shared infrastructure:** extend `Arc<Mutex<_>>` snapshots and command multiplexer so they expose hooks for non-blocking drains and reliability status.
2. **SSR reliability layer:** introduce duty scheduler/watcher, update CONTROL task loops to fold in scheduler, and verify telemetry publishes accurate duty.
3. **FanController LEDC updates:** implement monitor, ensure new LEDC APIs feed back into snapshots, and align with PWM hardware wiring.
4. **Non-blocking I/O:** rework formatter implementations to rely on DMA futures, update command multiplexer to await readiness, and ensure CONTROL tasks enqueue rather than blocking.

## Sources

- Project context describing Embassy executor, shared snapshots, and controllers (provided by orchestrator). 
- Inferred constraints from ESP32-C3 + esp-hal usage (no external docs consulted).

---
*Architecture research for: Hardware reliability fixes on LibreRoaster* 
*Researched: 2026-02-17*
