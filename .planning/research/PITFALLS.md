# Pitfalls Research

**Domain:** Embedded safety instrumentation for ESP32-C3 watchdog, over-temp, and LEDC guards
**Researched:** 2026-02-23
**Confidence:** MEDIUM

## Critical Pitfalls

### Pitfall 1: Heavy flash/guard work trips the Task WDT instead of the safety handler

**What goes wrong:** Erasing large flash regions, running instrumentation that polls sensors, or executing long LedcGuard sequences still blocks the idle task and causes `Task watchdog got triggered` logs before safety logic can run.

**Why it happens:** Espressif documentation explicitly calls out that erasing large flash areas or any lengthy, non-yielding task can make the Task WDT fire; the same MWDT timer backs both the Interrupt and Task WDTs, so any CPU that ignores `esp_task_wdt_reset` (or lets idle run) is considered hung. A concrete reproduction is the SPIFFS format example that kept wiping external flash and hit Task WDT every 5 s even though the operation eventually completed ([esp-idf#8135](https://github.com/espressif/esp-idf/issues/8135)).

**How to avoid:** Tune `CONFIG_ESP_TASK_WDT_TIMEOUT_S` or call `esp_task_wdt_init()` with a larger timeout before known heavy work, feed the WDT via `esp_task_wdt_reset()` from the long-lived task, or move the work to a lower-priority worker that yields often. For flash erases and the LedcGuard fade loop, add explicit yield points (`taskYIELD`/`vTaskDelay(1)`) so the idle task can run and keep the watchdog fed.

**Warning signs:** Repeated `task_wdt` panic prints during formatting/regression tests, especially when CPU0 shows the backtrace inside flash erase or your guard loops; the messages align with the duration of the long operation rather than a random bug.

**Phase to address:** Phase "v4.2 Watchdog Integration" (before enabling TWDT feed and guard loops in production.)

---

### Pitfall 2: Tight sampling or guard loops starve the idle task and trigger TWDT resets

**What goes wrong:** New watchdog/timeout loops (e.g., the over-temperature regression thread or LedcGuard watchdog) poll a sensor or hardware state continuously without yielding. The Task WDT sees the idle task never running and issues a reset even though the code is logically alive.

**Why it happens:** FreeRTOS on ESP32 does not round-robin by default; long loops must explicitly yield. As described in the Stack Overflow thread on `ESP32 Task Watchdog Triggered`, failing to call `taskYIELD()`/`vTaskDelay()` from a loop that does 100 k sensor reads causes the Task WDT to fire, even if the loop itself completes later.

**How to avoid:** Break loops into smaller batches, insert `taskYIELD()`/`vTaskDelay(1)`, or call `esp_task_wdt_reset()` while waiting for the next sample or guard timeout. If you need 1 kHz sampling, consider scheduling the guard on a dedicated FreeRTOS timer task so other tasks (and the idle hook) get CPU time. Also ensure sensors or LEDC guard operations do not hold mutexes that prevent `esp_task_wdt_reset` from running.

**Warning signs:** `task_wdt` prints show `CPU 0: main` or `LedcGuard` backtraces, loop durations that are multiples of the watchdog timeout, and instrumentation logs that stop during the busy loop.

**Phase to address:** Phase "Sensor/Timeout Tuning" (as soon as the new guard loops are drafted).

---

### Pitfall 3: Treating the internal temperature sensor as an accurate over-temp trip without guarding its ISR

**What goes wrong:** The regression test or the production guard triggers on builtin temperature thresholds that do not reflect ambient conditions (chip temperature only), or the ISR never fires because the cache is disabled during flash/timing-critical operations.

**Why it happens:** Espressif’s temperature sensor doc cautions the sensor is optimized for chip temperature and is not precise for ambient measurements; it also notes that threshold callbacks rely on interrupts that are deferred when cache is disabled unless `CONFIG_TEMP_SENSOR_ISR_IRAM_SAFE` is enabled and callbacks are IRAM-resident, so a cache flush or flash erase can silently block the interrupt [^temp].

**How to avoid:** Calibrate over-temperature limits using the chip temperature relative to your roast (log both chip sensor and external reference), treat the reading as a relative change instead of an absolute, and enable `CONFIG_TEMP_SENSOR_ISR_IRAM_SAFE` so the callback fires even during flash-intensive test setups. Keep the callback small and IRAM-safe to avoid missing the alert when the cache is off.

**Warning signs:** Threshold callbacks never execute during instrumentation (no log lines even when chip temperature rises), transmissions from the regression test show frozen stack traces around `temperature_sensor_monitor_cbs`, or the guard trips long before the roast’s thermocouple does.

**Phase to address:** Phase "Over-Temperature Regression" (before the regression test is used to gate releases).

---

### Pitfall 4: LedcGuard waits forever for fades that cannot be interrupted, starving the watchdog

**What goes wrong:** LedcGuard holds hardware/state locks while waiting for a fade to finish and then tries to confirm completion by polling, preventing the idle task from running and the TWDT from being reset.

**Why it happens:** The LEDC driver documentation states that once a fade starts, there is no way to stop it before it reaches the target duty (the hardware doesn’t offer a preempt) [^ledc]. Guard logic that busy-waits for the fade or repeatedly calls `ledc_set_duty` with no yield will therefore monopolize the CPU.

**How to avoid:** Restructure LedcGuard to register a fade-end callback via `ledc_cb_register`, let the callback or a timer task signal completion, and keep the guard’s critical section short so FreeRTOS can run idle. When polling completion is unavoidable, insert `taskYIELD()`/`vTaskDelay(1)` and call `esp_task_wdt_reset()` during the wait.

**Warning signs:** LedcGuard’s logs show repeated fade requests and the Task WDT triggers with backtraces involving `ledc_fade_start`. The guard may also hold mutexes that stop async sensor tasks from feeding the watchdog.

**Phase to address:** Phase "LedcGuard Timeout" (final phase for PWM-based safety controls).

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Cranking up TWDT timeout until heavy operations finish | Tests stop failing | Hides real blocking operations and leaves production blind | Only as a stop-gap while refactoring the blocking path |
| Busy-loop guard waiting for fade/sensor without yielding | Simplifies guard state machine | Starves idle task, triggers TWDT, and gutters instrumentation | Never—prefer a deferred callback or timer |
| Logging inside `esp_task_wdt_isr_user_handler` to surface faults | Easy debugging output | ISR limitations (no `ESP_LOGx`, risk of reentries) and possible stack corruption | Only if you move logging to a non-ISR-safe path and keep ISR minimal |

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Flash/partition operations + Task WDT | Erase/format runs as part of regression test and trips WDT | Either increase TWDT timeout with `esp_task_wdt_init` or split work across yield points before running the erase, as noted in the ESP-IDF task watchdog guide and issue #8135 |
| Async sensor loops + TWDT | Sensor sampling floods CPU and never yields, starving the idle task | Insert `vTaskDelay(1)`/`taskYIELD()` or call `esp_task_wdt_reset()` inside the loop (Stack Overflow thread) so FreeRTOS can feed the watchdog |
| LedcGuard + LEDC fades | Guard polls fade completion without IRAM-safe callbacks, locking timers | Register fade-end callbacks via `ledc_cb_register`, keep callbacks in IRAM, and avoid busy waits (LEDC doc) |

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Tight sampling loops without yielding | `task_wdt` fires, CPU0 stuck in guard, other tasks starve | Break work into chunks, add `taskYIELD()`/`vTaskDelay(1)` between iterations | When total loop runtime > TWDT timeout (often just a few seconds) |
| Temperature threshold ISR skipped during cache-disabled flash ops | Callback never runs despite sensor crossing threshold, regression test fails | Enable `CONFIG_TEMP_SENSOR_ISR_IRAM_SAFE` and keep callback and helpers in IRAM | When tests erase flash or disable cache (common in regression harness) |

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Leaving TWDT configured to only warn instead of panic | Heater or actuator keeps running after timeout because system just logs and keeps going (default behavior) | Set `CONFIG_ESP_TASK_WDT_PANIC` or handle `esp_task_wdt_isr_user_handler` so the WDT forces a safe reset; avoid merely logging in the ISR |

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Over-temp guard based on uncalibrated chip sensor | Operators see alarms that drift with ambient temperature or never fire because chip temp lags load | Present chip temp as relative delta, add external reference, and document why thresholds do not match thermocouple readings (per sensor doc) |

## "Looks Done But Isn't" Checklist

- [ ] **TWDT options:** Timeout increased but tasks still never call `esp_task_wdt_reset()`; confirm the long path feeds the watchdog or restructures the work.
- [ ] **LedcGuard fade wait:** Guard says fade finished, but LEDC hardware was still running; inspect callback/interrupt instead of polling.
- [ ] **Over-temp ISR:** Callback registered but never executes during flash erase; verify `CONFIG_TEMP_SENSOR_ISR_IRAM_SAFE` is set and callback lives in IRAM.
- [ ] **Async sensors:** New watchdog added but asynchronous sampling tasks not subscribed to TWDT, so the guard only sees the idle task timing out.

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Flash/guard work tripping TWDT | MEDIUM | Split the workload, make it yield-friendly, and add `esp_task_wdt_reset()` before/after the heavy block. |
| Tight guard loops stalling idle | LOW | Insert delays/yields, or move guard onto a dedicated timer task; re-run instrumentation to confirm idle hook runs. |
| Missed over-temp interrupt | MEDIUM | Move callback to IRAM, enable `CONFIG_TEMP_SENSOR_ISR_IRAM_SAFE`, and run regression tests while the cache is disabled to ensure the ISR fires. |
| LedcGuard busy wait | MEDIUM | Use fade-end callbacks, limit guard to state transitions, and monitor `ledc_get_duty` from a yielding task. |

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Pitfall 1 (heavy flash) | Phase "v4.2 Watchdog Integration" | Run flash/format regression with instrumentation enabled and confirm no `task_wdt` logs. |
| Pitfall 2 (tight loops) | Phase "Sensor/Timeout Tuning" | Stress-test guard loops at target sample rates and verify idle hook `esp_task_wdt_reset()` runs regularly. |
| Pitfall 3 (temperature sensor) | Phase "Over-Temperature Regression" | Disable caches/flash, raise chip temp, and confirm the threshold callback fires and maps to the calibration that ops see. |
| Pitfall 4 (LedcGuard fade) | Phase "LedcGuard Timeout" | Start a fade from the guard and ensure it completes via callback without blocking `ledc_cb_register`. |

## Sources

- ESP-IDF Task Watchdog documentation (timeout tuning, default behavior, `CONFIG_ESP_TASK_WDT_PANIC`): https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/wdts.html
- SPIFFS format watchdog issue showing TWDT firing during flash erase: https://github.com/espressif/esp-idf/issues/8135
- Stack Overflow thread explaining busy loops must yield to avoid task watchdog: https://stackoverflow.com/questions/78614192/esp32-task-watchdog-triggered
- ESP-IDF temperature sensor guide (chip-only accuracy, IRAM-safe callbacks): https://raw.githubusercontent.com/espressif/esp-idf/master/docs/en/api-reference/peripherals/temp_sensor.rst
- ESP-IDF LEDC guide (fades cannot be interrupted, callback placement): https://raw.githubusercontent.com/espressif/esp-idf/master/docs/en/api-reference/peripherals/ledc.rst

---
*Pitfalls research for: Watchdog and safety guards on LibreRoaster’s ESP32-C3 stack*
*Researched: 2026-02-23*
