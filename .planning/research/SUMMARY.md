# Project Research Summary

**Project:** LibreRoaster v5.3 Deep Bug Analysis & Defect Report
**Domain:** Brownfield embedded firmware defect audit and evidence-backed reporting
**Researched:** 2026-04-16
**Confidence:** HIGH

## Executive Summary

LibreRoaster v5.3 is best treated as an **audit-overlay milestone**, not a feature-delivery or architecture-rewrite effort. The product here is an implementation-ready defect report for an existing embedded Rust firmware system plus its Python, shell, HIL, replay, and planning-visible evidence paths. The research consistently points to one approach: preserve the runtime, reuse current TRACE/STATUS/HIL/replay assets, and add a normalized defect catalog plus evidence index that can directly feed the next remediation roadmap.

The recommended build order is to trust the evidence pipeline before trusting any finding. Experts would first define the audit charter, bug bar, and evidence rules; then verify host capture and analysis tooling; then investigate transport/control-loop boundaries before lower-level hardware and documentation drift. Stack additions should stay lightweight and audit-focused: `cargo-nextest`, `cargo-hack`, `cargo-llvm-cov`, `cargo-deny`, `cargo-audit`, `cargo-geiger`, `uv`, `ruff`, `pytest`, and `ShellCheck`.

The biggest risks are scope drift, false confidence in existing diagnostics, and weak evidence quality. Mitigate them by keeping fixes out of this milestone, separating severity from confidence, requiring artifact-backed findings, and using HIL selectively for target-sensitive bugs only. If the team follows that discipline, the output should be a high-signal defect inventory rather than a vague bug brainstorm.

## Key Findings

### Recommended Stack

Research favors **augmenting the current repo**, not introducing a new platform. Keep Rust pinned at `1.88.0`, use nightly only for tool-only needs, and extend existing Python reporting instead of adding a new service or dashboard.

**Core technologies:**
- **Rust 1.88.0 stable + nightly (tools-only):** deterministic audit runs without changing the firmware build path.
- **cargo-nextest 0.9.133:** machine-readable host regression evidence with JUnit output.
- **cargo-hack 0.6.44:** feature-matrix checking across `std`, `test`, `regression`, and `embedded` modes.
- **cargo-llvm-cov 0.8.5:** host-path coverage evidence to show what the audit actually exercised.
- **cargo-deny 0.19.4 + cargo-audit 0.22.1:** dependency and RustSec risk inventory for brownfield coverage.
- **cargo-geiger 0.13.0:** unsafe hotspot inventory to correlate with defect-prone surfaces.
- **uv + ruff + pytest:** lightweight, reproducible checks for Python evidence tooling.
- **ShellCheck:** table-stakes static analysis for shell glue and build scripts.

**Critical version requirements:**
- Rust toolchain should remain pinned to **`1.88.0`**.
- Use the listed audit tool versions from `STACK.md` for reproducible runs.

### Expected Features

The milestone’s table stakes are about **audit quality**, not new user functionality. The report must prove whole-repo coverage, use evidence-backed findings, apply an embedded-appropriate criticity model, produce implementation-ready defect records, and specify validation expectations per finding.

**Must have (table stakes):**
- Whole-repo investigation coverage map across firmware, scripts, HIL, diagnostics, and planning-visible behavior.
- Evidence-backed finding standard with code/artifact references and bounded failure chains.
- Embedded-specific criticity model plus explicit confidence labels.
- Implementation-ready defect records with proposed fix direction and post-fix validation path.
- Structured report with inventory table, detailed findings, deferred items, and remediation slicing.

**Should have (differentiators):**
- Cross-artifact correlation from code to TRACE to replay/HIL to docs.
- Root-cause clustering and fix sequencing hints for the follow-up remediation milestone.
- Machine-readable defect inventory (`JSON`) alongside the Markdown narrative.
- False-positive / needs-validation quarantine section.

**Defer (v2+ / next milestone):**
- Actual remediation work.
- Large automation frameworks, observability rewrites, or architecture overhauls.
- Formal vulnerability disclosure workflow unless true security defects are found.

### Architecture Approach

The architecture recommendation is clear: keep firmware as the **evidence producer**, reuse host tooling as the **evidence collector/analyzer**, and add a host/docs-only **audit catalog layer** that turns subsystem findings into a roadmap-ready defect inventory. Audit the evidence pipeline first, then transport and control-loop boundaries, then hardware-sensitive flows, then planning-visible contract drift.

**Major components:**
1. **Existing firmware runtime** — produces real behavior plus STATUS/TRACE/guard/safe-shutdown evidence.
2. **Existing host evidence tools** — capture HIL/serial runs, analyze traces, and normalize replay outputs.
3. **New audit catalog layer** — stores defect records, evidence links, criticity, confidence, and fix guidance.
4. **Planning artifacts** — preserve scope, sequencing, and roadmap handoff for the remediation milestone.

### Critical Pitfalls

1. **Firmware-only audit bias** — force coverage of `scripts/`, `tests/hardware/`, and planning-visible assets.
2. **Treating diagnostics as ground truth** — validate manifest -> telemetry -> report -> replay before using them as critical evidence.
3. **Host-only proof for target-sensitive bugs** — require target-aware evidence for timing, watchdog, guard, transport, and sensor issues.
4. **Reporting integration failures as isolated component bugs** — deduplicate by broken contract and end-to-end symptom.
5. **Weak evidence quality** — reject findings that lack reproducible triggers, bounded impact, or implementation-ready fix direction.

## Implications for Roadmap

Based on combined research, the roadmap should be structured around **evidence trust first, investigation second, packaging last**.

### Phase 1: Audit Charter and Evidence Contract
**Rationale:** The audit fails if bug definitions, confidence rules, and evidence thresholds are fuzzy.
**Delivers:** Audit scope, bug bar, criticity rubric, confidence labels, defect schema, evidence index skeleton.
**Addresses:** Coverage declaration, evidence-backed findings, milestone boundaries.
**Avoids:** Scope explosion, inconsistent severity, weak findings.

### Phase 2: Evidence Pipeline Verification
**Rationale:** Host capture and analysis tooling must be trusted before they can support firmware conclusions.
**Delivers:** Verified TRACE/HIL/replay/reporting chain, documented blind spots, minimal tooling/doc fixes if evidence capture is broken.
**Uses:** Existing HIL/replay assets plus `uv`, `ruff`, `pytest`, `ShellCheck`.
**Implements:** Host evidence tooling boundary from `ARCHITECTURE.md`.

### Phase 3: Static Whole-Repo Audit
**Rationale:** Once the evidence path is trustworthy, broad inspection can generate high-quality hypotheses quickly.
**Delivers:** Subsystem notes and initial defect inventory across firmware, transport, tooling, and planning-visible behavior.
**Addresses:** Whole-repo coverage map and implementation-ready record format.
**Avoids:** Firmware-only bias and giant flat reports.

### Phase 4: Targeted Reproduction and Correlation
**Rationale:** Highest-risk hypotheses should be confirmed with the cheapest valid proof, escalating to HIL only when needed.
**Delivers:** Confirmed/likely findings backed by traces, tests, replay outputs, and selective on-target validation.
**Uses:** `cargo-nextest`, `cargo-hack`, `cargo-llvm-cov`, HIL scenarios, replay artifacts.
**Implements:** Cross-boundary evidence-first triage.

### Phase 5: Criticity Ranking and Deduplicated Defect Catalog
**Rationale:** The next milestone needs a clean backlog, not raw observations.
**Delivers:** Prioritized defect catalog, root-cause clusters, validation guidance, remediation sequencing hints.
**Addresses:** Criticity model, structured report, machine-readable inventory.
**Avoids:** Duplicate tickets, confidence/severity confusion, symptom-only fixes.

### Phase 6: Roadmap Handoff Package
**Rationale:** The milestone closes only when the research is directly consumable for requirements and roadmap generation.
**Delivers:** `DEFECT_REPORT.md`, `defects.json`, evidence index, roadmap implications, deferred/hypothesis appendix.
**Addresses:** Structured report and follow-up scoping needs.
**Avoids:** Re-investigation during remediation planning.

### Phase Ordering Rationale

- Verify the evidence pipeline before using it to diagnose firmware defects.
- Front-load transport/control-loop boundaries because they can invalidate downstream symptoms.
- Use static and host-side checks early because they are cheap and deterministic; reserve HIL for target-sensitive confirmation.
- Normalize and deduplicate before roadmap handoff so the next milestone inherits scoped defects, not noisy observations.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 2:** Evidence-pipeline verification may need targeted review of replay/HIL/report fidelity if current artifacts disagree.
- **Phase 4:** Target-sensitive transport, watchdog, sensor, and control-loop defects may need `/gsd-research-phase` support when hardware timing questions appear.
- **Phase 5:** Root-cause clustering may need deeper follow-up if multiple findings indicate a structural boundary issue rather than isolated bugs.

Phases with standard patterns (skip research-phase):
- **Phase 1:** Audit charter, schema, and evidence contract are well-defined by the research.
- **Phase 3:** Whole-repo static audit workflow is already well-bounded.
- **Phase 6:** Packaging and roadmap handoff are standard once the catalog exists.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Strong repo-local grounding plus official tool documentation; recommendations are tightly scoped. |
| Features | MEDIUM-HIGH | High confidence on audit mechanics; slightly lower confidence on broader ecosystem expectations. |
| Architecture | HIGH | Strongly supported by repo structure and explicit reuse-first boundaries. |
| Pitfalls | MEDIUM-HIGH | Pitfalls align closely with embedded brownfield audit failure modes and project-local docs. |

**Overall confidence:** HIGH

### Gaps to Address

- **Evidence fidelity on current TRACE/HIL/replay outputs:** validate early so the report does not inherit tooling drift.
- **Host-vs-target proof matrix:** define explicitly during planning to avoid over-claiming on target-sensitive bugs.
- **Subsystem ownership for follow-up fixes:** defect records can name affected areas now, but fix ownership may need explicit assignment in roadmap planning.
- **Confidence thresholds for “likely” vs “confirmed”:** set these upfront so findings are ranked consistently.

## Sources

### Primary (HIGH confidence)
- Project-local research: `.planning/research/STACK.md`, `.planning/research/FEATURES.md`, `.planning/research/ARCHITECTURE.md`, `.planning/research/PITFALLS.md`
- Repo context cited across research: `.planning/PROJECT.md`, `tests/hardware/HIL-PLAYBOOK.md`, `tests/hardware/report_template.md`, `tests/hardware/validation_runner.py`, `tests/hardware/analysis.py`, `internalDoc/TRACEABILITY_MATRIX.md`
- Rust official docs: Cargo features and conditional compilation, Embedded Rust concurrency guidance

### Secondary (MEDIUM confidence)
- Tool docs: nextest, cargo-llvm-cov, cargo-deny, cargo-audit, cargo-geiger, cargo-hack, uv, Ruff, pytest, ShellCheck
- Standards/guidance: NIST SSDF, FIRST CVSS v4.0, MITRE CWE, SEI CERT C

### Tertiary (LOW confidence)
- Broader ecosystem popularity/trend assumptions where web-wide survey data was unavailable in this environment

---
*Research completed: 2026-04-16*
*Ready for roadmap: yes*
