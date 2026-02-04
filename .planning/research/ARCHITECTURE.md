# Architecture Research

**Domain:** Artisan command coverage & formatter hardening on ESP32-C3 (embassy-rs)
**Researched:** 2026-02-04
**Confidence:** MEDIUM (repo context available; limited external validation)

## Standard Architecture

### System Overview

```
┌──────────────────────────────────────────────────────────────────┐
│                          Artisan Client                          │
│            (PC app sending/receiving Artisan/Artisan+ cmds)      │
└───────────────┬───────────────────────────────┬──────────────────┘
                │ UART 115200 8N1               │
                │                                │
┌───────────────▼────────────────────────────────▼─────────────────┐
│                         ESP32-C3 Firmware                         │
├──────────────────────────────────────────────────────────────────┤
│  RTOS-less async (embassy) tasks                                  │
│  ┌──────────────────────────┐  ┌────────────────────────────┐     │
│  │ uart_reader_task         │  │ uart_writer_task           │     │
│  │ (rx bytes→frame)         │  │ (csv→tx bytes)             │     │
│  └─────────────┬────────────┘  └───────────┬────────────────┘     │
│                │                           │                      │
│        ┌───────▼────────┐          ┌───────▼─────────┐            │
│        │ parser/input   │          │ output formatter │            │
│        │ (Artisan cmd   │          │ (CSV/Artisan+    │            │
│        │ decode)        │          │ response encode) │            │
│        └───────┬────────┘          └───────┬─────────┘            │
│                │                           │                      │
│        ┌───────▼─────────────┐     ┌───────▼────────────┐         │
│        │ command dispatcher  │     │ roaster state      │         │
│        │ (Artisan handler)   │     │ (sensors, control  │         │
│        │                     │     │ loops, setpoints)  │         │
│        └─────────────────────┘     └────────────────────┘         │
└──────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| `uart_reader_task` | Pull bytes from UART, frame commands, push to command pipe | embassy UART driver + bounded channel |
| `parser/input` | Parse Artisan/Artisan+ commands, normalize arguments, emit typed `ArtisanCommand` | nom/hand-rolled parser with strict validation |
| `command dispatcher` | Map `ArtisanCommand` to control actions and responses; enforce mode constraints | match table with handler registry; command coverage matrix |
| `roaster state/control` | Maintain device state, apply heater/fan setpoints, compute metrics | state struct + control loop tasks |
| `output formatter` | Format status/acks/errors into CSV strings and new Artisan+ forms | formatter trait with golden outputs and property tests |
| `uart_writer_task` | Serialize formatted responses to UART with backpressure | embassy UART TX + ring buffer |
| `mock UART + integration harness` | Simulate RX/TX for tests; capture round-trips | in-memory channel implementing UART traits |

## Recommended Project Structure

```
firmware/
├── uart/                 # hardware driver + reader/writer tasks
├── input/                # command parser, normalization, error types
├── control/              # command dispatcher + roaster control handlers
├── output/               # formatters (Artisan/Artisan+), CSV encoder
├── test_support/         # mock UART, golden fixtures, fuzz helpers
└── tests/                # integration tests (cmd→resp), coverage matrices
```

### Structure Rationale

- **uart/**: isolates hardware concerns and buffering/backpressure from protocol logic.
- **input/**: keeps parsing and validation pure/testable; simplifies coverage measurement.
- **control/**: central command registry to ensure complete coverage and mode gating.
- **output/**: formatter variants and error shaping live together for consistency.
- **test_support/**: reusable mocks/fixtures prevent duplication across unit and integration tests.
- **tests/**: top-level round-trip specs that mirror real Artisan command flows.

## Architectural Patterns

### Pattern 1: Command Registry with Coverage Matrix

**What:** Single table mapping command strings → handler + formatter + capability flags (requires Artisan+, read-only, etc.) plus a test-visible matrix to assert coverage.

**When to use:** Growing protocol surface where omissions are easy; when both parser and dispatcher must stay in sync.

**Trade-offs:** Slight indirection cost; requires disciplined updates; simplifies completeness checks.

**Example:**
```rust
pub struct CommandSpec {
    pub name: &'static str,
    pub mode: Mode,
    pub handler: fn(&mut Ctx, Args) -> Result<Response, Error>,
}

pub static COMMANDS: &[CommandSpec] = &[
    cmd!("CHARGE", Mode::Artisan, handle_charge),
    cmd!("DRUM", Mode::ArtisanPlus, handle_drum),
];

pub fn dispatch(cmd: ArtisanCommand, ctx: &mut Ctx) -> Result<Response, Error> {
    COMMANDS
        .iter()
        .find(|c| c.name == cmd.name)
        .ok_or(Error::Unsupported)?
        .handler(ctx, cmd.args)
}
```

### Pattern 2: Formatter Golden + Property Tests

**What:** Golden CSV outputs for representative states plus property tests (e.g., numeric precision, field counts, NaN handling) to harden formatter.

**When to use:** Protocols consumed by brittle clients (Artisan CSV parsing) or when adding new fields.

**Trade-offs:** Golden files need maintenance; property tests add CI time but catch edge cases early.

**Example:**
```rust
proptest! {
    #[test]
    fn csv_has_fixed_columns(s in any<State>()) {
        let line = artisan::format(&s);
        prop_assert_eq!(line.split(',').count(), EXPECTED_COLS);
    }
}
```

### Pattern 3: End-to-End Loop with Mock UART

**What:** In-memory UART implementing `embedded_hal::serial::Read/Write` used by embassy tasks; drives real parser/dispatcher/formatter.

**When to use:** Validate command→response timing, backpressure, and error paths before hardware bring-up.

**Trade-offs:** Must model UART behavior (latency, partial frames); adds maintenance but enables deterministic tests.

## Data Flow

### Command Flow (runtime)

```
Artisan PC → UART RX → uart_reader_task
    → parser/input → ArtisanCommand
    → command dispatcher → control/state update
    → output formatter → CSV/Artisan+ string
    → uart_writer_task → UART TX → Artisan PC
```

### Test Flow (integration harness)

```
Test injects bytes → mock UART RX → reader task
    → parser → dispatcher → control/state
    → formatter → writer task → mock UART TX capture
    → assertions on bytes/strings + state snapshot
```

### Key Data Flows

1. **Command coverage:** Registry drives both parser acceptance tests and dispatcher match tests to ensure every command has parser, handler, and response.
2. **Formatter robustness:** State snapshots → formatter → golden/property assertions; errors surface as structured formatter errors before UART send.
3. **Integration loop:** Mock UART feeds real tasks; asserts timing/backpressure (bounded channels not overflowing) and correct responses for sequences.

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| 0-10 cmds | Direct match arms acceptable; manual tests fine |
| 10-40 cmds | Command registry + coverage matrix; golden outputs per mode |
| 40+ cmds | Codegen command tables; fuzz/parser property tests; split formatter modules |

### Scaling Priorities

1. **First bottleneck:** Parser drift vs dispatcher → mitigate with shared registry + coverage tests.
2. **Second bottleneck:** Formatter regressions → mitigate with golden/property suites and strict CI.
3. **Third bottleneck:** UART buffer overruns → mitigate with bounded channels, backpressure tests, and periodic flush.

## Anti-Patterns

### Anti-Pattern 1: Hand-coded parser branches without shared registry

**What people do:** Add match arms ad hoc in parser/dispatcher separately.
**Why it's wrong:** Leads to partial support and unhandled commands; coverage unknown.
**Do this instead:** Single source of truth registry consumed by parser tests and dispatcher.

### Anti-Pattern 2: Formatting directly in handlers

**What people do:** Each handler emits its own CSV fragments.
**Why it's wrong:** Inconsistent columns and precision; hard to audit.
**Do this instead:** Central formatter with golden tests; handlers return typed responses.

### Anti-Pattern 3: Tests bypassing UART tasks

**What people do:** Call handlers directly in tests.
**Why it's wrong:** Misses framing/backpressure and integration issues.
**Do this instead:** Use mock UART to drive actual tasks end-to-end.

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| Artisan / Artisan+ PC app | UART (115200 8N1) CSV protocol | Validate both legacy Artisan and Artisan+ command sets; maintain compatibility with strict CSV parsing. |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| uart_reader_task ↔ parser/input | Bytes over channel; framed lines | Keep framing tolerant to partial lines; expose hook for test injection. |
| parser/input ↔ command dispatcher | Typed `ArtisanCommand` | Enforce mode (Artisan vs Artisan+) and argument validation before dispatch. |
| dispatcher ↔ control/state | Direct calls with mutable context | Keep state mutations side-effect scoped; return typed response/errors. |
| formatter ↔ uart_writer_task | Strings/enums → bytes | Formatter should never panic; writer handles backpressure with bounded queue. |
| test_support ↔ tasks | Mock UART implementing serial traits | Allows deterministic integration tests without hardware. |

## Build Order (suggested for milestone)

1. Command registry + coverage matrix (enumerate full Artisan/Artisan+ set, mark implemented/unimplemented).
2. Formatter hardening (golden cases + property tests; error shaping for invalid inputs/NaNs/overflow).
3. Mock UART + test_support utilities (channels, timeouts, captured TX buffer).
4. Integration tests for command sequences (success + error paths; backpressure cases).
5. CI wiring (run unit + integration suites; optional fuzz seeds).

## Sources

- Existing LibreRoaster firmware modules (`uart/`, `input/`, `control/`, `output/`).
- Artisan protocol expectations from current project notes (no external spec consulted).

---
*Architecture research for: LibreRoaster Artisan integration polish*
*Researched: 2026-02-04*
