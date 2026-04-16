# Phase 104: Audit Foundation - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-16
**Phase:** 104-Audit Foundation
**Areas discussed:** Coverage Map Scope, Criticity Rubric Design, Confidence Label Definitions, Defect Record Schema

---

## Coverage Map Scope

| Option | Description | Selected |
|--------|-------------|----------|
| Whole-repo | Everything: firmware, scripts, tooling, tests, evidence artifacts | |
| Source code + scripts only | Firmware source + scripts + tooling only; exclude build/test artifacts | ✓ |
| Firmware only | Firmware only (src/, tests/); scripts/tooling deferred to later phase | |

**User's choice:** Source code + scripts only
**Notes:** User wants to include firmware (src/, tests/), scripts/, and tooling but exclude build artifacts.

---

## Criticity Rubric Design

### Number of Levels

| Option | Description | Selected |
|--------|-------------|----------|
| 4-level | Critical/High/Medium/Low | ✓ |
| 5-level | Add explicit Security category | |
| 2-level | Blocker/Deferrable only | |

**User's choice:** 4-level (Critical/High/Medium/Low)

### Detail Level

| Option | Description | Selected |
|--------|-------------|----------|
| Full definitions | Hard-coded definitions for each level | ✓ |
| Label only | Short label only, let evaluator decide | |
| Undefined | Leave empty — define as we find issues | |

**User's choice:** Full definitions

---

## Confidence Label Definitions

### Number of Tiers

| Option | Description | Selected |
|--------|-------------|----------|
| 3-tier | Confirmed/Likely/Needs Validation | ✓ |
| 2-tier | Confirmed vs Suspected | |
| 4+ tier | More than three tiers | |

**User's choice:** 3-tier (Confirmed/Likely/Needs Validation)

### Tier Labels

| Option | Description | Selected |
|--------|-------------|----------|
| Confirmed/Likely/Needs Validation | Standard audit terminology | ✓ |
| Verified/Probable/Speculative | Alternative terminology | |
| High/Medium/Low | Confidence-based labels | |

**User's choice:** Confirmed/Likely/Needs Validation

---

## Defect Record Schema

### Number of Fields

| Option | Description | Selected |
|--------|-------------|----------|
| All 6 fields | Summary, criticity, evidence, areas, fix, validation path | ✓ |
| Minimal | Just summary + criticity + affected areas | |
| Extended | All 6 + root cause, recurrence risk, related defects | |

**User's choice:** All 6 fields (recommended)

### Fix Description Detail

| Option | Description | Selected |
|--------|-------------|----------|
| Implementation-ready | Specific file, function, and approach to fix | ✓ |
| High-level | What to fix without implementation details | |
| Defer to later phases | Leave empty, add details later | |

**User's choice:** Implementation-ready (recommended)

---

## Deferred Ideas

None — discussion stayed within phase scope.