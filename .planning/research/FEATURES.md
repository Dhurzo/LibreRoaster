# Feature Landscape: Artisan Serial Protocol

**Project:** LibreRoaster v1.5 — Full Artisan Serial Protocol Implementation
**Domain:** Coffee Roaster Firmware (ESP32-C3)
**Researched:** February 4, 2026
**Confidence:** HIGH — Protocol well-documented through official Artisan docs and community implementations

## Executive Summary

Artisan is the dominant open-source coffee roasting software, communicating with roasters via a text-based serial protocol originally designed for the TC4 Arduino shield. The protocol follows a **master-slave polling model** where Artisan (master) sends commands and the roaster (slave) responds. The protocol is **asynchronous and line-oriented**, with ASCII commands terminated by newlines. Key characteristics:

- **Baud rate:** 115200 (8N1) — industry standard for TC4-compatible devices
- **Command-response cycle:** Artisan polls at configurable intervals (typically 1 second)
- **Bidirectional flow:** Artisan sends control commands; roaster sends temperature data
- **Initialization required:** Handshake sequence must complete before data logging begins

For LibreRoaster, the existing `ArtisanFormatter` implements READ response, and `OT1`/`IO3` command parsing exists. The gap is full bidirectional protocol support including proper handshake, command acknowledgment, and consistent data formatting.

---

## Table Stakes

Features users expect. Missing = Artisan will not connect or function correctly.

| Feature | Why Expected | Complexity | Dependencies | Implementation Notes |
|---------|--------------|------------|--------------|---------------------|
| **Initialization Handshake** | Artisan requires channel configuration before polling begins | Low | None | Must respond to `CHAN,xxxx` with `#` acknowledgment; accept `UNIT,F/C` |
| **READ Command Response** | Core data exchange — Artisan polls temperatures every ~1 second | Low | ArtisanFormatter (existing) | Returns comma-separated values: `ET,BT,ET2,BT2,ambient,fan%,heater%` |
| **OT1 Heater Control** | Primary heater power control (0-100%) | Low | Existing command parser | Command: `OT1,XX`; implement PWM output on heater |
| **IO3 Fan Control** | Fan speed control via PWM | Low | Existing command parser | Command: `IO3,XX`; implement PWM output on fan |
| **Baud Rate 115200** | Standard TC4 protocol speed | Low | None | Configure UART to 115200,8,N,1 |
| **Line-Based Serial Protocol** | Protocol uses newline-terminated ASCII commands | Low | None | Parse commands until `\n`, respond with newline-terminated lines |
| **Temperature Scaling** | Artisan expects temperatures in configured units | Low | None | Support `UNIT,F` and `UNIT,C` commands; convert internally |
| **Channel Mapping Response** | Artisan maps received values to configured channels | Low | ArtisanFormatter | First two values = ET (environment/bean temp), ambient optional |

### Handshake Protocol Details

Artisan performs this initialization sequence on connection:

```
1. Artisan → Device: CHAN,1200  (configures channel mapping)
2. Device → Artisan: #           (ACK required; artisan ignores message after #)
3. Artisan → Device: UNIT,F     (or UNIT,C - temperature units)
4. Device → Artisan: (no response expected for UNIT)
5. Artisan → Device: FILT,0     (optional filtering level)
6. Device → Artisan: #           (ACK for FILT if implemented)
7. Artisan → Device: READ\n     (first data poll)
8. Device → Artisan: temps\n    (response format below)
```

**CRITICAL:** Without proper `#` acknowledgment for `CHAN` command, Artisan will not proceed to temperature polling. This is a common integration failure point.

### READ Response Format

The standard TC4/Artisan response format:

```
ET,BT,ET2,BT2,ambient,fan_duty,heater_duty
```

Example response: `185.2,192.3,-1.0,-1.0,24.5,45,75`

| Position | Value | Notes |
|----------|-------|-------|
| 1 | ET (Environmental Temp) | Primary temperature |
| 2 | BT (Bean Temp) | Secondary temperature |
| 3 | ET2 | Third channel (use -1 if unused) |
| 4 | BT2 | Fourth channel (use -1 if unused) |
| 5 | Ambient | Optional ambient temperature |
| 6 | Fan Duty | 0-100 (ArtisanTC4_56 device type) |
| 7 | Heater Duty | 0-100 (ArtisanTC4_56 device type) |

**Note:** Unused channels should return `-1` or `-1.0`, not `0` or empty string.

---

## Differentiators

Features that set LibreRoaster apart. Not expected by users, but valuable competitive features.

| Feature | Value Proposition | Complexity | Dependencies | Notes |
|---------|------------------|------------|--------------|-------|
| **Dual UART Channels (CDC)** | Existing USB CDC dual-channel support enables independent data/logging streams | Medium | Existing USB CDC | Use one channel for Artisan, another for debugging/logging |
| **Extended Command Set** | Support for `OT2`, `UP`, `DOWN`, `DPIN`, `APIN` beyond basic OT1/IO3 | Low | Existing parser | Full TC4 command compatibility |
| **PID Control Mode** | Onboard PID temperature control with SV setpoint commands | Medium | PID library, heater control | Command: `SV,XXX` sets PID setpoint; artisan receives computed power via ArtisanTC4_56 |
| **Rate-of-Rise (RoR) Reporting** | Calculate and report temperature derivative | Medium | Temperature history buffer | Can be sent as extra channels or calculated by Artisan |
| **Auto-Detection of Charge** | Automatically detect when beans are loaded | Medium | Temperature threshold logic | Trigger roast start based on rapid temp rise |
| **WebSocket Bridge** | Bridge serial protocol to WebSocket for web dashboards | High | WebSocket server | Enable Artisan connection via network, not just USB |
| **MODBUS Support** | Industrial protocol for commercial roaster integration | High | MODBUS library | Alternative to TC4 protocol for enterprise deployments |
| **Background PID Control** | PID runs without Artisan connected | Low | PID library | Autonomous operation; Artisan reads state when connected |
| **Roast Profile Execution** | Execute stored roasting profiles on device | Medium | EEPROM/profile storage | Artisan sends target temps; device follows profile |

### PID Control Mode Details

When Artisan is configured for PID control (device type `ArduinoTC4_78`):

```
Artisan → Device: SV,180    (set target to 180°C)
Device: Computes required power based on PID algorithm
Device → Artisan: [temps]\n  (includes heater% in response)

Artisan → Device: PID,1    (enable PID mode)
Artisan → Device: PID,0    (disable PID mode)
```

The roaster's PID algorithm maintains temperature at SV by modulating heater power. Artisan monitors the process but control logic resides on the device.

### UP/DOWN Commands for Fine Control

```
OT1,UP   → increase heater power by 5%
OT1,DOWN → decrease heater power by 5%
IO3,UP   → increase fan speed by 5%
IO3,DOWN → decrease fan speed by 5%
```

Useful for UI sliders where users increment/decrement rather than set absolute values.

---

## Anti-Features

Features to explicitly NOT build. Common mistakes or protocol violations that cause issues.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| **Non-ACK'd CHAN Response** | Artisan will hang waiting for acknowledgment | Always respond with `#` to `CHAN,xxxx` command |
| **Slow Response Times** | Artisan timeout defaults to 1 second; slow responses cause missed samples | Respond to READ within 100ms; process commands asynchronously |
| **Arbitrary Serial Output** | Any output not matching expected format breaks Artisan parsing | Only output: `#...` acknowledgments and READ responses |
| **Missing Newline Termination** | Protocol is line-oriented; Artisan reads until `\n` | Always terminate responses with `\n` |
| **Using 0 for Unused Channels** | Artisan interprets 0 as valid temperature (0°C/32°F) | Use `-1` or `-1.0` for unused temperature channels |
| **Ignoring UNIT Command** | Artisan sends temperature in configured units; ignoring causes wrong readings | Parse and respect `UNIT,F` and `UNIT,C` |
| **Blocking on Serial Read** | Long serial reads delay READ response, causing Artisan timeouts | Use non-blocking serial with timeouts; process in main loop |
| **Floating-Point Precision Errors** | Malformed floats (e.g., `185.23.4`) break parsing | Format with 1 decimal place: `%.1f` |
| **Hardcoded Channel Mapping** | Artisan may use non-default channel configuration | Support `CHAN,xxxx` for ET/BT assignment |

### Common Protocol Pitfalls

**Pitfall: Arduino Reset on Serial Connect**
- **Problem:** Connecting to serial port resets the Arduino, causing Artisan to miss initialization
- **Solution:** Delay initialization by 2 seconds after serial connection; Artisan handles this gracefully

**Pitfall: Missing Ambient Temperature**
- **Problem:** Some protocol versions expect ambient in position 5
- **Solution:** Include ambient temperature (can be room temp or calculated cold-junction comp)

**Pitfall: Response Order Matters**
- **Problem:** Artisan expects `ET,BT,ET2,BT2,ambient,fan,heater` in that exact order
- **Solution:** Return values in documented order; swap channels if Artisan configured differently via `CHAN`

---

## Feature Dependencies

```
ARTISAN CONNECTION
│
├── Serial Configuration (115200, 8N1)
│   │
│   └── Command Parser (existing: OT1, IO3)
│       │
│       ├── Handshake Handler
│       │   └── ACK Response (#) [BLOCKING - must work]
│       │
│       ├── Temperature Reporting
│       │   ├── ArtisanFormatter (existing)
│       │   ├── Channel Mapping Support
│       │   └── UNIT Conversion
│       │
│       └── Control Commands
│           ├── OT1 (heater) [existing]
│           ├── IO3 (fan) [existing]
│           ├── OT2 (secondary heater/fan)
│           └── UP/DOWN (incremental control)
│
└── Extended Features (optional)
    ├── PID Control
    │   ├── SV Setpoint Command
    │   ├── PID Algorithm
    │   └── Heater Modulation
    │
    ├── Profile Execution
    │   ├── EEPROM Storage
    │   └── Profile Interpolation
    │
    └── MODBUS Bridge
        ├── MODBUS RTU Protocol
        └── Register Mapping
```

### Minimum Viable Protocol Implementation

For v1.5, implement in this order:

1. **Phase 1: Core Handshake**
   - Respond to `CHAN,xxxx` with `#`
   - Accept `UNIT,F`/`UNIT,C`
   - Support `FILT,0` (optional)

2. **Phase 2: Reliable READ Response**
   - Consistent temperature formatting
   - Proper channel ordering
   - `-1` for unused channels

3. **Phase 3: Control Commands**
   - OT1 heater control (existing)
   - IO3 fan control (existing)
   - UP/DOWN incremental control

4. **Phase 4: Extended Compatibility**
   - OT2 secondary output
   - PID mode (if hardware supports)
   - MODBUS (future)

---

## Device Type Selection

Artisan supports multiple device types. For LibreRoaster, recommend:

| Device Type | Use Case | Data Channels | Control Channels |
|-------------|----------|---------------|-----------------|
| `ArduinoTC4` | Basic logging | ET, BT | None |
| `ArduinoTC4_56` | Control logging | ET, BT | Heater %, Fan % |
| `ArduinoTC4_34` | 4-channel logging | ET, BT, ET2, BT2 | None |
| `ArduinoTC4_78` | PID control | ET, BT, SV, Internal | Heater % (computed) |

**Recommendation:** Use `ArduinoTC4_56` for LibreRoaster — enables heater/fan control visibility in Artisan charts.

---

## MVP Recommendation

For v1.5 MVP, prioritize:

1. **Handshake completion** — CHAN acknowledgment, UNIT acceptance
2. **Consistent READ response** — proper format, unused channels as `-1`
3. **Control command reliability** — OT1/IO3 with acknowledgment

Defer to post-MVP:
- **PID mode** — requires PID library integration
- **MODBUS** — enterprise feature for commercial roasters
- **Profile execution** — complex state machine needed

---

## Sources

- [Artisan Official Documentation](https://artisan-scope.org/devices/arduino/) — Device configuration and protocol overview
- [Artisan MODBUS Documentation](https://artisan-scope.org/devices/modbus/) — Extended protocol documentation
- [TC4-Emulator Reference](https://github.com/FilePhil/TC4-Emulator) — Protocol implementation example
- [aArtisanQ PID Firmware](https://github.com/greencardigan/TC4-shield/tree/master/applications/Artisan/aArtisan) — Full command reference
- [Homeroasters Forum — TC4 Integration](https://homeroasters.org/forum/viewthread.php?thread_id=5393) — Community troubleshooting
- [Controlling Hottop with Artisan](https://artisan-roasterscope.blogspot.com/2013/02/controlling-hottop.html) — Control command examples

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Core Protocol | HIGH | Well-documented, multiple reference implementations |
| READ Response Format | HIGH | Standard TC4 format, verified from multiple sources |
| Control Commands | HIGH | OT1/IO3 commands documented in official sources |
| Handshake Requirements | HIGH | CHAN acknowledgment is critical and documented |
| PID Mode | MEDIUM | General concept understood; implementation details vary by firmware |
| MODBUS | LOW | Not needed for MVP; documented but not verified |
