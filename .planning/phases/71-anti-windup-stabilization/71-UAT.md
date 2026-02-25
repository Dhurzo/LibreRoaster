---
status: complete
phase: 71-anti-windup-stabilization
source:
  - 71-01-SUMMARY.md
  - 71-02-SUMMARY.md
  - 71-03-SUMMARY.md
started: 2026-02-24T12:00:00Z
updated: 2026-02-24T12:05:00Z
---

## Current Test

[testing complete]

## Tests

### 1. Integrator clamps when saturation hits
expected: Drive the heater toward saturation and watch the realtime telemetry/log output (e.g., Artisan STATUS) so saturation_active becomes true, integrator_clamped stays true, the reported mv equals the applied_output, and the integrator value stops growing until the guard clears.
result: pass

### 2. Derivative spikes match real PV motion
expected: Toggle setpoints without actual PV change and confirm SystemStatus/telemetry keeps derivative_rate steady (or zero) and derivative_available false, then move the PV and see derivative_rate spike while derivative_available becomes true.
result: pass

### 3. Anti-windup flags decorate stage logs and STATUS tail
expected: Observe the heartbeat stage logs and guard/watchdog lines for saturation_active, integrator_clamped, derivative_available flags plus the filtered PV/MV/integrator/derivative tail; the tail should remain 16 columns with the anti-windup bits in the same order as before.
result: pass

## Summary

total: 3
passed: 3
issues: 0
pending: 0
skipped: 0

## Gaps

[none yet]
