# Stack Research

**Domain:** ESP32-C3 Rust firmware (Artisan protocol edge-case fixes)
**Researched:** 2026-02-17
**Confidence:** MEDIUM

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| Rust toolchain | 1.88 (project-pinned) | Build + test harness | Existing toolchain already validated for firmware + host tests; no change required for CRLF/ROR fixes. |
| esp-hal | 1.0.0 | HAL for ESP32-C3 | Current esp-rs HAL baseline; no stack change needed for formatter fixes. |
| embassy-time | 0.5.0 | Time/interval utilities | Existing async timing stack; keep for ROR/delta_bt timing logic. |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| embedded-hal | 1.0.0 | Trait compatibility | Keep as the stable HAL trait set used by esp-hal and existing drivers. |
| std test harness | Rust 1.88 | Host-side tests | Use with `--features test` to validate CRLF terminator and ROR state updates on desktop. |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| cargo test | Run protocol edge-case tests | Use `cargo test --features test` for host tests; no new runner required. |
| cargo fmt | Ensure consistent formatting | Keep existing formatting; no changes required for this milestone. |

## Installation

```bash
# No new dependencies required for this milestone.

# Run host-side tests for edge cases
cargo test --features test
```

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| Built-in Rust test harness | defmt-test | Only if the edge-case tests must run on-target instead of host. |
| Unit tests with fixed inputs | proptest/quickcheck | Use only if you need property-based coverage of formatter invariants. |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| Regex/string heavy parsing crates | Adds allocations and code size for simple CRLF/state rules | Continue using current parser + formatter utilities. |
| On-target test frameworks for host-validatable logic | Slows iteration and complicates CI for formatting bugs | Host tests under `--features test`. |

## Stack Patterns by Variant

**If validating CRLF/formatter logic on desktop:**
- Use `std` + `--features test`
- Because terminator formatting and ROR state are deterministic and do not need hardware

**If validating transport framing on hardware:**
- Use current esp-hal + CDC/UART drivers
- Because CRLF behavior should be verified on actual transport only if host tests uncover integration gaps

## Version Compatibility

| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| esp-hal@1.0.0 | embedded-hal@1.0.0 | esp-hal implements the 1.0.0 HAL traits per docs.rs. |
| embassy-time@0.5.0 | Rust 1.88 toolchain | Matches existing project pin; no change needed for this milestone. |

## Sources

- https://docs.rs/esp-hal/latest/esp_hal/ — esp-hal latest version and documentation (HIGH)
- https://docs.rs/embassy-time/latest/embassy_time/ — embassy-time latest version and documentation (HIGH)
- https://docs.rs/embedded-hal/latest/embedded_hal/ — embedded-hal latest version and documentation (HIGH)

---
*Stack research for: ESP32-C3 Rust firmware (Artisan protocol edge-case fixes)*
*Researched: 2026-02-17*
