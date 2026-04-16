# Technology Stack

**Project:** LibreRoaster v5.3 deep bug analysis & defect report
**Researched:** 2026-04-16
**Overall confidence:** HIGH

## Scope of This Stack Recommendation

This is **not** a feature-delivery stack. It is the minimum practical stack to run a **whole-repo brownfield defect audit** and produce an **implementation-ready defect report** across:

- embedded Rust firmware
- host-side Python scripts
- shell tooling
- planning-visible behavior proven by generated evidence

The repo already has strong ingredients: Rust 1.88, host/embedded feature splits, regression scripts, HIL capture scripts, and quality-baseline parsing. The right move for this milestone is to **add audit signal and evidence normalization**, not a new platform.

## Recommended Stack

### Must-have / table stakes

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| Rust toolchain pin | `1.88.0` stable + `nightly` (tools-only) | Deterministic audit runs for firmware and host crates | The repo already pins `rust-version = 1.88`; keep builds reproducible and use nightly only where a tool truly requires it. |
| `cargo-nextest` | `0.9.133` | Reliable host-side regression execution with machine-readable evidence | Better than raw `cargo test` for audit work because it supports retries, partitioning, JUnit, and stable reporting. |
| `cargo-llvm-cov` | `0.8.5` | Coverage evidence for testable host paths | Useful for defect triage because it shows what code the current audit actually exercised; do **not** treat it as a quality vanity metric. |
| `cargo-deny` | `0.19.4` | Dependency graph risk inventory | Catches advisories, duplicate crates, banned crates, and source drift in one pass; high signal for brownfield repos. |
| `cargo-audit` | `0.22.1` | RustSec vulnerability scan | Narrower than `cargo-deny` but still worth running because it is the canonical RustSec-oriented audit surface. |
| `cargo-geiger` | `0.13.0` | Unsafe usage inventory | Important in embedded Rust: unsafe is sometimes necessary, but the audit should report where it exists and whether suspected defects cross those boundaries. |
| `cargo-hack` | `0.6.44` | Feature-matrix checking | This repo has `std`, `test`, `regression`, and `embedded` features; brownfield bugs often hide in untested feature combinations. |
| `uv` | latest stable | Reproducible Python tool/runtime management for scripts | The repo has multiple Python scripts but no Python project manager. `uv` is the lightest way to make script lint/test runs repeatable without introducing Poetry/PDM scope. |
| `ruff` | latest stable | Python linting/formatting for `scripts/` and `tests/hardware/` | Fast, low-friction, and enough for this milestone. It catches a large class of scripting defects without building a Python platform. |
| `pytest` | `9.x` | Host-side tests for Python parsing/report helpers | Needed only for repo-local Python helpers and evidence parsers; gives reproducible defect reproductions for scripts without needing hardware every run. |
| `ShellCheck` | latest stable | Static analysis for shell scripts | The repo already relies on bash entrypoints; ShellCheck is the standard high-signal tool for finding quoting, pipefail, and portability bugs. |

### Supporting workflow components

| Component | Purpose | When to Use |
|-----------|---------|-------------|
| Timestamped audit artifact folder (for example `artifacts/defect-audit/<UTC-run-id>/`) | Preserve raw evidence and normalized findings | Every audit run; the defect report should link back to raw logs, not just summaries. |
| Normalized defect inventory (`JSON` + `Markdown`) | One place to merge Rust, Python, shell, HIL, and planning-visible findings | Generate after all tools finish so follow-up remediation can scope from one report. |
| Existing `scripts/quality_baseline.py` extended into an audit normalizer | Reuse current parsing/reporting logic instead of inventing a new reporting service | Best fit here because the repo already parses clippy/test outputs into finding records. |
| JUnit/XML + JSON outputs from test runners | Machine-readable evidence for failures, flakes, and re-runs | Store from `nextest` and `pytest` for report traceability. |

## How Each Tool Fits LibreRoaster Specifically

### Rust / firmware side

1. **Keep normal firmware build paths unchanged.**
   - `cargo build --target riscv32imc-unknown-none-elf --features embedded`
   - `cargo check` for embedded-only surfaces
   - Do **not** turn this milestone into a toolchain migration.

2. **Run host-testable paths with `cargo-nextest`.**
   - Replace audit-oriented `cargo test` loops with `cargo nextest run` where possible.
   - Export JUnit and archive it with the defect report.
   - Best targets: existing integration/regression tests under `tests/`.

3. **Use `cargo-hack` to sweep feature combinations.**
   - This matters here because bugs can differ across `std`, `test`, `regression`, and `embedded` modes.
   - Use `cargo hack check --each-feature` broadly.
   - Use `cargo hack test --each-feature` only for host-testable combinations.
   - For `embedded`, prefer `check`/`build`, not fake host execution.

4. **Use `cargo-llvm-cov` only on host-target code paths.**
   - Useful for answering: “which defect-prone areas were actually exercised?”
   - Do **not** force coverage onto the ESP32-C3 binary path; that adds cost without milestone value.
   - Combine with `nextest` because the tool supports it directly.

5. **Keep both `cargo-deny` and `cargo-audit`.**
   - `cargo-audit` is the direct RustSec scan.
   - `cargo-deny` adds dependency-policy checks and duplicate-version visibility.
   - For a brownfield defect report, both belong in the evidence pack.

6. **Keep `cargo-geiger` as an audit lens, not a gate.**
   - Report unsafe hotspots and correlate them with actual suspected defects.
   - Do not fail the milestone just because unsafe exists; fail only when unsafe is unexplained or implicated.

### Python / host tooling side

1. **Introduce `uv`, not a full Python packaging platform.**
   - Enough to pin and run `ruff`/`pytest` consistently.
   - Good fit because the repo has scripts, not a Python application.

2. **Lint all Python scripts with `ruff`.**
   - Focus paths: `scripts/*.py`, `tests/hardware/*.py`.
   - Especially valuable here because these files produce evidence artifacts and defect-report inputs.

3. **Add targeted `pytest` coverage for parser/report helpers only.**
   - Good candidates: trace parsing, metadata loading, report assembly, CSV/JSON normalization.
   - Avoid rewriting hardware-capture flows into large simulated test suites for this milestone.

### Shell / glue tooling side

1. **Run `ShellCheck` on `scripts/*.sh` and `build-firmware.sh`.**
   - This is table stakes for bash-heavy brownfield repos.
   - It will catch real defect classes already relevant here: quoting bugs, subshell status handling, masked failures, and brittle conditionals.

### Evidence capture / report production

1. **Promote raw outputs to first-class artifacts.**
   - Keep raw `clippy` JSON, `nextest` JUnit, `pytest` JUnit, shellcheck output, and HIL logs.
   - The final defect report should reference artifact paths, not paraphrase them.

2. **Extend, don’t replace, existing report plumbing.**
   - `scripts/quality_baseline.py` already models findings.
   - Extend that pattern into a repo-wide `defect_audit` aggregator instead of adding an issue tracker, database, or SaaS reporting layer.

3. **Reuse existing HIL/diagnostic evidence flows.**
   - `tests/hardware/validation_runner.py`
   - `tests/hardware/analysis.py`
   - `scripts/traceability_matrix.py`
   - `scripts/replay_safe_shutdown.py`
   - These already produce planning-visible evidence. The milestone needs to correlate their failures and inconsistencies, not replace them.

## Recommended Commands / Installation

```bash
# Rust toolchains
rustup toolchain install 1.88.0
rustup toolchain install nightly

# Rust audit tools
cargo +stable install --locked cargo-nextest@0.9.133
cargo +stable install --locked cargo-llvm-cov@0.8.5
cargo +stable install --locked cargo-deny@0.19.4
cargo +stable install --locked cargo-audit@0.22.1
cargo +stable install --locked cargo-geiger@0.13.0
cargo +stable install --locked cargo-hack@0.6.44

# Python tool runner
curl -LsSf https://astral.sh/uv/install.sh | sh

# Python audit tools (managed by uv)
uv tool install ruff
uv tool install pytest

# Shell static analysis
# Prefer distro package for ShellCheck (apt/dnf/brew/pacman depending on environment)
```

## Minimal Audit Pipeline for This Milestone

Use this ordering because it front-loads cheap, deterministic signal before expensive or hardware-coupled checks:

1. `cargo clippy --all-targets --all-features --message-format=json`
2. `cargo hack check --each-feature`
3. `cargo nextest run` for host-testable suites with JUnit output
4. `cargo llvm-cov nextest` for host coverage evidence
5. `cargo geiger --all-targets --all-features`
6. `cargo deny check`
7. `cargo audit`
8. `uvx ruff check scripts tests/hardware`
9. `uv run pytest` for Python helper tests
10. `shellcheck scripts/*.sh build-firmware.sh`
11. Existing HIL/trace replay flows for evidence-backed runtime defect reproduction
12. Aggregate everything into one defect inventory and Markdown report

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Rust test runner | `cargo-nextest` | raw `cargo test` | `cargo test` is fine for local loops, but weaker for audit evidence, retries, partitioning, and JUnit output. |
| Python environment management | `uv` | Poetry / PDM / pip-tools | Those are heavier than needed for a scripts-first repo and would create packaging/process scope unrelated to the milestone goal. |
| Dependency risk checks | `cargo-deny` + `cargo-audit` | only one of them | Using both gives broader brownfield coverage with little extra setup. |
| Feature-matrix checks | `cargo-hack` | ad-hoc shell loops over features | `cargo-hack` is purpose-built, less error-prone, and easier to keep reproducible. |
| Shell analysis | `ShellCheck` | manual review only | Manual shell review misses common quoting and exit-status defects too easily. |

## Nice-to-have, but Optional for This Milestone

| Tool | Version | Value | Why Optional |
|------|---------|-------|--------------|
| `cargo-fuzz` | `0.13.1` | Good for parser/protocol boundary bugs (`input/parser.rs`, Artisan command parsing) | Valuable, but fuzzing is better as a targeted follow-up once the first audit identifies the highest-risk inputs. |
| `cargo machete` / `cargo +nightly udeps` | existing repo usage | Helpful for dependency hygiene evidence | Useful to keep, but dependency cleanup is not the primary milestone output. |
| SARIF export / GitHub code scanning ingestion | n/a | Better IDE/PR surfacing of findings | Good later, but not required to produce the initial implementation-ready defect report. |

## What NOT to Add

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| New observability stack (`defmt`, OpenTelemetry, Grafana, ELK, etc.) | Too much scope for an analysis-only milestone; the repo already has TRACE, replay, and HIL evidence | Reuse existing diagnostics and artifact flows. |
| New bug database or web dashboard | The output needed here is a defect report plus raw evidence, not a new product surface | Generate Markdown + JSON artifacts in-repo. |
| Formal methods stack (`Kani`, `Prusti`, `MIRAI`, TLA+`) | High setup cost and likely to delay the audit instead of informing it | Defer until the bug inventory shows a narrow safety-critical hotspot that merits it. |
| Repo-wide container/devbox rebuild | Useful only if reproducibility is currently broken | Prefer pinned Rust toolchains, `uv`, and timestamped artifacts first. |
| Node/JS report-rendering toolchain | Adds a new ecosystem for little gain | Keep report generation in Rust/Python/shell already present in the repo. |
| Logging-stack rewrite or runtime architecture changes | This milestone is analysis-first, not remediation-first | Capture defects against current architecture, then scope fixes separately. |

## Integration Notes for Existing LibreRoaster Assets

- Keep `scripts/run-regression-checks.sh`, but move audit-grade test execution toward `cargo-nextest` outputs.
- Keep `scripts/dependency-audit.sh`; fold its output into the unified defect inventory instead of treating it as a separate report.
- Extend `scripts/quality_baseline.py` rather than starting from zero; it already models findings, severities, and tiers.
- Reuse `tests/hardware/runs/` and replay artifacts as evidence sources for planning-visible/runtime defects.
- Maintain the current split between host validation and real embedded/HIL validation; do not try to make every embedded failure reproducible in a host-only harness during this milestone.

## Sources

- Repo context: `/home/juan/Repos/LibreRoaster/Cargo.toml`, `scripts/quality_baseline.py`, `scripts/run-regression-checks.sh`, `scripts/dependency-audit.sh`, `tests/hardware/validation_runner.py`
- Nextest docs and release page: https://nexte.st/ and https://github.com/nextest-rs/nextest
- cargo-llvm-cov docs/release: https://github.com/taiki-e/cargo-llvm-cov
- cargo-deny docs/release: https://embarkstudios.github.io/cargo-deny/ and https://github.com/EmbarkStudios/cargo-deny
- RustSec / cargo-audit docs: https://rustsec.org/ and https://docs.rs/crate/cargo-audit/latest
- cargo-geiger docs: https://docs.rs/crate/cargo-geiger/latest
- cargo-hack docs/release: https://github.com/taiki-e/cargo-hack
- uv docs: https://docs.astral.sh/uv/
- Ruff docs: https://docs.astral.sh/ruff/
- pytest docs: https://docs.pytest.org/en/stable/
- ShellCheck site/docs: https://www.shellcheck.net/
- cargo-fuzz docs/release: https://github.com/rust-fuzz/cargo-fuzz and https://rust-fuzz.github.io/book/cargo-fuzz.html
