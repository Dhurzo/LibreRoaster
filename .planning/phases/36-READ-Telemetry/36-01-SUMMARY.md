# Phase 36-01: READ Telemetry - Summary

**Executed:** 2026-02-07
**Status:** Complete

## Changes Made

### 1. Added comment for BT2/ET2 support (src/output/artisan.rs)
- Added comment: "store value for future et2 and bt2 support"
- Placed above format_read_response_full function
- Documents placeholder values for disabled channels

### 2. Updated format_read_response_full with comment (src/output/artisan.rs)
- Existing function already outputs: `ET,BT,-1,-1,-1,FAN,HEATER\r\n`
- One-decimal format already correct (e.g., "75.0")
- Added documentation comment for future thermocouple support

### 3. Added error handling to ReadStatus handler (src/control/roaster_refactored.rs)
- Added call to `format_read_response_full()`
- Added validation: response must not be empty and must end with `\r\n`
- On malformed output: stops heater and panics

### 4. Added test for one-decimal format (src/output/artisan.rs)
- Added `test_format_read_response_full_one_decimal_format`
- Verifies FAN shows "75.0" and HEATER shows "100.0"

## Test Coverage

All READ-related tests passing:
- `test_format_read_response` - Basic 4-value format
- `test_format_read_response_seven_values` - Full 7-value format
- `test_unused_channels_return_negative_one` - BT2/ET2 return -1
- `test_response_terminates_with_crlf` - CRLF termination
- `test_format_read_response_full_uses_status_values` - Status values used
- `test_format_read_response_full_one_decimal_format` - One-decimal verification

## Verification

| Check | Status |
|-------|--------|
| cargo check --features std | ✓ Pass |
| Code compiles without errors | ✓ Pass |
| CSV format: ET,BT,-1,-1,-1,FAN,HEATER | ✓ Pass |
| One-decimal format (75.0) | ✓ Pass |
| Comment on variable for future support | ✓ Pass |
| Error handling: stop heater + panic | ✓ Pass |
| Tests pass | ✓ Pass |

## Commits

- `feat(36-01): add BT2/ET2 comment and error handling`
- `feat(36-01): add one-decimal format test`
