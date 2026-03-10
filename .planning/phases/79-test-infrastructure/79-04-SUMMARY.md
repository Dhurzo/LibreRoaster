---
phase: 79-test-infrastructure
plan: 04
subsystem: testing
tags: [rust, uart, heapless, testing]

# Dependency graph
requires:
  - phase: 79-03
    provides: Core/alloc stub helpers exported via `libreroaster::common` so host suites share the same shim.
provides:
  - `MockUartDriver` now drains RX data, honors EOF, and resets streaming state so host tests stop seeing phantom bytes.
  - Streaming expectations include the trailing `0` and the multi-command test splits newline-delimited commands before parsing so the parser never feeds more than one command into a heapless `Vec`.
affects:
  - Phase 79-05 (roaster_sync guard) can rely on a stable host mock UART suite before addressing the async mutex bursts.

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Host mock UART semantics now mimic the emulator by draining the RX buffer after a read and resetting `has_data()`/`remaining_data_len()` based on EOF.
    - Streaming/multi-command suites now split newline-delimited chunks and cap the TX buffer to 256 bytes so heapless-backed helpers remain within their allocation.

key-files:
  created: []
  modified: [tests/mock_uart.rs]

key-decisions:
  - "Cap the mock TX buffer to 256 bytes and drop the oldest bytes when the limit is exceeded so heapless-backed transmit helpers never overflow while still exposing the latest output for assertions."
  - "Split the combined multi-command read chunk by `\r\n` and parse each Artisan command individually so `heapless::Vec` in the parser never sees more than four tokens."

patterns-established:
  - "MockUartDriver drains and clears the RX buffer once `read_bytes` hits EOF, keeping `has_data()` false and `remaining_data_len()` zero until new data arrives."
  - "Tests now expect the streaming chunk `AD\r\nOT1 50`, use a larger buffer to capture the trailing `0`, and verify no data remains afterwards."

# Metrics
duration: 4 min
completed: 2026-02-28
---

# Phase 79 Plan 04: Test Infrastructure Summary

**Mock UART buffering now mirrors the emulator: reads drain the RX stream, buffered responses respect the heapless limit, and streaming/multi-command suites capture every byte so `cargo test --test mock_uart` succeeds.**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-28T19:42:51Z
- **Completed:** 2026-02-28T19:47:33Z
- **Tasks:** 3
- **Files modified:** 1

## Accomplishments

- Drained `MockUartDriver`’s RX buffer after each read, set EOF so `has_data()`/`remaining_data_len()` flip to zero, and reset the streaming state when new input arrives so host suites stop seeing phantom bytes.
- Streamed data now uses a 10-byte buffer to capture `AD\r\nOT1 50`, and tests assert the buffer is empty afterward so the trailing `0` is no longer dropped.
- Capped the mock TX buffer to 256 bytes, split the multi-command chunk into newline-delimited commands before parsing, and confirmed `cargo test --test mock_uart` reports every mock test as `ok` with no heapless overflows.

## Task Commits

1. **Task 1: Drain rx buffer and mark EOF once commands are consumed** - `9b7d0db` (`fix(79-04): drain mock uart rx buffer`)
2. **Task 2: Ensure streaming/multiple command tests match final bytes and keep TX buffers bounded** - `8451d69` (`fix(79-04): align mock uart streaming expectations`)
3. **Task 3: Prove the host mock UART suite completes** - no code change (cargo test --test mock_uart)

**Plan metadata:** `docs(79-04): complete host mock UART plan`

## Files Created/Modified

- `tests/mock_uart.rs` — drained the RX buffer at EOF, capped the TX buffer to mimic heapless limits, and tightened the streaming and multi-command suites so every byte (including the trailing `0`) is verified.

## Decisions Made

- Cap the mock TX buffer at 256 bytes so heapless-backed transmit helpers never overflow while still exposing the latest response for assertions.
- Split multi-command reads on `\r\n` before calling `parse_artisan_command` so the parser’s heapless `Vec` only sees a single command’s tokens at a time.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None. The failing mock UART tests described in the plan now pass.

## User Setup Required

None — no external services were introduced.

## Next Phase Readiness

- Host mock UART buffering now mirrors the emulator, so the test infrastructure is stable enough for Phase 79-05 (guarding `roaster_sync`) to proceed.
- `tests/mock_uart.rs` is deterministic on x86_64, so downstream suites can rely on the emulator-friendly semantics while wider integration work continues.

---
*Phase: 79-test-infrastructure*
*Completed: 2026-02-28*
