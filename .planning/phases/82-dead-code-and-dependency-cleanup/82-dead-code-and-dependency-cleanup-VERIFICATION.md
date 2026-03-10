---
phase: 82-dead-code-and-dependency-cleanup
verified: 2026-03-07T13:08:54Z
status: passed
score: 6/6 must-haves verified
---

# Phase 82: Dead Code and Dependency Cleanup Verification Report

**Phase Goal:** Users can remove dead code and unused dependencies in controlled batches with evidence-backed safety and no behavior regressions.
**Verified:** 2026-03-07T13:08:54Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
| --- | --- | --- | --- |
| 1 | Users can review a module-by-module dead code inventory that includes risk levels and evidence snippets before any deletion. | ✓ VERIFIED | `quality/dead-code/README.md` explains the risk buckets, links directly to `scripts/dead-code-inventory.sh`, and points reviewers at `quality/dead-code/inventory/dead-code-inventory.json` for the concrete `code`, `span`, and `message` fields. |
| 2 | The inventory is generated on demand by a reproducible script that reruns dead_code-aware linting and captures evidence metadata. | ✓ VERIFIED | `scripts/dead-code-inventory.sh` reruns `cargo clippy --message-format=json`, captures git/toolchain/timestamp metadata, writes timestamped snapshots plus the stable pointer, and the resulting `quality/dead-code/inventory/dead-code-inventory.json` contains dozens of `dead_code` entries with spans. |
| 3 | Dead code is removed in small batches where each candidate references the inventory entry and the removal run passes the gated test baseline. | ✓ VERIFIED | `quality/dead-code/removal-guidelines.md` walks batch owners back to the inventory before touching modules, and `scripts/dead-code-removal.sh` records the module list while running `cargo test --locked --lib --tests --no-fail-fast` plus `scripts/quality-baseline.sh`. |
| 4 | Each removal batch records how tests and the quality baseline behaved so reviewers can see the regression verification for DC-02. | ✓ VERIFIED | The runner appends a `Gate summary` with PASS/FAIL, test log paths, and baseline exit codes to `quality/dead-code/batches/<name>.md`, and the guidelines describe reviewing that summary before merging. |
| 5 | Users can run a combined `machete` + `cargo +nightly udeps` workflow that respects an allowlist before removing dependencies. | ✓ VERIFIED | `scripts/dependency-audit.sh` executes `cargo machete --with-metadata --skip-target-dir` plus `cargo +nightly udeps`, annotates the logs, and exits non-zero only when new unused crates outside `.planning/quality/dependency-allowlist.toml` are found. |
| 6 | Every audit run references the allowlist and records its findings so reviewers can justify each deletion. | ✓ VERIFIED | The audit runner loads `.planning/quality/dependency-allowlist.toml` to tag each `udeps` finding and writes `quality/dead-code/dependency/audit-<timestamp>-udeps.log`; `quality/dead-code/dependency-allowlist.md` documents how to update the allowlist, reread the logs, and sign off. |

**Score:** 6/6 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
| --- | --- | --- | --- |
| `scripts/dead-code-inventory.sh` | Clippy/JSON runner that emits live dead-code snapshots plus git metadata. | ✓ VERIFIED | Runs `cargo clippy --locked --lib --tests --benches --examples --all-features --message-format=json`, filters `dead_code` entries, captures git/toolchain/timestamp context, and writes both timestamped and pointer JSON files before printing annotated risk hints. |
| `quality/dead-code/README.md` | Risk classification guidance that references inventory outputs. | ✓ VERIFIED | Defines high/medium/low buckets, lists required evidence (inventory entry, tests, justification), and instructs reviewers to rerun `scripts/dead-code-inventory.sh` and tie every removal back to `quality/dead-code/inventory/dead-code-inventory.json`. |
| `scripts/dead-code-removal.sh` | Batch runner that applies candidate modules, runs gating scripts, and records logs. | ✓ VERIFIED | Requires `BATCH_NAME`/`MODULES`, snapshots the module list, runs `cargo test` + `scripts/quality-baseline.sh`, logs statuses, and surfaces the summary in `quality/dead-code/batches/<name>.md`. |
| `quality/dead-code/removal-guidelines.md` | How-to for forming batches, linking inventory, and surfacing evidence. | ✓ VERIFIED | Directs users to the inventory, explains how to populate the `MODULES` list, details executing the runner, and tells reviewers to cite the gate summary/logs and triage failures. |
| `scripts/dependency-audit.sh` | Audit runner that executes machete + nightly udeps honoring allowlist. | ✓ VERIFIED | Runs `cargo machete --with-metadata --skip-target-dir`, then `cargo +nightly udeps --quiet`, tags allowlisted crates, writes annotated logs under `quality/dead-code/dependency`, and exits only if unauthorized unused crates appear. |
| `.planning/quality/dependency-allowlist.toml` | Declarative allowlist entries with rationale. | ✓ VERIFIED | Contains multiple `[[allow]]` tables (`serde`, `embedded-hal`, `embassy-usb`, etc.) with `package`, `reason`, and `expires`, covering intentional dependencies the audit runner must skip. |
| `quality/dead-code/dependency-allowlist.md` | Documentation for keeping the allowlist and logs in sync. | ✓ VERIFIED | Explains how to add `[[allow]]` entries, rerun `scripts/dependency-audit.sh`, and how reviewers validate log timestamps, allowlist rationales, and sign-off. |

### Key Link Verification

| From | To | Via | Status | Details |
| --- | --- | --- | --- | --- |
| `scripts/dead-code-inventory.sh` | `quality/dead-code/inventory` | Filters `dead_code` lints into JSON and copies the latest snapshot to the pointer file. | WIRED | Script writes timestamped files plus `dead-code-inventory.json`, so inventory consumers always read data with git/toolchain metadata. |
| `quality/dead-code/README.md` | `scripts/dead-code-inventory.sh` | Risk guidance references the script and instructs reviewers to rerun it before removal batches. | WIRED | README explicitly cites the script’s command line and expects reviewers to use its outputs when labeling evidence. |
| `scripts/dead-code-removal.sh` | `scripts/quality-baseline.sh` | Invokes the baseline script after `cargo test` to enforce DC-02 gates. | WIRED | Runner calls `bash scripts/quality-baseline.sh`, streams logs, and stores the exit code in each batch summary. |
| `quality/dead-code/removal-guidelines.md` | `quality/dead-code/README.md` | Guidelines direct users back to the inventory before forming batches. | WIRED | Section “Forming a removal batch” begins with “Start at the dead-code inventory described in `quality/dead-code/README.md`,” ensuring DC-02 remains linked to DC-01. |
| `scripts/dependency-audit.sh` | `.planning/quality/dependency-allowlist.toml` | Loads `ALLOWLIST_FILE`, parses entries, and annotates udeps output. | WIRED | Python block reads allowlist entries, builds `allowmap`, and prints allowlist reasons/expires into `audit-<timestamp>-udeps.log`. |
| `quality/dead-code/dependency-allowlist.md` | `scripts/dependency-audit.sh` | Documentation describes how the audit runner consumes the allowlist and where to find log files. | WIRED | Early section names the script and allowlist, and a later section outlines how reviewers trace sign-off back to the generated `audit-*.log` files. |

### Requirements Coverage

| Requirement | Status | Blocking Issue |
| --- | --- | --- |
| DC-01 | ✓ SATISFIED | None (inventory script + README deliver module-level evidence before removal). |
| DC-02 | ✓ SATISFIED | None (batch runner + guidelines ensure every removal is gate-checked and logged). |
| DC-03 | ✓ SATISFIED | None (dependency audit runner + allowlist + documentation keep machete/udeps runs auditable). |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
| --- | --- | --- | --- | --- |
| None detected in the inspected artifacts. | - | - | - | No TODO/HACK placeholders or stub handlers were found. |

### Human Verification Required

None — the scripts and documentation can be verified entirely via code inspection.

### Gaps Summary

No gaps; all artifacts exist, are substantive, and are wired as required, so the phase goal is fully satisfied in the codebase.

---

_Verified: 2026-03-07T13:08:54Z_
_Verifier: Claude (gsd-verifier)_
