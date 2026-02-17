# Architecture Research

**Domain:** Artisan protocol command/response handling in ESP32-C3 roaster firmware
**Researched:** 2026-02-17
**Confidence:** MEDIUM

## Standard Architecture

### System Overview

```
┌────────────────────────────────────────────────────────────────────────┐
│                          Serial I/O Layer                              │
├────────────────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌─────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ Serial Input │→ │ Parser      │→ │ Multiplexer │→ │ ArtisanInput │  │
│  └──────────────┘  └─────────────┘  └──────────────┘  └──────────────┘  │
├────────────────────────────────────────────────────────────────────────┤
│                         Command/Control Layer                          │
├────────────────────────────────────────────────────────────────────────┤
│  ┌──────────────────────────────┐  ┌────────────────────────────────┐  │
│  │ RoasterControl               │→ │ ArtisanCommandHandler           │  │
│  │ (system status snapshot)     │  │ (interprets Artisan commands)   │  │
│  └──────────────────────────────┘  └────────────────────────────────┘  │
├────────────────────────────────────────────────────────────────────────┤
│                          Formatting/Output Layer                       │
│  ┌──────────────────────────────────────┐  ┌────────────────────────┐  │
│  │ ArtisanFormatter                      │→ │ Serial Output          │  │
│  │ (READ CSV, ROR/delta state)          │  │ (terminator correctness)│  │
│  └──────────────────────────────────────┘  └────────────────────────┘  │
└────────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| Serial Input | Receives bytes, frames lines | UART task + line buffer |
| Parser | Converts ASCII to ArtisanCommand | tokenization + enum mapping |
| Multiplexer | Routes commands to correct channel/task | channel send + filter/units handling |
| ArtisanInput Task | Feeds command pipeline | queue reader + dispatch |
| RoasterControl | Owns system status snapshot | state struct + control loop |
| ArtisanCommandHandler | Applies commands to control/state | handler match + update state |
| ArtisanFormatter | Builds READ response CSV + derived metrics | stateful formatter |
| Serial Output | Writes response with correct terminator | UART write + flush |

## Recommended Project Structure

```
src/
├── input/                 # Serial ingest + parsing
│   ├── parser.rs          # Artisan command parsing
│   └── multiplexer.rs     # Channel routing
├── control/               # Control loop + handlers
│   ├── roaster_refactored.rs  # status snapshot + dispatch
│   └── handlers.rs        # ArtisanCommandHandler
├── output/                # Protocol formatting
│   └── artisan.rs         # READ response + ROR/delta metrics
├── tasks/                 # RTOS tasks
│   └── artisan_input.rs   # channel reader
└── tests/                 # unit/integration tests
    └── artisan_protocol.rs
```

### Structure Rationale

- **input/:** parser and multiplexer are the earliest integration points for command correctness.
- **control/:** status updates must be centralized to keep READ responses consistent.
- **output/:** terminator correctness and derived metrics belong in formatter.

## Architectural Patterns

### Pattern 1: Single-Source Formatter Policy

**What:** One formatter owns CSV shape, units, and line termination.
**When to use:** Protocol responses must be consistent across tasks.
**Trade-offs:** Adds state to formatter; tests must validate both values and terminator.

**Example:**
```rust
// Pseudocode
fn format_read_response(status: &SystemStatus) -> String {
    let csv = format!("{:.1},{:.1},{:.1},{:.1}", et, bt, fan, heater);
    format!("{}\r\n", csv)
}
```

### Pattern 2: Derived-Metric State in Formatter

**What:** ROR/delta metrics are computed in formatter from prior samples.
**When to use:** Derived metrics are output-only and should not affect control logic.
**Trade-offs:** Requires explicit update cadence and tests for first-sample behavior.

**Example:**
```rust
// Pseudocode
fn update_delta_bt(&mut self, bt: f32, now_ms: u64) {
    let dt = now_ms - self.last_bt_ms;
    self.delta_bt = if dt > 0 { (bt - self.last_bt) / dt as f32 } else { 0.0 };
}
```

### Pattern 3: Snapshot-Then-Format

**What:** READ uses a single status snapshot to avoid mixed-timestamp fields.
**When to use:** Commands return multi-field telemetry.
**Trade-offs:** Requires explicit snapshot creation in RoasterControl or handler.

## Data Flow

### Request Flow (READ)

```
READ command
    ↓
Parser → Multiplexer → ArtisanInput Task
    ↓
RoasterControl updates status snapshot
    ↓
ArtisanFormatter formats CSV + terminator
    ↓
Serial Output writes bytes
```

### State Management

```
SystemStatus snapshot
    ↓ (read)
ArtisanFormatter ←→ ROR/Delta state cache
    ↓ (format)
READ response string
```

### Key Data Flows

1. **READ response:** snapshot → formatter → CRLF-terminated CSV
2. **Delta/ROR update:** status temperatures → formatter cache → derived values in output

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| Single device | Current pipeline is sufficient |
| Multiple devices | Extract per-device formatter state |
| High-rate sampling | Decouple sample tick from READ commands |

### Scaling Priorities

1. **First bottleneck:** formatter state drift if updates only on READ; fix by sampling cadence.
2. **Second bottleneck:** inconsistent terminator when multiple outputs; fix by single formatter policy.

## Anti-Patterns

### Anti-Pattern 1: Terminator Logic Split Across Tasks

**What people do:** Append `\n` in formatter but `\r\n` in output task.
**Why it's wrong:** Creates inconsistent framing and READ parsing failures.
**Do this instead:** Keep terminator policy in `ArtisanFormatter` only.

### Anti-Pattern 2: ROR/delta Computed in Control Loop

**What people do:** Update delta/ROR inside control logic alongside PID.
**Why it's wrong:** Derived metrics leak into control responsibilities.
**Do this instead:** Compute in formatter using status snapshot inputs.

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| Artisan (host app) | ASCII serial protocol | Requires exact terminator and field order |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| Parser ↔ Multiplexer | enum + channel | Ensure READ path preserved for response triggering |
| RoasterControl ↔ ArtisanCommandHandler | direct call | READ should update status snapshot only |
| RoasterControl ↔ ArtisanFormatter | status snapshot | Single snapshot per READ |
| ArtisanFormatter ↔ Serial Output | formatted string | Formatter owns terminator policy |

## Integration Points for Edge-Case Fixes

### READ Response Terminator Correctness

- **Where change lives:** `src/output/artisan.rs` (formatter) and tests.
- **Integration point:** Formatter returns CRLF-terminated string; Serial Output sends as-is.
- **Data flow change:** None, but enforce single source of truth for terminator.

### delta_bt / ROR State Update Behavior

- **Where change lives:** `src/output/artisan.rs` (formatter state), optional helper in `control/` if snapshot timing needs to be centralized.
- **Integration point:** Formatter updates derived metrics on each READ (or on sample tick if available), using the same status snapshot used for formatting.
- **Data flow change:** Add explicit formatter update call before format if not already done.

## Suggested Build Order

1. **Define formatter contract:** Update formatter to own terminator and add tests for CRLF.
2. **Stabilize delta/ROR update timing:** Ensure formatter updates derived metrics with a consistent cadence.
3. **Wire integration tests:** End-to-end READ response includes correct terminator and stable ROR/delta.

## Sources

- LibreRoaster pipeline description from milestone context (unverified, no codebase review)

---
*Architecture research for: Artisan protocol edge-case fixes*  
*Researched: 2026-02-17*
