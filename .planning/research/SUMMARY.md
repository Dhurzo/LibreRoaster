# Project Research Summary

**Project:** LibreRoaster
**Domain:** Embedded hardware reliability for SSR/fan control with ARTISAN+ serial I/O
**Researched:** 2026-02-17
**Confidence:** MEDIUM

## Executive Summary

LibreRoaster remains a tight ESP32-C3 firmware platform that keeps Artisan in command of SSRs, fans, and telemetry while maintaining safety and responsiveness. Experts build this type of product by pairing embassy-rs async control loops with deterministic LEDC math and non-blocking UART/USB stacks so the hardware never starves for attention when a heavy roaster profile runs.

Research points the project toward a phased, reliability-first approach: solidify shared snapshots and SSR duty scheduling, strengthen the FanController/LEDC wiring with deterministic math helpers and dedicated monitors, and finally refactor the command multiplexer so DMA-backed USB/UART paths never block the executor. Each phase leans on the async stack (`esp-hal` with `embedded-io-async` and `embassy-usb`) uncovered in STACK.md and the architectural patterns described in ARCHITECTURE.md.

Key risks include SSR overheating from too-frequent cycles, LEDC timer/fade collisions, and transport-layer FIFO overflows that silence Artisan. Mitigate them by enforcing datasheet cycle times and saturating math, serializing LEDC updates through the FanController monitor, and gating command outputs behind DMA-ready futures with back-pressure awareness.

## Key Findings

### Recommended Stack

The async reliability milestone sits atop a consistent embassy-based stack: `esp-hal` drives hardware LEDC/UART with the `unstable` feature, `embedded-io-async` standardizes trait compatibility, and `embassy-usb` (plus the Synopsys OTG driver) keeps USB CDC traffic off the SSR/Fan loops. Supporting crates (`fixed`, `fugit`, `heapless`) keep math deterministic and buffers allocation-free while `embassy-executor` already serves as the runtime.

**Core technologies:**
- `esp-hal` 1.0.0 (with the `unstable` feature) — gives async LEDC/UART control so FanController and SSR logic can tune PWM without blocking.
- `embedded-io-async` 0.6.1 — ties USB CDC and UART drivers together under common traits so futures-driven tasks stay interoperable.
- `embassy-usb` 0.5.1 (with `embassy-usb-synopsys-otg` 0.3.1) — delivers lock-free CDC ACM endpoints that cooperate with the executor when streaming telemetry.

### Expected Features

FEATURES.md lays out a clear MVP: accurate SSR duty math, FanController LEDC updates, and non-blocking UART/USB I/O are table stakes, while LEDC fade ramps and SSR duty validation are differentiators. Telemetry expansion and PWM reconfiguration stay deferred to v2+ until the reliable baseline is in place.

**Must have (table stakes):**
- SSR duty math tied to LEDC PWM — Artisan expects precise power control tied to PWM resolution.
- FanController LEDC updates — fans need steady PWM frequency/atomic duty changes.
- Non-blocking UART + USB I/O — dual transports must not block the control loop.

**Should have (competitive):**
- SSR duty validation loop — verifies commanded vs applied duty and retries.
- LEDC fade-style fan ramps — smooth transitions avoid mechanical/audible shock.
- Asynchronous transport back-pressure handling — keeps Artisan formatting intact under load.

**Defer (v2+):**
- Telemetry channel highlighting SSR duty vs Artisan commands.
- Dynamic PWM frequency reconfiguration across hardware variants.

### Architecture Approach

ARCHITECTURE.md recommends splitting the firmware into `control/`, `state/`, `hardware/`, and `output/` layers running atop the embassy executor. Shared `Arc<Mutex<ControllerState>>` snapshots act as the glue between control loops, hardware drivers, and the command multiplexer so non-blocking tasks never fight over locks.

**Major components:**
1. Embassy executor + control tasks — orchestrate SSR, fan, and heater loops with soft priorities.
2. Command multiplexer + formatter traits — serialize outputs, enforce CRLF, and expose DMA-ready futures for USB/UART.
3. FanController/SSR monitors + shared snapshots — keep LEDC/SSR writes synchronized with telemetry and reliability guards.

### Critical Pitfalls

1. **SSR switching cycle violation** — enforce minimum cycle times (1–2 seconds) and gate updates so relays stay within design limits.
2. **LEDC timer and fade collisions** — dedicate timers, serialize writes, and use `ledc_set_duty_and_update` plus `ledc_find_suitable_duty_resolution` to avoid overflows.
3. **Asynchronous UART/USB ignoring back-pressure** — install UART driver with buffers and event queues, enable flow control, and keep CDC config stable to prevent FIFO overrun/reconnects.

## Implications for Roadmap

Based on research, suggested phase structure:

### Phase 1: Reliability Foundation
**Rationale:** Shared snapshots, deterministic math, and SSR duty scheduling create the non-blocking infrastructure that later phases build on, matching ARCHITECTURE.md’s “stabilize shared infrastructure” step.
**Delivers:** SSR duty math mapped to LEDC timers, FanController LEDC updates, and the reliability scheduler for steady actuator state.
**Addresses:** Table-stake features (SSR duty math, FanController updates, non-blocking UART/USB scaffolding) and pitfall 1.
**Avoids:** SSR switching cycle violation by enforcing datasheet guardrails and saturating math (`fixed`, `fugit`).

### Phase 2: Fan and Verification Enhancements
**Rationale:** After the loops are stable, add monitoring/fade helpers without disrupting the base path, mirroring the “FanController LEDC updates” and monitoring components in ARCHITECTURE.md.
**Delivers:** LEDC fade-style fan ramps, SSR duty validation/watcher, FanController LEDC monitor tied to shared snapshots.
**Uses:** Stack helpers `fixed`, `fugit`, and the LEDC driver from `esp-hal` with the async executor.
**Implements:** FanController monitor + shared snapshot synchronization to detect faded or missing writes.

### Phase 3: Async Transport Integration
**Rationale:** Non-blocking USB/UART must wait until the hardware loops are predictable, then replace blocking writes with DMA-backed command multiplexers described in ARCHITECTURE.md.
**Delivers:** Non-blocking UART + USB I/O transport tasks, asynchronous transport queues/back-pressure handling, and DMA-aware formatter futures.
**Addresses:** Non-blocking I/O features & differentiators; prepares for telemetry and debugging.
**Avoids:** Pitfall 3 by installing UART event queues, enabling flow control, and keeping USB CDC config stable.

### Phase Ordering Rationale
- Dependencies force the shared snapshots and SSR/Fan math in Phase 1 before fancier validation/fade helpers or USB/ UART refactors can safely execute.
- Architecture guides separate layers (control/state/output) so Phases 2/3 map to FanController/Formatter refinements without changing the base executor.
- The roadmap explicitly targets the top pitfalls in the order they become dangerous (cycles → LEDC → transports).

### Research Flags
Phases likely needing deeper research during planning:
- **Phase 2:** LEDC timer/fade collisions need hardware verification and safeguards around `ledc_set_duty` concurrency.
- **Phase 3:** USB CDC resilience (sleep/pin changes, debugger halts) and UART overflow handling demand live validation with Artisan hosts.

Phases with standard patterns (skip research-phase):
- **Phase 1:** Embassy executor + shared snapshot pattern are well-documented via ARCHITECTURE.md and existing code, so no extra research is needed.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | MEDIUM | Docs.rs sources (esp-hal, embassy-usb, embedded-io-async) are solid but rely on current dependencies. |
| Features | MEDIUM | ESP-IDF docs and existing code provide coverage, but some behaviors (fade helpers, validation loop) still need prototyping. |
| Architecture | MEDIUM | Architecture document is internally consistent but lacks external validation (no new docs beyond project context). |
| Pitfalls | MEDIUM | Based on ESP-IDF v5.5.2 guidance and Infoneva field notes; should be verified on actual hardware. |

**Overall confidence:** MEDIUM

### Gaps to Address
- **LEDC timing/resolution trade-offs:** Confirm `ledc_find_suitable_duty_resolution` with the planned 25 kHz/8-bit config and ensure fades complete before new writes.
- **USB CDC stability during debugging/sleep:** Validate that the Synopsys OTG driver + `embassy-usb` stack survive debugger pauses and avoid phantom disconnects described in PITFALLS.md.

## Sources

### Primary (HIGH confidence)
- https://docs.rs/esp-hal/latest/esp_hal/ — esp-hal peripheral docs covering async LEDC/UART features used in the stack.
- https://docs.rs/embassy-usb/latest/embassy_usb/ — embassy-usb async CDC ACM stack documentation.
- ESP-IDF LEDC/UART/USB OTG guides (v5.5.2) referenced across PITFALLS.md for timer/fade/thread-safety and overflow/pin stability warnings.

### Secondary (MEDIUM confidence)
- https://docs.rs/embedded-io-async/latest/embedded_io_async/ — async trait alignment between UART and USB.
- INFONEVA cycle-time guide (July 18, 2025) for SSR switching limits.

### Tertiary (LOW confidence)
- Project context notes in ARCHITECTURE.md (executor, shared snapshots, controllers) inferred from internal architecture but without external verification.

---
*Research completed: 2026-02-17*
*Ready for roadmap: yes*
