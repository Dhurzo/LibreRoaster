# LibreRoaster

## What This Is

ESP32-C3 firmware for coffee roaster control with ARTISAN+ serial protocol compatibility. Allows Artisan coffee roasting software to read temperature data and control heater/fan output via UART or USB CDC.

## Core Value

Artisan can read temperatures and control heater/fan during a roast session via serial connection.

## Current Milestone: v5.3 Deep Bug Analysis & Defect Report

**Goal:** Audit the whole repository to identify likely bugs, rank their criticity, and produce an implementation-ready defect report for a follow-up milestone.

**Target features:**
- Deep brownfield audit of firmware, scripts, tooling, and planning-visible behavior
- Structured report listing each bug, criticity, supporting evidence, and proposed fix description
- Milestone roadmap focused on investigation, evidence gathering, and report production rather than code changes

## Requirements

### Validated

- ✓ Artisan can read roaster telemetry over the serial protocol during a roast session.
- ✓ Artisan can control heater and fan outputs through the firmware command path.
- ✓ The project ships embedded diagnostics, traceability, and HIL-oriented tooling that support validation and audits.

### Active

- [ ] Audit the whole repository for likely defects across firmware, scripts, tooling, and planning-visible behavior.
- [ ] Produce a milestone report that records each bug, criticity, supporting evidence, and implementation-ready remediation guidance.
- [ ] Define follow-up implementation scope based on the confirmed bug inventory.

### Out of Scope

- Implementing bug fixes in this milestone — this cycle is for investigation, evidence, and scoping.
- Net-new product capabilities unrelated to defect discovery — keep the milestone focused on brownfield quality risks.

## Current State

- v5.2 shipped a flashable embedded build, unified error taxonomy, TRACE instrumentation, manifest-aware HIL validation, and safe-shutdown replay artifacts for diagnostics audits.
- The repo now spans firmware, host-side scripts, validation tooling, and planning artifacts that interact closely enough for regressions to cross subsystem boundaries.
- Phase 104 was left as the next planning thread for diagnostics replay automation, but this milestone shifts first to a broad defect hunt and prioritized report.

## Context

- LibreRoaster is already in active use as an embedded firmware + tooling codebase, so brownfield defects may exist in runtime control paths, diagnostics automation, or integration boundaries.
- Recent work increased instrumentation and auditability, which makes this a good time to inspect the system for correctness gaps before adding more functionality.
- The intended deliverable is a report that can seed a dedicated remediation milestone rather than a mixed analysis-and-fix milestone.

## Constraints

- **Scope**: Whole repository audit — cover firmware, scripts, tooling, and planning-visible behavior.
- **Delivery**: This milestone produces analysis and a bug report, not code fixes.
- **Quality bar**: Every reported bug should include criticity and enough context to plan remediation work.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Treat this milestone as a brownfield defect audit instead of a feature milestone | The current need is confidence in existing behavior and a prioritized bug backlog | — Pending |
| Require implementation-ready fix descriptions in the report | The follow-up milestone should be able to scope remediation directly from the findings | — Pending |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

<details>
<summary>Previous project context</summary>

Historical milestone write-ups (v5.1, v4.5, etc.) remain available in `.planning/MILESTONES.md` and the per-milestone archives.

</details>

---
*Last updated: 2026-04-16 after v5.3 milestone start*
