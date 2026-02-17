# Feature Research

**Domain:** Embedded hardware reliability (SSR duty, FanController, async I/O)
**Researched:** 2026-02-17
**Confidence:** MEDIUM

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist. Missing these = product feels incomplete.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| SSR duty math tied to LEDC PWM | Heater control is driven by SSRs; users expect power level to follow commands precisely, or temperatures overshoot/undershoot | MEDIUM | Map Artisan duty requests to LEDC timers (freq+resolution) so 0‑100% maps to 0‑255 duty, then use PWM update cycle to avoid jitter. See ESP-IDF LEDC timing constraints for meaningful frequency/resolution choices. |
| FanController LEDC updates | Fan speed adjustments must reach the hardware without jumps; fans tolerate steady PWM frequencies only | MEDIUM | Update the configured LEDC channel (timer 25 kHz/8-bit) any time FanController sees a new speed value; ensure `set_duty`/`update_duty` pairing is atomic so the PWM frequency stays constant. |
| Non-blocking UART + USB I/O | Dual outputs already exist (USB CDC + UART multiplexer); blocking serial writes would stall command handling under load | HIGH | Install UART driver with event queues and keep USB CDC writes off the main control stack, borrowing the async UART task pattern from `peripherals/uart/uart_async_rxtxtasks`. |

### Differentiators (Competitive Advantage)

Features that set the product apart. Not required, but valuable.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| SSR duty validation loop | Detects mismatches between commanded and applied duty and recovers before temperature swings are visible | MEDIUM | Sample `current_duty` from the shared PWM state, compare to requested percentage, and re-issue LEDC updates until within tolerance; takes advantage of LEDC duty-resolution helpers to skip unsupported combo. |
| LEDC fade-style fan ramps | Smooth fan transitions protect mechanical hardware and avoid audible noise spikes | MEDIUM | Use `ledc_set_fade`/`ledc_fade_start` when FanController requests large delta so the hardware interpolates; fall back to direct `set_duty` for small adjustments. |
| Asynchronous I/O back-pressure handling | Keeps Artisan-formatting tests passing while avoiding dropped bytes even when serial output is busy | HIGH | Drive USB CDC and UART writers through RTOS-safe queues & callbacks instead of synchronous `write_bytes`; let transport tasks signal when FIFO drains so control tasks never await serial completions. |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem good but create problems.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Polling delays between SSR adjustments | “Just wait and toggle the SSR again” | Blocks the control loop and still misses precise PWM timing, so temperature regulation oscillates | Update PWM duty once per control cycle with LEDC helpers; leverage LEDC timer resolution to respect frequency laws. |
| Blocking UART/USB writes | “Simpler code; just write and wait” | Serial writes become serialization points that stall heating/fan tasks, leading to missed commands when Artisan floods the bus | Use interrupt-driven drivers with queues and event handlers so transport layers notify when ready. |
| Excessive telemetry on UART while tuning | “More debug prints give confidence” | Floods UART/USB path, interferes with Artisan framing, and can corrupt commands | Emit telemetry only via dedicated debug channel or log level gating; keep operational command path minimal. |

## Feature Dependencies

```
SSR duty math
    └──requires──> FanController LEDC timer config (25 kHz, 8-bit)
                       └──requires──> LEDC global slow clock + duty resolution helpers

FanController LEDC updates
    └──requires──> GPIO pin (FAN_PWM_PIN) and LEDC channel ownership

Non-blocking UART/USB I/O
    └──requires──> uart_driver_install + USB CDC stack + dual-output multiplexer config

LED fade-style ramp
    └──enhances──> FanController LEDC updates

Asynchronous transport queues
    └──enhances──> Non-blocking UART/USB I/O
```

### Dependency Notes

- **SSR duty math requires LEDC timer config:** Duty updates only obey expected percentages when timers provide the frequency/resolution pair supported by the hardware (per LEDC doc). If the timer or clock source changes, the mapping must be recalculated.
- **FanController LEDC updates need GPIO ownership:** The fan PWM pin must remain bound to a single LEDC channel; any reconfiguration (e.g., board variants without LEDC) means falling back to placeholder logic.
- **Non-blocking I/O depends on driver install:** Both UART and USB CDC paths require event queues so that transport tasks can signal readiness without stopping the control/event loop.
- **LED fade ramps enhance FanController updates:** Smoothing for large deltas reduces mechanical stress and audible noise without rewriting the base `set_duty` path.
- **Async queues enhance blocking avoidance:** Queuing ensures that when one transport is busy (USB CDC with host unresponsive), the other channel and the control loop continue running.

## MVP Definition

### Launch With (v1)

Minimum viable product — what's needed to validate the concept.

- [ ] SSR duty math mapped to LEDC timers — accurate 0–100% range with LEDC helper utilities
- [ ] FanController LEDC updates for actual hardware fan control — harness existing LEDC channel and GPIO
- [ ] Non-blocking UART + USB I/O transport tasks — driver event queues replace synchronous writes

### Add After Validation (v1.x)

Features to add once core is working.

- [ ] LEDC fade-based fan ramps for large deltas — use `ledc_set_fade` + callbacks when transitioning between extremes
- [ ] SSR duty verification/guardrail task — monitor PWM state and re-issue updates when actual duty drifts

### Future Consideration (v2+)

Features to defer until product-market fit is established.

- [ ] Telemetry channel that reports real‑time SSR duty vs Artisan commands — useful for field diagnostics but not required for control
- [ ] Dynamic reconfiguration of PWM frequency per hardware variant — adds flexibility but complicates calibration across boards

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| SSR duty math accuracy | HIGH | MEDIUM | P1 |
| FanController LEDC updates | HIGH | MEDIUM | P1 |
| Non-blocking UART/USB | HIGH | HIGH | P1 |
| LEDC fade-style fan ramps | MEDIUM | MEDIUM | P2 |
| SSR duty watchdog | MEDIUM | MEDIUM | P2 |

**Priority key:**
- P1: Must have for launch
- P2: Should have, add when possible
- P3: Nice to have, future consideration

## Competitor Feature Analysis

| Feature | Competitor A | Competitor B | Our Approach |
|---------|--------------|--------------|--------------|
| SSR duty control | Artisan host leaves SSR to controller with simple delays | Legacy roaster firmwares often use blocking timers and no smoothing | Map Artisan duty percentages to LEDC timers with helpers; keep PWM updates non-blocking and verify actual duty values. |
| Fan speed PWM | Generic fans on USB controllers usually have fixed PWM frequency, causing noise when toggled | Raspberry Pi-based controllers toggle fans via GPIO without duty smoothing | Use LEDC with fixed 25 kHz/8-bit config plus optional fade helpers for smooth transitions. |
| Serial transport behavior | Artisan USB driver is streaming but can block if host stalls | UART-only firmwares drop bytes when overrun due to poll loops | Install UART driver with event queues and drive USB CDC through asynchronous tasks so both paths stay responsive. |

## Sources

- ESP-IDF LED Control (LEDC) documentation, esp-idf v5.2: https://docs.espressif.com/projects/esp-idf/en/latest/esp32c3/api-reference/peripherals/ledc.html
- ESP-IDF UART driver overview (async queue pattern), esp-idf v5.2: https://docs.espressif.com/projects/esp-idf/en/latest/esp32c3/api-reference/peripherals/uart.html
- `src/hardware/fan.rs` (current FanController + LEDC wiring) for existing PWM configuration and placeholders

---
*Feature research for: Embedded hardware reliability (SSR, fan, async serial)*
*Researched: 2026-02-17*
