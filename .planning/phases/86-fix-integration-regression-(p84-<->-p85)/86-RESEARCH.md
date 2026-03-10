# Phase 86: Fix Integration Regression (P84 <-> P85) - Research

**Researched:** 2026-03-08
**Domain:** Integration Testing / Telemetry Expansion
**Confidence:** HIGH

## Summary

Phase 85 expanded the `STATUS` command telemetry from 16 to 18 columns by adding `command_latency_us` and `max_command_latency_us` fields to `SystemStatus`. While the production code and some hardware validation scripts were updated, the Rust integration tests (`tests/regression_status.rs` and `tests/fault_injection_scenarios.rs`) were left in a broken state. They currently fail to compile due to missing fields in `SystemStatus` initializers and would fail assertions due to hardcoded 16-column expectations.

**Primary recommendation:** Update the broken integration tests to initialize the new `SystemStatus` fields and adjust assertions to expect the 18-column telemetry layout.

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust | 1.88+ | Primary language | System implementation |
| Artisan Protocol | V1.0 | Serial communication | Industry standard for roasters |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| python3 | 3.10+ | Validation analysis | Hardware acceptance reporting |
| bash | 5.0+ | Quality orchestration | CI/CD and local verification |

## Architecture Patterns

### Telemetry Layout (18 Fields)
The `ArtisanFormatter::format_status_response` follows this exact CSV order:
1.  `et` (Environment Temp)
2.  `bt` (Bean Temp)
3.  `heater` (SSR Output %)
4.  `fan` (Fan Output %)
5.  `watchdog_flag` (1=OK, 0=Fail)
6.  `failure_count` (Consecutive WD fails)
7.  `failure_reason` (String or "none")
8.  `guard_timeouts` (Cumulative counter)
9.  `regression_flag` (1=Active, 0=Idle)
10. `pv` (Process Value)
11. `mv` (Manipulated Value)
12. `integrator_value` (PID Integral)
13. `derivative_value` (PID Derivative)
14. `saturation_flag` (1=Saturated)
15. `integrator_clamp_flag` (1=Clamped)
16. `derivative_available_flag` (1=Available)
17. `command_latency_us` (New: Last cmd latency)
18. `max_command_latency_us` (New: Peak cmd latency)

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Latency Measurement | Manual timers in tests | `command_latency_us` | Use the real instrumentation fields |
| CSV Parsing | Manual splitting in Python | `csv` module | Handles edge cases and headers better |

## Common Pitfalls

### Pitfall 1: Compilation Errors in Integration Tests
**What goes wrong:** Integration tests use struct literal syntax for `SystemStatus`, which breaks when new fields are added.
**How to avoid:** Use `..SystemStatus::default()` or `..create_test_status()` where possible, but for specific test fixtures, ensure all 18 fields are initialized.

### Pitfall 2: Feature Gate Mismatch
**What goes wrong:** `tests/regression_status.rs` and `tests/fault_injection_scenarios.rs` are gated behind `#[cfg(feature = "regression")]`.
**How to avoid:** Always run tests with `--features regression` to verify these files.

## Code Examples

### Correcting SystemStatus Initializer (Rust)
```rust
// In tests/regression_status.rs or tests/fault_injection_scenarios.rs
SystemStatus {
    // ... existing fields ...
    command_latency_us: 0,
    max_command_latency_us: 0,
    ..SystemStatus::default() // Use default for remaining fields if appropriate
}
```

### Updating Assertions (Rust)
```rust
// Update column count checks
let parts: Vec<&str> = formatted.split(',').collect();
assert_eq!(parts.len(), 18, "STATUS must have exactly 18 columns");

// Update hardcoded expectation strings
let expected = "25.0,150.0,0.0,0.0,1,0,none,0,1,150.0,75.0,12.0,0.24,1,1,1,0,0";
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| 16-column STATUS | 18-column STATUS | Phase 85 | Adds HIL/Latency observability |
| Manual string checks | `csv.DictReader` | Phase 85 | More robust Python analysis |

## Open Questions

1. **Should tests simulate non-zero latency?**
   - Recommendation: For regression tests, using `0` for latency fields is sufficient to verify the column order and count. Fault-injection tests could potentially use non-zero values if they want to verify threshold-based logic in `analysis.py`, but it's not strictly required for Phase 86's goal of "restoring broken tests".

## Sources

### Primary (HIGH confidence)
- `src/output/artisan.rs` - Implementation of 18-column formatter.
- `src/config/constants.rs` - `SystemStatus` struct definition.
- `tests/hardware/validation_runner.py` - Reference for 18-column CSV mapping.
- `Cargo.toml` - Feature definitions.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Core project technology.
- Architecture: HIGH - Telemetry layout is locked in `artisan.rs`.
- Pitfalls: HIGH - Compilation and count errors verified via `cargo test`.

**Research date:** 2026-03-08
**Valid until:** 2026-04-08
