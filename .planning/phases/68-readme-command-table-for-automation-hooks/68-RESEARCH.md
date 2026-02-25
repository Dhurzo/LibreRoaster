# Phase 68: README Command Table for Automation Hooks - Research

**Researched:** 2026-02-23
**Domain:** Documentation discoverability for automation hooks
**Confidence:** MEDIUM

## Summary

This phase is about making the automation hooks that already exist in the firmware discoverable inside `README.md`: the Supported Artisan Commands table must now list `REG` and `STATUS/STAT`, and the document must point automation engineers to `internalDoc/INSTRUMENTATION_README.MD` so they know how those hooks behave.

I traced the commands through `src/input/parser.rs` (STATUS/STAT alias, `REG` returns `ArtisanCommand::RunRegression`), the regression task (`SAFETY OT-REGRESSION` log, watchdog-feeding dance), and `ArtisanFormatter::format_status_response` (nine-field CSV with watchdog/guard/regression columns). These are the facts the planner needs to shape the wording and placement of the new rows.

- `REG` enqueues an over-temperature regression (`request_regression`) that ramps heater + fan to 100%, forces an emergency shutdown, feeds the watchdog, and emits `SAFETY OT-REGRESSION` so instrumentation can correlate the regression state.
- `STATUS`/`STAT` returns `ET,BT,Heater,Fan,WatchdogOK,WatchdogFailures,LastWatchdogReason,LEDCGuardTimeouts,RegressionActive`; `STATUS` is the automation hook that keeps the legacy READ values and appends deterministic safety telemetry.
- Automation readers should land on `internalDoc/INSTRUMENTATION_README.MD` immediately after seeing those commands so they can consume the column definitions and regression notes without guessing.

**Primary recommendation:** Extend the Supported Artisan Commands table with explicit `REG` and `STATUS/STAT` rows that explain each hook’s automation purpose and add a sentence beside the table pointing to `internalDoc/INSTRUMENTATION_README.MD` for the CSV and regression details.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Markdown / CommonMark | n/a | Format `README.md` and `internalDoc/INSTRUMENTATION_README.MD` | Both documents already live in Markdown; contributors expect to edit them as plain text tables and links. |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `internalDoc/INSTRUMENTATION_README.MD` | current | Source of instrumentation-grade explanations for `STATUS` and regression expectations | Quote or link it when describing automation hooks and CSV columns. |
| `src/input/parser.rs` | Rust 1.88 | Source of command availability and aliases (STAT) | Confirm the README description matches actual parsing logic. |
| `src/output/artisan.rs` | Rust 1.88 | Defines the CSV formatter (`format_status_response`) | Copy the column order and field semantics directly from the implementation. |
| `src/safety/regression.rs` | Rust 1.88 | Details what running `REG` does and how the regression task behaves | Mention `SAFETY OT-REGRESSION` output so automation knows what log to watch for. |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Writing a new automation guide inside `README.md` | Create a dedicated `Automation Hooks` section with full column explanations | Duplicates the instrumentation doc, risks drifting from the tested CSV description, and splits readers between two sources. |
| Introducing a separate CSV spec in code comments | Link directly to instrumentation code in `src/output/artisan.rs` | Less discoverable for non-Rust readers than pointing to the curated Markdown guide. |

**Installation:**
```bash
# No dependencies; update `README.md` (and optionally the nearby description) with a Markdown-savvy editor.
```

## Architecture Patterns

### Recommended Project Structure
```text
README.md
├── Features
├── Async Architecture
├── Supported Artisan Commands    # table that must host REG/STATUS rows
├── Quick Start
└── ...

internalDoc/INSTRUMENTATION_README.MD   # referenced for CSV and regression metadata
```

### Pattern 1: Command Table Extension
**What:** Treat the Supported Artisan Commands table as the single source of truth for the commands Artisan-friendly users and automation engineers can send. Add new rows for `REG` and `STATUS/STAT` (with alias mention) so automation hooks sit alongside legacy READ/OT/IO entries.
**When to use:** Always update this table when a new command is exported or automation relies on hidden hooks.
**Example:** see the Markdown snippet in the Code Examples section.

### Anti-Patterns to Avoid
- **Cramming instrumentation info into long paragraphs:** Automation hooks are best discovered directly in the table; keep the row descriptions short and add a single nearby sentence pointing to the instrumentation guide.
- **Missing the `STAT` alias:** Readers (and tests) expect both `STATUS` and `STAT`; forgetting the alias in prose can confuse automation authors who poll with `STAT`.
- **Describing `REG` without mentioning `SAFETY OT-REGRESSION` or watchdog implications:** Automation needs to know what log to expect and that regression collisions should be avoided.

## Don't Hand-Roll
| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Describing the `STATUS` CSV by hand in README | A new mini-spec embedded near the table | Reference `internalDoc/INSTRUMENTATION_README.MD` (already lists the nine columns and automation guidance) | Keeps instrumentation expectations centralized and prevents divergence from the implementation. |
| A standalone regression workflow doc | Additional prose in the `REG` row plus the instrumentation guide link | Point readers to the existing regression task description and mention the emitted `SAFETY OT-REGRESSION` string from `src/safety/regression.rs`. | Avoids duplication and ensures readers know which log entry proves the regression ran. |

**Key insight:** These automation hooks surface data that already exists; focus the README update on discoverability (table rows + link), not on re-implementing the instrumentation guide inside the README.

## Common Pitfalls

### Pitfall 1: Leaving the lower CSV columns undocumented
**What goes wrong:** Automation authors see a `STATUS` command but have no idea what `WatchdogFailures` or `RegressionActive` mean.
**Why it happens:** The old README only documented `READ`, so the new automation-only fields were never written down.
**How to avoid:** Mention that `STATUS` returns `ET,BT,Heater,Fan,WatchdogOK,WatchdogFailures,LastWatchdogReason,LEDCGuardTimeouts,RegressionActive` and point to the instrumentation guide for column meanings.
**Warning signs:** Automation harnessers still ask about `RegressionActive` or re-interpret `STATUS` as `READ`.

### Pitfall 2: Forgetting to flag `STAT` as an alias
**What goes wrong:** Automation tooling that polls `STAT` fails because documentation only mentions `STATUS`.
**Why it happens:** The parser allows both, but README historically documented only one form.
**How to avoid:** Label the row as `STATUS/STAT` and, if needed, call out “STAT is an alias that behaves identically.”
**Warning signs:** Users report `STATUS` works but `STAT` is “unknown command”.

### Pitfall 3: Burying the instrumentation link far from the table
**What goes wrong:** Readers miss the automation guide and remain unaware of the instrumentation expectations.
**Why it happens:** README sections can drift apart in big docs.
**How to avoid:** Add a concise sentence adjacent to the Supported Artisan Commands table referencing `internalDoc/INSTRUMENTATION_README.MD` for automation expectations.
**Warning signs:** Automation threads ask “Where is the STATUS CSV defined?” even after the README is updated.

## Code Examples

### Support table snippet
```markdown
| Command          | Description |
|------------------|-------------|
| `REG`            | Triggers the over-temperature regression that emits `SAFETY OT-REGRESSION`; automation should treat it as a regression driver & watch the instrumentation log. |
| `STATUS/STAT`    | Polls watchdog/guard/regression telemetry (ET,BT,Heater,Fan,WatchdogOK,WatchdogFailures,LastWatchdogReason,LEDCGuardTimeouts,RegressionActive); see `internalDoc/INSTRUMENTATION_README.MD` for column meanings. |
```

### Automation reference sentence
```markdown
Automation harnesses should consult `internalDoc/INSTRUMENTATION_README.MD` for the `STATUS` CSV columns and any `REG` regression notes so they can parse watchdog/guard/regression telemetry without altering the legacy `READ` stream.
```

## State of the Art
| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| The Supported Artisan Commands table only listed `READ` plus actuator commands, leaving REG/STATUS buried in code. | Add explicit `REG` and `STATUS/STAT` rows plus a sentence pointing to the instrumentation guide. | 2026-02-23 | Automation engineers can discover the hooks from the README and understand the CSV format. |
| README did not mention `internalDoc/INSTRUMENTATION_README.MD`. | Mention the instrumentation guide immediately around the automation hooks so readers know where to look for column definitions. | 2026-02-23 | Keeps README and instrumentation doc in sync, reduces duplicate specs. |

## Open Questions
1. **Placement of the instrumentation link:** Should the new sentence live inside the Supported Artisan Commands section, immediately after the table, or inside the Artisan Connection section that already references Artisan integration? _Recommendation:_ keep it adjacent to the table so automation hook readers do not need to scroll to another section.

## Sources

### Primary (HIGH confidence)
- `README.md` (current Supported Artisan Commands table; lacks REG/STATUS, contains table format to follow). Lines 44-59 show the existing commands. |
- `internalDoc/INSTRUMENTATION_README.MD` (defines the STATUS CSV columns, guidance that automation should poll STATUS, and context for automation). Lines 17-37 provide column definitions and reasoning.
- `src/input/parser.rs` (handles STATUS/STAT aliases and REG command). Lines 67-107 define allowed commands. |
- `src/output/artisan.rs` (format_status_response returns the nine values in the exact order automation must parse). Lines 138-165 show the string and constant order. |
- `src/safety/regression.rs` (walks through what `REG` does, the `SAFETY OT-REGRESSION` output, and the regression watchdog feed). Lines 40-85 confirm the regression behavior. |

### Secondary (MEDIUM confidence)
- `.planning/phases/68-readme-command-table/68-01-PLAN.md` (execution plan that mirrors the current phase; ensures the README change pattern is already defined). |

### Tertiary (LOW confidence)
- None.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH – Markdown + existing docs are the only tools required and their usage is well known.
- Architecture: HIGH – README structure, instrumentation link, and table expansion rely on explicit repo artefacts and code.
- Pitfalls: MEDIUM – placement preferences (table vs. connection section) are partly subjective. |

**Research date:** 2026-02-23
**Valid until:** 2026-03-25
