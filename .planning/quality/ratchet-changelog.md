# Quality Policy Ratchet Changelog

This changelog records all policy version updates and their rationale.

## Update Policy

Every ratchet change requires:
1. **Policy version bump** (semver: MAJOR.MINOR.PATCH)
2. **Human-readable delta** explaining what changed and why
3. **Tier impact assessment** - which tiers are affected

## Ratchet Cadence

Recommended: Review and apply ratchets at phase boundaries (e.g., after Phase 82 dead-code cleanup, before Phase 83 modernization).

---

## v1.0.0 (2026-03-07)

**Status:** INITIAL BASELINE

### Changes

- Initial policy release defining:
  - Gate order: fmt → clippy → test
  - Canonical commands for each gate with lockfile enforcement
  - Compact output mode with all-failures collection
  - Host-safe test scope (--lib --tests)
  
### Tier Structure

- **T1 Critical** (blocking): `src/safety/**`, `src/control/**`, `src/input/parser.rs`, `src/output/artisan.rs`, `src/config/**`
- **T2 Core** (informational): `src/hardware/**`, `src/application/**`, `src/input/multiplexer.rs`, `src/output/traits.rs`
- **T3 Support** (informational): `src/logging/**`, `src/common/**`, `tests/**`

### Rule Identifiers

Findings include policy references in format: `QG-{GATE}-{TIER}`
- Examples: `QG-CLIPPY-T1`, `QG-TEST-T2`, `QG-FMT-T3`

### Rationale

This baseline establishes the quality contract for v5.0. Tiered enforcement allows:
- Strict checking on safety/control/protocol modules (Tier 1)
- Gradual ratcheting of lower-risk modules over time
- Clear path to tighten quality without blocking ongoing development

---

## v1.1.0 (2026-08-04)

**Status:** GATE COMMAND CORRECTION

### Changes

- `[gates.test]` command corrected to the working host-test invocation:
  `cargo test --locked --target x86_64-unknown-linux-gnu --features test --lib --tests --no-fail-fast`.
  - The previous command (`cargo test --locked --lib --tests --no-fail-fast`) failed at link time on every run (`undefined symbol: _embassy_time_now`), because host tests need the `test` feature to provide the Embassy time driver.
- `quality-baseline.sh` aligned with this policy: `cargo fmt --all -- --check`, clippy with `--all-targets` + the curated `-W` flags, and the corrected test command.

### Tier Impact

- No tier changes (T1/T2/T3 module mappings unchanged).

---

*For policy details, see `baseline-policy.toml`*
