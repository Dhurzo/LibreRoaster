# Phase 29: UART Logging Guide - Context

**Gathered:** 2026-02-05
**Status:** Ready for planning

<domain>
## Phase Boundary

Create user-facing documentation explaining v1.7 UART logging features for debugging. The guide helps power users understand log output format, interpret different log levels, and recognize patterns that indicate problems. Scope is limited to documenting existing features — adding new logging capabilities belongs in future phases.

</domain>

<decisions>
## Implementation Decisions

### Audience profile
- Target: Power users who can connect to serial console, read timestamps, and interpret values
- Goal: Help these users interpret what the logs mean, not how to set up serial connections
- Tone: Technical but accessible — assume comfort with reading output, not with coding or firmware

### Guide structure
- Progressive complexity: Start simple, add depth as reader advances
- Four sections in order: Overview → Format → Levels → Troubleshooting
- Each section builds on the previous

### Section content
- **Overview**: Purpose and value — explain what UART logging is and why it's useful for debugging Artisan communication issues
- **Format**: Annotated examples — provide sample log output with annotations showing each element (timestamp, channel prefix, message)
- **Levels**: Level meanings — explain what each log level means and when users see them (INFO, DEBUG, WARN, ERROR)
- **Troubleshooting**: Problem patterns — describe common log patterns that indicate problems and what they mean

### Log format coverage
- Complete format syntax: [HH:MM:SS.mmm] [CHANNEL] message
- Include annotations explaining each component
- Channel prefixes explained: [USB], [UART], [SYSTEM] differences

### Claude's Discretion
- Exact formatting of annotated examples (visual style, spacing)
- Whether to include setup prerequisites section
- Terminology choices for technical terms
- Any diagrams or visual aids if needed

</decisions>

<specifics>
## Specific Ideas

- Guide focuses on interpretation, not configuration — power users already connect to serial
- Annotated log samples with callouts showing format elements
- Progressive from basic format understanding to recognizing abnormal patterns

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 29-uart-logging-guide*
*Context gathered: 2026-02-05*
