# Phase 80: Handler Pattern - Research

**Researched:** 2026-02-28
**Domain:** Rust command dispatch and Artisan+ control harmonization
**Confidence:** MEDIUM

## Summary

I traced the existing `RoasterControl` stack around `process_artisan_command()` and the handler chain defined in `control::handlers`. The `RoasterCommandHandler` trait already centralizes `RoasterCommand` handling (`Temperature`, `Safety`, `Artisan`, `System`), so most Artisan commands that mutate heater/fan state already walk through that pipeline even if the Artisan parser method currently maps them through a huge `match`. The remaining responsibilities (start/stop/session metadata, status formatting, units, panic-safe fan clamps) live alongside `process_artisan_command` and keep touching `SystemStatus`, the heater, and the serializer buffers.

Delegating the `SetHeater`, `SetFan`, `IncreaseHeater`, `DecreaseHeater`, and related manual controls to `ArtisanCommandHandler` via the `RoasterCommandHandler` trait will re-use the single source of truth for `manual_heater`, `manual_fan`, and the PID toggles while preserving the `status` flags that `update_control()` and the status formatter rely on. The remaining Artisan-specific work (start/stop, status responses, OT2 safety clamp, regression/unit toggles) should stay in `process_artisan_command` because it interacts with the broader control loop (heater/fan hardware, `temp_handler`, `temp_settings`).

**Primary recommendation:** Route manual Artisan commands to `process_command()` so the existing `ArtisanCommandHandler` handles all heater/fan state changes, and leave only session-level or telemetry commands in `process_artisan_command()`.

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust | 1.88 (2021 edition) | Language and crate edition used by `libreroaster`. | Project Cargo manifest pins this release, so every module, including the handler trait, targets it.
| embassy-time | 0.5.0 | `Instant`/`Duration` for `RoasterCommandHandler::handle_command` and `RoasterControl` timing (`process_command`, `update_control`). | `process_command` already passes `embassy_time::Instant` through the handler chain.
| log | 0.4.27 | Logging inside `process_artisan_command`, handlers, and the control loop. | Used pervasively for status reports and warnings, so handlers must keep logging consistent.

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| heapless | 0.9.2 | Heapless buffers/strings in `application::tasks` Artisan parser and output channels. | Artisan command path already depends on heapless `String`/`Vec`, so the handler refactor must keep those buffers fed and drained cleanly.
| embassy-executor | 0.9.1 | Runtime for `control_loop_task`, which invokes `process_artisan_command`. | Use when scheduling async `read_sensors` + command processing.
| alloc (core/alloc) | — | `Box`, `Vec`, and other heap types in `RoasterControl`, e.g., handler storage and status responses. | Required for the handler trait objects (`Box<dyn Heater>`, `alloc::vec::Vec`) even under `#![no_std]`.

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `RoasterCommandHandler` chain | inline `match` inside `process_artisan_command` | Duplicates state updates (`manual_heater`, `status` flags, logging) and risks drift between command paths.
| Having `RoasterControl` mutate manual values directly | routing through `ArtisanCommandHandler` | Harder to keep `manual_heater`/`manual_fan` in sync with `status` + `process_output()` fan writes.

**Installation:**
```bash
cargo fetch
```

## Architecture Patterns

### Recommended Project Structure
```
src/
├── control/
│   ├── abstractions.rs   # `RoasterCommandHandler` trait + errors
│   ├── handlers.rs       # temperature, safety, artisan, system handlers
│   └── roaster_refactored.rs # central state + `process_artisan_command`
├── application/
│   └── tasks.rs          # command parser that feeds `process_artisan_command`
└── config/
    └── constants.rs      # `ArtisanCommand`, `RoasterCommand`, `SystemStatus`
```

### Pattern 1: Roaster Command Handler Chain
**What:** `RoasterCommandHandler` defines `can_handle`/`handle_command`, and `RoasterControl::process_command` iterates over a fixed array of handlers before mutating hardware/status.
**When to use:** Any `RoasterCommand` that affects PID, SSR output, or message safety flags.
**Example:**
```rust
// Source: src/control/roaster_refactored.rs
let mut handlers: [&mut dyn RoasterCommandHandler; 4] = [
    &mut self.safety_handler,
    &mut self.temp_handler,
    &mut self.artisan_handler,
    &mut self.system_handler,
];

for handler in &mut handlers {
    if handler.can_handle(command) {
        let result = handler.handle_command(command, current_time, &mut self.status);
        self.status.fault_condition = self.safety_handler.is_emergency_active();
        return result;
    }
}
```

### Anti-Patterns to Avoid
- **Monolithic Artisan match:** Dropping every Artisan manual call into `process_artisan_command` duplicates the logic already guarded by the handlers (manual heater/fan, delta increments) and makes `manual_heater`/`manual_fan` drift.
- **Mixing handler state:** Directly mutating `self.artisan_handler` from `process_artisan_command` without the trait loop bypasses `can_handle` and its invariants (e.g., `status.pid_enabled` updates).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Command dispatch + manual controls | Another long match that sets PWM/fan values manually | Feed `RoasterCommand::SetHeaterManual`, `SetFanManual`, `IncreaseHeater`, `DecreaseHeater` into `process_command` so `ArtisanCommandHandler` handles them | The handler already clamps values, maintains `manual_heater`/`manual_fan`, and keeps `status` fields consistent with `update_control()`.
| Manual `SystemStatus` bookkeeping | Re-implement `status.artisan_control`, `status.pid_enabled`, `status.ssr_output` updates inside `process_artisan_command` | Let the handler modify the shared status within `handle_command` | Leaving these assignments in `process_artisan_command` causes dual sources of truth and confuses `process_output()`/status reports.

**Key insight:** `ArtisanCommandHandler` is the single authority for Artisan manual controls; reuse it rather than spreading heater/fan logic across `process_artisan_command` and handlers.

## Common Pitfalls

### Pitfall 1: Manual state divergence
**What goes wrong:** `manual_heater`, `manual_fan`, and `status` flags fall out of sync, so `update_control()` sends stale SSR/fan values.
**Why it happens:** Artisan commands mutate the heater/fan in both `process_artisan_command` and `ArtisanCommandHandler` when the handler is bypassed.
**How to avoid:** Always route manual Artisan commands through `process_command()` (with the handler loop) so a single block touches the status and manual fields.
**Warning signs:** `status.fan_output` no longer matches `manual_fan`, fan speed reported in control loop deviates from expectation.

### Pitfall 2: Missing safety clamp after `SetFanSpeed`
**What goes wrong:** OT2 `SetFanSpeed` reports a clamp (`was_clamped`) but the heater keeps heating, which violates the safety requirement.
**Why it happens:** The clamp logic lives in `process_artisan_command`, so moving only the fan command into the handler without preserving the clamp reaction would leave heater power unchanged.
**How to avoid:** Retain the clamp path in `process_artisan_command` (stop heater when `was_clamped` true) while still invoking `process_command` for the fan update.
**Warning signs:** `capture_ssr_monitor_metrics()` not called when `was_clamped`, heater log keeps reporting power after OT2 errors.

### Pitfall 3: Losing SSR status refreshes
**What goes wrong:** `status.ssr_hardware_status` contains stale values, which confuses the status formatter and guard diagnostics.
**Why it happens:** The current match updates this field before replies (Status/Read) and after StartRoast; refactoring might forget to refresh it at the same points.
**How to avoid:** Keep the explicit `self.status.ssr_hardware_status = self.heater.get_status()` calls wherever the handler refactor originally touched them (status commands, start/stop transitions).
**Warning signs:** Logged responses show `SsrHardwareStatus::NotDetected` even though the SSR is powered.

## Code Examples

### Handler Chain Dispatch
```rust
// Source: src/control/roaster_refactored.rs
if handler.can_handle(command) {
    let result = handler.handle_command(command, current_time, &mut self.status);
    self.status.fault_condition = self.safety_handler.is_emergency_active();
    return result;
}
```

### ArtisanCommandHandler Manual Controls
```rust
// Source: src/control/handlers.rs
match command {
    RoasterCommand::SetHeaterManual(value) => { /* sets status.artisan_control, ssr_output, manual_heater */ }
    RoasterCommand::SetFanManual(value) => { /* mirrors fan_output and manual_fan */ }
    RoasterCommand::IncreaseHeater => { /* uses HEATER_DELTA, updates ssr_output */ }
    RoasterCommand::DecreaseHeater => { /* same with downward delta */ }
    _ => Err(RoasterError::InvalidState),
}
```

## State of the Art
| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Single `match` in `process_artisan_command` handled every Artisan command, kept manual state, and talked to the heater/fan directly. | Handler trait chain continues to manage manual heater/fan commands via `process_command`, while `process_artisan_command` stays focused on start/stop, telemetry, OT2 clamps, and status serialization. | Target of Phase 80 (REF-01). | Reduces duplication, ensures `ArtisanCommandHandler` remains the canonical source for manual outputs, and keeps `process_artisan_command` scoped to session-level logic.

**Deprecated/outdated:** Large match blocks for manual controls—the handler chain already implements the same validations and status updates.

## Open Questions
1. **Should StartRoast be refactored into a handler or stay in `process_artisan_command`?**
   - What we know: StartRoast touches `enable_pid_control`, `temp_handler`, state transitions, and SSR status.
   - What's unclear: Whether a new handler should own those cross-cutting responsibilities or if `process_artisan_command` should remain the orchestrator.
   - Recommendation: Keep it in `process_artisan_command` for now and ensure any handler invocation it triggers still updates `SystemStatus` the same way.
2. **Where should the OT2 clamp (heater shutdown when `was_clamped` is true) live after the refactor?**
   - What we know: The current match stops the heater and logs the event while still routing the fan command through `process_command`.
   - What's unclear: Whether that clamp should move closer to the handler or remain adjacent to Artisan parsing.
   - Recommendation: Keep the clamp logic next to `SetFanSpeed` so it can still call `self.heater.set_power(0.0)` after the handler runs.

## Sources
### Primary (HIGH confidence)
- `src/control/handlers.rs` – defines `RoasterCommandHandler`, `ArtisanCommandHandler`, and the manual fan/heater logic that `process_command` already uses for those commands.
- `src/control/roaster_refactored.rs` – holds `process_artisan_command`, `process_command`, and the handler dispatch loop that must be aligned with this phase.
- `Cargo.toml` – pins Rust 1.88 and the dependencies (`embassy-time`, `log`, `heapless`, etc.) that the handler work relies on.

### Secondary (MEDIUM confidence)
- `src/application/tasks.rs` – shows how `process_artisan_command` is invoked from the Artisan command loop, so the refactor must keep telemetry/response behavior.

### Tertiary (LOW confidence)
- N/A

## Metadata
**Confidence breakdown:**
- Standard stack: HIGH – directly observable in `Cargo.toml`.
- Architecture: MEDIUM – based on current code paths and the handler trait.
- Pitfalls: MEDIUM – inferred from duplication risk and log usage.

**Research date:** 2026-02-28
**Valid until:** 2026-03-30
