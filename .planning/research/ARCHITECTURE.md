# Architecture Patterns

**Domain:** Deep defect analysis and evidence-backed reporting for an existing embedded Rust firmware + tooling repo
**Researched:** 2026-04-16
**Confidence:** HIGH

## Recommended Architecture

Treat the v5.3 milestone as an **audit overlay on top of the existing firmware/tooling architecture**, not as a new runtime subsystem. The firmware should remain the evidence producer; host scripts and planning artifacts should become the evidence organizer. That keeps the 100 ms control loop stable and makes the output directly usable by a later remediation roadmap.

### Recommended System Shape

```text
┌─────────────────────────────────────────────────────────────────────┐
│ Existing firmware runtime (preserve)                               │
│                                                                     │
│ UART/USB tasks -> command queues/mux -> control_loop_task          │
│ -> RoasterControl -> sensors / SSR / fan / watchdog / telemetry    │
│ -> STATUS + TRACE + guard/safe-shutdown output                     │
└──────────────────────────────┬──────────────────────────────────────┘
                               │ evidence emission
                               ▼
┌─────────────────────────────────────────────────────────────────────┐
│ Existing host evidence tools (reuse first)                         │
│                                                                     │
│ tests/hardware/validation_runner.py                                 │
│ tests/hardware/analysis.py                                          │
│ scripts/traceability_matrix.py                                      │
│ replay-report.json / traceability replay artifacts                  │
└──────────────────────────────┬──────────────────────────────────────┘
                               │ normalized runs + parsed traces
                               ▼
┌─────────────────────────────────────────────────────────────────────┐
│ New audit layer (host/docs only)                                   │
│                                                                     │
│ audit inventory -> subsystem findings -> per-defect evidence pack  │
│ -> final defect catalog/report -> remediation-ready backlog input   │
└─────────────────────────────────────────────────────────────────────┘
```

## Component Boundaries

| Component | Responsibility | Communicates With |
|-----------|---------------|-------------------|
| Firmware runtime (`src/application`, `src/control`, `src/hardware`, `src/input`, `src/output`, `src/safety`) | Produce the real behavior under audit and emit STATUS/TRACE/guard evidence | UART/USB tasks, host capture tools |
| Transport ingestion (`src/hardware/uart/tasks.rs`, `src/hardware/usb_cdc/tasks.rs`, `src/input/multiplexer.rs`) | Show whether defects start at parsing, queueing, fallback, or active-channel arbitration | TRACE events, command channels, control loop |
| Control/state core (`src/application/tasks.rs`, `src/application/service_container.rs`, `src/control/roaster_refactored.rs`) | Central defect hotspot for timing, ownership, ordering, and state consistency | Sensors, actuator drivers, watchdog, formatter |
| Evidence capture tooling (`tests/hardware/validation_runner.py`) | Capture reproducible HIL/serial evidence into timestamped run folders | Firmware serial output, scenario manifest |
| Evidence analysis tooling (`tests/hardware/analysis.py`, trace parser, replay artifacts) | Convert raw runs/logs into threshold results, trace matrices, replay verification | Run CSVs, metadata JSON, TRACE logs |
| New audit catalog layer (recommended new milestone artifacts) | Convert subsystem findings into a roadmap-ready defect inventory | Existing evidence artifacts, planning docs |
| Planning docs (`.planning/PROJECT.md`, milestone roadmap/audit docs) | Preserve scope, criticity, sequencing, and follow-up remediation inputs | Final defect report |

## Integration Strategy

### Principle: Prefer host-side integration over firmware changes

For this milestone, add **documentation/reporting artifacts first**, and only modify firmware/scripts when a missing observation point prevents evidence-backed conclusions.

That means:

- Reuse existing TRACE, STATUS, HIL manifests, replay artifacts, and diagnostics docs.
- Add a defect catalog and evidence index before adding new telemetry.
- Only extend runtime instrumentation when a suspected bug cannot be localized from current outputs.

## New vs Modified Artifacts

### New artifacts recommended

| Artifact | Why it should exist |
|---------|----------------------|
| `.planning/audits/v5.3/DEFECT_REPORT.md` | Final human-readable report with criticity, evidence, and remediation guidance |
| `.planning/audits/v5.3/defects.json` | Machine-readable inventory for later roadmap scoping and status tracking |
| `.planning/audits/v5.3/EVIDENCE_INDEX.md` | Maps each defect ID to logs, traces, HIL runs, replay artifacts, tests, and source files |
| `.planning/audits/v5.3/subsystems/` | Per-subsystem working notes: firmware core, transports, diagnostics, scripts, docs-visible behavior |
| `.planning/audits/v5.3/evidence/` | Snapshots or copied references to generated trace matrices, analysis outputs, and replay summaries |

### Modified artifacts recommended

| Artifact | Modification | Why |
|---------|--------------|-----|
| `tests/hardware/scenario_manifest.json` | Add or tag defect-focused scenarios only where existing HIL coverage is insufficient | Reuse the current evidence path instead of inventing a new one |
| `tests/hardware/HIL-PLAYBOOK.md` | Add “defect audit capture” instructions | Makes evidence collection repeatable for audit runs |
| `tests/hardware/report_template.md` | Add defect ID / evidence reference placeholders if HIL reports are promoted into audit evidence | Lets one run support both validation and defect reporting |
| `internalDoc/TRACEABILITY_MATRIX.md` | Add defect-triage usage notes and expected audit outputs | Keeps TRACE interpretation consistent across auditors |
| `.planning/PROJECT.md` | Update validated/active items as findings become confirmed | Keeps milestone scope accurate |
| `.planning/ROADMAP.md` / `.planning/MILESTONES.md` | Update after report completion | Seeds the remediation milestone from confirmed defects |

### Firmware/runtime changes: only if needed

| Artifact | Change threshold |
|---------|------------------|
| `src/application/tasks.rs` | Only if existing stage/TRACE events do not isolate a timing/order defect |
| `src/logging/traceability.rs` | Only if current TRACE fields cannot identify cause/effect between queue, actuation, telemetry, and guard |
| `src/config/constants.rs` / `SystemStatus` | Only if a later defect hypothesis needs a missing observable field |

## Evidence Flow

The final report should not contain free-form bug claims. Each defect should be backed by a traceable evidence chain.

### Evidence pipeline

```text
Runtime behavior
  -> STATUS CSV snapshots
  -> TRACE lines
  -> guard/watchdog counters
  -> safe-shutdown traces
  -> HIL telemetry.csv + metadata.json
  -> replay-report.json / replay artifacts

Host normalization
  -> traceability matrix output
  -> threshold analysis reports
  -> scenario/run metadata
  -> subsystem notes

Audit synthesis
  -> defect record (ID, subsystem, symptom, criticity)
  -> linked evidence list
  -> suspected root cause
  -> proposed fix direction
  -> verification plan for remediation milestone
```

### Required defect record shape

Each entry in the new defect catalog should include at least:

| Field | Purpose |
|------|---------|
| `defect_id` | Stable reference for roadmap and remediation |
| `subsystem` | Firmware core, transport, diagnostics, host tooling, docs-visible behavior |
| `symptom` | What appears wrong |
| `impact` | User/safety/operability consequence |
| `criticality` | Critical / High / Medium / Low |
| `confidence` | Confirmed / Likely / Suspected |
| `evidence` | Exact files, logs, trace IDs, scenarios, tests |
| `root_cause_hypothesis` | Best current explanation |
| `proposed_fix` | Implementation-ready remediation direction |
| `verification_after_fix` | How the next milestone proves closure |

## Audit Order: What to inspect first

Use the repo’s existing observability to move from highest-leverage boundaries inward.

### 1. Evidence inventory and reproducibility baseline

Audit first:
- existing milestone artifacts
- TRACE docs
- HIL manifest/playbook
- replay artifacts
- test layout

Why first: the milestone fails if findings cannot be reproduced or cited.

Read/generate:
- `.planning/PROJECT.md`
- `.planning/ROADMAP.md`, `.planning/MILESTONES.md`
- `internalDoc/TRACEABILITY_MATRIX.md`
- `tests/hardware/METHODOLOGY.md`, `HIL-PLAYBOOK.md`, `scenario_manifest.json`
- current replay artifacts and threshold reports

### 2. Host-side tooling and evidence parsers

Audit second:
- `tests/hardware/validation_runner.py`
- `tests/hardware/analysis.py`
- trace/replay scripts

Why second: if the tooling miscaptures or misinterprets output, later firmware findings will be misleading.

### 3. Transport ingestion and channel arbitration

Audit third:
- UART task path
- USB CDC task path
- command queues / fallback paths
- `CommandMultiplexer`

Why third: many whole-system bugs in this repo can originate before command handling proper, especially around dual-channel behavior, queue depth, and active-channel ownership.

### 4. Control loop orchestration and shared state

Audit fourth:
- `control_loop_task`
- `ServiceContainer`
- async/sync roaster ownership handoff
- stage instrumentation / timing / watchdog ordering

Why fourth: this is the highest-risk integration boundary for subtle brownfield defects.

### 5. Domain logic and actuation boundaries

Audit fifth:
- `RoasterControl`
- handler/policy split
- PID/manual interaction
- SSR/fan/sensor update ordering
- safety shutdown behavior

Why fifth: by this point transport and control-loop evidence already tell you whether logical defects are local or cross-boundary.

### 6. Hardware-facing reliability and HIL scenarios

Audit sixth:
- sensor conversion and MAX31856 path
- LEDC guard / SSR / fan behavior
- watchdog / regression flows
- scenario thresholds and run analysis

Why sixth: these need the strongest evidence burden and should be targeted using earlier findings, not explored blindly.

### 7. Planning-visible behavior and user-facing auditability

Audit last:
- whether docs, templates, and report artifacts match actual behavior
- whether milestone outputs are enough for a follow-up fix roadmap

Why last: documentation defects matter, but only after the real system and evidence path are understood.

## Patterns to Follow

### Pattern 1: Evidence-First Defect Triage
**What:** Open a defect only when there is at least one concrete artifact proving or strongly indicating it.
**When:** Always.

**Example:**
```text
TRACE log shows queue_enqueue but no queue_dequeue for TraceId 184
+ queue depth remains elevated
+ watchdog/guard metadata degrades
= defect entry: likely queue starvation or task handoff defect
```

### Pattern 2: Reuse Existing Evidence Channels Before Adding New Ones
**What:** Prefer STATUS, TRACE, HIL CSV, metadata, and replay artifacts over new custom probes.
**When:** Brownfield audits where instrumentation already exists.

### Pattern 3: Subsystem Notes Feed a Single Canonical Report
**What:** Let auditors work per subsystem, but require every confirmed finding to land in one defect catalog.
**When:** Whole-repo audits.

### Pattern 4: Separate Observation from Remediation
**What:** Proposed fixes belong in the report, but code changes do not belong in this milestone.
**When:** This v5.3 milestone specifically.

## Anti-Patterns to Avoid

### Anti-Pattern 1: Writing the report from code inspection alone
**Why bad:** It produces plausible but unverified defects and weak remediation scope.
**Instead:** Require logs, tests, traces, HIL runs, or replay evidence for every non-trivial finding.

### Anti-Pattern 2: Adding heavy new runtime diagnostics everywhere
**Why bad:** Can change timing and hide or create bugs in the control loop.
**Instead:** Add minimal targeted observation only when existing TRACE/STATUS is insufficient.

### Anti-Pattern 3: Mixing confirmed bugs with speculative smells
**Why bad:** Makes a later remediation roadmap impossible to size.
**Instead:** Classify entries as Confirmed / Likely / Suspected and roadmap only from the confirmed+likely set.

### Anti-Pattern 4: Auditing firmware before auditing the capture scripts
**Why bad:** A bad parser or capture loop contaminates every downstream conclusion.
**Instead:** Validate the evidence pipeline first.

## Scalability Considerations

| Concern | At current repo scale | For this milestone |
|---------|------------------------|--------------------|
| Number of subsystems | Moderate but cross-coupled | Use subsystem notes, then merge into one defect catalog |
| Evidence volume | TRACE + HIL + replay can grow quickly | Keep an evidence index instead of burying raw logs in the final report |
| Confidence management | Some findings will be inference-heavy | Explicitly track Confirmed/Likely/Suspected |
| Remediation planning | Later milestone needs implementation-ready scope | Keep defect fields structured and stable |

## Suggested Build Order for Milestone Phases

### Phase 1: Audit scaffold and evidence contract

Create the audit folder structure and defect schema first.

Outputs:
- `DEFECT_REPORT.md` skeleton
- `defects.json` schema/template
- `EVIDENCE_INDEX.md`
- subsystem note files

### Phase 2: Evidence pipeline verification

Verify that TRACE parsing, HIL capture, threshold analysis, and replay artifacts are trustworthy.

Outputs:
- evidence-pipeline verification note
- list of any gaps in current tooling
- minimal script/doc changes only if needed

### Phase 3: Static brownfield repo audit

Inspect firmware, scripts, tooling, and planning-visible behavior for likely defects and record hypotheses.

Outputs:
- subsystem findings
- initial defect inventory with confidence labels

### Phase 4: Targeted evidence collection

Use existing tests, traces, HIL scenarios, and replay flows to confirm or reject the strongest hypotheses.

Outputs:
- linked logs, trace matrices, HIL reports, replay confirmations
- defect entries upgraded from suspected to likely/confirmed

### Phase 5: Criticity ranking and remediation framing

Normalize duplicates, rank impact, define fix directions, and identify prerequisite work.

Outputs:
- prioritized defect catalog
- remediation notes per defect
- dependency chains between fixes

### Phase 6: Roadmap handoff packaging

Produce the final report in a form that a follow-up milestone can directly consume.

Outputs:
- final defect report
- machine-readable defect list
- recommended remediation sequencing

## Why this order

- The repo already has observability; exploit it before changing code.
- Tooling must be trusted before it is used as evidence.
- Transport/control-loop defects can invalidate downstream symptoms, so they should be audited before low-level driver nuance.
- The final consumer is a remediation roadmap, so structured defect data must exist before broad investigation starts producing findings.

## Sources

- `.planning/PROJECT.md`
- `.planning/ROADMAP.md`
- `.planning/MILESTONES.md`
- `.planning/codebase/ARCHITECTURE.md`
- `.planning/codebase/INTEGRATIONS.md`
- `.planning/codebase/TESTING.md`
- `src/lib.rs`
- `src/main.rs`
- `src/application/tasks.rs`
- `src/application/service_container.rs`
- `src/control/roaster_refactored.rs`
- `src/hardware/uart/tasks.rs`
- `src/hardware/usb_cdc/tasks.rs`
- `src/input/multiplexer.rs`
- `src/logging/traceability.rs`
- `internalDoc/TRACEABILITY_MATRIX.md`
- `internalDoc/INSTRUMENTATION_README.MD`
- `tests/hardware/METHODOLOGY.md`
- `tests/hardware/validation_runner.py`
- `tests/hardware/analysis.py`
- `tests/hardware/report_template.md`
- `replay-report.json`

---
*Architecture research for: LibreRoaster v5.3 deep bug analysis & defect reporting*
