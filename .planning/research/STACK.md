# Stack Research: ARTISAN+ Protocol Testing

## Protocol Overview

ARTISAN+ is a serial communication protocol used by Artisan coffee roasting software to interface with roaster controllers.

## Key Components

### UART Configuration
- **Baud Rate**: 115200 (standard)
- **Pins**: TX=20, RX=21 (ESP32-C3)
- **Format**: 8N1 (8 data bits, no parity, 1 stop bit)

### Command Set

| Command | Format | Description |
|---------|--------|-------------|
| READ | `READ` | Request current status |
| START | `START` | Start roast sequence |
| STOP | `STOP` | Emergency stop |
| OT1 | `OT1 <0-100>` | Set heater power (percentage) |
| IO3 | `IO3 <0-100>` | Set fan speed (percentage) |

### Response Formats

**READ Response**: `ET,BT,Power,Fan`
- ET: Environmental temperature (°C)
- BT: Bean temperature (°C)
- Power: Heater output (0-100)
- Fan: Fan speed (0-100)

**Continuous Output**: `time,ET,BT,ROR,Gas`
- time: Seconds since start (format: X.XX)
- ET: Environmental temperature
- BT: Bean temperature
- ROR: Rate of rise (°C/s)
- Gas: Heater power (0-100)

## Test Scenarios

### Parser Tests
1. Valid commands parse correctly
2. Invalid commands return ParseError
3. Boundary values (0, 100) work
4. Out-of-range values (>100) rejected
5. Whitespace handling
6. Case sensitivity

### Formatter Tests
1. READ response format matches specification
2. Continuous output format matches specification
3. Temperature values formatted correctly
4. Percentage values formatted correctly

### Integration Tests
1. READ → response cycle
2. OT1 → heater control enabled
3. IO3 → fan control enabled
4. STOP → emergency stop response
5. START → roast state change

## Test Files Location

- `src/input/parser.rs` — Command parsing (has tests)
- `src/output/artisan.rs` — Response formatting (needs tests)
- `examples/artisan_test.rs` — Example file (has issues)

## Coverage Gaps

1. No tests for `MutableArtisanFormatter`
2. No integration tests for command → response flow
3. No tests for UART driver mocking
4. Example file uses incorrect API signatures

---

*Research date: 2026-02-04*
