# Phase 30: Troubleshooting & Quick Start - Context

**Gathered:** 2026-02-05
**Status:** Ready for planning

<domain>
## Phase Boundary

Create a troubleshooting guide covering common connection and Artisan issues, plus a one-page quick start reference card. LED status indicators are out of scope.

</domain>

<decisions>
## Implementation Decisions

### Issue Organization
- Categorized by connection type: USB → UART → Artisan
- Not by symptom or root cause

### USB Connection Coverage
- USB CDC issues (device not detected, driver issues)
- Port/baud issues (COM port conflicts, baud rate mismatch)

### UART Connection Coverage
- Resource conflicts only (serial monitor conflicts, port in use)
- Wiring and settings issues deferred

### Artisan-Specific Coverage
- Connection drops (device not showing, stale readings)
- Event sync problems (roast events, button config)
- Config mismatch (baud rate, port config, extra characters)

### Quick Start Card Format
- Linear step-by-step format with icons
- Covers full workflow: Flash → Connect → Configure → Start roast

### Out of Scope
- LED status indicators documentation (not applicable for this hardware)

</decisions>

<specifics>
## Specific Ideas

- "You decide" for detailed structure within these decisions
- Reference existing guides (FLASH_GUIDE.md, ARTISAN_CONNECTION.md) for consistency

</specifics>

<deferred>
## Deferred Ideas

- UART wiring/pin issues — future documentation update
- UART settings issues — future documentation update

</deferred>

---

*Phase: 30-troubleshooting-quick-start*
*Context gathered: 2026-02-05*
