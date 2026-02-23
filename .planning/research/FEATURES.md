# Feature Research: Watchdog Timer Safety for LibreRoaster

**Domain:** Safety-critical control loop for the Artisan serial protocol on ESP32-C3
**Researched:** 2026-02-23
**Confidence:** HIGH

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist. Missing these = product feels incomplete.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| TWDT-backed 100 ms control cycle | A roaster must recover from stuck loops; TWDT (Task Watchdog) built on MWDT ensures stuck tasks raise interrupts and either prints backtrace or resets the system if not fed in time [[Watchdogs](https://docs.espressif.com/projects/esp-idf/en/latest/esp32c3/api-reference/system/wdts.html#task-watchdog-timer-twdt)] | MEDIUM | Guarantee that each cycle calls `esp_task_wdt_reset()` (current observers already enforce 100 ms cadence) and record the reset source so the next boot knows whether a watchdog triggered; stage actions allow interrupts → CPU reset → system reset so the observable fallback is: log/panic → safe reset → restart. |
| Over-temperature detection & shutdown | Prevent heater lock-on by removing duty whenever the internal temp passes safe thresholds; built-in temp sensor is good for chip temperature [[Temperature Sensor](https://docs.espressif.com/projects/esp-idf/en/latest/esp32c3/api-reference/peripherals/temp_sensor.html)] | MEDIUM | Every over-temp event must cut LEDC heating channels _and_ report telemetry/backtrace; regression test proves the detection works by driving temperature readings above the limit and seeing the same safe-shutdown path executed. |
| LedcGuard timeout guard | LEDC APIs are not thread-safe and can keep PWM hardware in an undefined state; existing LEDC cycle guard + fan serialization means we already expect the `LEDC` driver to be run inside a critical section [[LEDC](https://docs.espressif.com/projects/esp-idf/en/latest/esp32c3/api-reference/peripherals/ledc.html#timer-configuration)] | HIGH | Guard verifies the spinlock progress: if LEDC channel update takes longer than the allowed guard window, abort the fade, mark heater offline, and feed the TWDT with the guard failure reason so we can log it. Also ensures the guard hooks into existing telemetry (`async-lock-depth-metrics`, `LED cycle guard`). |

### Differentiators (Competitive Advantage)

Features that set the product apart. Not required, but valuable.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Multi-stage observale watchdog transitions | Document each stage (interrupt warning, CPU reset, system+RTC reset) so operators can see whether we panicked or reset; this visibility makes safety behavior auditable | MEDIUM | TWDT stage actions are configurable via `wdt_hal_config_stage()`; log the stage before the next action fires and include stage metadata in telemetry so we can correlate logs with reset reasons. |
| Regression proven over-temp path | Replays the safety shutdown path in CI by forcing the sensor and seeing the heater/fan disable; tests the instrumentation, not just the `LEDC` duty cutoff | MEDIUM | Use harness (existing USB/test infra) to stimulate the over-temp condition and verify recorded telemetry labeled `OT-REGRESSION`; also ensures watchers remain satisfied (watchdog never fires during the regression). |
| LedcGuard event telemetry | Reports guard timeouts, the spinlock that triggered, and which cycle (100 ms control tick) was interrupted | MEDIUM | Connects to existing instrumentation that already tracks async lock depth; makes observable guarantees that safety features emit `LED-GUARD` logs and drive the `Fan` to idle state before a reset. |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem good but create problems.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Letting the watchdog silently reset without safe shutdown actions | Fastest path to get back online | Hides the root cause, leaves heater on while CPU resets (heater stays on until reset finishes, defeating safety goal) | Record the current duty, immediately zero the heaters/fans, log the reason, then allow the TWDT to reset if we still cannot recover. |
| Trusting LEDC fade-plus-spinlock to never hang | Hardware PWM fade is convenient; keeping the guard low reduces code | LEDC APIs are not thread-safe; spinlock hangs have been seen when duty updates race with other hardware interactions, risking heater lock-on | Keep the LEDC cycle guard + explicit LedcGuard that aborts fades if they exceed the 100 ms tick window. |
| Sampling temperature infrequently to avoid false positives | Reduces sensor noise/processing | Delayed response allows runaway heat between samples and may defeat regression tests that expect timely detection | Keep the sensor read on each async tick, use hysteresis thresholds plus `OT1` telemetry flag to suppress chatter while guaranteeing a safety cutoff within the 100 ms cycle. |

## Feature Dependencies

```
[TWDT + 100 ms control tick]
    └──requires──> [Async-safe cycle guard + embassy mutex protected control state]
        └──reinforces──> [Over-temp shutdown path (must feed watchdog after disabling heater)]
        └──enhances──> [LedcGuard logging (guard can feed TWDT with the guard-failure marker)]

[Over-temperature detection]
    └──requires──> [Temperature sensor handle + async sensor read scheduling] 
        └──requires──> [Instrumentation telemetry (OT1, USB harness) for observability]

[LedcGuard timeout guard]
    └──requires──> [Existing LEDC cycle guard + fan serialization (so guard can park the fan before resetting heater)]
```

### Dependency Notes

- **TWDT requires the 100 ms control tick provided by the async-safe loop** because the loop is the natural place to feed the watchdog and verify heater/fan state after each action.
- **Over-temp detection needs the async sensor read pipeline** (the same path that already uses exporter telemetry) so we have consistent readings to compare against thresholds and so the regression test can programmatically manipulate sensor inputs.
- **LedcGuard builds on the existing LEDC cycle guard + fan serialization** so that a guard timeout not only logs an error but also leaves the outputs in a known-safe configuration before the watchdog decides to panic/reset. This prevents conflicting commands when the guard and the control task both want to talk to LEDC. 

## MVP Definition

### Launch With (v1)

- [ ] `TWDT` feed tied to the 100 ms control loop (guarantee every loop either feeds the watchdog or halts before missing the deadline). Essential because without it heater lock-on cannot be detected in software.
- [ ] Over-temperature detection that instantly cuts LEDC heater duty, stops the fan briefly, and records the event + log so a human can verify the safety path. Without it we risk heater runaway.
- [ ] `LedcGuard` spinlock timeout guard that aborts fades/updates after the guard window, logs a `LED-GUARD` event, and leaves the outputs safe before the watchdog is allowed to panic/reset. Ensures the LEDC driver cannot hang the heater cycle.

### Add After Validation (v1.x)

- [ ] Telemetry dashboards that tag watchdog resets, over-temp events, and guard timeouts so operators can see patterns across roasts.
- [ ] TWDT user handles subscribed to the heater/fan tasks separately so longer-running maintenance jobs can still report progress and avoid false positives during forced sequences.

### Future Consideration (v2+)

- [ ] Multi-point temperature fusion (internal + external thermocouple) so the over-temp logic has redundant inputs before cutting the heater.
- [ ] Hardware fault injection test bench that exercises each WDT stage and ensures the regression harness detects the intended reset reason. 

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| TWDT-anchored control loop | HIGH | MEDIUM | P1 |
| Over-temperature detection + regression proof | HIGH | MEDIUM | P1 |
| LedcGuard timeout + log | HIGH | HIGH | P1 |
| Safety telemetry/observable reset logs | MEDIUM | MEDIUM | P2 |
| Regression harness for guard + TWDT interplay | MEDIUM | LOW | P2 |

**Priority key:**
- P1: Must have for launch
- P2: Should follow once P1 user value is validated

## Competitor Feature Analysis

| Feature | Competitor A | Competitor B | Our Approach |
|---------|--------------|--------------|--------------|
| Heater lock protection | Mechanical thermostat cutoff → slow hysteresis, no software visibility | Static firmware timeout that resets but leaves heaters enabled until reset completes | TWDT + LedcGuard + over-temp detection combine to trip before runaway, log the cause, and drive outputs to safe state before allowing watchdog reset. |
| Over-temperature visibility | Simple LED indicator or beeper | Vendor-specific logging requiring manual dumps | Regression-tested over-temp path that emits `OT-REGRESSION` telemetry and preserves the shutdown reason for post-mortem, making safety behavior auditable. |

## Sources

- Espressif `Watchdogs` documentation (Task Watchdog, IWDT, timeout stages, configs) — https://docs.espressif.com/projects/esp-idf/en/latest/esp32c3/api-reference/system/wdts.html
- Espressif Temperature Sensor driver (range configuration, threads, over-limit events) — https://docs.espressif.com/projects/esp-idf/en/latest/esp32c3/api-reference/peripherals/temp_sensor.html
- Espressif LEDC controller traits (timer/channel config, driver warnings about thread safety) — https://docs.espressif.com/projects/esp-idf/en/latest/esp32c3/api-reference/peripherals/ledc.html

---
*Feature research for: Watchdog safety Milestone v4.2*
*Researched: 2026-02-23*
