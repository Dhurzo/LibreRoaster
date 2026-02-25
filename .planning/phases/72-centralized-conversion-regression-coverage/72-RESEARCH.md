# Phase 72: Centralized conversion & regression coverage - Research

**Researched:** 2026-02-24
**Domain:** Embedded MAX31856 conversion + deterministic regression instrumentation
**Confidence:** MEDIUM

## Summary

Phase 72 must centralize every MAX31856 read behind a single helper so control, telemetry, regression runs, and the deterministic STATUS tail share the same scaling, two's-complement math, and per-sensor fault mapping. I mapped the current `hardware::max31856` driver, `ServiceContainer::roaster_async_sensor_read`, `control_loop_task` instrumentation, and the STATUS formatter to understand what an automated regression harness would have to match.

The standard approach is a `SensorConversionHub` (a new `src/hardware/sensors/conversion.rs` module) that owns retries, faults, offsets, and exposes host hooks for deterministic tests. The control loop will call that helper through `ServiceContainer` so the `StageTracker`, watchdog, LEDC guard, and telemetry all operate on the same calibrated beans/environment temperatures.

Regression coverage comes from two places: a feature-flagged harness that replays ADC/register sequences through the shared helper, and unit/regression tests (using `embedded-hal-mock`) that assert the 0.0078125 °C LSB, two's-complement conversion, rounding, and fault reporting. The Artisan STATUS/READ helpers already include deterministic tests, so the conversion helper must fail the same way the telemetry would if the math drifted.

- Centralize conversion and offsets inside a `SensorConversionHub` so `read_sensors`, telemetry, and the regression harness all share the same math, caching, and fault visibility.
- Anchor the control loop instrumentation (watchdog, LEDC guard, `StageTracker`, `SystemStatus` tail, and `ArtisanFormatter`) around the shared conversion path so 100 ms snapshots remain comparable to regression outputs.
- Treat regression harness runs as feature-flagged replays of ADC and offset sequences, with `embedded-hal-mock` driving the MAX31856 transactions and STATUS artifacts recorded for automation.

**Primary recommendation:** Implement a `SensorConversionHub` that exports the shared MAX31856 conversion/rescaling/fault logic (with host-friendly fixtures) and make every control loop, telemetry emitter, and regression harness call it so STATUS snapshots remain deterministic and regression samples match production math.

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `embedded-hal` | 1.0.0 | Defines the async/sync SPI traits used by MAX31856 and the regression helper so both targets and host fixtures share a single trait set. | Keeps the conversion helper portable (device + host) and matches the `embedded-hal-mock` version used in the regression suite.
| `esp-hal` | ~1.0 | Drives the SPI peripheral, TIMG `PeriodicTimer`, and board GPIO/LEDC used by the MAX31856 driver and LEDC guard. | The embedded executable already uses the same crate; reusing it preserves deterministic timing anchors and hardware access.
| `embassy-time` | 0.5.0 | Provides `Timer::after` delays for the MAX31856 conversion, watchdog feed intervals, and stage timer instrumentation without blocking the executor. | Embeds the existing 100 ms loop alignment and conversion cadence, so new helpers remain compatible with the current async executor.

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `embedded-hal-mock` | 0.11.1 | Scripts deterministic SPI transactions (ADC, fault registers, offsets) for regression/unit tests. | Use for host regression harnesses, conversion math tests, and failure replays instead of hitting real hardware.
| `heapless` | 0.9.2 | Constructs bounded strings used by `ArtisanFormatter`/status logging so telemetry always emits the same CSV tail. | Use when formatting STATUS/READ responses or regression payloads to avoid heap/non-determinism.
| `embassy-sync` | 0.6.1 | Offers the `ServiceContainer` async/sync mutexes that guard the control loop, regression task, and instrumentation. | Needed whenever regression runs or telemetry writes share `RoasterControl` state without deadlocking.
| `libm` | 0.2 | Provides `f32` math for conversion rounding and derivative filtering, keeping no-std builds deterministic. | Use inside the conversion helper and telemetry math where `std` floating-point helpers are unavailable.

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `embedded-hal-mock` | Manual `mockall` SPI stubs or handwritten host wrappers | `mockall` can emulate async executor behavior, but `embedded-hal-mock` already mirrors the `embedded-hal` 1.0 API and is lightweight for conversion math; use `mockall` only when testing executor behavior adds value.

**Installation:**
```bash
cargo add embedded-hal@1.0.0 embedded-hal-mock@0.11.1 embassy-time@0.5.0 esp-hal@~1.0 heapless@0.9.2 embassy-sync@0.6.1 libm@0.2
```

## Architecture Patterns

### Recommended Project Structure
```
src/
├── control/            # PID, RoasterControl, safety handlers, stage-aware logging
├── hardware/
│   ├── sensors/
│   │   └── conversion.rs  # SensorConversionHub wraps MAX31856, offsets, retries, fault state
│   └── max31856.rs     # Low-level SPI driver called by the conversion hub
├── application/
│   └── tasks.rs         # Control loop, StageTracker, telemetry + regression triggers
├── safety/
│   └── regression.rs    # Feature-flagged regression runner that uses the conversion hub
└── output/
    └── artisan.rs       # Deterministic STATUS/READ formatter that the regression harness validates
```

### Pattern 1: SensorConversionHub (shared conversion + test hooks)
**What:** Wrap `hardware::max31856::Max31856` inside a conversion hub that (1) schedules retries, (2) reads the fault register per chip, (3) applies bean/environment offsets, (4) caches the last good sample, and (5) exposes host hooks for regression fixtures.
**When to use:** Every MAX31856 consumer (control loop, telemetry, regression harness, and COMMAND READ responses).
**Example:**
```rust
async fn roaster_async_sensor_read() -> Result<(), ContainerError> {
    let mut guard = ServiceContainer::get_instance().roaster.lock().await;
    let roaster = guard.as_mut().ok_or(ContainerError::NotInitialized)?;
    let sample = SensorConversionHub::sample().await?;
    roaster.update_temperatures(sample.bean_temp, sample.env_temp, sample.timestamp)
        .map_err(|_| ContainerError::NotInitialized)
}
```
// Source: src/application/service_container.rs:162-188 (this flow currently calls `read_sensors`; it should be redirected to the new hub)

### Anti-Patterns to Avoid
- **Duplicate conversions per caller:** Every code path that translated the 24-bit registers into floats would drift over time (different rounding, faulty sign-handling). Route everything through the new hub.
- **Blocking conversions inside the control loop:** Spin loops for the 160 ms conversion (currently in `hardware/max31856.rs`) freeze the executor and starve the watchdog. Always await `Timer::after(Duration::from_millis(160))` inside the hub.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| MAX31856 conversion math (0.0078125 °C LSB, two's complement, faults). | A bespoke converter in every consumer or a hand-rolled table of rounding code. | The central conversion helper that reads the fault register, mirrors the datasheet scaling, and replays ADC/offset sequences through `embedded-hal-mock` (alias `SensorConversionHub`). | Sharing a single helper keeps telemetry, CONTROL and regression data synchronized and prevents sporadic telemetry/regression mismatches when a rounding tweak inadvertently diverges.
| Regression instrumentation that replays MAX31856 data while the 100 ms loop runs. | A standalone regression task that pokes registers while the control loop also polls the sensors. | The `regression_task`/`request_regression` flow that is gated behind `#[cfg(feature = "regression")]` and uses the hub's deterministic fixtures. | Re-using the hub plus feature gating keeps watchdog/LEDC instrumentation unaffected and ensures `SystemStatus` snapshots recorded during regression are the same CSV columns automation expects.

**Key insight:** Once the hub is the single source of truth, any deviation between telemetry and regression (or between control and READ responses) immediately surfaces as a failing test instead of silent instrumentation drift.

## Common Pitfalls

### Pitfall 1: Central conversion hub becoming a single point of failure
**What goes wrong:** Locking all MAX31856 reads behind one async helper without per-channel fault handling causes the entire 100 ms control loop to stall the moment a CHIP_SEL reports an open thermocouple or over-range fault.
**Why it happens:** The MAX31856 reports faults in `REG_FAULT` (open circuit, short to VCC/GND, etc.) and requires clearing each channel separately; naive centralization ignores that and waits for every chip to return a valid sample.
**How to avoid:** Track per-sensor state, poll the fault register after each ADC read, log/propagate the fault to telemetry, and let the loop continue using cached samples or defaults for the faulty sensor. Expose these health flags to `SystemStatus` so regression automation can assert their presence.
**Warning signs:** StageTracker stays in `SensorRead` for >200 ms, watchdog logs `Sensor read error`, telemetry stops updating bean/ambient temperatures for one channel, or `SystemStatus::fault_condition` flips due to repeated retries.

### Pitfall 2: Regression harness using different conversion math than telemetry
**What goes wrong:** Regression tests replay ADC + offsets through ad-hoc helpers or canned floats, so the 16-column STATUS tail produced during regression doesn’t match the production CSV; automation thinks the watchdog or derivative data regressed even while the real loop still works.
**Why it happens:** The conversion code in `hardware/max31856.rs` is duplicated by whoever writes regression tests, and subtle differences in rounding/overflow handling create mismatched `pv/mv/integrator/derivative` columns.
**How to avoid:** Replay ADC sequences through the centralized helper, then serialize the same `SystemStatus` fields that `ArtisanFormatter::format_status_response` uses. Add regression fixtures that assert the 16-column order (already taught by `src/output/artisan.rs` tests) so any divergence makes the regression fail.
**Warning signs:** STATUS regression tests start failing with new derivative/integrator values, or automation tooling reports mismatched `pv/mv` columns even though the actual control loop was unchanged.

## Code Examples

### Control loop instrumentation detects stage regressions
```rust
stage_tracker.set_stage(ControlLoopStage::SensorRead);
let sensor_err = ServiceContainer::roaster_async_sensor_read().await.err();
...
stage_tracker.set_stage(ControlLoopStage::WatchdogFeed);
let watchdog_snapshot = ServiceContainer::with_roaster_async(|roaster| {
    let status = roaster.status_mut();
    ... // update watchdog_feed_ok, ledc_guard_timeouts, regression flag
})
```
// Source: src/application/tasks.rs:137-360 — the StageTracker logs each phase, feeds watchdogs, and writes `SystemStatus` spokes used by automation.

### Artisan STATUS formatter locks column ordering
```rust
format!(
    "{:.1},{:.1},{:.1},{:.1},{},{},{},{},{},{:.1},{:.1},{:.1},{:.2},{},{},{}",
    status.env_temp,
    status.bean_temp,
    status.ssr_output,
    status.fan_output,
    // watchdog + regression + PV/MV/integrator/derivative flags
)
```
// Source: src/output/artisan.rs:138-179 — regression tests already assert this 16-column CSV, so the conversion helper must produce values that match these columns.

## State of the Art
| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Each consumer called `hardware::max31856::read_temperature(_async)` and repeated the two's-complement math and fault handling. | All reads traverse `SensorConversionHub`, which gets the raw ADC+status registers, applies offsets, caches samples, and exposes deterministic fixtures for regression. | Target: Phase 72 (2026-02, v4.2 rollout) | Regression tests and telemetry share identical math, so STATUS snapshots in automation always match production readings.
| Regression instrumentation run via separate helpers that did not reuse the conversion path. | `regression_task` replays ADC/offset sequences through the hub behind a `regression` feature flag while `control_loop_task` keeps feeding watchdogs/LEDC and StageTracker logs. | Phase 72 planning | Automation now sees `SystemStatus` values aligned with the regression harness, keeping watchdog/telemetry instrumentation stable.

**Deprecated/outdated:**
- Blocking, spin-loop conversions (`hardware/max31856.rs:72-132`). The async path with `embassy_time::Timer::after(Duration::from_millis(160))` must stay mandatory so the control loop and watchdog keep hitting the 100 ms guard.

## Open Questions

1. **What canonical ADC/offset sequences should the regression harness replay?**
   - What we know: `OverTempTestRunner` ramps the heater/fan and emits `SAFETY OT-REGRESSION` telemetry, but there is no shared data file with raw ADC words per sensor.
   - What's unclear: Are there recorded ADMIN logs (per bean/env sensor) that the harness should replay, or should the team author synthetic edge cases (open, short, CJ jumps)?
   - Recommendation: Commit a small `tests/fixtures/max31856/*.json` (or `const` arrays) that enumerate ADC+CJ/fault tuples for the hub to consume. Having a shared fixture keeps automation comparing the same sequences that the hub processes during regression.
2. **How should the regression harness get gated on target hardware?**
   - What we know: `regression_task` is triggered via `ArtisanCommand::RunRegression`, but there is no cargo feature guarding host-only code.
   - What's unclear: Should the regression helper compile into the default firmware, or only when `--features regression` is enabled, to avoid shipping extra instrumentation? How does the shared helper expose its deterministic fixtures without bloating the release? 
   - Recommendation: Wrap the regression runner, its `embedded-hal-mock` fixtures, and test-only hooks in `#[cfg(feature = "regression")]` so production builds stay lean, and document how to enable the feature when running automation.

## Sources

### Primary (HIGH confidence)
- https://datasheets.maximintegrated.com/en/ds/MAX31856.pdf – Defines 0.0078125 °C LSB, two's-complement conversion, fault registers, and the required timing for MAX31856 (the foundation for the shared helper)  
- src/application/tasks.rs – StageTracker, watchdog feed updates, LEDC guard telemetry, and regression command handling already exercise the instrumentation contracts the planner must reuse.  
- src/output/artisan.rs – STATUS/READ formatters and their regression tests lock the telemetry order and provide the deterministic columns that any shared conversion helper must honor.

### Secondary (MEDIUM confidence)
- https://learn.adafruit.com/adafruit-max31856-thermocouple-amplifier/overview – Confirms the advertised 0.0078125 °C resolution, wide temperature range, and the need to handle fault notifications (supports the datasheet claims).  
- src/safety/regression.rs – Shows how the regression task currently interacts with `ServiceContainer` and why grounding it behind a feature flag keeps watchdog instrumentation deterministic.

### Tertiary (LOW confidence)
- None.

## Metadata
**Confidence breakdown:**
- Standard stack: HIGH – the crates are listed in `Cargo.toml` and the planning docs already reference `embedded-hal`, `esp-hal`, and `embassy-time`.  
- Architecture: MEDIUM – the `SensorConversionHub` idea is built on the existing `ServiceContainer`, `control_loop_task`, and `hardware/max31856.rs` flow but still needs implementation detail decisions.  
- Pitfalls: MEDIUM – drawn from the repo’s `PITFALLS.md` and existing control/test code, but final implementation will need validation.

**Research date:** 2026-02-24
**Valid until:** 2026-03-25
