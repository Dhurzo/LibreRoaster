---
phase: 96-error-architecture-implementation
plan: 04
subsystem: error-handling
tags: [boundary-contracts, from-trait, control-flow]
duration: 1h 45m
completed: 2026-03-20
---

# Phase 96 Plan 04 Summary

**Boundary contracts and `?` operator plumbing for hardware → Roaster → App surfaces**

## Performance

- **Duration:** ~1h 45m (2026-03-20 14:15 → 16:00 UTC)
- **Tasks:** 4
- **Files touched:** `src/control/abstractions.rs`, `src/control/roaster_refactored.rs`, `src/error/app_error.rs`, test harness

## Accomplishments

1. Added `From` implementations that let every hardware error (`SsrError`, `FanError`, `UartError`, `InputError`) convert cleanly into `RoasterError`, and upgraded the control boundary to reuse the propagated source tokens.
2. Reworked `RoasterControl`’s policy application, guarded heater, and safe-shutdown paths to rely on the `?` operator instead of manual `map_err` wrappers, keeping instrumentation and metrics capture intact.
3. Expanded `AppError` unit tests to verify hardware → control → app conversions, serialization-ready `Display`/`Debug` output, and input/communication boundary contracts so the new plumbing stays exercised.
4. Ensured `cargo test --lib` covers every conversion path after the refactor.

## Task Commits

1. **Task 1 – Hardware → control `From` conversions** – wired `SsrError`, `FanError`, `UartError`, and `InputError` into `RoasterError` so the control plane can `?` through hardware helpers.
2. **Task 2 – Control → app conversions reuse new paths** – updated `apply_policy_outcome`, `apply_guarded_heater`, and `stop_streaming` to take advantage of the conversions and emit precise watchdog/fan state metadata.
3. **Task 3 – AppError testing** – added boundary-contract tests (hardware → control, control → app, input → app), extra `Display`/`Debug` coverage, and `source()` assertions so the diagnostics stay traceable.
4. **Task 4 – Verification** – ran `cargo test --lib` and reviewed the instrumentation traces for guard/telemetry output.

## Files Created/Modified

- `src/control/abstractions.rs` – exported new `From` conversions and tuned `RoasterError` message tokens.
- `src/control/roaster_refactored.rs` – replaced `map_err` plumbing with `?` and preserved guard metrics.
- `src/error/app_error.rs` – added hardware conversions, reused the new constant for `InputError`, and expanded the unit tests.


## Decisions Made

- Let hardware errors carry their `source` strings into `RoasterError::HardwareError` so telemetry can show the low-level reason.
- Sleep guards should never swallow control-flow errors—`?` keeps return paths simple and lets conversion logic live in one place.
- Input errors map to `InvalidState` with clear tokens, making parser/CLI problems reportable in diagnostics.

## Verification

- `cargo test --lib` (passes).

## Next Phase Readiness

- Ready for Plan 96-05: add mock hardware injection, error recovery tests, and regression runners now that the boundary contracts are wired.
