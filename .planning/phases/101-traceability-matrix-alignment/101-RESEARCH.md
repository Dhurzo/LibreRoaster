# Phase 101: Traceability Matrix Alignment - Research

**Researched:** 2026-03-20
**Domain:** Parser alignment with runtime TRACE events
**Confidence:** HIGH

## Summary

Phase 101 has a **clear alignment problem** between the runtime TRACE events emitted by the codebase and what the traceability_matrix.py parser expects. The parser was written based on the planned event names from Phase 97 research, but the actual implementation in src/logging/traceability.rs uses different step names. This misalignment prevents SOLID-03 from consuming live logs and breaks the TRACE flow restorability. The fix is straightforward: update the parser to match the actual runtime event names, add regression tests, and document the corrected flow.

## Root Cause Analysis

**Misalignment discovered:**

| Event Type | Parser Expects | Runtime Emits | Status |
|------------|----------------|---------------|---------|
| Enqueue | `command_enqueue` | `queue_enqueue` | ❌ Mismatch |
| Dequeue | `queue_dequeue` | `queue_dequeue` | ✓ Match |
| Fallback | (none) | `queue_fallback` | ❌ Missing |
| Actuation | `actuator_output` | `actuation` | ❌ Mismatch |
| Telemetry | `telemetry_emit` | `telemetry` | ❌ Mismatch |
| Guard | `guard_report` | `guard` | ❌ Mismatch |

**Sample log inconsistency:**
The file `logs/traceability/sample-trace.log` contains synthetic data using the parser's expected format (command_enqueue, etc.), which does NOT match what the actual firmware emits. This sample log needs to be regenerated with real TRACE events.

## Standard Stack

| Library | Version | Role |
|---------|---------|------|
| `python` | 3.13+ | Host-side trace parser script (already in use) |
| `unittest` | builtin | Regression test framework for parser validation |
| `pytest` | 1.4+ | Optional - alternative test runner (check if project uses) |

## Architecture Approach

### Parser Changes Required

**In scripts/traceability_matrix.py:**

1. **Update step name mapping in `_update_summary()`:**
   ```python
   # Current (WRONG):
   if step == "command_enqueue":
       summary.command = data.get("command", summary.command)
   # Change to (CORRECT):
   if step == "queue_enqueue":
       summary.command = data.get("cmd", summary.command)  # Note: "cmd" not "command"
   ```

2. **Update all step name comparisons:**
   - `queue_dequeue` → `queue_dequeue` (already correct)
   - `actuator_output` → `actuation`
   - `telemetry_emit` → `telemetry`
   - `guard_report` → `guard`

3. **Add support for `queue_fallback`:**
   - Handle cases where commands bypass the queue (queue full, overflow, etc.)
   - Update TraceSummary to track fallback events separately or merge with enqueue

4. **Update field name mapping:**
   - Runtime uses `cmd=<command>` (Debug format)
   - Parser expects `command=<command>`
   - Need to handle both or parse Debug-formatted command names

### Regression Test Design

**Test cases needed:**

1. **Happy path:** Complete trace with all steps (queue_enqueue → queue_dequeue → actuation → telemetry → guard)
2. **Fallback path:** queue_fallback event (when queue is full)
3. **Partial trace:** Missing actuation or telemetry (command rejected early)
4. **Mixed logs:** TRACE, STATUS, DEBUG lines interleaved
5. **Real-world sample:** Parse logs/traceability/sample-trace.log (regenerate with real events)

**Test structure:**
```python
class TestTraceabilityMatrix(unittest.TestCase):
    def test_parse_queue_enqueue(self): ...
    def test_parse_actuation(self): ...
    def test_parse_queue_fallback(self): ...
    def test_complete_trace_flow(self): ...
    def test_mixed_log_lines(self): ...
```

### Sample Log Regeneration

**Current sample-trace.log is synthetic and incorrect.** Must regenerate:

1. Capture real TRACE output from firmware during a test run
2. Use `python scripts/traceability_matrix.py <new_sample.log>` to verify it parses
3. Replace logs/traceability/sample-trace.log with real data

**Regeneration approach:**
- Run firmware with TRACE instrumentation enabled (already implemented in Phase 97)
- Execute a sequence of Artisan commands (STATUS, SET_TEMP, GUARD_CHECK, etc.)
- Capture output to file
- Verify parser produces meaningful matrix

## Documentation Updates

**Files to update:**

1. **INSTRUMENTATION_README.MD:**
   - Update TRACE stream documentation with actual event names
   - Clarify field names (cmd=, depth=, fallback=, etc.)
   - Add example TRACE entries with real format
   - Update regression triage workflow to reference corrected parser

2. **scripts/traceability_matrix.py (docstring):**
   - Update docstring to reflect actual event names
   - Update epilog/columns description if needed

3. **Phase 101 VERIFICATION.md (to be created):**
   - Document the alignment fix
   - Include before/after parser output
   - Show regression test results
   - Confirm SOLID-03 is now consumable

## Pitfalls / Risks

1. **Debug format parsing:** Runtime emits `cmd={:?}` which is Rust Debug format (e.g., `ArtisanCommand::STATUS`). Parser needs to handle this or we need to change runtime to emit simpler format.
2. **Backward compatibility:** No old trace logs exist (Phase 97 just shipped), so no backward compatibility needed. Safe to change parser.
3. **Queue_fallback handling:** Need to decide whether fallback events should appear in matrix or be treated as "command not processed". Decision: Include but mark as "fallback" status.
4. **Sample log regeneration:** Requires hardware or simulator access. If not available, create synthetic sample with correct format matching runtime behavior.

## Evidence / Sources

- Parser script: scripts/traceability_matrix.py (156 lines)
- TRACE implementation: src/logging/traceability.rs (TraceStep enum, format_trace_event functions)
- Sample log: logs/traceability/sample-trace.log (synthetic, incorrect format)
- Phase 97 research: .planning/phases/97-traceability-matrix-tooling/97-RESEARCH.md
- Phase 97 plans: .planning/phases/97-traceability-matrix-tooling/*-PLAN.md
- INSTRUMENTATION_README.MD: internalDoc/INSTRUMENTATION_README.MD

## Open Questions

1. **Command format:** Should we change runtime to emit `command=STATUS` instead of `cmd=ArtisanCommand::STATUS`? Or update parser to handle Debug format?
   - Recommendation: Update parser to handle Debug format for now (minimal change). Consider runtime improvement in future if Debug format is too verbose.
2. **Queue_fallback matrix placement:** Should fallback events appear as separate rows or be merged with enqueue?
   - Recommendation: Merge with enqueue but add a "fallback=true" flag in QueueDepth column.
3. **Sample log regeneration:** Can we regenerate with real hardware, or should we create a corrected synthetic sample?
   - Recommendation: Create corrected synthetic sample for now, document that real-world samples should be captured during HIL validation (Phase 98).

## Execution Notes

- Parser changes are localized to scripts/traceability_matrix.py
- No firmware changes needed (Phase 97 already implemented correct TRACE events)
- Regression tests can run without hardware (synthetic test data)
- Documentation updates are descriptive, not prescriptive
- SOLID-03 sign-off depends on this fix (parser must work with live logs)

**Research date:** 2026-03-20
**Ready for planning:** yes
