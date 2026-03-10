# Architecture Research

**Domain:** Embedded firmware quality hardening (Embassy async, ESP32-C3 roaster control)
**Researched:** 2026-03-07
**Confidence:** HIGH

## Standard Architecture

### System Overview

```
┌────────────────────────────────────────────────────────────────────────────┐
│                     Host/Validation Layer (new in v5.0)                   │
├────────────────────────────────────────────────────────────────────────────┤
│  Audit inventory   Quality gates   Hardware evidence pack (Artisan Scope) │
│  (dead code map)   (lint/test/RT)  (command trace + telemetry + logs)     │
└───────────────────────────────┬────────────────────────────────────────────┘
                                │ does not run in control loop
┌───────────────────────────────▼────────────────────────────────────────────┐
│                     Runtime Firmware (existing)                            │
├────────────────────────────────────────────────────────────────────────────┤
│  UART/USB reader tasks -> command queues -> multiplexer -> artisan_channel│
│                                      │                                     │
│                                      ▼                                     │
│                       control_loop_task (100 ms cadence)                   │
│                    process_artisan_command/process_command                 │
│                                      │                                     │
│                [Safety | Temperature | Artisan | System handlers]          │
│                                      │                                     │
│                 apply_guarded_heater + fan set_speed + watchdog feed       │
│                                      │                                     │
│            formatter/read status -> output_channel -> dual_output_task      │
└────────────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| `ServiceContainer` | Shared state + channels + async/sync access boundaries | `src/application/service_container.rs` with `EmbassyMutex` + `critical_section::Mutex` |
| `control_loop_task` | Deterministic orchestration (sensor/control/watchdog/telemetry) | `src/application/tasks.rs` 100 ms loop with stage tracking |
| `RoasterControl` + handlers | Command semantics, PID/manual transitions, safety control paths | `src/control/roaster_refactored.rs`, `src/control/handlers.rs` |
| Transport ingestion | Parse + queue + mux isolation per channel | `src/hardware/uart/tasks.rs`, `src/hardware/usb_cdc/tasks.rs`, `src/input/multiplexer.rs` |
| v5.0 quality layer (new) | Audit workflow, regression gates, hardware evidence packaging | New `.planning/quality/`, `tests/hardware/`, and validation scripts/docs |

## Recommended Project Structure

```
.planning/
├── quality/
│   ├── dead-code-inventory.md       # Candidate removals with evidence and owner
│   ├── dependency-map.md            # Module dependency map and refactor seams
│   └── gate-results/                # Per-run quality gate outputs
├── research/
│   └── ARCHITECTURE.md              # This document
tests/
├── hardware/
│   ├── artisan-scope-checklist.md   # Manual validation checklist
│   └── evidence-template.md         # Required artifacts for HW-01 proof
└── ... existing host tests ...
src/
├── control/
│   ├── roaster_refactored.rs        # Existing orchestration, incrementally slimmed
│   ├── handlers.rs                  # Existing command handlers, preserve authority
│   └── (new) command_router.rs      # Optional thin seam before handler chain
├── safety/
│   └── regression.rs                # Existing regression trigger path
└── application/
    └── tasks.rs                     # Existing cadence-critical loop, avoid heavy changes
```

### Structure Rationale

- **`.planning/quality/`:** Keeps refactor evidence and dead-code decisions out of runtime code and provides rollback context.
- **`tests/hardware/`:** Creates a repeatable, auditable evidence path for Artisan Scope without mixing manual procedures into unit tests.
- **`src/control/` seam-first updates:** Limits blast radius by extracting interfaces before moving behavior.

## Architectural Patterns

### Pattern 1: Shadow-First Dead-Code Removal

**What:** Mark candidates, map inbound callers, then remove in small batches guarded by target-specific checks.
**When to use:** Any cleanup touching control, safety, transport, or shared container state.
**Trade-offs:** Slower than bulk deletion, but much lower regression risk for real-time firmware.

**Example:**
```rust
// Step 1: deprecate + redirect
#[deprecated(note = "v5.0 cleanup candidate; remove after gate pass")]
pub fn legacy_path(...) { new_path(...) }

// Step 2: remove only after host + target gates are green
```

### Pattern 2: SOLID via Ports-and-Policies (incremental)

**What:** Keep hardware adapters (`Heater`, `Fan`, sensor hub) as ports; move command policy logic to handlers/router seams.
**When to use:** Large functions where policy and I/O are intertwined (`process_artisan_command`, `update_control` edges).
**Trade-offs:** Adds indirection, but improves testability and dead-code detection clarity.

**Example:**
```rust
pub trait ManualCommandPolicy {
    fn on_heater(&mut self, value: u8, status: &mut SystemStatus) -> Result<(), RoasterError>;
}

// Handler remains authoritative; RoasterControl applies outputs after policy success.
```

### Pattern 3: Out-of-Band Quality Gates

**What:** Run lint/test/timing checks outside the 100 ms loop; only expose lightweight counters in runtime state.
**When to use:** Any quality hardening effort that could add overhead in control path.
**Trade-offs:** Requires better tooling discipline, but preserves deterministic behavior.

## Data Flow

### Request Flow (runtime, preserved)

```
[Artisan Scope/UART/USB Command]
    ↓
[uart_reader_task | usb_reader_task]
    ↓
[CommandQueue + Multiplexer]
    ↓
[artisan_channel]
    ↓
[control_loop_task]
    ↓
[RoasterControl::process_artisan_command -> process_command -> handlers]
    ↓
[apply_guarded_heater / fan.set_speed / watchdog + STATUS/READ formatting]
```

### State Management (v5.0 additions)

```
[SystemStatus + queue/lock metrics]
    ↓
[Host gate runner captures snapshots]
    ↓
[.planning/quality/gate-results/*.md + hardware evidence bundle]
```

### Key Data Flows

1. **Safety-critical control flow:** command -> handler authority -> guarded actuation -> status snapshot, unchanged except seam extraction.
2. **Quality evidence flow (new):** tests/log snapshots/manual Artisan Scope run -> structured artifacts -> milestone audit decision.

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| Current (single-board dev + host tests) | Keep monolithic runtime; add only host-side quality tooling |
| Multi-board validation bench | Add board-profiled evidence templates and per-board gate result partitions |
| Ongoing milestone cadence | Automate quality gate matrix and require evidence artifacts before cleanup merges |

### Scaling Priorities

1. **First bottleneck:** confidence in dead-code safety; solve with module-level inventory + dependency map before deletion.
2. **Second bottleneck:** proving real-world behavior after refactors; solve with repeatable Artisan Scope evidence package.

## Anti-Patterns

### Anti-Pattern 1: Big-Bang Refactor in `RoasterControl`

**What people do:** Rewrite `process_artisan_command` + handler boundaries in one large PR.
**Why it's wrong:** Breaks traceability and makes safety regressions hard to isolate.
**Do this instead:** Extract seams first, move one command family at a time, verify after each slice.

### Anti-Pattern 2: Quality Checks Inside Control Path

**What people do:** Add heavy diagnostics, allocations, or verbose formatting inside 100 ms tick.
**Why it's wrong:** Risks missed deadlines and false watchdog/safety behavior changes.
**Do this instead:** Keep runtime metrics minimal; perform analysis in host tests and post-run tooling.

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| Artisan Scope (operator UI) | Existing serial protocol over USB/UART | Primary HW-01 validation driver; no protocol expansion required for v5.0 |
| Host CI/local runner (new) | `cargo check/test/clippy` + scripted artifact capture | Must include both host and `riscv32` checks to avoid host-only confidence |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| `hardware/*/tasks` <-> `ServiceContainer` | queues + channels | Preserve queue/multiplexer semantics while adding gate instrumentation |
| `control_loop_task` <-> `RoasterControl` | direct async closure call | Keep cadence-critical path stable; prefer wrapper seams over internal rewrites |
| `RoasterControl` <-> handlers | handler chain (`RoasterCommandHandler`) | Keep `ArtisanCommandHandler` authoritative for manual outputs |
| `RoasterControl` <-> heater/fan/watchdog | trait calls + status snapshots | Any refactor must preserve `apply_guarded_heater` and watchdog ordering |
| runtime telemetry <-> evidence artifacts (new) | STATUS/READ/log capture | New docs/scripts consume outputs without mutating runtime behavior |

## New vs Modified Components for v5.0

| Component | Type | Purpose |
|-----------|------|---------|
| `.planning/quality/dead-code-inventory.md` | New | Dead-code candidates with usage evidence and removal order |
| `.planning/quality/dependency-map.md` | New | Module coupling map to guide SOLID seam extraction |
| `tests/hardware/artisan-scope-checklist.md` | New | Manual real-roaster validation checklist with pass/fail criteria |
| `tests/hardware/evidence-template.md` | New | Standardized artifact pack format for milestone audits |
| `src/control/roaster_refactored.rs` | Modified (incremental) | Extract router/policy seams without changing command semantics |
| `src/application/tasks.rs` | Modified (minimal) | Keep stage/watchdog/status capture stable; only add lightweight hooks if needed |
| existing host tests (`tests/*`) | Modified/extended | Convert into quality gates for cleanup + SOLID refactor confidence |

## Suggested Build Order (Minimize Regression Risk)

1. **Baseline freeze + guardrails**
   - Capture current host/target green baseline (`cargo test`, `cargo check --target riscv32imc-unknown-none-elf`, clippy).
   - Record current safety/timing signals from `control_loop_task` logs and STATUS fields.

2. **Dead-code audit only (no removals yet)**
   - Produce `dead-code-inventory.md` and dependency map from actual call sites.
   - Classify candidates: runtime-critical, test-only, legacy/docs-only.

3. **Low-risk removals first**
   - Remove isolated dead code (unreferenced helpers/docs-only branches) in tiny PRs.
   - Run full gate matrix after each batch, including transport and concurrency tests.

4. **SOLID seam extraction before behavior movement**
   - Introduce thin routing/policy seams around command handling.
   - Keep command behavior byte-for-byte equivalent, verified by existing integration suites.

5. **Incremental behavior refactor behind proven seams**
   - Move one command family at a time; keep `ArtisanCommandHandler` and safety ordering authoritative.
   - Re-verify watchdog, SSR guard, queue depth, and lock-depth tests on each step.

6. **Hardware evidence phase (HW-01)**
   - Execute Artisan Scope checklist on real hardware.
   - Store artifact bundle (commands sent, serial responses, safety logs, observed actuator behavior).
   - Only close milestone once host gates and hardware evidence both pass.

## Sources

- Project milestone context: `.planning/PROJECT.md`
- Runtime orchestration: `src/application/tasks.rs`, `src/application/service_container.rs`, `src/application/app_builder.rs`
- Control boundaries: `src/control/roaster_refactored.rs`, `src/control/handlers.rs`
- Transport and mux: `src/hardware/uart/tasks.rs`, `src/hardware/usb_cdc/tasks.rs`, `src/input/multiplexer.rs`, `src/input/parser.rs`
- Existing instrumentation and regression hooks: `src/safety/regression.rs`, `src/application/queue_metrics.rs`
- Existing hardware verification references: `tests/TEST-01-SSR-Guard.md`, `tests/dual_channel_verification.md`, `internalDoc/ARTISAN_CONNECTION.md`

---
*Architecture research for: LibreRoaster v5.0 quality hardening*
*Researched: 2026-03-07*
