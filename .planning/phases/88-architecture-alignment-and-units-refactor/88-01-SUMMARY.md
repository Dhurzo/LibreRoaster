---
phase: 88-architecture-alignment-and-units-refactor
plan: 01
status: complete
wave: 1
commits:
  - hash: 8b4c46b
    message: "feat(88-01): promote stage_instrumentation to Tier 1 and wire UNITS through ManualCommandPolicy"
---

## Summary

**Phase:** 88 — Architecture Alignment and UNITS Refactor
**Plan:** 88-01 — Promote stage_instrumentation.rs to Tier 1 and wire UNITS through ManualCommandPolicy
**Status:** ✓ Complete

### What Was Built

1. **Tier 1 Promotion**: Moved `src/application/stage_instrumentation.rs` from Tier 2 to Tier 1 in baseline-policy.toml, making it a blocking critical module in the quality gate.

2. **SetUnits Command**: Added `RoasterCommand::SetUnits(bool)` variant to handle temperature scale preference (true = Fahrenheit, false = Celsius).

3. **ManualCommandPolicy Integration**: Implemented SetUnits handling in ArtisanCommandHandler's ManualCommandPolicy implementation:
   - Added `temp_settings: TemperatureSettings` field to ArtisanCommandHandler struct
   - Added SetUnits case to `evaluate()` method
   - Added SetUnits to `can_handle()` method

4. **Policy Wiring**: Modified `process_command()` in roaster_refactored.rs to convert `ArtisanCommand::Units` to `RoasterCommand::SetUnits` and route through `forward_artisan_manual_command()`, following the same pattern as other manual commands (SetHeater, SetFan, IncreaseHeater, DecreaseHeater).

### Files Modified

| File | Change |
|------|--------|
| `.planning/quality/baseline-policy.toml` | Added stage_instrumentation.rs to t1_critical.modules |
| `src/config/constants.rs` | Added `SetUnits(bool)` variant to RoasterCommand enum |
| `src/control/handlers.rs` | Added temp_settings field, implemented SetUnits in ManualCommandPolicy |
| `src/control/roaster_refactored.rs` | Wired UNITS through forward_artisan_manual_command |

### Verification

- ✅ Cargo check passes (only pre-existing warnings)
- ✅ All 115 tests pass
- ✅ stage_instrumentation.rs now in Tier 1 (blocking)
- ✅ SetUnits handled via ManualCommandPolicy pattern

### Notes

- The unused `temp_settings` field in RoasterControl generates a warning but is retained for potential future use or cleanup in a subsequent phase.
- The quality baseline shows some clippy warnings from existing code, but no new errors were introduced by this change.
