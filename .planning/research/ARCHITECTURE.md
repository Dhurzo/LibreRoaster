# Architecture Research

**Domain:** Embedded roaster safety control (LEDC + TWDT + regression safeguards)
**Researched:** 2026-02-23
**Confidence:** MEDIUM

## Standard Architecture

### System Overview

```
                                        ┌─────────────────────────┐
                                        │ instrumentation harness │
                                        │ (USB CDC + UART mux)     │
                                        └───────┬─────────────────┘
                                                ↓
        ┌────────────────────────┐    ┌──────────▼──────────┐
        │Async Execution Layer    │    │Safety Utilities     │
        │(embassy executor tasks) │    │ - WatchdogFeeder    │
        │  control_loop_task      │────│ - OverTempTestRunner│
        │  dual_output_task       │    │ - LedcGuardTimeout   │
        └────────────┬───────────┘    └──────────┬──────────┘
                     │                           ↓
              ┌──────▼──────────────────────────────┐
              │ServiceContainer (dual mutex / async)│
              │  - RoasterControl                    │
              │  - Artisan command channels          │
              │  - Watchdog / Safety handles         │
              └──────┬──────────────────────────────┘
                     │
             ┌───────▼────────────┐
             │LEDC subsystem       │
             │(LedcBus + LedcGuard)│
             │  Fan + SSR channels │
             └────────────────────┘
                     │
        ┌────────────▼────────────┐        ┌──────────────────┐
        │ESP32-C3 peripherals     │        │Temperature sensors│
        │ - SSR GPIO (Heat detect)│        │ (Max31856 BT/ET)  │
        │ - TWDT timer            │        └──────────────────┘
        └─────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| `ServiceContainer` (`src/application/service_container.rs`) | Dual mutex for tasks that need `RoasterControl`, artisan channels, and the soon-to-ship watchdog/over-temp handles. | `EmbassyMutex + critical_section` bridging sync/async contexts; new fields store singletons for watchdog feeder and regression runner. |
| `control_loop_task` (`src/application/tasks.rs`) | 100 ms timer loop that pumps sensors, updates control output, sends telemetry, and now feeds TWDT and the regression watchdog. | `embassy_time::Timer::after(Duration::from_millis(100))`, `ServiceContainer::with_roaster_async`, `MutableArtisanFormatter` streaming to `dual_output_task`. |
| `LedcBus` + `LedcGuard` (`src/hardware/ledc_bus.rs`) | Serialize fan/SSR writes, buffer applied duty, and now field a timeout-aware guard so long-running safety tests can force release. | `RefCell` per channel, `AtomicBool` guard, new timeout metadata tracked with `embassy_time::Instant`. |
| `WatchdogFeeder` (`src/safety/watchdog.rs`) **(new)** | Wraps ESP32-C3 `TWDT` peripheral, exposes `feed_async()` for tasks, and keeps a failure flag for telemetry. | Keeps a `esp_hal::twdt::Rwdt` or TWDT handle, stores last feed instant, integrates with `ServiceContainer`. |
| `OverTempTestRunner` (`src/safety/regression.rs`) **(new)** | Runs the regression scenario: escalate heater command, wait for sensor breakpoints, verify `RoasterControl` enters emergency, and publishes telemetry via `dual_output_task`. | Owned by `ServiceContainer`, invoked either at startup or via instrumentation control channel, uses `RoasterControl` to set artificial temperatures. |
| `dual_output_task` (`src/application/tasks.rs`) | Demultiplex telemetry to USB/UART and report watchdog/regression state back to host. | Reads from `ARTISAN_OUTPUT_CHANNEL`, uses `CommChannel` multiplexer, writes via `hardware::usb_cdc::driver` or `hardware::uart::driver`. |

## Recommended Project Structure

```
src/
├── application/
│   ├── app_builder.rs        # initialize peripherals + ServiceContainer
│   └── tasks.rs              # control loop, telemetry, new watchdog trigger points
├── hardware/
│   ├── ledc_bus.rs           # fan/SSR access + timeout-aware guard
│   ├── ledc_guard.rs         # (new) timeout/waiter abstraction shared by bus
│   └── usb_cdc/              # instrumentation harness (existing)
├── safety/
│   ├── watchdog.rs           # (new) TWDT feed + health status
│   └── regression.rs         # (new) over-temp regression/test runner
├── control/
│   └── roaster_refactored.rs # existing logic (updated to expose regression hooks)
└── config/                    # constants (TWDT feed cadence, over-temp thresholds)
```

### Structure Rationale

- **application/** keeps async boot orchestration, tasks, and ServiceContainer wiring in one place so the control loop and watchdog feed share the same `EmbassyMutex` protection.
- **hardware/** isolates low-level peripherals; the new `ledc_guard.rs` makes the timeout logic reusable between fan/SSR handlers and any future LEDC consumers.
- **safety/** groups TWDT and regression work under a safety contract so tests, guards, and telemetry can share configuration constants without polluting `control/`.
- **control/** continues to own PID/SSR semantics while exposing instrumentation hooks (status snapshot, command injection) used by the regression runner.

## Architectural Patterns

### Pattern 1: Guarded hardware access with timeouts

**What:** Wrap LEDC channel writes in a guard token that adds a watchdog timer. The guard token is dropped before any `await`, and a timeout mechanism forces release if a task stalls.

**When to use:** Hardware shares a single peripheral (LEDC) needing mutual exclusion between temperature control and safety tests.

**Trade-offs:** Guarantees no task monopolizes LEDC at the cost of extra bookkeeping and steady-state timer checks; the timeout should be longer than the expected command runtime to avoid false triggers.

**Example:**
```rust
let guard = ledc_bus.guard().acquire();
if guard.wait_timeout(Duration::from_millis(10)).is_err() {
    log::warn!("LEDC guard timeout before applying SSR duty");
}
guard.apply(SSR_DUTY)?;
// guard dropped before awaiting to avoid blocking other LEDC users
```

### Pattern 2: Safety orchestration via shared service container

**What:** Extend `ServiceContainer` to hold handles for the watchdog feeder and regression test runner so all async tasks (control loop + instrumentation) access them through the same locking strategy.

**When to use:** Tasks share critical hardware `RoasterControl` state plus new safety helpers; this keeps ownership consistent and avoids double-locking.

**Trade-offs:** Slightly more complex initialization but preserves the async/sync semantics already enforced by the container.

**Example:**
```rust
ServiceContainer::with_roaster_async(|roaster| {
    let status = roaster.get_status();
    watch_dog.feed(status.bean_temp);
});
WatchdogFeeder::get().feed_async().await?;
```

## Data Flow

### Request Flow

```
[Embassy Timer tick (100 ms)]
    ↓
[control_loop_task] → [ServiceContainer::with_roaster_async]
        → read sensors → update `SystemStatus`
        → compute PID/fan
        → blocking writes via `LedcBus` (+ guard timeout)
        → feed `WatchdogFeeder`
        → optionally start `OverTempTestRunner`
        → emit telemetry to `ARTISAN_OUTPUT_CHANNEL`
            → [dual_output_task] → USB/UART instrumentation
```

### Key Data Flows
1. **Control loop → hardware:** `control_loop_task` sends SSR/fan commands through `LedcBus` using the guard token, while LEDC guard timeouts prevent indefinite blocking. `RoasterControl` updates `SystemStatus` so telemetry stays accurate.
2. **Watchdog feed:** Immediately after `update_control`, the `WatchdogFeeder` gets the latest bean temperature and feeds the ESP32-C3 TWDT; failure to feed sets a status flag and emits an error over telemetry.
3. **Regression test instrumentation:** The regression runner listens for a special command (e.g., from USB CDC mux), ramps heater output/virtual temperature, and ensures `RoasterControl` triggers `emergency_shutdown`; results are pushed through the same telemetry pipeline.

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| Single device (current) | Monolithic control loop + watchdog feed is sufficient. Guard timeouts keep hardware usable despite single executor thread. |
| Fleet deployment (future) | Offload watchdog health reporting over USB/serial telemetry; keep watchdog feed local while telemetry states can be forwarded to an external watchdog router. |
| Safety certification | Regression runner can be run in isolation (via instrumentation command) before deployment to prove over-temp behavior; keep hardware guard/timeouts parameterized. |

### Scaling Priorities

1. **Control integrity:** The 100 ms loop must keep feeding the TWDT and LEDC updates before worrying about telemetry throughput.
2. **Guard resilience:** Timeout guard protects LEDC before instrumentation or watchdog expansion begins.

## Anti-Patterns

### Anti-Pattern 1: Blocking the watchdog feed inside longlasting LEDC operations

**What people do:** Hold the LEDC guard across asynchronous waits or instrumentation callbacks, preventing the 100 ms loop from feeding TWDT.
**Why it's wrong:** Watchdog resets trigger even though control logic is alive, and LEDC bus remains locked. |
**Do this instead:** Release the guard before any `await`, add a timeout in the guard, and keep feed logic inline with the control loop.

### Anti-Pattern 2: Running regression tests directly inside ISR or USB handlers

**What people do:** Kick off the over-temp regression within a USART interrupt or USB callback.
**Why it's wrong:** ISR context cannot safely block on `EmbassyMutex` and may starve the 100 ms loop, causing TWDT resets.
**Do this instead:** Use `ServiceContainer` to signal a spawned `SafetyRegressionRunner` task that runs under the executor and communicates via the existing telemetry channel.

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| ESP32-C3 TWDT | `WatchdogFeeder` (`src/safety/watchdog.rs`) holds the TWDT handle and exposes `feed_async()` | Initialized during `AppBuilder::build()` and woken inside `control_loop_task`. Failure status reported to `dual_output_task`. |
| USB CDC + UART drivers | Telemetry pipeline (`dual_output_task` in `src/application/tasks.rs`) publishes watchdog/regression status via `CommChannel` multiplexing. | Regression runner reuses this channel to report test success/failure. |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| `control_loop_task` ↔ `WatchdogFeeder` | Inline method calls after `update_control` | Must run before `Timer::after(100ms)` to keep TWDT happy. |
| `control_loop_task` ↔ `LedcBus` | `LedcChannelHandle::set_duty` + guard token | Guard timeout lives in `hardware/ledc_guard.rs`; new `apply_without_wait()` method ensures tests can't starve control. |
| `SafetyRegressionRunner` ↔ `RoasterControl` | `ServiceContainer::with_roaster_async` to inject pseudo-high temperatures / emergency triggers | Runner is triggered via instrumentation command or at startup, uses `status` snapshots for verification. |

### Build Order Considerations

1. **LED C guard timeout (modify `src/hardware/ledc_bus.rs` + add `src/hardware/ledc_guard.rs`):** Need timeout guard before regression test uses LEDC to avoid locking starvation.
2. **WatchdogFeeder initialization (`src/safety/watchdog.rs` + wiring in `AppBuilder`):** Must be ready before control loop spawns, so TWDT feed is available on first iteration.
3. **Over-temperature regression runner (`src/safety/regression.rs` + instrumentation hooks):** Depends on guarded LEDC writes and the watchdog feeder, so build last in this set.

## Sources

- Embedded logic from `src/application/tasks.rs`, `src/control/roaster_refactored.rs`, `src/hardware/ledc_bus.rs`.
- Domain constraints described in the milestone context (100 ms loop, ServiceContainer, LEDC bus guard).

---
*Architecture research for: Embedded roaster safety control (TWDT + regression + LEDC guard)*
*Researched: 2026-02-23*
