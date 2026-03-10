---
phase: 47-deterministic-fan-control
plan: 01
subsystem: hardware
tags: [esp-hal, ledc, fan, ssr, mutex, embedded]

# Dependency graph
requires:
  - phase: 46-ssr-reliability-foundation
    provides: ["SSR cycle guard, LEDC monitoring, saturation math"]
provides:
  - "LedcBus serializes fan/SSR channel access and exposes guard-aware handles."
  - "FanController drives the bus handles, rounds duties, and reports the hardware-applied speed."
  - "Main initialization wires the shared bus into the fan and SSR controllers."
  - 47-deterministic-fan-control/47-02
  - 48-async-transport-resilience

# Tech tracking
tech-stack:
  added: [portable-atomic, libm]
  patterns: ["LedcBus guard with handles", "FanController reports applied duty"]

key-files:
  created: [src/hardware/ledc_bus.rs]
  modified: [src/hardware/fan.rs, src/application/app_builder.rs, src/main.rs, Cargo.toml]

key-decisions:
  - "FanController now depends on LedcChannelHandle so state mirrors the shared bus writes."
  - "AppBuilder no longer configures LEDC directly; the fan is wired before it is built."

patterns-established:
  - "Shared LedcBus ensures fan and SSR share a guard while logging contention and exposing duty reads."
  - "FanController picks set_duty or start_duty_fade based on the duty delta to minimize audible jumps."

duration: 11 min 30 sec
completed: 2026-02-17
---

# Phase 47 Plan 01: Deterministic Fan Control Summary

**Deterministic LEDC bus with guard-aware fan/SSR handles keeps the timer serialized and telemetry aligned.**

## Performance

- **Duration:** 11 min 30 sec
- **Started:** 2026-02-17T12:30:04Z
- **Completed:** 2026-02-17T12:41:34Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments

- Added `LedcBus` with a spin guard, duty tracking, and handles that log guard waits while providing `set_duty`, `start_duty_fade`, and duty readouts.
- Reworked `FanController` to depend on `LedcChannelHandle`, fade large deltas, and reflect the hardware-applied speed; AppBuilder now requires a fan implementation.
- Boot path builds the shared bus, wires fan/SSR handles into their controllers, and feeds the static instances to the application.

## Task Commits

1. **Task 1: Implement the serialized LEDC bus** - `1c832bd` (feat)
2. **Task 2: Rework `FanController` to drive the bus** - `32b091a` (feat)
3. **Task 3: Create the bus in `main.rs` for the fan path** - `cc16c31` (feat)

**Plan metadata:** docs(47-01): complete deterministic fan control plan

## Files Created/Modified

- `src/hardware/ledc_bus.rs` - guard-backed bus with handles for fan and SSR, plus duty tracking.
- `src/hardware/fan.rs` - FanController now consumes `LedcChannelHandle`, chooses fades, and reports the applied duty.
- `src/application/app_builder.rs` - builder now requires a wired fan implementation instead of configuring LEDC itself.
- `src/main.rs` - LEDC bus initialization, fan controller wiring, and SSR handle usage.
- `Cargo.toml` - added portable-atomic (guard support) and libm (rounding helper).

## Decisions Made

- FanController now depends on `LedcChannelHandle` so it can drive the shared bus and surface the applied duty instead of maintaining ad-hoc PWM state.
- AppBuilder no longer configures LEDC hardware because the bus wiring happens in `main.rs` before the application is built.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Removed AppBuilder's LEDC fallback**

- **Found during:** Task 2 (FanController rewrite)
- **Issue:** AppBuilder still tried to configure LEDC directly via `FanController::with_ledc`, but that API no longer exists once the shared bus owns the channels.
- **Fix:** AppBuilder now errors if no fan implementation is supplied so the fan wiring exclusively lives in `main.rs`.
- **Files modified:** `src/application/app_builder.rs`
- **Verification:** `cargo check --package libreroaster`
- **Commit:** `32b091a`

**Total deviations:** 1 auto-fixed (Rule 3 - Blocking).**
**Impact on plan:** Necessary to keep builder wiring aligned with the new bus-based fan control.

## Issues Encountered

- None

## Authentication Gates

- None

## Next Phase Readiness

- The shared bus and fan wiring are ready for Plan 47-02 to bring SSR control and telemetry onto the mutex-protected path.
- Deterministic fan/SSR behavior now supports Phase 48’s async transport resilience work by stabilizing LEDC timing.

---
*Phase: 47-deterministic-fan-control*  
*Completed: 2026-02-17*
