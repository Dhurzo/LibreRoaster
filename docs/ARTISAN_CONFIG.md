# Artisan Configuration Guide for LibreRoaster

**Last updated:** 2026-09-09  
**Firmware version:** v0.1+ (protocol compatible with Artisan ArduinoTC4 driver)

---

## 1. Quick Start

### Physical Connection Options

| Transport | Port | Baud | Notes |
|-----------|------|------|-------|
| **USB CDC** (native) | `/dev/ttyACM0` (Linux) / `COMx` (Windows) | 115200 8N1 | Preferred — no extra wiring |
| **UART0** | GPIO20 (RX), GPIO21 (TX) | 115200 8N1 | Requires USB-UART adapter |

> **Important:** The ESP32-C3 USB CDC uses the **USB-Serial-JTAG** peripheral. On Linux it appears as `/dev/ttyACM0`. On Windows it may need the `esp32_usb_jtag` driver or appear as a generic CDC device.

---

## 2. Artisan Device Configuration

### 2.1 Open Artisan → Config → Device

```
Device Type: Arduino
Port:        [your port, e.g. /dev/ttyACM0 or COM3]
Baud Rate:   115200
Data Bits:   8
Parity:      None
Stop Bits:   1
Flow Control: None
```

### 2.2 Extra Settings (Critical)

In the same **Device** tab, ensure these are set:

| Setting | Value | Why |
|---------|-------|-----|
| **DTR on connect** | ❌ **Unchecked (Disabled)** | Firmware ignores DTR/RTS flow-control lines; leave disabled for a clean session |
| **RTS on connect** | ❌ **Unchecked (Disabled)** | Firmware ignores DTR/RTS; no GPIO0 handling in firmware |
| **Timeout (ms)** | 2000 | LibreRoaster control loop tick ≈ 330ms; 2s covers 6 ticks |
| **Inter-byte timeout (ms)** | 100 | Parser expects complete lines |

---

## 3. Artisan → Config → Port (ArduinoTC4 Driver)

LibreRoaster speaks the **TC4 serial protocol** (ArduinoTC4 compatible). Configure the driver:

```
Driver: ArduinoTC4
Port:   [same as above]
```

### TC4 Driver Options (Advanced)

Click **Configure** next to the driver dropdown:

| Option | Setting | Notes |
|--------|---------|-------|
| **Channels** | 2 | LibreRoaster has BT + ET (CHAN;2 acknowledged) |
| **Temperature Unit** | Celsius / Fahrenheit | Sent via `UNITS;C` or `UNITS;F` on connect |
| **Filter** | 70 (default) | Acknowledged via `FILT;70` — firmware stores first value only |
| **Poll Rate (Hz)** | 1 | Artisan polls `READ` ~1Hz; firmware responds immediately |

> **Note:** The `CHAN`/`UNITS`/`FILT` handshake commands are **accepted even when the safety latch is armed** (firmware v0.1+). Artisan can reconnect to a latched device without power cycling.

---

## 4. Session Workflow

### 4.1 Normal Session Sequence

```
1. Artisan opens serial port
2. Artisan sends handshake (auto on connect):
   CHAN;2          → #2
   UNITS;C         → #OK
   FILT;70,70,70,70 → #OK
3. Artisan polls READ (≈1 Hz):
   READ            → AMB,ET,BT,0.0,0.0[,HEATER,FAN,SV]
4. Operator sends commands during roast:
   OT1 75          → manual heater 75%
   OT2 50          → manual fan 50%
   PID;SV;200      → set PID target 200°C
   START           → begin roast (enables PID)
   STOP            → end roast (cooldown fan 100%)
```

### 4.2 Handshake Details

| Command | Sent by Artisan | LibreRoaster Response | Purpose |
|---------|-----------------|----------------------|---------|
| `CHAN;2` | Auto on connect | `#2` | Channel count acknowledgement |
| `UNITS;C` / `UNITS;F` | Auto on connect | `#OK` | Temperature scale |
| `FILT;70` | Auto on connect | `#OK` | Filter coefficient (stored, not used) |

> **Protocol Note:** Responses are `#`-prefixed because Artisan's ArduinoTC4 driver **only accepts empty or `#`-prefixed lines during initialisation**. A plain `OK` would cause "Arduino could not set temperature unit" and infinite re-init loop.

---

## 5. Temperature Units (°C / °F)

LibreRoaster supports **both scales**. The conversion is applied to **all temperature-bearing fields**:

| Field | Converted? |
|-------|------------|
| ET, BT, AMB | ✅ Yes |
| SV (setpoint) | ✅ Yes |
| PV, MV, Integrator | ❌ No (percent, not temperature) |
| Heater, Fan | ❌ No (percent) |
| ROR (rate of rise) | ✅ Yes — °C/min or °F/min |

### Switching Units Mid-Session

```
UNITS;F  → #OK  (all subsequent READ/STATUS/telemetry in °F)
UNITS;C  → #OK  (back to °C)
```

Setpoints sent in °F (`PID;SV;392` = 200°C) are **converted to °C internally** before validation. The valid target range is **50–300°C** (122–572°F).

---

## 6. Control Modes

### 6.1 Manual Mode (Artisan Sliders)
```
OT1 75     → Heater 75% (0-100)
OT2 50     → Fan 50% (0-100, floor 20% if heater > 0)
IO3 50     → Alias for OT2
DCFAN 50   → Alias for OT2
UP / DOWN  → Heater ±5%
```
- **PID disabled**, `artisan_control = true`
- Safety backstops active: comms-idle (15s), max roast time (30min), probe-stuck warning (120s)

### 6.2 PID Mode (Artisan PID Dialog)
```
PID;ON           → StartRoast (enables PID, clears I-term)
PID;SV;200       → Set target 200°C (display units)
PID;T;2.0;0.25;0.05 → Set KP/KI/KD
PID;LIMIT;0;100  → Output limits
PID;CHAN;2       → Feedback channel (1=ET, 2=BT default)
PID;CT;1000      → Cycle time ms (10-60000)
PID;OFF          → StopRoast (disables PID)
```

> **Important:** Artisan's ramp/soak re-sends `PID;SV` on every step. LibreRoaster **only enables PID on the first call**; subsequent `SV` updates the target without resetting the integrator (Bug A3 fix).

### 6.3 Profile Mode (Artisan Profile Tab)
```
PROFILE;0,50;120,150;300,200;480,225
FANPROFILE;0,30;60,60;300,80
START            → Begin profile roast (enables PID + profile follow)
```
- Linear interpolation between setpoints
- Max 16 setpoints per profile
- Temperatures in **display units** (converted to °C internally)

---

## 7. Telemetry & Monitoring

### 7.1 READ Response (Polled by Artisan)

**PID Disabled (5 fields):**
```
AMB,ET,BT,0.0,0.0
```
Example: `0.0,120.3,150.5,0.0,0.0`

**PID Enabled (8 fields):**
```
AMB,ET,BT,0.0,0.0,HEATER,FAN,SV
```
Example: `0.0,120.3,150.5,0.0,0.0,75.0,50.0,200.0`

> **AMB** is always `0.0` (no ambient sensor on hardware).

### 7.2 STATUS Response (Deep Diagnostics)
```
STATUS → 20 CSV fields
```
| # | Field | Units | Notes |
|---|-------|-------|-------|
| 1 | ET | °C/°F | Display scale |
| 2 | BT | °C/°F | Display scale |
| 3 | Heater | % | 0-100 |
| 4 | Fan | % | 0-100 |
| 5 | Watchdog OK | 0/1 | Software watchdog |
| 6 | Watchdog Failures | count | Consecutive |
| 7 | Last Watchdog Reason | token | `timeout`, `watchdog_unavailable`, etc. |
| 8 | LEDC Guard Timeouts | count | SSR cycle guard |
| 9 | Regression Active | 0/1 | Overtemp regression run |
| 10 | PV | °C/°F | PID process variable |
| 11 | MV | % | PID manipulated variable |
| 12 | Integrator | % | PID I-term |
| 13 | Derivative | °C/min or °F/min | **Display scale / min** |
| 14 | Saturation | 0/1 | PID output at limit |
| 15 | Integrator Clamp | 0/1 | Anti-windup active |
| 16 | Derivative Avail | 0/1 | Valid derivative |
| 17 | Cmd Latency | µs | Last command |
| 18 | Max Cmd Latency | µs | Peak observed |
| 19 | Temp Scale | 0/1 | 0=°C, 1=°F |
| 20 | Fault Flag | 0/1 | Safety latch armed |

### 7.3 Continuous Telemetry Stream (Opt-in)
```
STREAM;ON  → #OK  (enables spontaneous # lines)
STREAM;OFF → #OK  (disables)
```

**Stream format (once per second, `DEFAULT_OUTPUT_INTERVAL_MS = 1000ms`):**
```
#<time_s>,ET,BT,ROR,Gas
```
Example: `#123.45,120.3,150.5,12.50,75.0`

- `time_s`: Seconds since `START` (or boot if no roast)
- `ROR`: °C/min or °F/min (display scale)
- `Gas`: Heater output %

> **Default is OFF.** Artisan polls `READ` and doesn't need the stream. Enable only for custom dashboards.

### 7.4 #DUMP — Roast Log Recovery
```
#DUMP  → Full CSV dump of ring buffer (256 samples ≈ 4 min at 1Hz)
```
- Independent of `STREAM` flag
- Always logs during active roast (`START` → `STOP`)
- Preserves **newest** rows (end of roast) when buffer exceeds dump size
- Header: `#DUMP time_s,bt,et,heater,fan,target,ror`

---

## 8. Safety & Emergency Commands

### 8.1 Operator Commands
| Command (wire) | Action | Recovery |
|---------|--------|----------|
| `STOP` | Emergency stop, heater 0%, fan 100% sticky, **arms safety latch** (`ArtisanCommand::EmergencyStop` → `handle_emergency_stop`) | `PID;OFF` (unconditional un-latch to `Idle`), or `START`/`PREHEAT` (deliberate re-energize, clears latch) |
| `PID;OFF` (also `PID,OFF`) | Stop path, **clears latch if armed** (`ArtisanCommand::Stop` → `clear_emergency_explicit` + `handle_stop`), heater 0% | — |

> **Key distinction:** `STOP` arms the safety latch. Recovery is `PID;OFF` (or `START`/`PREHEAT` as deliberate re-energize). Bare `OFF`, `ESTOP` and `StopRoast` are **not** wire commands: bare `OFF` parses as `ERR unknown_command`, there is no `ESTOP` verb in `src/input/parser.rs:227-364`, and `StopRoast` is an internal `RoasterCommand` only (`src/config/constants.rs:785`). This prevents accidental re-energizing after a safety event.

### 8.2 Safety Backstops (Automatic)
| Backstop | Trigger | Action |
|----------|---------|--------|
| **Over-temp** | BT/ET ≥ 260°C | Emergency shutdown |
| **Probe stuck (PID)** | BT flat <1°C for 120s with heater on (`ssr_output > 0.0`, S1 fix; `PROBE_STUCK_HEATER_MIN_PCT` retained as constant only) | Emergency shutdown |
| **Probe stuck (Manual)** | BT flat <1°C for 120s with heater on (`ssr_output > 0.0`) | **Warning** `ERR probe_stuck_warning`; latch at 300s |
| **Comms idle** | No command 15s @ heater >0 or roast active | Emergency shutdown |
| **Max roast time** | 30min (1800s) @ heater >0 or Heating/Stable | Emergency shutdown |
| **Sensor stale** | No valid reading 1s | Emergency shutdown |
| **RTC Watchdog** | Control loop hangs >2.2s | **CPU reset** (hardware) |
| **SSR cycle guard** | Write attempted <100ms since last | Reject / adopt as setpoint |
| **Fan floor** | Heater >0 & fan <20% | Raise fan to 20% |

### 8.3 Safety Fault Notification (Wire)
Since v0.1 (Audit A-TC4), internal safety traps emit on the wire:
```
ERR safety_fault <reason>
```
Emitted **once per latch event** (not every tick). Reasons: `Overtemp`, `Probe stuck`, `Comms idle timeout`, `Maximum roast time exceeded`, `Temperature sensor timeout`, `Sensor fault`, `Watchdog failure`, `Heater control failure`.

---

## 9. Troubleshooting

### 9.1 Artisan Shows "Arduino could not set temperature unit"
- **Cause:** Handshake response not `#`-prefixed
- **Fix:** Ensure LibreRoaster firmware ≥ v0.1 (sends `#OK` for UNITS/FILT)

### 9.2 READ Returns `0.0,0.0,0.0,0.0,0.0`
- **Cause:** Sensors not initialized or SPI wiring issue
- **Check:** MAX31856 CS pins (GPIO4=BT, GPIO3=ET), SPI wiring (SCK=6, MOSI=7, MISO=5)

### 9.3 Heater Won't Go Above 0%
- **Cause:** Safety latch armed (`fault_condition = true`)
- **Check:** `STATUS` field 20 = 1 → send `PID;OFF` (or `START`/`PREHEAT` to re-energize) to clear
- **Also check:** `SSR hardware status` (field in STATUS) = `NotDetected` or `Error` → check GPIO1 current-sense circuit or use `no-heat-sense` feature

### 9.4 Fan Stays at 0% in PID Mode
- **Cause:** No `FANPROFILE` sent, `OT2` not set
- **Fix:** Send `OT2 50` or `FANPROFILE` — firmware enforces **20% minimum** when heater > 0 (`FAN_MIN_SAFETY_PCT`)

### 9.5 `ERR channel_full command_dropped`
- **Cause:** Command burst >16 commands in one tick (Artisan startup burst)
- **Fix:** Artisan retries automatically; firmware channel size = 16

### 9.6 Telemetry Corrupted / Garbled Lines
- **Cause:** `esp_println` logs sharing USB/UART with protocol (Bug #6)
- **Workaround:** Flash without `instrumentation` feature (log level = Warn)
- **Permanent fix:** Requires dedicated UART1 log sink (planned Fase 6)

### 9.7 PID Output Stays at 0% After `PID;SV`
- **Cause:** `PID;SV` sent but `PID;ON` never received
- **Fix:** Ensure Artisan sends `PID;ON` (maps to `START`) before `PID;SV`

### 9.8 RoR False Trip on Light Roast Turnaround
- **Symptom:** `ERR safety_fault Bean temperature rate-of-rise exceeded` at ~0.6°C/s after charge
- **Cause:** Light roast turnaround spike in soft band (0.5–1.0°C/s)
- **Status:** Two-tier guard (A-TC4-D) — soft band needs 12 consecutive ticks (~3.7s). Thresholds provisional (`MAX_BT_RATE_OF_RISE=0.5`, `MAX_BT_RATE_OF_RISE_HARD=1.0`). May need HIL calibration.

---

## 10. Advanced: Custom Client Integration

### 10.1 Minimal Command Set for Custom Clients
```
READ              → Poll temperatures (1Hz)
OT1 <0-100>       → Manual heater
OT2 <0-100>       → Manual fan
START             → Begin roast (enables PID at current target)
STOP              → End roast (cooldown)
#DUMP             → Get roast log on reconnect
STATUS            → Deep diagnostics
```

### 10.2 Line Framing
- **Commands:** Terminated by `\n` (LF) or `\r\n` (CRLF)
- **Responses:** Always `\r\n` terminated
- **Encoding:** ASCII / UTF-8

### 10.3 Delimiter Flexibility
All commands accept **space**, `;`, `,`, or `=` as parameter delimiter:
```
OT1 75
OT1;75
OT1,75
OT1=75
```
All equivalent.

---

## 11. Firmware Build Features Relevant to Artisan

| Feature | Effect | Use Case |
|---------|--------|----------|
| `embedded` | Real hardware build (required for device) | Production flash |
| `simulated-sensors` | Synthetic temperature curves | Host testing / CI |
| `no-heat-sense` | Disables GPIO1 current-sense check | Boards without detection circuit |
| `instrumentation` | Debug log level (corrupts wire!) | **Never in production** |
| `regression` | Overtemp self-test runner | Hardware validation |

---

## 12. Version Compatibility

| LibreRoaster | Artisan | Notes |
|--------------|---------|-------|
| v0.1+ | 2.8+ | Full TC4 compatibility |
| v0.1+ | 2.7 | Works, may need manual driver config |

---

## 13. Reference: Complete Command List

### Handshake (accepted while latched)
```
CHAN;<rate>     → #<rate>
UNITS;C|F       → #OK
FILT;<val>      → #OK
```

### Polling
```
READ            → AMB,ET,BT,0.0,0.0[,HEATER,FAN,SV]
STATUS / STAT   → 20-field CSV
```

### Control
```
START           → Begin roast (PID + profile; also clears latch as deliberate re-energize)
STOP            → Emergency stop (arms latch; recovery via PID;OFF / START / PREHEAT)
PID;OFF         → Stop + clear latch (recovery to Idle; also PID,OFF)
PREHEAT <temp>  → Preheat ramp (also clears latch as deliberate re-energize)
```

### Manual Actuator
```
OT1 <0-100>     → Heater %
OT2 <0-100>     → Fan % (clamped, floor 20% if heater>0)
IO3 <0-100>     → Alias OT2
DCFAN <0-100>   → Alias OT2
UP / DOWN       → Heater ±5%
```

### PID
```
PID;ON          → StartRoast
PID;OFF         → StopRoast
PID;SV;<temp>   → Set target (display units)
PID;T;<kp>;<ki>;<kd> → Gains
PID;LIMIT;<min>;<max> → Output limits
PID;CHAN;<1|2>  → Feedback channel
PID;CT;<ms>     → Cycle time (10-60000)
PIDGAIN <kp> <ki> <kd> → Alternative gains syntax
```

### Profiles
```
PROFILE;t1,temp1;t2,temp2;...
FANPROFILE;t1,fan1;t2,fan2;...
SETTARGET <temp> → Alias PID;SV
PREHEAT <temp>   → Preheat ramp
```

### Telemetry
```
STREAM;ON|OFF   → Enable/disable # stream
#DUMP           → Ring buffer CSV dump
REG             → Run regression self-test
```

---

## 14. Support & Debugging

### Enable Debug Logging (Host Only)
```bash
cargo build --features test,instrumentation
# Run host tests with debug output
```

### Capture Wire Traffic
```bash
# Linux: monitor USB CDC
cat /dev/ttyACM0 | tee artisan_log.txt

# Or use picocom/minicom with logging
picocom -b 115200 /dev/ttyACM0 --omap crcrlf -l artisan_session.log
```

### Report Issues
Include:
1. Artisan version
2. Connection type (USB CDC / UART)
3. Wire log (captured as above)
4. `STATUS` output at time of issue
5. Firmware git commit (`git rev-parse HEAD`)

---

*Generated from LibreRoaster source code analysis. For protocol implementation details, see `src/input/parser.rs`, `src/output/artisan.rs`, and `docs/PROTOCOL.md`.*