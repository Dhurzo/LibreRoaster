# Phase 43: UART Logging Redirect - Context

**Gathered:** 2026-02-08
**Status:** Ready for planning

## Phase Boundary

Redirect all logging to UART0 while keeping USB Serial dedicated to Artisan commands only. Clean separation between debug output and protocol communication.

## Implementation Decisions

### Log Levels
- All log levels go to UART0: DEBUG, INFO, WARN, ERROR
- No filtering — complete logging for debugging

### UART Log Format
- Timestamps included: `[HH:MM:SS] LEVEL: message`
- Example: `[14:32:15] INFO: Roaster initialized`

### USB Serial Behavior
- Artisan commands only
- No startup banner
- No error messages
- No output whatsoever except Artisan protocol responses

### Claude's Discretion
- Exact timestamp implementation (RTC-based, boot time)
- UART baud rate configuration (115200 matching Artisan)
- Logging macro implementation details

## Specific Ideas

- USB Serial must remain clean for Artisan software compatibility
- UART logging for development/debugging purposes

## Deferred Ideas

- None — discussion stayed within phase scope

---

*Phase: 43-uart-logging-redirect*
*Context gathered: 2026-02-08*
