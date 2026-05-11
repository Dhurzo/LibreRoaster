# Artisan TC4 Protocol — Quick Reference Card

## Handshake (on connect)

```
Artisan sends:     CHAN;1200\n
Device responds:   # Active channels set to 1200\n

Artisan sends:     UNITS;C\n
Device responds:   # OK\n

Artisan sends:     FILT;70,70,70,70\n
Device responds:   # OK\n
```

⚠️ **All responses MUST start with `#`** — Artisan checks `result.startswith('#')`.

## Temperature Polling (loop)

```
Artisan sends:     READ\n
Device responds:   25.0,185.3,201.4,0.0,0.0\n
                    ↑      ↑     ↑     ↑    ↑
                  ambient  ET    BT   ch3  ch4
```

3-value format (CHAN;1200):    `ambient,ET,BT`
5-value format (CHAN;1234):    `ambient,ET,BT,ch3,ch4`
PID ON adds:                   `...,...,...,...,...,heater,fan,SV`

## PID Commands (semicolon delimiter!)

```
PID;ON\n              → Enable PID
PID;OFF\n             → Disable PID
PID;SV;150\n          → Set setpoint 150°C
PID;T;2.0;0.5;1.0\n   → Set Kp, Ki, Kd
PID;CHAN;2\n          → PID input = channel 2 (BT)
PID;CT;1000\n         → Cycle time 1000ms
PID;LIMIT;0;100\n     → Output limits 0-100%
```

⚠️ **Delimiter is `;` (semicolon), NOT `,` (comma)!**

## Manual Control

```
OT1 75\n    → Heater 75%
OT2 50\n    → Fan 50%
IO3 50\n    → Fan 50% (alternative)
```

## Line Endings

- Commands end with `\n`
- Responses end with `\r\n` (Artisan strips last 2 chars with `[:-2]`)
- Baud: 115200

## CHAN Channel Map

| Digit | Meaning |
|-------|---------|
| 0     | Inactive |
| 1     | TC1 (thermocouple 1) |
| 2     | TC2 (thermocouple 2) |
| 3     | TC3 |
| 4     | TC4 |

Example: `CHAN;1200` = TC1→ET, TC2→BT, ch3=off, ch4=off
