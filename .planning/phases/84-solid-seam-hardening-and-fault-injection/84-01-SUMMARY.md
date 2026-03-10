---
phase: 84-solid-seam-hardening-and-fault-injection
plan: 01
subsystem: control
tags: [rust, embassy, embedded-hal, policy-pattern, ports-and-policies, solid]

# Dependency graph
requires:
  - phase: 83-rust-modernization-and-unsafe-surface-audit
    provides: Modernized Rust code with reduced unsafe surface
provides:
  - Ports-and-policies trait definitions (ManualPolicyOutcome, SafetyPolicyOutcome)
  - ManualCommandPolicy trait for handler policy evaluation
  - SafetyPolicy trait for safety command handling
  - RoasterControl as single writer for heater/fan/watchdog
  - Policy hand-off logging for instrumentation
affects: [84-02-stage-instrumentation, 84-03-fault-injection]

# Tech tracking
tech-stack:
  added: []
  patterns: [ports-and-policies, single-writer-authority, policy-evaluation-hand-off]

key-files:
  created: [src/control/policies.rs]
  modified: [src/control/handlers.rs, src/control/roaster_refactored.rs, src/control/mod.rs]

key-decisions:
  - "Handler policy evaluation returns outcomes without hardware writes"
  - "RoasterControl applies all hardware writes after policy evaluation"
  - "Safety and manual commands use policy traits; temperature/system use legacy handlers"

patterns-established:
  - "Policy hand-off: Handler evaluates command, returns outcome, RoasterControl applies hardware"
  - "Single writer: Only RoasterControl calls heater.set_power() and fan.set_speed()"

# Metrics
duration: ~8 min
completed: 2026-03-08
---

# Phase 84 Plan 01: SOLID Seam Hardening Summary

**Ports-and-policies trait contracts that centralize hardware authority in RoasterControl**

## Performance

- **Duration:** ~8 min
- **Started:** 2026-03-08T12:59:46Z
- **Completed:** 2026-03-08T13:08:13Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments

- Created `src/control/policies.rs` with `ManualPolicyOutcome` and `SafetyPolicyOutcome` structs that describe desired heater/fan state without touching hardware
- Defined `ManualCommandPolicy` and `SafetyPolicy` traits that handlers implement to return policy outcomes
- Updated `ArtisanCommandHandler` and `SafetyCommandHandler` to implement the new policy traits
- Modified `RoasterControl::process_command()` to use policy evaluation and centralized hardware writes
- Added `apply_policy_outcome()` and `apply_safety_outcome()` methods that log policy inputs for instrumentation

## Task Commits

Each task was committed atomically:

1. **Task 1: Define manual and safety policy contracts** - `e4e9d17` (feat)
2. **Task 2: Refactor handlers to implement the policies** - `e9777d3` (feat)
3. **Task 3: Let RoasterControl be the sole hardware writer** - `79bb1cf` (feat)

**Plan metadata:** (docs commit will follow this summary)

## Files Created/Modified

- `src/control/policies.rs` - Policy outcome structs and trait definitions
- `src/control/handlers.rs` - Added ManualCommandPolicy and SafetyPolicy implementations
- `src/control/roaster_refactored.rs` - Added policy-based command processing with centralized hardware writes
- `src/control/mod.rs` - Exported new policies module

## Decisions Made

- Handler authority stays in handlers (policy evaluation) while hardware authority stays in RoasterControl (hardware writes)
- Legacy handlers (TemperatureCommandHandler, SystemCommandHandler) continue using RoasterCommandHandler trait for backward compatibility
- Policy inputs are logged via `debug!` macros for instrumentation at each hand-off point

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- Serialport crate dependency has a build issue with ssmarshal/serde_core - tests work without it; this is a pre-existing infrastructure issue not introduced by this plan.

## Next Phase Readiness

- Phase 84-02 (stage instrumentation) can now use the policy hand-off points for tracking SensorRead→ControlUpdate→LedcWrite→WatchdogFeed→TelemetryEmit
- Phase 84-03 (fault injection harness) can extend the policy pattern to inject failures and verify STATUS telemetry captures safe responses

---
*Phase: 84-solid-seam-hardening-and-fault-injection*
*Completed: 2026*
-03-08