# LibreRoaster — Pre-Flight Checklist

**Purpose:** Mandatory verification before first power-on with real hardware.
Complete every item in order. Do NOT skip steps.

**Last updated:** 2026-05-28

---

## Phase 0 — Build Verification (Computer)

Run these commands before touching hardware.

```bash
# 1. Embedded build must compile clean (zero warnings)
cargo build --release --target riscv32imc-unknown-none-elf --features embedded

# 2. Host test suite (expect 3 pre-existing ROR mock-time failures in artisan_integration_test)
cargo test --target x86_64-unknown-linux-gnu --features test

# 3. Format + clippy gate
cargo fmt --all -- --check && cargo clippy --locked --all-targets -- -W clippy::unwrap_used -W clippy::expect_used -W clippy::panic
```

- [ ] Embedded build: **zero errors, zero warnings**
- [ ] Host tests: same failure count as baseline (3 ROR mock-time)
- [ ] Clippy: clean

---

## Phase 1 — Visual Inspection (Unpowered)

With a multimeter in continuity/resistance mode, board **unpowered**.

### Power Rails

- [ ] 3.3V rail measures **3.3V ±5%** (3.15V – 3.45V) when powered
- [ ] NO 5V connection to any ESP32-C3 pin
- [ ] 100 µF electrolytic + 0.1 µF ceramic decoupling on 3.3V near ESP32-C3

### SPI Bus (MAX31856 Thermocouples)

Verify wiring matches the **firmware pinout** (not any other document):

| Signal | GPIO | Wire Color (note yours) |
|--------|------|------------------------|
| SCLK | **GPIO 6** | |
| MOSI (SDI) | **GPIO 7** | |
| MISO (SDO) | **GPIO 5** | |
| CS ET | **GPIO 3** | |
| CS BT | **GPIO 4** | |

- [ ] SCLK → GPIO 6 (not GPIO 7)
- [ ] MOSI → GPIO 7 (not GPIO 5)
- [ ] MISO → GPIO 5 (not GPIO 6)
- [ ] CS ET → GPIO 3
- [ ] CS BT → GPIO 4
- [ ] MAX31856 VCC = **3.3V** (NOT 5V) — measure with multimeter
- [ ] Type-K thermocouple polarity: yellow (+), red (−)
- [ ] Both CS lines have no external pull-up/pull-down (firmware drives them)

### Actuators

| Signal | GPIO | Frequency |
|--------|------|-----------|
| SSR (heater) | **GPIO 10** | 310 Hz |
| Fan (MOSFET) | **GPIO 9** | 25 kHz |
| Status LED | **GPIO 8** | — |

- [ ] SSR control line → GPIO 10 (not any other pin)
- [ ] Fan PWM line → GPIO 9
- [ ] Status LED → GPIO 8 (if applicable)

### Strapping Pin Protection

- [ ] **GPIO 9**: 10 kΩ pull-up to 3.3V installed
- [ ] **GPIO 9**: Fan MOSFET gate has pull-down (keeps fan OFF during boot)
- [ ] **GPIO 10**: 10 kΩ pull-down to GND installed (keeps SSR OFF during boot)
- [ ] **GPIO 8**: LED circuit does NOT pull LOW during boot (push-pull safe)
- [ ] **GPIO 2**: Nothing connected (strapping pin, avoided)

### Communication

- [ ] UART RX → GPIO 20 (from CH341 TX)
- [ ] UART TX → GPIO 21 (to CH341 RX)
- [ ] CH341 VCC = 3.3V (not 5V) — or level-shifted if 5V CH341
- [ ] USB cable connected to ESP32-C3 native USB port (not UART adapter)

---

## Phase 2 — First Power-On (Heater DISCONNECTED)

**⚠️ CRITICAL: Do NOT connect the heater element yet.**

1. Flash firmware:
   ```bash
   cargo espflash flash --release --target riscv32imc-unknown-none-elf \
     --features embedded --monitor
   ```

2. Verify boot sequence in serial monitor:

- [ ] See `LibreRoaster v5.1 starting...`
- [ ] See `Hardware initialized`
- [ ] See `Sensors initialized (BT: GPIO4, ET: GPIO3)` (non-simulated build)
- [ ] See `Hardware watchdog initialized (RTC WDT)`
- [ ] See `USB CDC initialized`
- [ ] See `Wake the f*** up samurai we have beans to burn!`
- [ ] NO panic messages
- [ ] LED on GPIO8 turns ON (steady, not blinking)

3. GPIO state verification with multimeter:

- [ ] GPIO 10 (SSR): **< 0.1V** (LOW — heater off)
- [ ] GPIO 9 (Fan): **< 0.1V** (LOW — fan off)
- [ ] GPIO 8 (LED): **~3.3V** (HIGH — LED on)
- [ ] GPIO 3 (CS ET): **~3.3V** (HIGH — CS inactive)
- [ ] GPIO 4 (CS BT): **~3.3V** (HIGH — CS inactive)

---

## Phase 3 — Sensor Verification

Connect via USB CDC (`/dev/ttyACM0`) using a serial terminal or Artisan.

### Temperature Reading

Send `READ` and verify response:

```
Expected: ET_temp,BT_temp,heater_pct,fan_pct
Example:  23.5,24.1,0.0,0.0
```

- [ ] `READ` returns 4 comma-separated values
- [ ] ET and BT are reasonable ambient temps (18°C – 30°C)
- [ ] Heater = 0.0, Fan = 0.0 (both off)

### Sensor Self-Test

- [ ] ET temperature is not 0.0 or -999 (indicates SPI failure)
- [ ] BT temperature is not 0.0 or -999
- [ ] ET and BT are within ~2°C of each other at ambient
- [ ] Touch BT thermocouple → BT reading rises within 2 seconds
- [ ] Touch ET thermocouple → ET reading rises within 2 seconds

**If any sensor reads 0.0 or absurd values → STOP. Do NOT proceed.** SPI wiring issue.

### Status Report

Send `STATUS` and verify:

- [ ] Returns 20-field CSV line
- [ ] `watchdog_feed_ok=true`
- [ ] `state=Idle`
- [ ] `ssr_output=0.0`
- [ ] `fan_output=0.0`

---

## Phase 4 — Actuator Verification (Heater STILL Disconnected)

### Fan Test

Send these commands via serial:

```
IO3 50
```

- [ ] Fan spins at ~50% speed
- [ ] Multimeter shows PWM on GPIO9 (~1.6V average at 50% duty)

```
IO3 100
```

- [ ] Fan spins at full speed

```
IO3 0
```

- [ ] Fan stops completely

### SSR Test (Heater Disconnected — Verify GPIO Only)

```
OT1 50
```

- [ ] Multimeter shows PWM activity on GPIO10 (~1.6V average at 50% duty)

```
OT1 0
```

- [ ] GPIO10 returns to < 0.1V (LOW)

### Emergency Stop Test

```
OT1 80
IO3 80
STOP
```

- [ ] After STOP: GPIO10 goes LOW (heater off)
- [ ] Fan goes to 100% briefly then off (emergency behavior)
- [ ] STATUS shows `state=EmergencyStop`

---

## Phase 5 — Artisan Connectivity Test

1. Configure Artisan to connect to `/dev/ttyACM0` (USB CDC)
2. Set baud rate: **115200** (or leave auto for USB CDC)
3. Start Artisan connection

- [ ] Artisan connects without errors
- [ ] Temperature readings appear in Artisan plot
- [ ] ET and BT values update every ~1 second
- [ ] Send `OT1 30` from Artisan → SSR responds
- [ ] Send `IO3 50` from Artisan → Fan responds
- [ ] Artisan handshake does not crash firmware (FILT, CHAN, UNITS commands)

---

## Phase 6 — Connect Heater (Final Step)

**Only proceed if ALL previous phases passed.**

- [ ] Verify heater element is properly mounted in roaster
- [ ] Verify thermal fuse (260°C) is in series with heater element
- [ ] Connect SSR output to heater AC circuit
- [ ] Ensure fire extinguisher is accessible
- [ ] Ensure area is well-ventilated

### First Heat Test

```
OT1 10
```

- [ ] Heater produces gentle warmth (10% duty)
- [ ] Temperature readings in Artisan RISE
- [ ] No smoke, no unusual smells

```
OT1 0
```

- [ ] Heater turns off
- [ ] Temperature stabilizes then falls

### Safety Shutdown Test

```
OT1 50
```
Wait 5 seconds, then:

```
STOP
```

- [ ] Heater cuts immediately
- [ ] Fan goes to emergency speed
- [ ] System enters `EmergencyStop` state
- [ ] STATUS confirms `state=EmergencyStop`

---

## Quick Reference — Commands for Testing

| Command | Effect | Expected Response |
|---------|--------|-------------------|
| `READ` | Poll temperatures | `ET,BT,0.0,0.0` |
| `STATUS` | Full diagnostics | 20-field CSV |
| `IO3 50` | Fan to 50% | `OK` |
| `IO3 0` | Fan off | `OK` |
| `OT1 50` | Heater to 50% | `OK` |
| `OT1 0` | Heater off | `OK` |
| `STOP` | Emergency stop | `OK` |
| `CHAN;1200` | Handshake | `OK` |
| `UNITS;C` | Celsius mode | `OK` |
| `FILT;70,70,70,70` | Filter config | `OK` |

---

## Emergency Procedures

### If firmware panics or crashes
1. The hardware watchdog (RWDT) will reset the CPU in ~2 seconds
2. After reset, all outputs initialize to OFF (LOW)
3. If reset loop occurs (blinking LED), disconnect heater and investigate

### If temperature exceeds 260°C
- Firmware triggers automatic emergency shutdown
- Heater forced to 0%, fan forced to 100%
- If firmware cannot control heater, the thermal fuse (260°C) is the last line of defense

### If SSR appears stuck ON
1. Send `STOP` command immediately
2. Disconnect heater AC power
3. Check SSR for welded contacts (measure resistance across SSR output with power off)
4. Do NOT reconnect heater until SSR is verified functional

---

## Sign-Off

| Phase | Status | Date | Notes |
|-------|--------|------|-------|
| 0 — Build | ☐ Pass / ☐ Fail | | |
| 1 — Visual | ☐ Pass / ☐ Fail | | |
| 2 — Boot | ☐ Pass / ☐ Fail | | |
| 3 — Sensors | ☐ Pass / ☐ Fail | | |
| 4 — Actuators | ☐ Pass / ☐ Fail | | |
| 5 — Artisan | ☐ Pass / ☐ Fail | | |
| 6 — Heater | ☐ Pass / ☐ Fail | | |

**All phases must pass before regular operation.**
