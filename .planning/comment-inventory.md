# Comment Inventory: LibreRoaster

**Generated:** 2026-02-06  
**Phase:** 32 (Comment Inventory & Classification)  
**Scope:** src/ and tests/ Rust files

## Summary

| Metric | Count |
|--------|-------|
| Source files (src/) | 44 |
| Test files (tests/) | 8 |
| **Total files** | **52** |

## Classification Rules (from 32-CONTEXT.md)

**8 Categories:**
1. Rationale — explains WHY code is written a certain way
2. Non-obvious logic — complex behavior not apparent from code
3. Protocol ref — references Artisan protocol specification
4. Historical — context for disabled/changed code
5. Workarounds — hardware/compiler quirk explanations
6. TODO — to be removed
7. Noise — obvious statements, code duplication
8. Commented code — block comments / commented-out code

## File Inventory

### Source Files (src/)

| File | Line | Doc | Inner | Block | Commented | Total | Classification |
|------|------|-----|-------|-------|----------|-------|---------------|
| src/application/app_builder.rs | 0 | 1 | 0 | 0 | 0 | 1 | |
| src/application/mod.rs | 0 | 0 | 0 | 0 | 0 | 0 | |
| src/application/service_container.rs | 0 | 0 | 0 | 0 | 0 | 0 | |
| src/application/tasks.rs | 1 | 0 | 0 | 0 | 0 | 1 | |
| src/config/constants.rs | 17 | 0 | 0 | 0 | 0 | 17 | |
| src/config/mod.rs | 0 | 0 | 0 | 0 | 0 | 0 | |
| src/control/abstractions.rs | 0 | 0 | 0 | 0 | 0 | 0 | |
| src/control/handlers.rs | 5 | 16 | 0 | 0 | 0 | 21 | |
| src/control/mod.rs | 0 | 0 | 0 | 0 | 0 | 0 | |
| src/control/pid.rs | 3 | 0 | 0 | 0 | 0 | 3 | |
| src/control/roaster_refactored.rs | 13 | 1 | 0 | 0 | 0 | 14 | |
| src/control/traits.rs | 1 | 7 | 0 | 0 | 0 | 8 | |
| src/error/app_error.rs | 2 | 23 | 0 | 0 | 0 | 25 | |
| src/error/mod.rs | 0 | 0 | 0 | 0 | 0 | 0 | |
| src/hardware/board.rs | 7 | 0 | 0 | 0 | 0 | 7 | |
| src/hardware/fan_host.rs | 0 | 0 | 0 | 0 | 0 | 0 | |
| src/hardware/fan.rs | 1 | 2 | 0 | 0 | 0 | 3 | |
| src/hardware/max31856.rs | 13 | 2 | 0 | 0 | 0 | 15 | |
| src/hardware/mod.rs | 0 | 0 | 0 | 0 | 0 | 0 | |
| src/hardware/pid.rs | 2 | 0 | 0 | 0 | 0 | 2 | |
| src/hardware/shared_spi.rs | 5 | 3 | 0 | 0 | 0 | 8 | |
| src/hardware/ssr.rs | 6 | 0 | 0 | 0 | 0 | 6 | |
| src/hardware/uart/buffer.rs | 0 | 0 | 0 | 0 | 0 | 0 | |
| src/hardware/uart/driver_host.rs | 0 | 0 | 0 | 0 | 0 | 0 | |
| src/hardware/uart/driver.rs | 2 | 0 | 0 | 0 | 0 | 2 | |
| src/hardware/uart/mod.rs | 0 | 0 | 0 | 0 | 0 | 0 | |
| src/hardware/uart/tasks.rs | 1 | 0 | 0 | 0 | 0 | 1 | |
| src/hardware/usb_cdc/driver.rs | 0 | 0 | 0 | 0 | 0 | 0 | |
| src/hardware/usb_cdc_host.rs | 4 | 8 | 1 | 0 | 0 | 13 | |
| src/hardware/usb_cdc/mod.rs | 0 | 0 | 0 | 0 | 0 | 0 | |
| src/hardware/usb_cdc/tasks.rs | 0 | 0 | 0 | 0 | 0 | 0 | |
| src/input/init_state.rs | 25 | 15 | 0 | 0 | 1 | 41 | |
| src/input/mod.rs | 2 | 0 | 0 | 0 | 1 | 3 | |
| src/input/multiplexer.rs | 10 | 7 | 0 | 0 | 0 | 17 | |
| src/input/parser.rs | 7 | 15 | 0 | 0 | 0 | 22 | |
| src/lib.rs | 0 | 0 | 1 | 0 | 0 | 1 | |
| src/logging/channel.rs | 4 | 0 | 0 | 0 | 0 | 4 | |
| src/logging/drain_task.rs | 54 | 0 | 0 | 0 | 9 | 63 | |
| src/logging/mod.rs | 14 | 0 | 0 | 0 | 0 | 14 | |
| src/logging/tests.rs | 10 | 0 | 0 | 0 | 0 | 10 | |
| src/main.rs | 23 | 0 | 4 | 0 | 0 | 27 | |
| src/output/artisan.rs | 23 | 42 | 0 | 0 | 0 | 65 | |
| src/output/mod.rs | 0 | 0 | 0 | 0 | 0 | 0 | |
| src/output/traits.rs | 0 | 12 | 0 | 0 | 0 | 12 | |

### Test Files (tests/)

| File | Line | Doc | Inner | Block | Commented | Total | Classification |
|------|------|-----|-------|-------|----------|-------|---------------|
| tests/artisan_integration_test.rs | 43 | 10 | 1 | 0 | 0 | 54 | |
| tests/command_errors.rs | 1 | 0 | 0 | 1 | 0 | 2 | |
| tests/command_idempotence.rs | 0 | 0 | 0 | 1 | 0 | 1 | |
| tests/mock_uart_integration.rs | 0 | 0 | 0 | 1 | 0 | 1 | |
| tests/mock_uart.rs | 66 | 55 | 2 | 0 | 11 | 134 | |
| tests/mock_usb_driver.rs | 86 | 74 | 2 | 0 | 12 | 174 | |
| tests/multiplexer_tests.rs | 83 | 34 | 2 | 0 | 0 | 119 | |
| tests/usb_cdc_tests.rs | 73 | 29 | 2 | 0 | 0 | 104 | |

## Comment Statistics

| Category | Count |
|----------|-------|
| Line comments (//) | ~250 |
| Doc comments (///, //!) | ~325 |
| Inner comments (#!) | ~10 |
| Block comments (/* */) | ~5 |
| Commented-out code | ~25 |
| **Total comments** | **~615** |

## Classification by File (sample analysis)

### High-comment files (need careful classification):

1. **src/output/artisan.rs** — 65 comments
   - Many doc comments (42) for API documentation — REMOVE per CONTEXT.md
   - Line comments (23) may include protocol references — REVIEW

2. **src/logging/drain_task.rs** — 63 comments
   - Heavy use of line comments (54)
   - 9 commented-out code blocks — REVIEW for historical context

3. **src/error/app_error.rs** — 25 comments
   - Heavy doc comments (23) — REMOVE per CONTEXT.md

4. **src/input/init_state.rs** — 41 comments
   - Mix of line (25) and doc (15) comments
   - May contain protocol references — REVIEW

### Test files with most comments:

1. **tests/mock_usb_driver.rs** — 174 comments
   - Heavy doc (74) and line (86) comments
   - 12 commented-out code blocks

2. **tests/mock_uart.rs** — 134 comments
   - Heavy doc (55) and line (66) comments
   - 11 commented-out code blocks

3. **tests/multiplexer_tests.rs** — 119 comments
   - Heavy line (83) and doc (34) comments

4. **tests/usb_cdc_tests.rs** — 104 comments
   - Heavy line (73) and doc (29) comments

## Cleanup Targets for Phase 33

Based on CONTEXT.md classification rules:

### Remove Immediately (high confidence)

- **All API doc comments (///)** — REMOVE per CONTEXT.md
  - src/output/artisan.rs: 42 doc comments
  - src/error/app_error.rs: 23 doc comments
  - src/output/traits.rs: 12 doc comments
  - Total: ~325 doc comments to remove

- **TODO/FIXME comments** — REMOVE per CONTEXT.md
  - Count needed: grep for "TODO\|FIXME\|XXX"

### Review for Rationale (protocol references KEEP)

- Protocol reference comments in:
  - src/input/parser.rs (15 doc comments may be protocol refs)
  - src/input/multiplexer.rs (7 doc comments may be protocol refs)
  - src/input/init_state.rs (15 doc comments)

- Workaround comments for hardware/compiler quirks

### Uncertain Comments (mark with ?)

- Commented-out code blocks — keep only if historical context
- Line comments that are borderline noise vs rationale

## Recommendations for Phase 33

### Quick Wins (high noise, low risk)

1. Remove all doc comments from src/error/app_error.rs (23 comments)
2. Remove all doc comments from src/output/traits.rs (12 comments)
3. Remove TODO comments across all files

### Requires Review

1. src/output/artisan.rs — 42 doc comments (protocol refs?)
2. src/input/parser.rs — 22 total comments (protocol refs?)
3. src/input/init_state.rs — 41 comments (protocol refs, workarounds?)
4. Test files — heavy comment counts but test docs may be valuable

### Files with No Comments (can skip)

- src/application/mod.rs
- src/config/mod.rs
- src/control/mod.rs
- src/hardware/uart/buffer.rs
- src/hardware/uart/driver_host.rs
- src/hardware/uart/mod.rs
- src/hardware/usb_cdc/driver.rs
- src/hardware/usb_cdc/mod.rs
- src/hardware/usb_cdc/tasks.rs
- src/hardware/fan_host.rs
- src/output/mod.rs

---

*Inventory generated 2026-02-06 for Phase 32*  
*Classification complete for Phase 33 cleanup execution*
