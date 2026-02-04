# Stack Research

**Domain:** ESP32-C3 firmware (Embassy) with Artisan+/UART command surface
**Researched:** 2026-02-04
**Confidence:** HIGH

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| proptest | 1.9.0 | Property-based tests for parser/formatter edge cases | Strong shrinking + strategy combinators make it practical to fuzz invalid/edge commands without hand-writing cases; works under `cfg(test)` with `std` only, keeping firmware size unchanged. |
| insta | 1.46.3 | Snapshot testing of command → response pairs | Fast approval-style workflow to lock formatter golden outputs per Artisan/Artisan+ spec; rediffs make regressions obvious and keep specs executable. |
| mock-embedded-io | 0.1.0 | Mock UART implementing `embedded-io`/`embedded-io-async` | Matches existing `embedded-io 0.7.x` stack and Embassy async traits, enabling end-to-end command/response tests without HAL hardware. |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| rstest | 0.26.1 | Table-driven/unit test parameterization | Enumerate full Artisan/Artisan+ command matrix without boilerplate; combine with proptest for fuzz + deterministic cases. |
| embedded-io-cursor | 0.1.0 | Lightweight in-memory `Read`/`Write` for fixtures | Use when only simple byte sinks/sources are needed; cheaper than mocks, works in `no_std` with `alloc`. |
| cargo-llvm-cov | 0.8.2 | Coverage runner using LLVM source-based instrumentation | Run `cargo llvm-cov` on host to prove command coverage and formatter branches; supports workspaces and merges unit + integration tests. |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| cargo-llvm-cov | Coverage reporting | Install via `cargo install cargo-llvm-cov`; run on host (`--no-report --lcov` for CI artifacts). |
| cargo-insta (optional) | Snapshot review CLI | `cargo insta review` to accept/reject formatter snapshot updates. |

## Installation

```bash
# Test-only dependencies
cargo add --dev proptest@1.9.0 insta@1.46.3 rstest@0.26.1

# UART mocking + fixtures
cargo add --dev mock-embedded-io@0.1.0 embedded-io-cursor@0.1.0

# Coverage tooling (binary install)
```

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| proptest | quickcheck | If compile times from proptest strategies become a bottleneck and only light randomness is needed. |
| mock-embedded-io | mockall | When you need precise call expectations or to simulate errors beyond byte streams. |
| insta | expect-test | If you prefer inline expected strings in code and can tolerate noisier diffs for multi-line responses. |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| embedded-hal-mock for UART | Targets `embedded-hal` 0.2 peripherals; does not implement `embedded-io` stream traits or Embassy async, so coverage misses actual UART path. | mock-embedded-io or embedded-io-cursor |
| cargo-tarpaulin | Not reliable with `-Z instrument-coverage`, RISC-V targets, or async-heavy code; misses async branch coverage. | cargo-llvm-cov |

## Stack Patterns by Variant

**If building firmware (no_std/esp32c3):**
- Keep new crates as `dev-dependencies` behind `cfg(test)` to avoid firmware size/regressions.
- Continue using Embassy executor/serial drivers; mocks live only in host tests.

**If running host-side integration tests:**
- Enable `test` feature to pull `std` and allow proptest/insta/mocks.
- Use `mock-embedded-io` (async) plus `embedded-io-cursor` for deterministic fixtures; layer formatter on top and assert snapshots.

## Version Compatibility

| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| mock-embedded-io 0.1.0 | embedded-io 0.7.1 | Implements both blocking and async traits used by current stack. |
| embedded-io-cursor 0.1.0 | embedded-io 0.7.1 | Works in `no_std` with `alloc`; enable `std` feature for host tests. |
| proptest 1.9.0 / insta 1.46.3 | Rust 1.88 (project toolchain) | Require `std`; safe as dev-deps gated by `cfg(test)`. |
| cargo-llvm-cov 0.8.2 | rustc >= 1.70 | Supports source-based coverage; run on host only. |

## Sources

- https://crates.io/api/v1/crates/proptest — confirm 1.9.0 latest (2025-10-26)
- https://crates.io/api/v1/crates/insta — confirm 1.46.3 latest (2026-02-02)
- https://crates.io/api/v1/crates/mock-embedded-io — confirm 0.1.0 latest (2025-05-05)
- https://crates.io/api/v1/crates/embedded-io-cursor — confirm 0.1.0 latest (2025-09-18)
- https://crates.io/api/v1/crates/rstest — confirm 0.26.1 latest (2025-07-27)
- https://crates.io/api/v1/crates/cargo-llvm-cov — confirm 0.8.2 latest (2026-01-27)

---
*Stack research for: LibreRoaster v1.2 integration polish*
*Researched: 2026-02-04*
