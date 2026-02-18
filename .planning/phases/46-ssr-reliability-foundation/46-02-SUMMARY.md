---
phase: 46-ssr-reliability-foundation
plan: 02
subsystem: control
tags: [ssr, scheduler, telemetry, tests]

# Dependency graph
requires:
  - phase: 46-ssr-reliability-foundation
    provides: "SSR duty math, guard constants, and instrumentation from Plan 46-01"
provides:
  - "SsrCycleGuard with allow/mark/busy APIs anchored to SSR_CYCLE_GUARD_MS durations"
  - "RoasterControl gating with `apply_guarded_heater` and `SystemStatus::ssr_cycle_guard_busy_until_ms` telemetry"
  - "Host-run scheduler tests that prove the 1 000 ms datasheet window and busy timestamp"
  - "46-03-ssr-reliability-foundation (drift monitoring and telemetry)"
  - "47-deterministic-fan-control (needs reliable SSR busy reporting before LEDC serialization)"

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Guarded hardware updates that re-evaluate busy milliseconds for telemetry without leaking `Instant`."
    - "Host-friendly scheduler tests that drive simulated `Instant::from_micros` values for deterministic timing."

key-files:
  created:
    - src/control/ssr_scheduler.rs
    - tests/ssr_scheduler.rs
  modified:
    - src/control/roaster_refactored.rs
    - src/config/constants.rs
    - src/logging/channel.rs
    - src/hardware/ssr.rs

key-decisions:
  - "`log_channel!` now logs via `log::info!` whenever the build target is not `riscv32`, keeping `esp_println` for hardware only and allowing `cargo test` to run on x86 hosts."
  - "SSR duty rounding uses integer math instead of `FloatCore::round` so the helper compiles in the `no_std` driver without pulling in extra traits."
  - "SystemStatus surfaces the busy window as a millisecond difference so telemetry stays `Copy` while still showing when the guard ends."

patterns-established:
  - "Guarded SSR writes through `apply_guarded_heater` that update both hardware and `ssr_cycle_guard_busy_until_ms`."
  - "Scheduler regression tests that simulate embassy instants and fail fast if the guard allows a cycle sooner than 1 000 ms."

# Metrics
completed: 2026-02-17
---

# Phase 46 Plan 02 Summary

**SSR scheduling with guard telemetry, RoasterControl gating, and regression coverage for the 1 s window**

## Performance

- **Duration:** 5 min
- **Started:** 2026-02-17T10:57:49Z
- **Completed:** 2026-02-17T11:02:49Z
- **Tasks:** 3
- **Files modified:** 14

## Accomplishments
- Added `SsrCycleGuard` that tracks the last cycle, exposes `next_cycle_allowed`/`mark_cycle`/`busy_until`, and publishes the API via `control::mod`.
- Wired `RoasterControl` to the guard, gating manual and PID writes, logging when the guard blocks, and reporting `ssr_cycle_guard_busy_until_ms` for telemetry clients.
- Created `tests/ssr_scheduler.rs` so host targets can simulate instants and confirm the guard refuses commands before 1 000 ms while reporting the busy timestamp.

## Task Commits

1. **Task 1: Implement `SsrCycleGuard` module** - `790a150` (feat)
2. **Task 2: Gate RoasterControl heater updates with the guard** - `8b7299e` (feat)
3. **Task 3: Add guard timing regression tests** - `54906f4` (test)

**Plan metadata:** docs(46-02): complete SSR guard plan

## Files Created/Modified
- `src/control/ssr_scheduler.rs` - new guard that tracks duty windows and exposes busy timestamps.
- `src/control/roaster_refactored.rs` + `src/config/constants.rs` - guard wiring, `apply_guarded_heater`, and `ssr_cycle_guard_busy_until_ms` telemetry.
- `tests/ssr_scheduler.rs` - host-friendly regression suite for the SSR guard timing.
- `src/logging/channel.rs` + `src/hardware/ssr.rs` - test-friendly logging and duty rounding required to keep host builds compiling.

## Decisions Made
- `log_channel!` now routes non-`riscv32` builds through `log::info!`, reserving `esp_println` for embedded hardware so host tests stay wired into the logging stack.
- SSR duty rounding uses integer math instead of relying on `FloatCore`, keeping the helper `no_std` friendly and avoiding extra dependencies.
- `SystemStatus` now surfaces ms until the guard ends rather than storing an `Instant`, keeping telemetry `Copy` while still reporting busy windows.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added a host-friendly logging branch**
- **Found during:** Task 3 (Scheduler tests)
- **Issue:** `log_channel!` invoked `esp_println` while building for x86 hosts, but `esp_println` isn’t available outside `riscv32`.
- **Fix:** The macro now uses `log::info!` whenever the build target is not `riscv32`, keeping the embedded branch intact for hardware.
- **Files modified:** `src/logging/channel.rs`
- **Verification:** `cargo test --target x86_64-unknown-linux-gnu --test ssr_scheduler`
- **Commit:** `54906f4`

**2. [Rule 3 - Blocking] Rounded SSR duty without pulling in `FloatCore`**
- **Found during:** Task 3 (Scheduler tests)
- **Issue:** Host builds lacked `FloatCore`’s `round`, so `percentage_to_ledc_duty` could not compile.
- **Fix:** Replaced the rounding math with integer math that adds 0.5 before truncation and clamps, removing the extra trait dependency.
- **Files modified:** `src/hardware/ssr.rs`
- **Verification:** `cargo test --target x86_64-unknown-linux-gnu --test ssr_scheduler`
- **Commit:** `54906f4`

**3. [Rule 3 - Blocking] Simplified busy-window math**
- **Found during:** Task 3 (after the first host compile failure)
- **Issue:** `busy_window_ms` compared a `u64` to a `u128`, which failed in `no_std` builds.
- **Fix:** Returned `as_millis()` directly so the helper stays in `u64` space and compiles cleanly.
- **Files modified:** `src/control/roaster_refactored.rs`
- **Verification:** `cargo test --target x86_64-unknown-linux-gnu --test ssr_scheduler`
- **Commit:** `54906f4`

**Total deviations:** 3 auto-fixed (all blocking)

**Impact on plan:** Each auto-fix was necessary to keep the scheduler tests and guard wiring working on both host and embedded targets.

## Issues Encountered
- None.

## User Setup Required
- None.

## Next Phase Readiness
- Ready for `46-03` so SSR drift monitoring can reuse the guard API and busy timestamp.
- No blockers remain for Phase 47; SSR commands now report busy windows before FanController writes.
