# Artisan Source Code Analysis — TC4 Implementation

> Generated from Artisan commit `729c416ccf92d5c91d14db1ac84aa4728ecdeed4`
> Repository: https://github.com/artisan-roaster-scope/artisan

---

## Handshake Implementation (comm.py)

### CHAN Command — Full Code (lines 6979–7008)

```python
# Artisan sends CHAN to map thermocouple channels
if 28 in self.aw.qmc.extradevices:  # +ArduinoTC4_34
    vals = ['1','2','3','4']
    try:
        if self.arduinoETChannel and self.arduinoETChannel != 'None' and self.arduinoETChannel in vals:
            vals.pop(vals.index(self.arduinoETChannel))
        if self.arduinoBTChannel and self.arduinoBTChannel != 'None' and self.arduinoBTChannel in vals:
            vals.pop(vals.index(self.arduinoBTChannel))
    except Exception:
        pass
    command = 'CHAN;' + et_channel + bt_channel + vals[0] + vals[1]
else:
    # No extra device — 2-channel mode
    command = 'CHAN;' + et_channel + bt_channel + '00'

self.SP.write(str2cmd(command + '\n'))
self.SP.flush()
libtime.sleep(.1)
result = self.SP.readline().decode('utf-8')[:-2]

if result.startswith('#') and chan is None:
    # Handshake continues with UNITS
    self.ArduinoIsInitialized = 1
```

**Key observation**: If `result.startswith('#')` is False, Artisan may NOT proceed with UNITS/FILT. The `#` prefix is **mandatory**.

### UNITS Command — Full Code (lines 7013–7020)

```python
# OK. NOW SET UNITS
self.SP.reset_input_buffer()
self.SP.reset_output_buffer()
command = 'UNITS;' + self.aw.qmc.mode + '\n'   # 'C' or 'F'
self.SP.write(str2cmd(command))
self.SP.flush()
libtime.sleep(.1)
result = self.SP.readline().decode('utf-8')[:-2]
```

### FILT Command — Full Code (lines 7023–7029)

```python
# OK. NOW SET FILTER
self.SP.reset_input_buffer()
self.SP.reset_output_buffer()
filt = ','.join([str(f) for f in self.aw.ser.ArduinoFILT])
command = 'FILT;' + filt + '\n'   # Default: "FILT;70,70,70,70"
self.SP.write(str2cmd(command))
result = self.SP.readline().decode('utf-8')[:-2]
```

---

## READ Response Parsing — Full Code (comm.py:7034–7057)

```python
# READ TEMPERATURE
command = 'READ\n'
self.SP.reset_input_buffer()
self.SP.reset_output_buffer()
self.SP.write(str2cmd(command))
self.SP.flush()
libtime.sleep(.1)
rl = self.SP.readline().decode('utf-8', 'ignore')[:-2]
res = [('-1' if el.strip() == '' else el) for el in rl.rsplit(',')]

# response: list ["t0","t1","t2"]  with t0 = internal temp; t1 = ET; t2 = BT on "CHAN;1200"
# response: list ["t0","t1","t2","t3","t4"]  with t0 = internal temp; t1 = ET; t2 = BT, t3 = chan3, t4 = chan4 on "CHAN;1234" if ArduinoTC4_34 is configured
# after PID_ON: + [,"Heater", "Fan", "SV"]

if self.arduinoETChannel == 'None':
    t1 = -1
else:
    try:
        t1 = float(res[1])
    except Exception:
        t1 = -1

if self.arduinoBTChannel == 'None':
    t2 = -1
else:
    try:
        t2 = float(res[2])
    except Exception:
        t2 = -1
```

### Response value mapping

| Index | Content | Notes |
|-------|---------|-------|
| `res[0]` | Ambient/internal temp | Not used for ET/BT display |
| `res[1]` | ET | Mapped from channel configured in CHAN |
| `res[2]` | BT | Mapped from channel configured in CHAN |
| `res[3]` | Channel 3 | Only if CHAN configured 3+ channels |
| `res[4]` | Channel 4 | Only if CHAN configured 4+ channels |
| `res[5]` | Heater duty % | Only when PID ON |
| `res[6]` | Fan duty % | Only when PID ON |
| `res[7]` | SV (setpoint) | Only when PID ON |

Empty values are replaced with `-1` before parsing.

---

## PID Commands Implementation (pid_control.py)

### PID;ON (line 1506)

```python
self.aw.ser.SP.write(str2cmd('PID;ON\n'))
```

### PID;OFF (line 1558)

```python
self.aw.ser.SP.write(str2cmd('PID;OFF\n'))
```

### PID;SV (line 1757)

```python
self.aw.ser.SP.write(str2cmd('PID;SV;' + str(sv) + '\n'))
```

### PID;T — Set Parameters (line 1906)

```python
self.aw.ser.SP.write(str2cmd('PID;T;' + str(kp) + ';' + str(ki) + ';' + str(kd) + '\n'))
```

### PID;CHAN — Set Input Channel (line 1910)

```python
if source is not None and source in {1, 2, 3, 4}:
    libtime.sleep(.03)
    self.aw.ser.SP.write(str2cmd('PID;CHAN;' + str(source) + '\n'))
```

### PID;CT — Set Cycle Time (line 1913)

```python
if cycle is not None:
    libtime.sleep(.03)
    self.aw.ser.SP.write(str2cmd('PID;CT;' + str(cycle) + '\n'))
```

### PID;LIMIT — Set Output Limits (line 1505)

```python
duty_min = min(100, max(0, self.dutyMin))
duty_max = min(100, max(0, self.dutyMax))
self.aw.ser.SP.write(str2cmd('PID;LIMIT;' + str(duty_min) + ';' + str(duty_max) + '\n'))
```

---

## TC4 Shield Firmware Reference

Source: https://github.com/greencardigan/TC4-shield/blob/master/applications/Artisan/aArtisan/trunk/src/aArtisan/commands.txt

### Command Format

```
COMMAND;param1;param2;...\n
```

- Command name: first 5 characters significant
- Parameter delimiter: semicolon (`;`), comma (`,`), or space (` `)
- Line terminator: newline (`\n`)
- 1-based port indexing

### Known Commands

| Command | Parameters | Description |
|---------|-----------|-------------|
| `READ` | none | Request temperature data |
| `CHAN;` | ijkl | Set active channels (0=off, 1-4=TC1-4) |
| `UNITS;` | C or F | Set temperature units |
| `FILT;` | f1,f2,f3,f4 | Set filter values |
| `PID;ON` | none | Enable PID control |
| `PID;OFF` | none | Disable PID control |
| `PID;SV;` | setpoint | Set PID setpoint |
| `PID;T;` | kp;ki;kd | Set PID parameters |
| `PID;CHAN;` | channel | Set PID input channel |
| `PID;CT;` | milliseconds | Set PID cycle time |
| `PID;LIMIT;` | min;max | Set PID output limits |
| `OT1` | 0-100 | Heater power % |
| `OT2` | 0-100 | Fan speed % |
| `IO3` | 0-100 | Alternative fan speed % |

---

## LibreRoaster Compatibility Matrix

Based on this analysis, here's what LibreRoaster needs to implement for full Artisan compatibility:

### Must Have (connection won't work without these)

| Requirement | Status | Notes |
|-------------|--------|-------|
| CHAN response starts with `#` | ❓ Check format_chan_ack() | Artisan checks `startswith('#')` |
| UNITS response starts with `#` | ❌ Currently sends `"OK"` | Must change to `"# OK"` or similar |
| FILT response starts with `#` | ❌ Currently sends `"OK"` | Must change to `"# OK"` or similar |
| READ response: comma-separated | ✅ | Correct format |
| READ field order: ambient,ET,BT | ✅ | Changed to AMB,ET,BT,0.0,0.0 |
| Parse `PID;ON` (semicolon) | ❌ Parser uses comma | Must handle semicolon |
| Parse `PID;OFF` (semicolon) | ❌ Parser uses comma | Must handle semicolon |
| Parse `PID;SV;value` (semicolon) | ❌ Parser uses comma | Must handle semicolon |

### Nice to Have

| Feature | Artisan Support | Notes |
|---------|----------------|-------|
| PID;T params | ✅ Artisan sends it | Should parse and apply |
| PID;CHAN | ✅ Artisan sends it | Set PID input channel |
| PID;CT | ✅ Artisan sends it | Set cycle time |
| PID;LIMIT | ✅ Artisan sends it | Set output limits |
| OT1/OT2 manual control | ✅ Core feature | Already implemented |
