# Stack Research

**Domain:** Embedded Rust firmware quality hardening (v5.0)
**Researched:** 2026-03-07
**Confidence:** HIGH

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| Rust toolchain pin | `1.88.0` stable + `nightly` (tools-only) | Deterministic lint/build behavior for ESP32-C3 while still enabling nightly-only audits | `esp-hal 1.0.0` has `rust-version = 1.88.0`; pinning avoids drift from `stable` moving under us. Nightly is needed only for `cargo-udeps` execution, not firmware builds. |
| `cargo-udeps` | `0.1.60` | High-signal unused dependency detection | Best catch for truly unused Cargo deps in CI, but requires nightly to run; ideal as scheduled/PR quality gate, not every local build. |
| `cargo-nextest` | `0.9.129` | Reliable host test orchestration + machine-readable reports | Better failure isolation/retries/reporting than plain `cargo test`; fits existing host-test strategy for regression-proof refactors. |
| `cargo-llvm-cov` | `0.8.4` | Coverage baseline and fail-under gates during dead-code removal | Gives line/region coverage and integrates with `cargo nextest`; use as safety net before deleting questionable paths. |
| `cargo-modules` | `0.25.0` | Module-boundary visualization and cycle/orphan detection | Practical governance tool for SOLID-oriented refactors in Rust: structure/dependency/orphan views make boundary regressions visible. |
| `cargo-deny` | `0.19.0` | Dependency policy (advisories, source constraints, duplicate crate control) | Keeps cleanup/refactors from silently increasing supply-chain risk while dependency graph changes. |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `serialport` (dev-dependency) | `4.8.1` | Host-side serial session capture against real firmware ports | Use in a dedicated hardware validation harness to emit timestamped command/response evidence from USB CDC or UART. |
| `csv` (dev-dependency) | `1.4.0` | Deterministic evidence artifacts consumable by auditors | Use with `serialport` harness to write command/result timelines (`STATUS`, `READ`, `OT1`, `IO3`, `START/STOP`) for HW-01 evidence packs. |
| `cargo-machete` | `0.9.1` | Fast stable pre-check for unused dependencies | Use as a fast local/PR preflight before the deeper nightly `cargo-udeps` pass. |
| `cargo-geiger` | `0.13.0` | Unsafe usage trend tracking during refactors | Keep as an audit metric so SOLID-driven refactors do not grow unsafe surface unexpectedly. |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| `clippy` + existing `clippy.toml` | Best-practice enforcement (ownership, error handling, API shape) | Keep current denies (`unwrap_used`, `expect_used`, `panic`) and add staged profile for `pedantic`/selected `nursery` lints as warnings first, then promote stable wins. |
| Rustc lint policy (`[lints.rust]`) | Dead-code and boundary hardening at compile time | Explicitly enforce `dead_code`, `unused_must_use`, `unreachable_pub`, `unsafe_op_in_unsafe_fn`; use targeted `#[allow]` only with rationale. |
| `nextest` profile config (`.config/nextest.toml`) | Deterministic CI behavior and test evidence outputs | Add profile-level timeouts/retries and JUnit output for roadmap audit artifacts. |

## Installation

```bash
# Keep firmware toolchain deterministic
rustup toolchain install 1.88.0
rustup toolchain install nightly

# Quality hardening tools
cargo +stable install --locked cargo-nextest@0.9.129
cargo +stable install --locked cargo-llvm-cov@0.8.4
cargo +stable install --locked cargo-deny@0.19.0
cargo +stable install --locked cargo-machete@0.9.1
cargo +stable install --locked cargo-modules@0.25.0
cargo +stable install --locked cargo-geiger@0.13.0
cargo +stable install --locked cargo-udeps@0.1.60

# Typical execution split
cargo +stable machete
cargo +nightly udeps --all-targets --all-features
```

## Integration with Current LibreRoaster Workflow

- Keep firmware build/test baseline unchanged (`cargo build --target riscv32imc-unknown-none-elf`, existing host tests), then layer quality tooling as separate gates.
- Add a `quality-audit` pipeline that runs in this order: `clippy` -> `machete` -> `udeps` -> `nextest` -> `llvm-cov` -> `geiger` -> `cargo deny check`.
- Run `udeps` and `llvm-cov` primarily on host-target test matrix first; only include embedded target where signal is proven useful.
- Reuse existing instrumentation docs/flows (`internalDoc/INSTRUMENTATION_README.MD`) and add a host-side hardware-evidence harness that writes CSV artifacts under a predictable evidence folder.
- Treat SOLID governance as measurable architecture hygiene: snapshot `cargo modules structure/dependencies` before and after refactors and require no new cycles/orphans unless justified.

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| `cargo-udeps` + nightly | Only `cargo-machete` | Use only `machete` if team cannot run nightly in CI; accept lower precision and more manual review. |
| `cargo-nextest` | `cargo test` only | Keep plain `cargo test` for smallest local loops; use `nextest` in CI/audit paths requiring retries, partitioning, and reports. |
| `serialport`+`csv` Rust harness | Manual Artisan screenshots only | Screenshots are acceptable for quick smoke checks, but not for reproducible HW-01 evidence or timeline correlation. |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| Migrating runtime logging to `defmt` for this milestone | v4.x decision already validated `log + esp-println`; switching now adds risk with no direct value to dead-code/SOLID goals | Keep logging stack unchanged; focus on quality gates and refactor governance. |
| `anyhow`/`thiserror` in firmware hot path | Broad dynamic error plumbing can blur embedded error boundaries and increase coupling during SOLID refactors | Keep explicit domain errors (`RoasterError` family) and improve conversion boundaries incrementally. |
| Single-tool dead-code judgment | Any one tool misses cases (e.g., codegen/feature-gated/re-export paths) and can cause unsafe deletions | Combine rustc lints + `machete` + `udeps` + coverage + targeted manual review. |
| Heavy formal-methods tooling as gate (Prusti/Kani/MIRAI) | Valuable but high setup and CI cost for current milestone scope; can block delivery | Defer formal methods to a later safety-assurance milestone after v5.0 cleanup baseline is stable. |

## Stack Patterns by Variant

**If running local fast feedback (developer loop):**
- Use `cargo clippy`, `cargo machete`, `cargo nextest run`.
- Because these give high signal quickly without nightly/toolchain friction.

**If running PR/CI quality gates:**
- Add `cargo +nightly udeps`, `cargo llvm-cov --fail-under-lines <target>`, `cargo deny check`, and `cargo geiger` trend capture.
- Because dead-code deletion and boundary refactors need stronger non-regression evidence before merge.

**If running hardware validation evidence capture (HW-01):**
- Run host harness with `serialport` + `csv` while executing Artisan Scope control actions.
- Because reproducible command/telemetry traces are auditable and roadmap-friendly, unlike ad-hoc screenshots.

## Version Compatibility

| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| `esp-hal@1.0.0` | `rustc@1.88.0` | Crate declares `rust-version: 1.88.0`; this should anchor firmware toolchain pinning. |
| `cargo-deny@0.19.0` | `rustc@1.88+` | Matches current firmware toolchain floor. |
| `cargo-llvm-cov@0.8.4` | `rustc@1.87+` | Compatible with pinned 1.88 toolchain. |
| `cargo-modules@0.25.0` | `rustc@1.86+` | Safe for current environment. |
| `cargo-udeps@0.1.60` | `nightly` runtime | Compiles on stable, but execution requires nightly (`cargo +nightly udeps`). |

## Sources

- https://docs.espressif.com/projects/rust/esp-hal/latest/ and `cargo info esp-hal` — verified `esp-hal` version/MSRV alignment.
- https://github.com/est31/cargo-udeps — verified nightly runtime requirement and current release.
- https://github.com/bnjbvr/cargo-machete — verified stable fast unused-dependency workflow and limitations.
- https://github.com/nextest-rs/nextest and https://nexte.st — verified current `cargo-nextest` release and CI-oriented usage.
- https://github.com/taiki-e/cargo-llvm-cov — verified coverage features, `nextest` integration, and current release.
- https://github.com/EmbarkStudios/cargo-deny — verified dependency governance checks and current release.
- https://github.com/regexident/cargo-modules — verified structure/dependency/orphan analysis commands.
- https://github.com/geiger-rs/cargo-geiger — verified unsafe usage audit positioning.
- https://docs.rs/serialport and https://docs.rs/csv — verified host evidence-capture crate capabilities.
- `internalDoc/INSTRUMENTATION_README.MD` and `internalDoc/ARTISAN_CONNECTION.md` — integration context for existing serial/instrumentation flow.

---
*Stack research for: LibreRoaster v5.0 quality hardening milestone*
*Researched: 2026-03-07*
