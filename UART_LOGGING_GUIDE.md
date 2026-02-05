# UART Logging Guide

This guide explains how to read and interpret LibreRoaster's UART logging output for debugging Artisan communication issues.

## Overview

LibreRoaster outputs diagnostic logs directly to UART0 at 115200 baud. These logs provide visibility into what's happening inside the device during a roast session.

**What UART logs help you do:**

- Trace Artisan protocol commands as they arrive and are processed
- See temperature readings and heater/fan commands
- Identify communication issues between Artisan and the device
- Monitor system events and state changes

**What you need:**

- Ability to connect to UART0 and read serial output (covered in ARTISAN_CONNECTION.md)
- Basic familiarity with reading serial terminal output

---

## Format

LibreRoaster uses channel-prefixed logging. Every log line follows this format:

```
[CHANNEL] message
```

**Components:**

| Component | Description |
|----------|-------------|
| `[CHANNEL]` | One of `[USB]`, `[UART]`, or `[SYSTEM]` — identifies the log source |
| `message` | The actual log content — varies by channel |

### Channel Meanings

| Channel | Purpose | Example |
|---------|---------|---------|
| `[USB]` | Artisan protocol traffic — commands and responses | `[USB] RX: READ` |
| `[UART]` | Device-to-device communication | `[UART] TX: 185.2,192.3,...` |
| `[SYSTEM]` | System-level events and diagnostics | `[SYSTEM] Temperature: 185.5°C` |

### Annotated Log Examples

```
[USB] RX: READ                              ← Artisan sent READ command
[UART] TX: 185.2,192.3,-1.0,-1.0,24.5,45,75 ← Device responding with temperature data
[SYSTEM] Temperature: 185.5°C                ← System logging temperature reading
[USB] RX: SET 200,100                       ← Artisan sent SET command (heater=200, fan=100)
```

### Reading Artisan Traffic

The `[USB]` channel shows all Artisan protocol communication:

- **RX** lines: Commands Artisan is sending to LibreRoaster
- **TX** lines: Responses LibreRoaster is sending to Artisan

Example RX lines you might see:

| RX Message | Meaning |
|------------|---------|
| `RX: READ` | Artisan requesting current sensor readings |
| `RX: SET 200,100` | Artisan setting heater to 200, fan to 100 |
| `RX: ON` | Artisan starting the roast session |
| `RX: OFF` | Artisan stopping the roast session |

Example TX lines show comma-separated values:

```
TX: 185.2,192.3,-1.0,-1.0,24.5,45,75
```

| Position | Meaning |
|----------|---------|
| 1 | ET (Environmental Temp) |
| 2 | BT (Bean Temp) |
| 3-4 | Reserved (currently -1.0) |
| 5 | Heater value |
| 6 | Fan value |
| 7 | System state |

---

## Levels

LibreRoaster uses three log levels through its `esp_println` output. This section explains what each level indicates when you see it.

### INFO Messages (Standard Operation)

INFO-level logs show normal operational events — things working as expected.

**What to expect during a normal roast:**

- Artisan commands received (`RX:`)
- Device responses sent (`TX:`)
- Temperature readings logged
- State transitions (start/stop)

**Example INFO sequence:**

```
[USB] RX: ON
[SYSTEM] Roaster state: ROAST
[SYSTEM] Temperature: 185.5°C
[USB] RX: READ
[UART] TX: 185.2,192.3,-1.0,-1.0,24.5,45,75
```

### DEBUG Messages (Detailed Diagnostics)

DEBUG messages provide additional detail for troubleshooting. These appear when developers need visibility into internal operations.

**What DEBUG logs might show:**

- Detailed command parsing steps
- Internal calculations or intermediate values
- Protocol-specific timing information

**When DEBUG logs appear:**

You may not see DEBUG logs during normal operation. They are enabled during development and debugging sessions.

### WARN Messages (Attention Needed)

WARN messages indicate something unusual that may need attention but doesn't prevent operation.

**Common WARN scenarios:**

- Intermittent communication glitches
- Unusual sensor readings that are within tolerance
- Artisan sending unexpected but valid commands

**Example WARN:**

```
[SYSTEM] WARN: ET sensor reading near upper limit (245.3°C)
```

### ERROR Messages (Problems)

ERROR messages indicate problems that require attention.

**Common ERROR scenarios:**

- Malformed Artisan commands
- Communication timeouts
- Invalid sensor readings
- State machine violations

**Example ERROR:**

```
[USB] ERROR: Malformed READ command received
[SYSTEM] ERROR: BT sensor disconnected (-273.2°C)
```

---

## Troubleshooting

Use UART logs to diagnose common problems with Artisan communication.

### Artisan Not Connecting

**Symptoms:** Artisan shows "Device not responding" or connection timeout

**Check in logs:**

1. Look for `[USB] RX:` messages — if you see these, Artisan IS connecting
   - `[USB] RX: ON` → Connection successful
   - No RX messages → Artisan not reaching the device

2. Check `[SYSTEM]` logs for device state:
   - `[SYSTEM] Roaster state: IDLE` → Device ready
   - `[SYSTEM] ERROR:` messages → Check error details

**Example — Connection working:**

```
[SYSTEM] USB connected
[SYSTEM] Roaster state: IDLE
[USB] RX: ON
[SYSTEM] Roaster state: ROAST
[USB] RX: READ
```

**Example — Connection failing (no USB messages):**

```
[SYSTEM] USB connected
[SYSTEM] Roaster state: IDLE
(no RX messages appear)
```

If no RX messages appear but USB shows connected, check Artisan settings:
- Verify correct serial port selected
- Confirm baud rate is 115200
- Check Artisan protocol settings

### Temperature Readings Stuck or Wrong

**Symptoms:** Artisan shows constant or unrealistic temperature values

**Check in logs:**

1. Monitor `[SYSTEM] Temperature:` logs for actual sensor readings
2. Compare with Artisan display

**Example — Normal operation:**

```
[SYSTEM] Temperature: 185.5°C
[SYSTEM] Temperature: 186.1°C
[SYSTEM] Temperature: 186.8°C
```

**Example — Sensor issue (-273.2°C = disconnected):**

```
[SYSTEM] ERROR: BT sensor disconnected (-273.2°C)
```

### Heater/Fan Commands Not Working

**Symptoms:** Setting values in Artisan has no effect

**Check in logs:**

1. Look for `[USB] RX: SET` messages
2. Verify TX responses include updated heater/fan values

**Example — SET command working:**

```
[USB] RX: SET 200,100
[SYSTEM] Temperature: 185.5°C
[UART] TX: 185.2,192.3,-1.0,-1.0,200,100,2
```

Position 5 = 200 (heater set), Position 6 = 100 (fan set)

**If you see RX: SET but TX shows old values:**

This indicates the SET command wasn't processed correctly. Check for ERROR messages that might explain why.

### Protocol Errors

**Symptoms:** Artisan reports protocol errors or "Invalid response"

**Check in logs:**

1. Look for `[USB] ERROR:` messages
2. Check for malformed commands in `[USB] RX:` lines

**Common protocol errors:**

| Error Pattern | Meaning | Action |
|---------------|---------|--------|
| `Malformed command` | Artisan sent unrecognized command | Check Artisan settings |
| `Invalid value` | SET command had out-of-range value | Reduce heater/fan values |
| `Timeout` | No response within expected time | Check USB connection |

### Log Pattern Quick Reference

| Pattern | Indicates | Check |
|---------|-----------|-------|
| No `[USB]` messages | Artisan not sending commands | Verify serial port and Artisan config |
| `[USB] RX:` but no `[UART] TX:` | Device not responding | Check device power, reset if needed |
| `-273.2°C` readings | Sensor disconnected | Physical inspection required |
| `ERROR: Malformed` | Protocol mismatch | Check Artisan protocol version |
| Constant values in TX | Stuck in loop or no sensor updates | Power cycle device |

---

## Using Logs Effectively

**Tips for productive troubleshooting:**

1. **Start logging before connecting Artisan** — Capture the full connection sequence
2. **Note the timestamp of when problems occur** — Helps correlate with Artisan events
3. **Watch for changes in patterns** — Sudden changes often indicate the root cause
4. **Compare working vs. failing sessions** — What changed between successful connections?

**Common workflow:**

1. Connect serial terminal, start logging
2. Open Artisan, attempt connection
3. If problems occur, review logs for:
   - `[SYSTEM]` state at connection time
   - `[USB]` traffic patterns
   - `[ERROR]` messages

The logs provide an exact record of what happened, making it easier to identify whether issues are with Artisan, the device, or the connection.
