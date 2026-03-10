# Phase 85: Hardware Acceptance Thresholds and Real Roaster Validation - Research

**Researched:** 2026-03-08
**Domain:** Hardware-In-the-Loop (HIL) Validation, Artisan Roaster Scope, Control Latency, Thermal Envelopes
**Confidence:** HIGH

## Summary

Phase 85 represents the final "real-world" gate for the LibreRoaster v5.0 firmware. It moves beyond unit and integration tests into physical validation using the Artisan Roaster Scope software. The research focuses on how to define, measure, and verify the performance of the firmware when controlling a physical SSR (Solid State Relay) and Fan on an ESP32-C3.

Key findings:
- **Artisan+ Protocol**: The firmware implements a CSV-based protocol over UART/USB, where commands like `OT1` (Heater) and `IO3` (Fan) drive actuators, and `READ` returns telemetry (`ET,BT,Power,Fan`).
- **Latency Measurement**: Command-to-actuator latency must be measured at the firmware level (time from parsing to PWM update) to eliminate serial transport jitter from the assessment.
- **Thermal Response**: "Response envelope" refers to the system's ability to follow a target profile (RTD/Thermocouple feedback vs command) within a specific ±X°C window.
- **Evidence Collection**: Validation requires capturing serial transcripts from the firmware alongside Artisan's own `.alog` files for cross-comparison.

**Primary recommendation:** Use a Python-based validation script (HIL-Runner) that captures serial telemetry, compares timestamps against defined thresholds, and generates a Markdown signoff report with plots.

## Standard Stack

The established libraries/tools for hardware validation in this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `pyserial` | 3.5+ | Serial communication | Standard for Python-to-Hardware communication. |
| `pandas` | 2.0+ | Data analysis | Efficiently handles CSV logs and threshold comparisons. |
| `matplotlib` | 3.5+ | Visualization | Generates the required plots for "Validation Evidence". |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `jinja2` | 3.1+ | Reporting | Generating the final Markdown/HTML signoff report. |
| `scipy` | 1.10+ | Signal processing | Calculating RMSE or finding peaks in latency jitter. |

**Installation:**
```bash
pip install pyserial pandas matplotlib jinja2
```

## Architecture Patterns

### Recommended Project Structure
```
tests/hardware/
├── validation_runner.py    # Python script to capture and analyze data
├── thresholds.json         # Numeric acceptance criteria
├── profiles/               # Reference roast profiles (Artisan .atp)
└── reports/                # Generated validation evidence
```

### Pattern 1: Command-to-Actuator Latency (C-A-L)
**What:** Measuring the time delta between the arrival of a command (e.g., `OT1 50`) and the point where the hardware state (PWM duty cycle) is updated.
**When to use:** Crucial for safety and control stability.
**Example:**
```rust
// In firmware: src/application/tasks.rs
let start_time = esp_timer_get_time();
if let Ok(cmd) = parse_artisan_command(buffer) {
    apply_command(cmd);
    let end_time = esp_timer_get_time();
    log_info!("CMD_LATENCY", "type: {:?}, ms: {}", cmd, (end_time - start_time) / 1000);
}
```

### Anti-Patterns to Avoid
- **Host-Only Timing:** Measuring latency solely on the PC side. This includes serial buffer delay and OS scheduling, which masks actual firmware performance.
- **Manual Log Analysis:** Manually checking 10 minutes of CSV data is error-prone. Always use automated pass/fail scripts.

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| CSV Parsing | `string.split(',')` | `pandas.read_csv` | Handles missing fields, types, and large datasets reliably. |
| Chart Generation | Custom SVG drawer | `matplotlib` | Provides professional-grade axis labeling, legends, and export. |
| Serial Capture | Bare `sys.stdin` | `pyserial` | Handles baud rates, timeouts, and port discovery across OSs. |

## Common Pitfalls

### Pitfall 1: SSR Frequency Aliasing
**What goes wrong:** At 1Hz SSR frequency, a command change at T=0.1s might not physically switch the SSR until T=1.0s.
**Why it happens:** Low-frequency PWM used for zero-crossing SSRs.
**How to avoid:** Define "actuator latency" as the time until the **PWM duty cycle register** is updated, not the physical output flip.

### Pitfall 2: Telemetry Jitter
**What goes wrong:** Artisan polls every 1s, but firmware might send at 1.1s due to task scheduling.
**How to avoid:** Use the firmware's internal `uptime_ms` in the telemetry CSV so the analysis script uses the "ground truth" time.

## Code Examples

### Threshold Definition (JSON)
```json
{
  "latency": {
    "max_ms": 500,
    "avg_ms": 100,
    "p95_ms": 300
  },
  "thermal_envelope": {
    "bt_max_error_deg": 10.0,
    "et_max_error_deg": 15.0
  },
  "safety": {
    "max_consecutive_watchdog_fails": 0,
    "max_ledc_guard_timeouts_per_run": 5
  }
}
```

### Validation Analysis (Python/Pandas)
```python
import pandas as pd

# Load captured firmware log
df = pd.read_csv("validation_run_20260308.csv")

# 1. Latency Check
latency_violations = df[df['latency_ms'] > 500]
print(f"Latency Violations: {len(latency_violations)}")

# 2. Safety Counter Check
if df['watchdog_fails'].max() > 0:
    print("FAIL: Watchdog tripped during run")

# 3. Envelope Check (if Artisan target is known)
# df['error'] = abs(df['bt_actual'] - df['bt_target'])
# if df['error'].max() > 10.0:
#     print("FAIL: Thermal envelope breached")
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Visual check of Artisan curves | Automated HIL thresholding | v5.0 | Unambiguous pass/fail; required for hardware signoff. |
| UART echo testing | Multi-channel telemetry validation | v4.0+ | Validates both USB and UART paths simultaneously. |

## Open Questions

1. **How to feed Artisan Target to the Analysis?**
   - Artisan saves `.alog` (JSON). The validation script needs to parse this and align it with the firmware's CSV log based on the "Start" marker.
2. **Acceptance for "Response Envelope" without a Roaster?**
   - If a real roaster isn't attached during some tests, we might need a "Thermal Sim" mode or just acknowledge that BT/ET will stay at ambient.
   - *Recommendation:* HW-02 explicitly says "real roaster", so a physical machine is assumed.

## Sources

### Primary (HIGH confidence)
- `tests/hardware/SCENARIO_MATRIX.md` - Established safety scenarios and telemetry format.
- `src/config/constants.rs` - Known hardware limits (1Hz SSR, 25kHz Fan).
- `tests/artisan_integration_test.rs` - Verified Artisan+ protocol implementation details.

### Secondary (MEDIUM confidence)
- Industry standard for PID latency in thermal systems (<1s is typical for slow heaters).

### Tertiary (LOW confidence)
- Artisan Scope JSON format (`.alog`) structure (needs verification of current version).

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Python ecosystem is the default for HIL.
- Architecture: HIGH - Log-and-Compare is the standard HIL pattern.
- Pitfalls: HIGH - Common issues with low-freq PWM and serial jitter are well-documented.

**Research date:** 2026-03-08
**Valid until:** 2026-04-07 (30 days)
