# Stack Research

**Domain:** LibreRoaster hardware reliability (SSR duty clamps, LEDC fan control, responsive UART/USB)
**Researched:** 2026-02-17
**Confidence:** MEDIUM

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| `esp-hal` (with the `unstable` feature) | 1.0.0 | Direct LEDC timers and async UART for ESP32-C3 | Docs show `ledc` lives behind `unstable` while `uart` implements `embedded-io-async`/`embedded-hal-async`, so enabling it gives FanController direct access to hardware PWM channels plus non-blocking serial primitives needed for the command multiplexer. |
| `embedded-io-async` | 0.6.1 | Byte-stream traits for async UART & USB CDC | Both `esp-hal::uart` and `embassy-usb` surface these traits, so reusing the same version keeps futures-based reads/writes compatible and lets the executor poll I/O without blocking SSR math updates. |
| `embassy-usb` | 0.5.1 | Asynchronous CDC ACM stack | Native async, lock-free endpoints, and built-in CDC class keep USB traffic off the critical SSR/Fan tasks; it integrates with the existing `embassy-executor` and reuses `embedded-io-async` so the executor stays responsive while USB transfers happen. |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `fixed` | 1.30.0 | Deterministic duty math with saturating arithmetic | FanController/SSR duty clamp routines use `fixed::Saturating` + static scaling to map 0..1 duty requests into LEDC resolution without overflow or floating round-off. |
| `fugit` | 0.3.9 | Frequency/duty conversions (`Rate`, `Duration`) | LEDC configuration examples already use `fugit::Rate::from_khz`, so reuse the same crate when computing timer ticks per SSR increment to keep hardware math aligned with controller units. |
| `embassy-usb-synopsys-otg` | 0.3.1 | Synopsys OTG driver for `embassy-usb` | Required glue for ESP32-C3 USB controller; use it when instantiating `embassy_usb::Builder` so the async stack talks to the on-chip hardware. |
| `heapless` | 0.8.0 | Allocate-free command / event buffers | Ring buffers such as `heapless::spsc::Queue` decouple UART/USB producers from the executor-driven consumers, so the non-blocking I/O paths never allocate and stay deterministic. |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| `embassy-executor` | Async runtime | Already present (`0.9.1`); keep in sync so LEDC/USB/UART tasks continue to run cooperatively without blocking the executor. |
| `cargo test --features embedded` | Validate async paths | Use hardware regression tests to exercise the non-blocking UART/USB stack and duty clamp logic before deploying. |

## Installation

```bash
# Add the USB + async helpers needed for the new milestone
cargo add embassy-usb@0.5.1 embassy-usb-synopsys-otg@0.3.1 embedded-io-async@0.6.1 fixed@1.30.0
```

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| `embassy-usb` + `embedded-io-async` | `usb-device` + manual `poll` loops | Only when rewriting the whole USB stack to a simpler blocking driver (e.g., for a throwaway prototype) and you can tolerate executor starvation. |
| `fixed` (saturating) + `fugit::Rate` | `f32` + `u32::saturating_*` | If FIR precision is not critical and you must drop the extra crate weight, but expect rounding errors and inefficient float math to complicate SSR clamp tests. |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| Blocking loops around `nb::block!`/`embedded-io::Read::read` | They starve the executor and delay LEDC updates, which is exactly what the milestone forbids | Use the async `embedded-io-async` traits and await readiness inside executor tasks so SSR math can continue during I/O. |
| `usb-device` synchronous `poll` strategy | Polling USB every cycle keeps the main loop busy and negates the new non-blocking guarantee | Let `embassy-usb` handle USB interrupts and futures; it already provides CDC ACM and cooperates with other async work. |

## Stack Patterns by Variant

**If clamping SSR duty for hardware safety:**
- Use `fixed::Saturating<FixedU16<_>>` together with `fugit::Rate` to derive the PWM steps that match LEDC resolution.
- Because deterministic, saturating math avoids wrapping duty and keeps the SSR within safe duty/time windows even under rapid setpoint changes.

**If USB/serial traffic must stay responsive:**
- Use `esp-hal::uart` in `Async` mode plus `embassy-usb` CDC under the `embassy-executor` runtime, all wired through `embedded-io-async` traits.
- Because the executor can now poll each stream independently and never waits on a blocking UART/USB transfer, keeping instrumentation and command handling alive while SSR math runs.

## Version Compatibility

| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| `esp-hal@1.0.0` | `embedded-io-async@0.6.1`, `embassy-sync@0.7.2` | Async UART/LEDC drivers were built with these versions; enabling `unstable` unlocks `ledc` while the async traits stay on 0.6.1 to match `embassy-usb`. |
| `embassy-usb@0.5.1` | `embedded-io-async@0.6.1`, `embassy-sync@0.7.2` | The docs list these exact dependencies, so keep the dependency graph aligned to avoid duplicate versions. |
| `embassy-usb-synopsys-otg@0.3.1` | `esp-hal@1.0.0` | esp-hal already exposes this driver, so add it once and re-use the HAL’s initialization. |
| `fixed@1.30.0` | Rust ≥ 1.85 (project uses 1.88) | The crate requires at least Rust 1.85; our toolchain already meets that, so no conflicts. |
| `fugit@0.3.9` | `esp-hal` timers | esp-hal examples use `fugit::Rate`, so staying on this release keeps conversions matching the HAL. |

## Sources

- https://docs.rs/esp-hal/latest/esp_hal/index.html — esp-hal peripheral overview, async/unstable features, LEDC + UART documentation (HIGH)
- https://docs.rs/esp-hal/latest/esp_hal/ledc/index.html — LEDC driver behind the `unstable` feature (HIGH)
- https://docs.rs/esp-hal/latest/esp_hal/uart/index.html — UART driver implementing `embedded-io-async`/`embedded-hal-async` traits (HIGH)
- https://docs.rs/embedded-io-async/latest/embedded_io_async/ — Async byte-stream traits that `esp-hal` and `embassy-usb` share (HIGH)
- https://docs.rs/embassy-usb/latest/embassy_usb/ — Async USB device stack, native CDC ACM, lock-free endpoints (HIGH)
- https://docs.rs/embassy-usb-synopsys-otg/latest/embassy_usb_synopsys_otg/ — Synopsys driver needed for ESP32-C3 (HIGH)
- https://docs.rs/fixed/latest/fixed/ — Fixed-point numbers with `Saturating` arithmetic (MEDIUM)
- https://docs.rs/fugit/latest/fugit/ — `Rate`/`Duration` helpers matching esp-hal examples (MEDIUM)
- https://docs.rs/heapless/latest/heapless/ — Static data structures for `spsc::Queue` command buffering (MEDIUM)

---
*Stack research for: LibreRoaster SSR/Fan/USB reliability milestone*
*Researched: 2026-02-17*
