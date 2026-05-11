# LibreRoaster Technical Risk Report

**Last updated:** 2026-05-02
**Scope:** current likely bugs, behavior mismatches, and structural risks visible from the live repository

This report is intentionally conservative. It distinguishes between confirmed implementation drift, likely defects, and architectural pressure points that are not yet proven failures but are credible bug sources.

## Severity scale

- **High** — likely to cause incorrect runtime behavior, compatibility breakage, or safety confusion
- **Medium** — likely to degrade correctness, observability, or maintainability under real use
- **Low** — current debt or inconsistency with lower immediate runtime impact

## 1. High — SSR timing drift between constants and hardware initialization

**Evidence**

- `src/config/constants.rs` declares `SSR_PWM_FREQUENCY_HZ = 1`
- `src/hardware/init.rs` configures the SSR LEDC timer with `Rate::from_hz(310)`

**Why this matters**

This is a direct mismatch between the documented/control-layer constant model and the actual configured hardware timer. Anyone tuning heater behavior from the constants or from the docs could reason about the wrong actuation characteristics.

**Impact**

- heater-control behavior may not match operator expectations,
- technical docs can become misleading,
- future safety or tuning changes can be based on false assumptions.

## 2. High — temperature command range exceeds the firmware’s own safe range

**Evidence**

- parser accepts setpoint-like values up to **300 °C** for `SETTARGET`, `PREHEAT`, and PID setpoint paths
- constants define `MAX_SAFE_TEMP = 250.0`
- emergency over-temperature threshold is `260.0`

**Why this matters**

The user-facing command surface allows target values that exceed the stated safe temperature range and approach or exceed the emergency threshold envelope.

**Impact**

- confusing operator semantics,
- control requests that are “accepted” but conflict with the documented safety story,
- higher risk of badly tuned roast scripts or automation pushing the system into protective shutdowns.

## 3. Medium — `FILT` parsing silently coerces bad input to zero

**Evidence**

`src/input/parser.rs` parses the first `FILT` token using permissive fallbacks and converts malformed input to `0` rather than rejecting the command.

**Why this matters**

The firmware currently behaves as if malformed filter input were valid. That makes protocol debugging harder and hides configuration errors from callers.

**Impact**

- silent acceptance of bad handshake payloads,
- reduced observability when diagnosing startup/configuration mismatches,
- drift between “acknowledged” and “meaningfully accepted.”

## 4. Medium — async sensor reads hold the main control object for a long time

**Evidence**

- `ServiceContainer::roaster_async_sensor_read()` holds the async mutex across the full sensor-read operation
- code comments explicitly note that this duration is around **160 ms**
- nominal PID/control cadence is **100 ms**

**Why this matters**

The firmware knows that sensor-read time exceeds the nominal cadence. That does not make it wrong, but it does mean responsiveness and scheduling assumptions are fragile.

**Impact**

- delayed command handling during sensor reads,
- more coupling between transport responsiveness and hardware timing,
- increased chance of queue-pressure or watchdog-adjacent behavior under stress.

## 5. Medium — dual sync/async ownership increases state-handoff complexity

**Evidence**

`ServiceContainer` maintains both:

- `roaster_sync`
- `roaster`

and lazily migrates state from sync storage into async storage on startup.

**Why this matters**

This is an architectural compromise. It keeps old sync paths alive, but it also creates a subtle initialization lifecycle where bugs can appear at the boundaries rather than inside the control logic itself.

**Impact**

- harder reasoning about ownership,
- more complicated startup invariants,
- greater risk when changing task spawn order or initialization logic.

## 6. Medium — documentation and implementation have drifted in multiple places

**Evidence**

Before this documentation update, several docs still described older protocol shapes, older test commands, or less precise runtime behavior. The SSR frequency mismatch is the most concrete example, but it is not the only one.

**Why this matters**

Documentation drift in firmware is a bug amplifier. It causes engineers to “fix” behavior that only exists on paper or to miss live behavior that the docs never mention.

**Impact**

- slower debugging,
- wrong operational assumptions,
- less trustworthy audits.

## 7. Medium — fixed-capacity output paths can under-deliver large diagnostic payloads

**Evidence**

- roast logging and output are built on fixed-capacity heapless strings and bounded channels
- `#DUMP` is explicitly a buffered diagnostic surface rather than a streaming file interface

**Why this matters**

The architecture favors bounded memory over lossless bulk transport. That is a valid embedded choice, but engineers should not assume that internal history buffers and externally retrievable payloads have identical capacity.

**Impact**

- partial diagnostic retrieval,
- operator confusion if “stored” is assumed to mean “fully exportable,”
- need for explicit tooling expectations around dump flows.

## 8. Low — parser error taxonomy is inconsistent for empty commands

**Evidence**

`ParseError::EmptyCommand` maps to different textual representations through `code()` and `message()`.

**Why this matters**

This is mostly a debugging and consistency problem, not a core runtime fault.

**Impact**

- noisier protocol diagnostics,
- inconsistent downstream error interpretation.

## 9. Low — clippy policy and source reality are not fully aligned

**Evidence**

`Cargo.toml` denies `unwrap`, `expect`, and `panic` in production code, but the repository still contains source locations where these patterns exist or are gated in ways that require careful review.

**Why this matters**

This is a quality-policy integrity issue. Either the policy needs tighter enforcement, or the source needs further cleanup.

**Impact**

- reduced confidence in static policy guarantees,
- review overhead when deciding whether a risky pattern is test-only or production-reachable.

## 10. Low — official-Artisan compatibility is strong at the serial layer but narrow elsewhere

**Evidence**

LibreRoaster implements a solid serial command surface but does not implement the wider Artisan ecosystem: file formats, Modbus, WebSockets, artisan.plus, or vendor-device breadth.

**Why this matters**

It is not a bug in the firmware, but it becomes a practical problem if users assume “Artisan-compatible” means “compatible with everything Artisan can do.”

**Impact**

- incorrect integration expectations,
- support confusion,
- need for explicit documentation boundaries.

## Summary

The two most important current issues are:

1. **the SSR timing mismatch between constants/docs and real hardware initialization**, and
2. **the accepted temperature-command range extending beyond the project’s own safe-temperature story**.

The rest of the risk surface is mostly about bounded-memory tradeoffs, startup/state-handoff complexity, and parser permissiveness. None of those should be ignored, but they are easier to reason about once the two high-severity mismatches are resolved.
