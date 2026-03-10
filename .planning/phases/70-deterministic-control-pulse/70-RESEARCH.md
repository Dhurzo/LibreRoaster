# Phase 70: Deterministic Control Pulse - Research

**Researched:** 2026-02-24
**Domain:** 100 ms deterministic PID control loop on embedded LibreRoaster
**Confidence:** MEDIUM

## Summary

Phase 70 must lock down the 100 ms tick (CONTROL-01) so every automation-visible action—sensor sample, PID update, LEDC write, watchdog feed, STATUS telemetry—happens before the next timer event. I traced the current `control_loop_task` through `ServiceContainer`, checked how `SystemStatus` and `ArtisanFormatter` serialize telemetry, and verified the watchdog, LEDC guard, and regression hooks already expose status flags the loop intends to reuse rather than replace.

The existing loop already orchestrates sensor reads, control updates, LEDC guard tracking, and telemetry emission inside a single `#[task]` anchored by `embassy_time::Timer::after(100ms)`. The biggest gap for planning is extending `SystemStatus`/`StatusResponse` to carry PV/MV/integrator/derivative/saturation indicators while preserving the deterministic column layout automations expect (per Decisions 69 and requirements TELE-01/TELE-02).

- Verified the per-tick choreography in `src/application/tasks.rs`: commands → async sensor read → control update → watchdog/guard instrumentation → continuous telemetry → Timer wait.
- Documented the current `SystemStatus` snapshot, the fields already wired into the `artisan` protocol, and where new integral/derivative columns must appear without shifting automation parsing.
- Confirmed `WatchdogFeeder`, `LedcGuard`, and `OverTemp` regression hooks in `ServiceContainer` are the instrumentation contracts that Phase 70 must reuse rather than re-creating.

**Primary recommendation:** Treat `control_loop_task` as the single authoritative pulse and expand its `SystemStatus` telemetry so each 100 ms iteration records PV/MV/integrator/derivative/saturation flags before `Timer::after(100)` resumes the next tick.

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `embassy-executor` | 0.9.1 | Drives `#[task]` futures such as `control_loop_task`, keeps the single-threaded executor deterministic | Every control/telemetry task in `src/application/tasks.rs` depends on this executor; splitting into additional runtimes would break the 100 ms cadence.
| `embassy-time` | 0.5.0 | Provides `Timer::after`, `Instant`, and `Duration`, so the loop can await exactly 100 ms before the next cycle | The loop’s heartbeat and watchdog time checks already rely on these primitives (see `control_loop_task` and `WatchdogFeeder`).
| `embassy-sync` | 0.6.1 | Adds `Channel`/mutex primitives for Artisan command and output flows that communicate with the loop | The deterministic sequence (command queue → status response → telemetry send) would be harder to guarantee without these synchronous channels.

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `esp-hal` | ~1.0 | Hardware access for heaters, fans, LEDC timers, and the GPIOs the loop controls | Embedded target only; required by `hardware::fan.rs`, `hardware::ssr.rs`, and `hardware::ledc_guard.rs` that the loop calls each tick.
| `portable-atomic` | 1.13 | Implements `LedcGuard`’s cross-target atomic lock and timeout counter | Needed whenever the loop timestamps guard timeouts (Section `status.ledc_guard_timeouts`) to keep instrumentation deterministic.
| `heapless` | 0.9.2 | Heapless `String` buffers used by the dual-output channel and `ArtisanFormatter` | Telemetry outputs and safety warnings (watchdog/LEDG timeouts) are sent through heapless strings so the loop can stay within deterministic RAM bounds.

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| A scattered set of sensor/PID/watchdog tasks | Brokered micro-tasks that race on separate `Timer::after` invocations | Losing the single 100 ms guard makes ordering unclear and lets the watchdog feed slip outside the documented instrumentation path, so automation cannot trust STATUS snapshots to cover the same cycle.
| Manual `esp_task_wdt_reset` calls sprinkled through handlers | Central `WatchdogFeeder` invoked through `ServiceContainer::with_watchdog` | Custom feeds would duplicate failure tracking and skip the telemetry fields (`watchdog_feed_ok`, `watchdog_last_failure`, `watchdog_consecutive_failures`) that automation relies on for CONTROL-01 diagnostics.

**Installation:**
```bash
cargo build --features embedded
```

## Architecture Patterns

### Recommended Project Structure
```
src/
├── application/     # single heartbeat tasks, command/output channels, ServiceContainer orchestration
├── control/         # PID handlers, output routing, status bookkeeping
├── hardware/        # LEDC guard, watchdog wrappers, MAX31856 helpers, fans/heaters
└── safety/          # regression harnesses and high-level failure instrumentation
```

### Pattern 1: Single deterministic heartbeat loop
**What:** `control_loop_task` (in `src/application/tasks.rs`) serializes each tick: drain Artisan commands, await the async sensor read, update PID/outputs, feed the watchdog, back-fill STATUS fields, emit telemetry if telemetry is active, then await `Timer::after(Duration::from_millis(100))`.
**When to use:** Every control change or telemetry snapshot that must be tied to the same 100 ms boundary (requirements CONTROL-01, TELE-01/TELE-02).
**Example:**
```rust
// Source: src/application/tasks.rs
loop {
    let current_time = Instant::now();
    // 1. Drain Artisan commands and service-mode instrumentation
    while let Ok(command) = cmd_channel.try_receive() { ... }

    // 2. Async MAX31856 reads via the ServiceContainer helper
    let sensor_err = ServiceContainer::roaster_async_sensor_read().await.err();

    // 3. Synchronously update control and record LEDC guard/watchdog instrumentation
    let _ = ServiceContainer::with_roaster_async(|roaster| {
        if let Ok(output) = roaster.update_control(current_time) { ... }
        let status = roaster.status_mut();
        // feed watchdog and publish guard timeouts
        match ServiceContainer::get_instance().with_watchdog(|watchdog| watchdog.feed_async(status.bean_temp)) { ... }
    });

    // 4. Emit Artisan telemetry before sleeping for the next 100 ms boundary
    if is_continuous_now { ... }

    Timer::after(Duration::from_millis(100)).await;
}
```

### Anti-Patterns to Avoid
- **Multiple overlapping loops for sensors, PID, and telemetry:** They make it impossible to guarantee telemetry spans the same tick that actuators saw. Always keep those steps inside `control_loop_task` and guard with the 100 ms `Timer`.
- **Manual watchdog feeds outside `ServiceContainer::with_watchdog`:** Doing so duplicates failure tracking and risks missing `_watchdog_last_failure` updates that automation relies on when telemetry flags a fault. Always feed through the shared `WatchdogFeeder`.

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Precise 100 ms wait/schedule | Busy-wait loops with `Instant::now` | `embassy_time::Timer::after(Duration::from_millis(100))` | `Timer::after` integrates with the `embassy` executor so every tick yields, keeping the watchdog and telemetry synchronized without burning CPU.
| Watchdog instrumentation that surfaces failures | Scattered direct `esp_task_wdt_reset` calls | `WatchdogFeeder` via `ServiceContainer::with_watchdog` | Centralizes failure reasons, consecutive counts, and telemetry fields (`watchdog_feed_ok`, `watchdog_last_failure`, `watchdog_consecutive_failures`).
| LEDC guard tracking state | Custom mutex/arbitration around LEDC updates | `hardware::ledc_guard::LedcGuard` | Already handles spin-loop acquisition, counts timeouts, and exposes a deterministic timeout counter that telemetry already reports.

**Key insight:** Reuse the existing executor-friendly `Timer`, `WatchdogFeeder`, and `LedcGuard` hooks so the loop stays deterministic; custom timers or guard state will drift and break telemetry expectations.

## Common Pitfalls

### Pitfall 1: Drifted 100 ms boundary because async work overran the timer
**What goes wrong:** Sensor reads, PID updates, and telemetry formatting all run inside the loop, so if one task blocks for >100 ms the watchdog feed and telemetry emission are delayed by the same amount.
**Why it happens:** The loop waits for `service_container::roaster_async_sensor_read().await` before continuing; if the MAX31856 read or PID update is slow, the subsequent watchdog feed and telemetry slip past the documented deadline.
**How to avoid:** Keep the per-tick work bounded, monitor durations with `Instant::now()` (already available in `control_loop_task`), and ensure the watchdog feed happens right after `update_control` before telemetry; if a new component is added, measure and log inside the same loop iteration before `Timer::after`.
**Warning signs:** `status.watchdog_feed_ok` toggles to `false`, `status.ledc_guard_timeouts` increments, or logs show `SAFETY WATCHDOG` messages before the 100 ms marker.

### Pitfall 2: Telemetry columns drift and break automation
**What goes wrong:** Adding PV/MV/integrator/derivative flags without locking their position shifts the rest of the `STATUS` CSV, which automation scripts already parse deterministically (Decision 69).
**Why it happens:** `ArtisanFormatter::format_status_response` currently emits ET, BT, heater, fan, watchdog/state, guard timeouts, regression flag; inserting new fields in the wrong spot corrupts older automation that expects 9 columns.
**How to avoid:** Extend `SystemStatus` with the new fields and append them consistently (e.g., after the existing actuator/state flags) while documenting the column order in the telemetry schema; treat the new fields as the tail so earlier columns remain stable.
**Warning signs:** Artisan automation starts logging parse errors, or STATUS commands suddenly have more/less than 9 comma-separated values when new instrumentation ships.

## Code Examples

Verified patterns from official sources:

### Control Loop Sequence
```rust
// Source: src/application/tasks.rs
let sensor_err = ServiceContainer::roaster_async_sensor_read().await.err();
let _update_result = ServiceContainer::with_roaster_async(|roaster| {
    match roaster.update_control(current_time) { ... }
    let status = roaster.status_mut();
    match ServiceContainer::get_instance().with_watchdog(|watchdog| watchdog.feed_async(status.bean_temp)) {
        Ok(_) => {
            status.watchdog_feed_ok = true;
            status.watchdog_last_failure = None;
            status.watchdog_consecutive_failures = 0;
        }
        Err(ContainerError::Watchdog(err)) => { ... }
        Err(ContainerError::WatchdogUninitialized) => { ... }
        Err(err) => warn!("Watchdog container error: {:?}", err),
    }
    status.ledc_guard_timeouts = ledc_guard::total_timeouts();
    Ok(())
});
Timer::after(Duration::from_millis(100)).await;
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Synchronous `read_sensors_sync` and ad‑hoc control updates | Asynchronous `read_sensors()` plus `roaster_async_sensor_read()` inside `control_loop_task`, with one loop servicing sensors, PID, watchdog, and telemetry | Early 2026 (v4.2 planning) | Prevents blocking operations from forcing multiple timers and keeps STATUS snapshots aligned with the actual actuator cycle.

**Deprecated/outdated:**
- `read_sensors_sync` – kept for host targets but no longer drives the real-time loop; rely on the async path so the 100 ms pulse is non-blocking.

## Open Questions

1. **What deterministic column ordering should PV/MV/integrator/derivative/saturation follow in STATUS telemetry?**
   - What we know: Existing STATUS response uses 9 comma-separated columns (ET, BT, heater, fan, watchdog flag, failure count, failure reason, guard timeouts, regression flag).
   - What’s unclear: Where exactly to insert the new five control terms so downstream automation parsing continues to work (especially anything that assumes column counts).
   - Recommendation: Append the new instrumentation fields at the tail of `format_status_response` and freeze the layout, documenting the exact schema in Phase 70 plans before implementation.

## Sources

### Primary (HIGH confidence)
- `Cargo.toml` – current dependency versions (`embassy-time`, `embassy-executor`, `embassy-sync`, `esp-hal`).
- `src/application/tasks.rs` – single 100 ms loop orchestrating commands, sensors, control updates, watchdog feeds, telemetry output, and the `Timer::after` cadence.
- `src/config/constants.rs` – defines `WATCHDOG_FEED_INTERVAL_MS`, `LEDC_GUARD_TIMEOUT_MS`, and `PID_SAMPLE_TIME_MS` that the loop must honor.
- `src/hardware/ledc_guard.rs` – existing LEDC guard instrumentation (timeout counter, guard token) that STATUS already exposes.
- `src/safety/watchdog.rs` – `WatchdogFeeder` wrapper used by the loop to feed the esp_task watchdog and record failure reasons for STATUS.
- `src/output/artisan.rs` – `ArtisanFormatter::format_status_response` and its deterministic CSV layout that automation parses.

### Secondary (MEDIUM confidence)
- `.planning/REQUIREMENTS.md` – CONTROL-01, TELE-01, TELE-02 define the phase’s responsibilities.
- `.planning/ROADMAP.md` – emphasizes enforcing the 100 ms pulse and STATUS telemetry requirements for v4.2.

### Tertiary (LOW confidence)
- None.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH – direct versions from `Cargo.toml`.
- Architecture: MEDIUM – inferred from `control_loop_task` requirements and the existing executor configuration.
- Pitfalls: MEDIUM – reasoned from the loop’s scheduling structure and telemetry exposure but not run-time validated.

**Research date:** 2026-02-24
**Valid until:** 2026-03-26
