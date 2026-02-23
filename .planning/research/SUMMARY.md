# Project Research Summary

**Project:** LibreRoaster
**Domain:** Embedded safety instrumentation for the Artisan command stack on ESP32-C3
**Researched:** 2026-02-23
**Confidence:** MEDIUM

## Executive Summary

LibreRoaster is a safety-critical embedded controller for Artisan roasting gear. Experts build it by layering an async executor, a dual-context `ServiceContainer`, and hardened peripheral drivers so the 100 ms control loop can stay deterministic while instrumentation commands and regression tests run alongside it.

The recommended approach stitches together the stack from `esp-hal` (LEDs, temperature sensors, system registers), the IDF watchdog shim, and `embassy-time` so the existing loop feeds the TWDT, cuts heater duty as soon as an over-temperature condition trips, and drops LEDC guard tokens before awaiting. Telemetry and regression runners share the same `dual_output_task`, keeping safety state observable while the watchdog or regression harness can inject test scenarios.

Key risks are long-running guard or flash operations that sideline the idle task, guard loops that spin without yielding, and the built-in temperature sensor being treated as an absolute over-temp answer. Mitigate these by inserting `taskYIELD()`/`vTaskDelay(1)`, feeding the watchdog from the loop before instrumentation work runs, keeping LEDC guard sections short, and enabling IRAM-safe threshold callbacks so regression/production signals fire even when the cache is disabled.

## Key Findings

### Recommended Stack

The research binds the safety logic to the existing asynchronous runtime by feeding TWDT from the 100 ms loop, relying on the guard-managed LEDC access layer, and pulling temperature readings through the HAL so the regression harness can verify the shutdown path.

**Core technologies:**
- `esp-hal` (esp32c3 + `ledc`, `tsens`, `system`): controls PWM, temp sensors, and system registers safely while exposing the temperature helpers needed by the regression path.
- `esp_bootloader_esp_idf`: links Task Watchdog APIs (`esp_task_wdt_*`) so the async loop can register/ feed TWDT and report the triggered task.
- `embassy-time`: orchestrates the deterministic 100 ms cycle that samples sensors, drives LEDC, and feeds the watchdog without blocking the executor.

### Expected Features

Launch requires the TWDT-backed control cycle, over-temperature detection that immediately shuts down the heater, and the LedcGuard timeout guard that logs failures and leaves the outputs safe. Differentiators include multi-stage watchdog telemetry, regression-proven over-temp tests, and LedcGuard event logs.

**Must have (table stakes):**
- TWDT-backed 100 ms control cycle — users expect the system to recover from stuck loops.
- Over-temperature detection & shutdown — the heater must stop when temperatures exceed safe thresholds.
- LedcGuard timeout guard — serializes LEDC updates and reports guard failures before the watchdog fires.

**Should have (competitive):**
- Multi-stage observable watchdog transitions — auditors see each stage so restarts are explainable.
- Regression-proven over-temp path — the regression harness forces the safe-shutdown sequence and labels it `OT-REGRESSION`.
- LedcGuard event telemetry — instrumentation reports guard events and ties them to the control tick.

**Defer (v2+):**
- Telemetry dashboards tagging watchdog resets, over-temp events, and guard timeouts (v1.x follow-up once the safety paths are validated).
- Multi-point temperature fusion and a fault-injection bench (longer-term v2+ work).

### Architecture Approach

The architecture centers on a `ServiceContainer` holding `RoasterControl`, `WatchdogFeeder`, and regression handles so `control_loop_task` can read sensors, update PID outputs, feed the TWDT, and optionally trigger safety tests while `dual_output_task` keeps telemetry flowing.

**Major components:**
1. `control_loop_task` / `dual_output_task` — run the 100 ms embassy loop, pump telemetry, and feed the watchdog/ regression runner.
2. `LedcBus` + `LedcGuard` — serialize fan/SSR writes, enforce timeout guards, and keep hardware access short before `await` points.
3. `Safety` helpers (`WatchdogFeeder`, `OverTempTestRunner`) — wrap TWDT feeds, regression scenarios, and telemetry reporting so instrumentation shares the same async container.

### Critical Pitfalls

1. **Heavy flash/guard work trips the Task WDT** — add yield points or break workloads so the idle task feeds TWDT instead of blocking during flash erases or guard fades.
2. **Tight sampling or guard loops starve idle** — batch loops, insert `taskYIELD()`/`vTaskDelay(1)`, or call `esp_task_wdt_reset()` so the watchdog sees progress.
3. **Uncalibrated temperature threshold callbacks** — enable `CONFIG_TEMP_SENSOR_ISR_IRAM_SAFE`, keep callbacks small/IRAM-safe, and calibrate the chip sensor against external references before trusting the guard.
4. **LedcGuard waits forever on fades** — replace busy waits with fade-end callbacks, yield while polling, and keep mutexes short to avoid starving the watchdog.

## Implications for Roadmap

### Phase 1: Watchdog Integration
**Rationale:** The 100 ms loop needs to feed the TWDT before any instrumentation or guard enhancements run so a stuck task is detectable early.
**Delivers:** deterministic `control_loop_task`, `WatchdogFeeder` wiring, and TWDT feeds through `esp_task_wdt_reset_user` plus telemetry for reset reasons.
**Addresses:** TWDT-backed cycle, over-temperature detection, logging the watchdog stages.
**Avoids:** Heavy flash/guard work tripping Task WDT by keeping all long-running work out of this phase.

### Phase 2: Guard Timeout Hardening
**Rationale:** LedcGuard timeouts and the LEDC guard token must exist before regression and instrumentation exercises the same hardware.
**Delivers:** `LedcGuard`/`LedcBus` timeout-aware guards, callback-based fade completion, and `ServiceContainer` fields for guard telemetry.
**Uses:** `esp-hal` LEDC primitives plus `embassy-time` timers so guard lookups yield instead of blocking.
**Implements:** the guarded hardware access pattern and shared service container wiring.
**Avoids:** LedcGuard busy waits starve the watchdog by enforcing short critical sections and callbacks.

### Phase 3: Regression & Observability
**Rationale:** Once the guard is stable, add regression test runners, instrumentation telemetry, and calibrate over-temperature responses so we can prove safe shutdowns.
**Delivers:** `OverTempTestRunner`, telemetry for guard/resets, regression harness commands, and documentation of stage transitions.
**Addresses:** LedcGuard telemetry, regression-proven over-temp path, and deferred telemetry dashboards.
**Avoids:** Uncalibrated temp sensors failing to fire by validating IRAM-safe callbacks under flash-intensive scenarios.

### Phase Ordering Rationale
- Build the watchdog loop first so the TWDT feed exists before any guard or regression work can accidentally starve idle.
- Layer guard timeout logic over that loop because LedcGuard depends on a healthy TWDT feed and existing LEDC synchronization.
- Add regression + telemetry last since they assume the guard and watchdog are stable and must reuse the same telemetry/ServiceContainer wiring.

### Research Flags
Phases likely needing deeper research during planning:
- **Phase 2:** Guard callback latency and LEDC fade semantics need experimentation to avoid busy-waiting traps.
- **Phase 3:** Over-temperature calibration and IRAM-safe sensor callbacks require sensor/flash gating tests before shipping.

Phases with standard patterns (skip research-phase):
- **Phase 1:** Assembling `esp-hal`, `esp_bootloader_esp_idf`, and `embassy-time` around the 100 ms loop follows well-documented practices.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | MEDIUM | Based on official ESP-IDF/esp-hal docs documented in `STACK.md`. |
| Features | HIGH | Feature priorities are derived from high-confidence research in `FEATURES.md`. |
| Architecture | MEDIUM | Internal component map is well defined but still evolving. |
| Pitfalls | MEDIUM | Sources cite ESP-IDF docs/issues with reproducible warnings. |

**Overall confidence:** MEDIUM

### Gaps to Address
- **Sensor calibration gap:** Need to validate chip-temperature thresholds against actual roasts and document how they relate to external probes.
- **Instrumentation timing:** Confirm regression harness and LEDC guard callbacks keep the idle task fed when flash operations / fades run concurrently.

## Sources

### Primary (HIGH confidence)
- `.planning/research/STACK.md` — Justifies the safety stack (esp-hal, esp_bootloader_esp_idf, embassy-time) with IDF docs.
- `.planning/research/FEATURES.md` — Prioritizes TWDT guard, over-temp shutdown, and telemetry/regression differentiators.

### Secondary (MEDIUM confidence)
- `.planning/research/ARCHITECTURE.md` — Maps components, data flows, and design patterns around the ServiceContainer.
- `.planning/research/PITFALLS.md` — Enumerates watchdog/LEDC pitfalls and prevention strategies drawn from ESP-IDF notes.

### Tertiary (LOW confidence)
- ESP-IDF issue #8135 (flash erase triggering Task WDT) — highlights heavy work blocking the idle task.
- Stack Overflow discussion (`ESP32 Task Watchdog Triggered`) — reinforces the need to yield in long loops.

---
*Research completed: 2026-02-23*
*Ready for roadmap: yes*
