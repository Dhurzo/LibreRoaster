# Pitfalls Research

**Domain:** Embedded hardware reliability for SSR duty control, fan LEDC updates, and non-blocking UART/USB transports
**Researched:** 2026-02-17
**Confidence:** MEDIUM

## Critical Pitfalls

### Pitfall 1: SSR switching cycle violates relay limits

**What goes wrong:**
Controlling the heater with very short cycle times (<<1 second) or using phase-angle bursts without throttling makes the SSR switch more often than it was designed for. The SSR overheats, EMI rises, and the apparent duty becomes inaccurate because the relay never reaches the intended steady ON/OFF proportion before the next cycle begins.

**Why it happens:**
Hands-on tuning tends to shorten the cycle time to chase faster temperature response, but Infoneva’s July 18, 2025 field guide for Eurotherm controllers shows that most SSRs are happiest with 1–2 second cycle times; shorter settings visibly increase switching noise, heat, and mode switching efforts.

**How to avoid:**
Enforce a minimum total cycle (ON+OFF) per SSR per the datasheet (start with 1–2 seconds) and gate duty updates so they only happen when the SSR is stable. Monitor SSR case temperature and electrical noise to verify the chosen cycle time. Zero-cross SSRs should run with these longer windows to give the switch a full AC half-cycle.

**Warning signs:**
SSR case temperature climbs even at moderate load, audible buzzing appears, EMI spikes show up on the mains, and logs emit frequent duty changes without corresponding output change.

**Phase to address:**
Phase 1 (Hardware reliability): define cycle-time guardrails before exposing duty adjustments.

---

### Pitfall 2: LEDC updates break when timers or fades collide

**What goes wrong:**
Fan speed or LED updates silently ignore duty writes, overflow hardware counters, or fall back to the same frequency for every channel because timers are shared inadvertently or because updates clash with ongoing fades.

**Why it happens:**
ESP-IDF’s LEDC documentation (v5.5.2) notes that the timer frequency and duty resolution are interdependent and that the hardware cannot set the duty cycle to exactly (2 ** resolution) when the resolution is already at its maximum—doing so causes an internal counter overflow. The same doc warns that `ledc_set_duty`, `ledc_set_duty_with_hpoint`, and `ledc_update_duty` are not thread-safe and that the module ignores duty changes while a fade is running in the hardware. Many teams reuse timers or call these APIs from multiple tasks, which steals PWM updates from the fan path.

**How to avoid:**
Plan timer allocation explicitly (one timer per independent frequency or resolution needs) and use `ledc_find_suitable_duty_resolution()` when selecting parameters. Route all fan updates through a single task or protect them with a mutex and switch to the thread-safe `ledc_set_duty_and_update()` if concurrent writers are unavoidable. Gate fades so other duty writes wait until the fade completes; clear and reconfigure a channel before writing new frequency/resolution to avoid the overflow warning.

**Warning signs:**
Firmware logs report errors like `E (196) ledc: requested frequency and duty resolution cannot be achieved`, every LEDC channel suddenly shares the same PWM frequency, or duty writes from one task have no effect when another fade is active.

**Phase to address:**
Phase 2 (Fan controller LEDC updates and test harness).

---

### Pitfall 3: Asynchronous UART/USB ignores backpressure

**What goes wrong:**
The new non-blocking UART/USB transport drops bytes, floods the existing Artisan parser, or the USB CDC console disappears during roasts because the FIFOs overflow or the peripheral is reconfigured unexpectedly.

**Why it happens:**
ESP-IDF’s UART driver guide (v5.5.2) makes clear that you must install the driver with RX/TX ring buffers, enable `UART_FIFO_OVF`/timeout interrupts, and drain them via the event queue. Skipping that leaves the hardware FIFO blind to host bursts, so `UART_FIFO_OVF` fires and data vanishes. The USB OTG console guide (v5.5.2) warns that if the application reconfigures USB pins, disables the peripheral, or enters light/deep sleep, the CDC device disappears and must be re-flashed; it also notes that the ROM implementation relies on serviced USB interrupts, so halting the CPU (e.g., for debugging) can cause host-side disconnects.

**How to avoid:**
Allocate sufficient UART buffers, process the event queue to acknowledge `UART_FIFO_OVF`, enable hardware flow control when the host can’t keep up, and never share the port with blocking readers/writers. Keep USB pins and the peripheral configuration stable, avoid deep/light sleep while CDC is active, and guard panic logging because USB CDC may silently drop early logs. Schedule console writes through the same async queue that the rest of the transport uses so Artisan parsing has predictable timing.

**Warning signs:**
The UART event queue reports `UART_FIFO_OVF`, throughput dips when host traffic spikes, USB CDC disappears after a sleep/pin change or while the debugger halts, and Artisan logs show missing commands right before the disconnect.

**Phase to address:**
Phase 3 (Non-blocking transport integration with Artisan protocol stack).

---

## Technical Debt Patterns

Shortcuts that seem reasonable but create long-term problems.

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Racing the SSR duty callback to zero waiting time | Faster apparent control loop | SSR heating, EMI, and premature failure | Never for hardware safety fixes |
| Sharing a single LEDC timer across unrelated fans/LEDs | Fewer timers consumed | Forces every channel to use the same frequency/resolution, adding jitter when secondary outputs need different behavior | Only when outputs must stay phase-locked and their frequency/resolution requirements match exactly |
| Replacing blocking serial reads with non-blocking calls without installing the UART driver or queue | Cleaner async API | Silent FIFO overflows, desynchronized Artisan frames, opaque bugs | Never—blocking tasks must be replaced with full async queue setup |

## Integration Gotchas

Common mistakes when connecting to external services.

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| USB OTG console vs TinyUSB | Enabling TinyUSB and rom-CDC simultaneously, then expecting the USB monitor to stay alive | Choose one stack—rom-CDC or TinyUSB—and disable the other. The USB console docs explicitly say the ROM CDC implementation cannot coexist with TinyUSB descriptors. |
| USB CDC and sleep/pin reconfiguration | Reconfiguring USB pins or letting the chip enter light/deep sleep while CDC is active, which makes the device disappear until reflashed | Keep the USB peripheral/pins fixed while CDC is enabled, avoid sleep modes, and document the initial upload re-flash path for recovery (ESP-IDF USB OTG console guide). |
| Artisan UART port reuse | Plugging async Artisan parsing into the same UART port without reinstalling the driver or event queue | Call `uart_driver_install()` once with RX/TX buffers and an event queue, handle `UART_FIFO_OVF` events, and let the existing Artisan transport share that queue rather than reinitializing the port. |

## Performance Traps

Patterns that work at small scale but fail as usage grows.

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Setting LEDC frequency too high | Duty resolution collapses (quantization, 50% steps) and fan speed feels coarse or wiggles | Use `ledc_find_suitable_duty_resolution()` and pick a frequency/resolution pair that keeps the required granularity | When the PWM frequency exceeds the resolution budget (see ESP-IDF LEDC doc error about impossible frequency/duty combos) |
| Ignoring UART FIFO overflow events | Artisan commands vanish under bursty input and the parser pauses waiting for missing bytes | Enable `UART_FIFO_OVF`/timeout interrupts, drain the event queue, and scale buffer sizes to the measured peak load | When a host floods the serial line faster than the ISR can empty the FIFO (e.g., debug logging turns up) |
| USB CDC disconnects during debugging | Monitor output freezes, the host drops the device if the CPU stops answering USB interrupts | Avoid halting USB-interrupt-using tasks for multiple milliseconds; prefer log buffers when paused | When breakpoints stop the CPU for hundreds of milliseconds, allowing host-side timeouts to fire (USB OTG console note) |

## Security Mistakes

Domain-specific security issues beyond general web security.

| Mistake | Risk | Prevention |
|---------|------|------------|
| Exposing SSR/fan controls via USB CDC without host authorization | A malicious USB host can immediately alter heater/fan duty | Keep CDC console off in production images, enable it via config flag only in dev builds, and gate command parsing with authentication/whitelisting. |
| Disabling UART flow control while keeping the port open | Hosts can overrun the UART, making firmware execute stale or malformed Artisan frames that trigger dangerous SSR/fan commands | Use RTS/CTS via `uart_set_hw_flow_ctrl()` when connecting to third-party hosts and validate every frame before touching hardware. |

## UX Pitfalls

Common user experience mistakes in this domain.

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Fan jitter because PWM resolution collapses at high frequency | Fans make noise and roast profiles fluctuate | Choose frequency/resolution pairs that keep at least 256 levels (see LEDC docs) and smooth transitions with fade helpers when needed. |
| SSR/heater response that looks precise but is unreliable | Roasters have to re-tune manually to compensate for overheating or buzzing | Surface monitored SSR temperature and cycle time warnings in diagnostics so operators can adapt settings. |
| USB console vanishes after sleep/pin change | Developer can’t debug roasts reliably when the console silently disappears | Document the initial upload path and avoid reconfiguring USB pins to keep the CDC device alive (per USB OTG console guide). |

## "Looks Done But Isn't" Checklist

Things that appear complete but are missing critical pieces.

- [ ] **SSR duty tuning:** Often set to the fastest possible cycle—verify with cycle-time guardrails and temperature monitoring.
- [ ] **LEDC reconfiguration:** Often assumes the hardware can swap frequencies or duties instantly—verify that fades complete and that timers don’t overflow or share frequencies unexpectedly.
- [ ] **Non-blocking UART:** Often swaps to `uart_read_bytes()` with zero queue—verify an event queue processes `UART_FIFO_OVF` and timeouts, and that flow control is engaged.
- [ ] **USB CDC console:** Often assumed to work if it boots once—verify it survives light/deep sleep, pin reconfig, and debugger pauses, and have a reflash recovery plan. 

## Recovery Strategies

When pitfalls occur despite prevention, how to recover.

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| SSR cycle too short | HIGH | Revert to datasheet cycle time, log SSR temperature, reintroduce zero-cross gating, and schedule a long(er) cycle before permissive commands. |
| LEDC updates blocked by fades/overflow | MEDIUM | Pause fades, reinitialize the timer, realign frequency/resolution with `ledc_find_suitable_duty_resolution()`, and serialize duty writes via a single task. |
| UART/USB FIFO overflow | MEDIUM | Flush the ring buffers, enlarge the queue, re-enable interrupts (`uart_enable_rx_intr()`), and re-sync Artisan commands by sending a known delimiter. |

## Pitfall-to-Phase Mapping

How roadmap phases should address these pitfalls.

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| SSR switching cycle violation | Phase 1: Hardware reliability + SSR tuning | Simulate extended cycle times in hardware tests and monitor SSR temperature/EMI. |
| LEDC timer/call collisions | Phase 2: Fan controller LEDC refactor | Run fan speed sweep tests while exercising fades and examine ESP-IDF LEDC error logs for overflow. |
| Non-blocking UART/USB overflow | Phase 3: Async transport integration | Flood the UART/USB path and confirm `UART_FIFO_OVF`/CDC disconnects are handled gracefully; log when USB disappears. |

## Sources

- Infoneva knowledge base, “Controller Cycle Time and SSR Performance in Eurotherm Controllers,” July 18, 2025 (cycle-time recommendations and SSR wear).  
- ESP-IDF Programming Guide v5.5.2 – LED Control (LEDC) API reference (frequency/duty constraints, thread-safety notes).  
- ESP-IDF Programming Guide v5.5.2 – Universal Asynchronous Receiver/Transmitter (UART) (FIFO overflow event, driver installation requirements).  
- ESP-IDF Programming Guide v5.5.2 – USB OTG Console (USB CDC fragility, sleep/pin/recovery warnings).  

---
*Pitfalls research for: Embedded hardware reliability (SSR, fan LEDC, UART/USB transports)*
*Researched: 2026-02-17*
