# LibreRoaster Integration Tests

This document describes the integration test suite — what each test covers,
how to run it, and whether it needs real hardware.

## Quick reference

| Layer | Language | Needs hardware? | Run command |
|-------|----------|----------------|-------------|
| Unit / module | Rust | No | `cargo test --lib` |
| Host integration | Rust | No | `cargo test --test <name> --features test` |
| HIL (Hardware-in-the-Loop) | Python 3 | Yes — ESP32-C3 with LibreRoaster firmware | `python tests/hardware/<script>.py` |

---

## 1. Host-side Rust integration tests

These run on x86_64 (your development machine). They validate formatting,
parsing, and state-machine logic using mocks and stubs — no ESP32-C3 required.

```bash
# Run all host integration tests
cargo test --features test --target x86_64-unknown-linux-gnu

# Run a specific test file
cargo test --test artisan_roast_simulation --features test
cargo test --test read_command_usb_test
```

### `tests/read_command_usb_test.rs` — 15 tests

Validates TC4-standard READ response formatting:

- **TC4 5-value format** (`AMB,ET,BT,0.0,0.0`) — PID off
- **TC4 8-value format** (`AMB,ET,BT,0.0,0.0,heater,fan,SV`) — PID on
- **Celsius / Fahrenheit** — temperature conversion, heater/fan % must NOT be converted
- **Invalid values** — NaN, Infinity, negative infinity → normalized to `0.0`
- **Sequential & mixed commands** — READ interleaved with OT1, IO3
- **USBMock scenarios** — partial byte accumulation, no-CR hold, consistency

### `tests/artisan_roast_simulation.rs` — 8 tests

Simulates a complete Artisan-driven roast in-process with stub hardware:

- Handshake parsing (CHAN, UNITS, FILT)
- Preheat command + temperature ramp (25 → 180°C)
- Profile and fan-profile loading
- Full lifecycle: Idle → START → OT1 → curve follow (150 → 220°C) → STOP → cooldown
- READ polling consistency during active roast
- STOP → heater=0, emergency fan
- Units switching mid-roast (C↔F)
- READ validity in every state (Idle, Preheating, Heating, Stable, EmergencyStop)

### Other notable host integration tests

| Test file | What it covers |
|-----------|----------------|
| `artisan_integration_test.rs` | Parser + formatter end-to-end flow |
| `roast_scenarios/` | Heating, roasting, and cooling phase state transitions |
| `roast_resilience_tests.rs` | Edge cases — charge detection, double START, preheat values |
| `mock_uart.rs` | UART mock (44 tests for command→response simulation) |
| `mock_usb_driver.rs` | USB CDC mock (12 tests for buffer management, errors) |
| `command_multiplexer_concurrency.rs` | Concurrent USB + UART command routing |
| `usb_cdc_tests.rs` | USB CDC command processing (requires `target_arch = "riscv32"`) |

---

## 2. Hardware-in-the-Loop (HIL) tests — Python

These scripts connect to an ESP32-C3 running LibreRoaster firmware over
USB CDC (native USB) or UART (GPIO20/21) at **115200 baud**.

### Prerequisites

```bash
pip install pyserial
```

Connect the ESP32-C3, ensure the firmware is flashed and running:

```bash
cargo build --release --target riscv32imc-unknown-none-elf --features embedded
cargo espflash flash --release --monitor
```

Find the port:

```bash
python tests/hardware/read_command_hil.py --list-ports
```

### `tests/hardware/read_command_hil.py` — 5 test phases

Validates the TC4-standard `READ` response against real hardware.

```bash
# Auto-detect port
python tests/hardware/read_command_hil.py

# Specify port
python tests/hardware/read_command_hil.py --port /dev/ttyACM0

# Validate test logic without hardware
python tests/hardware/read_command_hil.py --dry-run
```

| Phase | What it does | What it validates |
|-------|-------------|-------------------|
| Basic READ | Sends `READ`, parses response | TC4 5-field format, temperature ranges (0–350°C), CHAN3/CHAN4 = 0.0 |
| Multiple READs | Polls 5× at 0.5s intervals | Response consistency, temperature stability (±5°C) |
| Units C→F→C | `UNITS;F`, READ, `UNITS;C`, READ | °F > °C conversion, approximate °C×9/5+32 accuracy |
| Field order | READ and inspect CSV fields | AMB first (index 0), CHAN3/CHAN4 last |
| Terminators | Read raw bytes | No embedded CR/LF in response |

**On first run** the script checks if the firmware is alive by sending a READ
and waiting up to 4 seconds for a response. If none arrives it prints
diagnostics and exits — it will not hang.

### `tests/hardware/artisan_roast_hil.py` — 7 test phases

Simulates the complete Artisan+ serial protocol against real hardware.

```bash
# Auto-detect port
python tests/hardware/artisan_roast_hil.py

# Dry-run (validate logic only)
python tests/hardware/artisan_roast_hil.py --dry-run
```

| Phase | Commands sent | What it validates |
|-------|--------------|-------------------|
| 1. Handshake | `CHAN;1200`, `UNITS;C`, `FILT;70` | ACK `#1200`, `OK`, `OK` |
| 2. READ polling | 5× `READ` at 0.5s intervals | Valid TC4 response, temperature stability |
| 3. Units switch | `UNITS;F` → READ → `UNITS;C` | °C→°F conversion accuracy |
| 4. Manual control | `OT1 0`, `IO3 0`, `READ` | Commands accepted, READ still valid |
| 5. Profile | `PROFILE;...`, `FANPROFILE;...` | Parsed without error |
| 6. STATUS | `STATUS` | 19 CSV fields, parseable temperatures |
| 7. Emergency STOP | `STOP`, `READ` | Heater=0, response still valid |

**Safety**: The script sends `OT1 0` and `STOP` during cleanup. Do not leave
the roaster unattended during the test.

### Existing HIL infrastructure

The `tests/hardware/` directory also contains the scenario-manifest-driven
validation framework documented in `HIL-PLAYBOOK.md`:

| Tool | Purpose |
|------|---------|
| `validation_runner.py` | Stream STATUS telemetry to CSV for golden-output capture |
| `analysis.py` | Compare telemetry against thresholds, produce markdown reports |
| `scenario_manifest.json` | Declarative scenario definitions (watchdog, guard, comms) |
| `hardware_test_runner.py` | Build → flash → capture → parse → report pipeline |

---

## 3. Test flow diagrams

### READ command (host vs HIL)

```
┌──────────────────────────┐     ┌──────────────────────────────┐
│  Rust host test          │     │  Python HIL test             │
│                          │     │                              │
│  SystemStatus struct ──► │     │  serial port ──► READ\r ──► │
│  ArtisanFormatter::      │     │  ESP32-C3 (firmware)         │
│  format_read_response    │     │        │                     │
│       │                  │     │  ◄─── AMB,ET,BT,0.0,0.0     │
│  ◄── TC4 CSV string      │     │        │                     │
│       │                  │     │  validate:                   │
│  validate:               │     │  • 5 fields                  │
│  • 5 fields              │     │  • ranges 0-350°C            │
│  • °F conversion         │     │  • °F conversion             │
│  • NaN→0 normalization   │     │  • field order               │
│  • PID on/off formats    │     │  • no CR/LF                  │
└──────────────────────────┘     └──────────────────────────────┘
```

### Artisan roast lifecycle (HIL)

```
Time  Script                          ESP32-C3 (firmware)
────  ──────────────────────────────  ─────────────────────────
 0s   open serial port ─────────────► USB CDC init
 1s   CHAN;1200 ────────────────────► send ack "#1200"
 1s   UNITS;C ──────────────────────► send "OK"
 1s   FILT;70 ──────────────────────► send "OK"
 2s   READ ─────────────────────────► AMB,ET,BT,0.0,0.0
 2s   OT1 0 / IO3 0 ───────────────► (set outputs, no response)
 3s   READ ─────────────────────────► (verify still valid)
 3s   STATUS ───────────────────────► 20-field CSV
 4s   STOP ─────────────────────────► heater=0, fan=100%
 4s   READ ─────────────────────────► (verify heater=0 in response)
 5s   close port
```

---

## 4. Adding a new test

### Rust host test

1. Create `tests/my_test.rs` with `#![cfg(all(test, not(target_arch = "riscv32")))]`
2. Use mocks from `tests/mock_usb_driver.rs`, `tests/mock_uart.rs`, or `libreroaster::common::{StubHeater, StubFan}`
3. Run: `cargo test --test my_test --features test`

### Python HIL test

1. Create `tests/hardware/my_hil_test.py`
2. Use `serial` for port communication, follow the patterns in `hardware_test_helpers.py`
3. Always include a `verify_firmware_alive()` check on connect
4. Always clean up (`STOP`, `UNITS;C`) in a `finally` block
5. Support `--dry-run` and `--list-ports`
6. Run: `python tests/hardware/my_hil_test.py`
