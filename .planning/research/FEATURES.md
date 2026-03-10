# Feature Research: LibreRoaster v5.0 Quality Hardening

**Domain:** Brownfield embedded Rust firmware quality hardening (coffee roaster controller)
**Researched:** 2026-03-07
**Project:** LibreRoaster v5.0
**Confidence:** MEDIUM-HIGH

## Feature Landscape

### Table Stakes (Must Have)

These are expected in a mature embedded Rust firmware hardening milestone. Missing these usually leads to recurring defects, slower onboarding, and unsafe refactors.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| **Quality gate baseline (lint + formatting + fail policy)** | Mature Rust firmware teams codify baseline quality gates (`cargo fmt --check`, clippy sets, warning policy) so quality is enforced continuously, not by manual review. | MEDIUM | Dependencies: host/integration tests already in place; dual-channel queue + control modules must compile under stricter lints. Behavior in brownfield: start warn-only per module, then ratchet to deny in CI. |
| **Codebase audit inventory (module ownership + risk map)** | Brownfield hardening needs an explicit map of hot paths, unsafe-adjacent areas, and stale modules before deleting/refactoring code. | MEDIUM | Dependencies: existing Artisan protocol processing, safety instrumentation, SSR/fan control, and queue processing paths. Behavior: inventory first, edits second. |
| **Dead code elimination workflow (not one-shot deletion)** | Mature teams treat dead code removal as a pipeline: compiler lint signals, dependency-level checks, then behavior-preserving removals with test/hardware verification. | MEDIUM | Dependencies: existing host/integration tests are the first safety net; real-hardware validation is the second. Use `dead_code` lint + `cargo +nightly udeps` for unused dependencies, with explicit allowlist for intentional dormant items. |
| **Rust best-practices uplift pass** | Brownfield firmware accumulates style and API drift. A structured uplift (`cargo fix`, edition idioms, clippy cleanup, error/context normalization) is table stakes for maintainability. | MEDIUM | Dependencies: all existing feature areas, especially protocol handlers and safety reporting. Behavior: prefer mechanical transforms first, then semantic cleanups. |
| **Pragmatic SOLID alignment at hardware seams** | In embedded Rust, SOLID is expected mainly at boundaries (drivers, control strategies, command handlers), not as strict OO purity across all modules. | HIGH | Dependencies: existing SSR/Fan architecture, command processing handlers, dual-channel comm abstractions. Behavior: extract traits where hardware swap/testing needs it; avoid architecture churn in proven loops. |
| **Hardware realism validation path (HIL smoke path)** | Mature firmware hardening does not stop at host tests. It defines a repeatable path where host tooling (Artisan Scope) drives a real roaster and validates telemetry + safety invariants. | HIGH | Dependencies: Artisan protocol + telemetry output, watchdog/guard reporting, queue processing, SSR/Fan control loop. Behavior: scripted scenario set with pass/fail thresholds and rollback criteria. |

### Differentiators (Engineering Advantage)

These are not strictly required for v5.0 completion, but they materially improve reliability velocity and auditability.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Ratcheting lint policy by module criticality** | Lets the team harden critical safety/control modules first without blocking lower-risk areas; improves adoption vs. all-at-once deny policies. | MEDIUM | Start with safety + control + protocol modules; expand to full workspace after warning burn-down. |
| **Traceability matrix: command -> queue -> actuator -> telemetry -> guard events** | Makes regressions explainable and shortens incident triage when behavior differs between host tests and hardware runs. | HIGH | Builds on existing dual-channel communication and telemetry instrumentation. |
| **Artifact-backed HIL runs (scenario scripts + golden outputs)** | Converts hardware testing from tribal/manual process into repeatable evidence suitable for release gates and future regressions. | HIGH | Pair Artisan Scope scenario files with expected telemetry/safety envelopes and retain run artifacts per release. |
| **Fault-injection hardening scenarios** | Competitive reliability gain: validates watchdog/guard behavior under dropped messages, queue pressure, and sensor anomalies before field exposure. | HIGH | Depends directly on existing watchdog/guard reporting and queue processing architecture. |

### Anti-Features (Deliberately Avoid)

These are common quality-hardening traps in brownfield firmware.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **Big-bang architecture rewrite for "perfect SOLID"** | Looks like the fastest route to clean design. | High regression risk in control/safety loops; burns schedule on structural churn rather than defect removal. | Incremental seam extraction at high-leverage boundaries only. |
| **Enable all strict clippy groups globally on day one** | Signals rigor quickly. | `pedantic`/`restriction` contain intentionally noisy rules; creates churn and team fatigue, especially in legacy modules. | Curated lint profile with documented allowlist and ratchet plan. |
| **Delete anything flagged as unused without runtime validation** | Appears to reduce complexity fast. | Firmware often has conditionally used paths (feature flags, board-specific, recovery flows). Blind deletion can break field behavior. | Two-stage deletion: static signal + scenario verification (tests + HIL). |
| **Overbuild a full hardware lab orchestration platform in v5.0** | Promise of perfect automated realism. | Tooling scope can eclipse the hardening objective. | Keep a pragmatic scripted HIL path first; automate further in follow-up milestone. |
| **Refactor public protocol semantics during hardening** | Tempting to "clean up" protocol while touching handlers. | Changes external behavior and muddies regression attribution for quality milestone. | Freeze protocol semantics; harden implementation, tests, and observability only. |

## Feature Dependencies

```text
Existing foundation (already built):
  Artisan command processing + telemetry
  Safety instrumentation + watchdog/guard reporting
  Dual-channel communications + queue processing
  SSR/Fan control architecture + host/integration tests

[Quality gate baseline]
    └──enables──> [Dead code elimination workflow]
    └──enables──> [Best-practices uplift pass]

[Best-practices uplift pass]
    └──enables──> [Pragmatic SOLID seam extraction]

[Dead code elimination workflow]
    └──requires──> [Host/integration test coverage confidence]
    └──requires──> [Hardware realism validation path]

[Hardware realism validation path]
    └──requires──> [Artisan protocol + telemetry]
    └──requires──> [Safety/watchdog instrumentation]
    └──requires──> [Dual-channel queue correctness]
    └──requires──> [SSR/Fan control behavior]
```

### Dependency Notes

- **Dead code elimination depends on both test tiers:** host/integration tests catch immediate regressions, while real-hardware runs catch timing/actuation edge cases not visible in host-only tests.
- **SOLID alignment depends on existing architecture seams:** most value comes from tightening current boundaries (handlers, hardware traits), not adding new abstraction layers.
- **Hardware realism is an end-to-end dependency consumer:** it validates all existing core features together and should be the acceptance gate for hardening changes touching control or safety paths.

## MVP Recommendation (v5.0)

Prioritize these for milestone completion:

1. **Quality gate baseline + ratchet policy** - establish enforceable standards and prevent quality backsliding.
2. **Codebase audit + dead code elimination workflow** - reduce maintenance drag without blind deletions.
3. **Best-practices uplift + targeted SOLID seam alignment** - improve maintainability in high-change/high-risk modules.
4. **Hardware realism validation path with Artisan Scope** - verify hardening on real roaster behavior before closeout.

Defer to post-v5.0:

- **Fully automated lab orchestration** - valuable, but beyond pragmatic scope for first hardening release.
- **Global strict lint saturation in one pass** - adopt through ratcheting once top-risk modules stabilize.

## Feature Prioritization Matrix

| Feature | User/Engineering Value | Implementation Cost | Priority |
|---------|-------------------------|---------------------|----------|
| Quality gate baseline + ratchet | HIGH | MEDIUM | P1 |
| Audit + dead code elimination workflow | HIGH | MEDIUM | P1 |
| Hardware realism validation path (HIL smoke) | HIGH | HIGH | P1 |
| Best-practices uplift pass | HIGH | MEDIUM | P1 |
| Pragmatic SOLID seam alignment | MEDIUM-HIGH | HIGH | P2 |
| Fault-injection scenario suite | MEDIUM-HIGH | HIGH | P2 |
| Full lab automation platform | MEDIUM | HIGH | P3 |

**Priority key:**
- P1: Required for v5.0 hardening exit criteria
- P2: Strongly recommended if schedule allows
- P3: Follow-up milestone

## Sources

Primary (HIGH confidence):

- Rust compiler lint reference (`dead_code`, `unused_*`, warning levels): https://doc.rust-lang.org/rustc/lints/listing/warn-by-default.html
- Clippy lint categories and operational guidance: https://doc.rust-lang.org/clippy/
- Cargo automated fixes and edition migration workflow: https://doc.rust-lang.org/cargo/commands/cargo-fix.html
- Cargo profiles and build policy controls: https://doc.rust-lang.org/cargo/reference/profiles.html
- Rust 2024 edition timing/reference: https://doc.rust-lang.org/nightly/edition-guide/rust-2024/index.html
- Embedded HAL design goals and trait-boundary guidance: https://docs.rs/embedded-hal/latest/embedded_hal/
- Embedded test harness on real targets (`embedded-test` + `probe-rs run` model): https://docs.rs/embedded-test/latest/embedded_test/

Supporting (MEDIUM confidence):

- `cargo-udeps` usage and constraints (nightly needed to run): https://github.com/est31/cargo-udeps
- `cargo-llvm-cov` capabilities/limits and CI thresholds: https://github.com/taiki-e/cargo-llvm-cov
- `probe-rs` ecosystem maturity and host-target debugging tooling: https://github.com/probe-rs/probe-rs
- Rust API design recommendations (pragmatic maintainability over dogma): https://rust-lang.github.io/api-guidelines/
- `defmt` ecosystem and embedded logging/test context: https://defmt.ferrous-systems.com/

Notes on confidence:

- Google search tool was unavailable in this environment (403), so ecosystem trend claims are based on official docs and major project documentation rather than broad web survey.

---
*Feature research for: LibreRoaster v5.0 quality audit/hardening*
*Researched: 2026-03-07*
