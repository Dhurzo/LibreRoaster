# Artisan TC4 Serial Protocol Specification

> **Source**: Artisan Roaster Scope source code, commit `729c416ccf92d5c91d14db1ac84aa4728ecdeed4`
> **Repo**: https://github.com/artisan-roaster-scope/artisan
> **Key files**: `src/comm.py`, `src/pid_control.py`
> **TC4 firmware reference**: https://github.com/greencardigan/TC4-shield

---

## 1. Initialization Handshake Sequence

On connect, Artisan sends a 3-command handshake. **All responses must start with `#`** or Artisan will reject the connection.

### Sequence (comm.py:6940–7029)

```
Artisan                          Device
────────                         ──────
CHAN;1200\n              ───►    
                         ◄───    # Active channels set to 1200\n

UNITS;C\n  (or UNITS;F)  ───►    
                         ◄───    # ...\n

FILT;70,70,70,70\n       ───►    
                         ◄───    # ...\n
```

After successful handshake, `ArduinoIsInitialized = 1` and Artisan starts polling with `READ`.

---

## 2. CHAN Command (Channel Mapping)

### What Artisan sends (comm.py:6979–6992)

```python
# 2-channel mode (no extra device):
command = 'CHAN;' + et_channel + bt_channel + '00'
# Example: CHAN;1200  (TC1=ET, TC2=BT, chan3=off, chan4=off)

# 4-channel mode (+ArduinoTC4_34 extra device):
command = 'CHAN;' + et_channel + bt_channel + vals[0] + vals[1]
# Example: CHAN;1234  (all 4 channels active)
```

### What Artisan expects back (comm.py:6993–6996)

```python
result = self.SP.readline().decode('utf-8')[:-2]
if result.startswith('#'):
    # SUCCESS - continue handshake
```

**Required**: Response must start with `#`. Example: `# Active channels set to 1200`

**If response does NOT start with `#`**: Artisan may retry or raise an exception → **connection fails**.

### Channel numbering

| Digit | Physical port |
|-------|---------------|
| 1     | TC1 (thermocouple 1) |
| 2     | TC2 (thermocouple 2) |
| 3     | TC3 (thermocouple 3, optional) |
| 4     | TC4 (thermocouple 4, optional) |
| 0     | Channel inactive |

---

## 3. UNITS Command

### What Artisan sends (comm.py:7013–7020)

```python
command = 'UNITS;' + self.aw.qmc.mode + '\n'
# Examples:
#   UNITS;C\n   (Celsius)
#   UNITS;F\n   (Fahrenheit)
```

### What Artisan expects back

```python
result = self.SP.readline().decode('utf-8')[:-2]
```

Response should start with `#`. Artisan reads the line but does not strictly validate the content beyond expecting a response.

---

## 4. FILT Command (Filter Settings)

### What Artisan sends (comm.py:7023–7029)

```python
filt = ','.join([str(f) for f in self.aw.ser.ArduinoFILT])
command = 'FILT;' + filt + '\n'
# Example: FILT;70,70,70,70\n
```

The 4 values correspond to filter parameters for channels 1–4.

### What Artisan expects back

Response line. Should start with `#`.

---

## 5. READ Command (Temperature Polling)

### What Artisan sends (comm.py:7034–7042)

```python
command = 'READ\n'
self.SP.write(str2cmd(command))
self.SP.flush()
libtime.sleep(.1)
rl = self.SP.readline().decode('utf-8', 'ignore')[:-2]
res = [('-1' if el.strip() == '' else el) for el in rl.rsplit(',')]
```

### Response format (comm.py:7043–7044)

The response is a comma-separated list of decimal values. The number of values depends on the CHAN configuration:

| CHAN config | Extra device | Response format |
|-------------|-------------|-----------------|
| `CHAN;1200` | None | `ambient,ET,BT` (3 values) |
| `CHAN;1234` | +ArduinoTC4_34 | `ambient,ET,BT,chan3,chan4` (5 values) |
| `CHAN;123456` | +ArduinoTC4_56 | `ambient,ET,BT,chan3,chan4,chan5,chan6` (7 values) |
| PID ON | +ArduinoTC4_78 | `ambient,ET,BT,...,Heater,Fan,SV` (adds 3 values) |

### Response parsing (comm.py:7046–7057)

```python
# res[0] = ambient/internal temperature
# res[1] = ET (environment temperature, channel 1)
# res[2] = BT (bean temperature, channel 2)
# res[3] = channel 3 (if configured)
# res[4] = channel 4 (if configured)

t1 = float(res[1])  # ET
t2 = float(res[2])  # BT
```

### Field order (IMPORTANT)

```
res[0] → Ambient/Internal temperature
res[1] → ET (channel mapped to ET in CHAN)
res[2] → BT (channel mapped to BT in CHAN)
res[3] → Channel 3 (if active)
res[4] → Channel 4 (if active)
```

**Values of `-1` are used for inactive channels.**

---

## 6. PID Commands (TC4 Firmware PID Mode)

Artisan can delegate PID control to the TC4 firmware. These commands are sent when the user configures "PID on TC4" mode in Artisan.

### PID;ON — Enable PID (pid_control.py:1506)

```python
self.aw.ser.SP.write(str2cmd('PID;ON\n'))
```

### PID;OFF — Disable PID (pid_control.py:1558)

```python
self.aw.ser.SP.write(str2cmd('PID;OFF\n'))
```

### PID;SV;\<setpoint\> — Set Setpoint (pid_control.py:1757)

```python
self.aw.ser.SP.write(str2cmd('PID;SV;' + str(sv) + '\n'))
# Example: PID;SV;150\n
```

### PID;T;\<kp\>;\<ki\>;\<kd\> — Set PID Parameters (pid_control.py:1906)

```python
self.aw.ser.SP.write(str2cmd('PID;T;' + str(kp) + ';' + str(ki) + ';' + str(kd) + '\n'))
# Example: PID;T;2.0;0.5;1.0\n
```

### PID;CHAN;\<channel\> — Set PID Input Channel (pid_control.py:1910)

```python
self.aw.ser.SP.write(str2cmd('PID;CHAN;' + str(source) + '\n'))
# Example: PID;CHAN;2\n  (use BT channel as PID input)
```

### PID;CT;\<ms\> — Set PID Cycle Time (pid_control.py:1913)

```python
self.aw.ser.SP.write(str2cmd('PID;CT;' + str(cycle) + '\n'))
# Example: PID;CT;1000\n  (1 second cycle)
```

### PID;LIMIT;\<min\>;\<max\> — Set Output Limits (pid_control.py:1505)

```python
self.aw.ser.SP.write(str2cmd('PID;LIMIT;' + str(duty_min) + ';' + str(duty_max) + '\n'))
# Example: PID;LIMIT;0;100\n
```

**All PID commands use SEMICOLON (`;`) as delimiter, not comma.**

---

## 7. Control Commands (OT1, OT2, IO3)

These commands control heater and fan manually.

| Command | Description | Format |
|---------|-------------|--------|
| `OT1` | Heater power (0-100%) | `OT1 75\n` |
| `OT2` | Fan speed (0-100%) | `OT2 50\n` |
| `IO3` | Alternative fan control | `IO3 50\n` |

---

## 8. Protocol Reference (TC4 commands.txt)

From the TC4 shield firmware reference:
https://github.com/greencardigan/TC4-shield/blob/master/applications/Artisan/aArtisan/trunk/src/aArtisan/commands.txt

- Command delimiter: newline (`\n`)
- Parameter delimiters: comma, space, semicolon, or equals sign
- Command names: first 5 characters are significant (case-insensitive after first char)
- All commands use 1-based indexing (physical ports 1-4)

---

## 9. Artisan Configuration for LibreRoaster

To connect Artisan to LibreRoaster:

| Setting | Value |
|---------|-------|
| Device | Arduino (TC4) |
| Port | USB CDC or UART0 |
| Baud rate | 115200 |
| ET Channel | Channel 1 |
| BT Channel | Channel 2 |

### Extra devices (if needed)

| Device | Effect |
|--------|--------|
| +ArduinoTC4_34 | Reads 4 channels instead of 2 |
| +ArduinoTC4_56 | Reads 6 channels |
| +ArduinoTC4_78 | Enables PID control on TC4 firmware |

---

## 10. Common Pitfalls

1. **Handshake acks must start with `#`** — Artisan checks `result.startswith('#')` for CHAN. If your firmware sends `"OK"` instead of `"# OK"`, the handshake fails.

2. **PID commands use `;` not `,`** — Artisan sends `PID;ON`, not `PID,ON`. Your parser must handle semicolon-delimited PID commands.

3. **READ response order is AMBIENT,ET,BT** — not ET,BT,HEATER,FAN. The first value is ambient/internal temperature.

4. **Empty values become `-1`** — Artisan replaces empty fields with `-1`: `res = [('-1' if el.strip() == '' else el) for el in rl.rsplit(',')]`

5. **Line endings** — Artisan expects `\n` line endings. It strips the last 2 characters (`[:-2]`) from responses, suggesting it expects `\r\n`.

6. **CHANG vs CHAN** — The original TC4 uses `CHANG` command (5-char significant). Artisan sends `CHAN;` with semicolon.

---

## Source Code References

| What | File | Lines |
|------|------|-------|
| Handshake init | `comm.py` | 6940–6980 |
| CHAN command | `comm.py` | 6979–6992 |
| CHAN response check | `comm.py` | 6993–6996 |
| UNITS command | `comm.py` | 7013–7020 |
| FILT command | `comm.py` | 7023–7029 |
| READ command | `comm.py` | 7034–7042 |
| READ response parsing | `comm.py` | 7043–7157 |
| PID;ON | `pid_control.py` | 1506 |
| PID;OFF | `pid_control.py` | 1558 |
| PID;SV | `pid_control.py` | 1757 |
| PID;T (params) | `pid_control.py` | 1906 |
| PID;CHAN | `pid_control.py` | 1910 |
| PID;CT | `pid_control.py` | 1913 |
| PID;LIMIT | `pid_control.py` | 1505 |
