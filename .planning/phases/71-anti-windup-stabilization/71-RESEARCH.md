# Phase 71: Anti-windup stabilization - Research

**Researched:** 2026-02-24
**Domain:** Anti-windup PID + filtered derivative instrumentation on LibreRoaster’s 100 ms loop
**Confidence:** MEDIUM

## Summary

Phase 71 hardens the PID stack so actuator commands stop growing once the LEDC guard or saturation flag fires, and it recomputes the derivative term from the actual MAX31856 PV stream with filtering so STATUS telemetry only spikes when the plant really moves.

- `RoasterControl::update_control` already tracks the desired/applied SSR duty, feeds watchdogs through `ServiceContainer`, and records `SystemStatus::saturation_active`, `integrator_clamped`, and `derivative_available`, but the PID implementation is still a stub that simply clamps `error * 2.0`. We need a real integrator, derivative, and anti-windup-aware update cycle that cooperates with `apply_guarded_heater` when LEDC guard rejects new cycles or the output hits 100%.
- The derivative rate is computed from the last PV sample (see `self.last_pv_sample`) and the elapsed time, but there is no filtering or smoothing yet. Derivative instrumentation currently toggles `derivative_available` based on whether a previous sample exists. We need a filter (IIR or windowed average) so telemetry only shows spikes when PV changes beyond noise.
- `apply_guarded_heater` is the only place that knows about LEDC saturation and guard busy time; it already flips `saturation_active`/`integrator_clamped`. The anti-windup logic can hook into its outcome (desired vs. applied) and use that to gate integrator accumulation.
- `SystemStatus` already exposes the required fields (PV, MV, integrator_value, derivative_rate, saturation flags), and `ArtisanFormatter::format_status_response` appends them to the STATUS CSV tail, so we can reuse the existing telemetry layout as long as we feed real numbers.

## Standard Stack

| Library | Version | Purpose |
|---------|---------|---------|
| `embassy-executor` | 0.9.x | Drives `control_loop_task`, keeping the 100 ms heartbeat single-threaded. |
| `embassy-time` | 0.5.x | Provides `Instant`, `Duration`, and `Timer::after` for precise loop cadence and derivative timing. |
| `embassy-sync` | 0.6.x | Powers `ServiceContainer` channels that orchestrate sensors, commands, watchdog, and telemetry inside each tick. |
| `esp-hal` | ~1.0 | Required for real LEDC/fan/SSR hardware that the loop controls and guards. |
| `heapless` | 0.9.x | Used by `ArtisanFormatter` and safety strings to keep telemetry deterministic without dynamic allocation. |

## Architecture Patterns

### Pattern: Anti-windup gate inside `RoasterControl`
**What:** `RoasterControl::update_control` asks `ServiceContainer::with_roaster_async` to apply a `desired_output`, then `apply_guarded_heater` tells whether the SSR cycle was rejected or clamped. We use that signal to pause integrator accumulation (set `SystemStatus::integrator_clamped`) and keep telemetry `saturation_active` true until the actuator is back under control.
**How:** Record `(desired_output, applied_output)` plus guard-busy status, forward them to the new PID implementation (`CoffeeRoasterPid`), and let the PID return a tuple with `integrator_term`, `derivative_contribution`, `output`. Avoid updating the integral when `applied_output` differs from `desired_output` because the actuator cannot accept more energy.

### Pattern: Filtered derivative from MAX31856 PV samples
**What:** Derivative is computed from `SystemStatus::pv` samples that already come from `MAX31856`. Instead of raw deltas, pass the PV through a lightweight filter (single-pole low-pass or a moving median) so we only expose spikes when the PV change exceeds sensor noise.
**How:** Keep `last_pv_sample` and `last_pv_sample_time` from `RoasterControl`, compute `dt`, and update `status.derivative_rate` with `filter_alpha * instantaneous_rate + (1 - filter_alpha) * previous_derivative`. Flip `status.derivative_available` only when `dt` is sane and the filter settles.

## Pitfalls

1. **Integrating through saturation:** If we keep accumulating the integrator even while the LEDC guard rejects cycles, the PID overshoots and oscillates. Stop adding integral terms when `applied_output - desired_output` is negative or guard is busy, but allow a controlled release (e.g., only resume integration once the actuator confirms it can apply the requested duty).
2. **Derivative noise:** Without filtering, small MAX31856 jitter produces telemetry spikes that make derivative availability meaningless. A simple IIR filter keyed to `dt` keeps telemetry demonstrating actual plant motion.

## Code Examples

### Existing control-to-watchdog pipeline
```rust
let control_snapshot = ServiceContainer::with_roaster_async(|roaster| roaster.update_control(current_time)).await;
match control_snapshot {
    Ok(snapshot) => {
        // snapshot exposes desired/applied values, fan output, and the status fields we will now populate
    }
    Err(err) => warn!("Control update error: {:?}", err),
}
```

### Current derivative calculation (needs filtering)
```rust
let derivative_rate_opt = self.last_pv_sample.zip(self.last_pv_sample_time).and_then(|(prev_temp, prev_time)| {
    let dt = current_time.duration_since(prev_time).as_millis();
    if dt == 0 { return None; }
    let delta = current_pv - prev_temp;
    Some(delta / (dt as f32 / 1000.0))
});
self.status.derivative_rate = derivative_rate_opt.unwrap_or(0.0);
self.status.derivative_available = derivative_rate_opt.is_some();
```

## State of the Art

| Old Approach | Desired Approach | Impact |
|--------------|------------------|--------|
| Stubbed PID, integral = `desired_output` | Stateful PID tracking integrator/derivative with anti-windup hooks | Telemetry shows real integrator term and actuator stops integrating while saturated (CONTROL-02). |
| Raw PV delta derivative | Filtered derivative that reacts only to real PV motion | Derivative availability flag reports `true` only when filter output is meaningful (CONTROL-03). |

## Open Questions

1. **Filter time constant:** What `alpha` or window length balances jump detection vs noise? `PID_SAMPLE_TIME_MS` is 100 ms, so a single-pole filter (~0.2-0.5) might be enough, but we should document the choice.
2. **Integrator release policy:** Should we resume integrating immediately once the actuator accepts the request again, or wait for a few cycles with `saturation_active` false? Recording guard busy vs saturation counts may help tune this.

## Sources

- `src/control/roaster_refactored.rs` – `update_control`, `apply_guarded_heater`, PV bookkeeping, and instrumentation snapshots.
- `src/control/pid.rs` – current stub and target for PID anti-windup logic.
- `src/config/constants.rs` – `SystemStatus` fields (`pv`, `mv`, `integrator_value`, `derivative_rate`, `saturation_active`, `integrator_clamped`, `derivative_available`).
- `src/output/artisan.rs` – `format_status_response` already emits the required CSV tail; we only need to feed it better numbers.
- `src/hardware/max31856.rs` – asynchronous PV reads that drive the control loop and form the base of the derivative term.
- `src/application/tasks.rs` – the single `control_loop_task` that orchestrates sensors, control updates, watchdog feeding, and telemetry emission every 100 ms.

## Metadata

**Research date:** 2026-02-24
**Valid until:** 2026-03-25
