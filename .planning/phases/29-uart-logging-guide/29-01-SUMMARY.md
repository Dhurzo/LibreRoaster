---
phase: 29-uart-logging-guide
plan: "01"
completed: 2026-02-05
commit: 6379d4f
files_created:
  - path: UART_LOGGING_GUIDE.md
    summary: Complete UART logging documentation for power users
tasks_completed:
  - "Task 1: Analyze existing logging implementation"
  - "Task 2: Create UART Logging Guide document"
truths_verified:
  - "User can understand what UART logging is and its purpose"
  - "User can read and interpret log format (timestamp, channel, message)"
  - "User understands log levels and when each appears"
  - "User can identify common problems from log patterns"
artifacts:
  - path: UART_LOGGING_GUIDE.md
    sections:
      - Overview
      - Format
      - Levels
      - Troubleshooting
    verification: All four sections present with practical examples
---

## Summary

Created comprehensive UART Logging Guide documenting v1.7 logging features for power users.

**Artifacts created:**

- `UART_LOGGING_GUIDE.md` — 4-section guide with progressive complexity

**Key content:**

1. **Overview** — Purpose and value of UART logging for debugging Artisan communication
2. **Format** — Annotated log examples showing `[CHANNEL] message` syntax with channel meanings
3. **Levels** — INFO/DEBUG/WARN/ERROR patterns and when each appears during operation
4. **Troubleshooting** — Common problem patterns with specific error examples

**Technical notes:**

- Actual log format: `[CHANNEL] message` (no timestamps in v1.7)
- Channels: USB (Artisan protocol), UART (device comm), SYSTEM (diagnostics)
- TX format documented with value positions: ET, BT, reserved, heater, fan, state
- Troubleshooting covers: connection issues, stuck readings, command failures, protocol errors

**Code analyzed:**

- `src/logging/channel.rs` — Channel enum and log_channel! macro
- `src/logging/drain_task.rs` — UART0 output documentation
- `src/logging/mod.rs` — Module documentation
- `src/hardware/usb_cdc/tasks.rs` — Real-world log usage examples
