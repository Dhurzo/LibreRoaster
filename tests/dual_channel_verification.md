// Dual-channel Artisan communication verification
// This file documents what needs to be tested and provides test patterns
// that can be run when hardware is available.

# Dual-Channel Artisan Communication Test Plan

LibreRoaster accepts Artisan commands on two transports — native USB CDC
(`/dev/ttyACM0`) and UART0 (`GPIO20`/`GPIO21`). The command multiplexer
(`src/input/multiplexer.rs`) lets **one** of them own the Artisan session at a
time so the two wires cannot interleave responses or commands.

Session rules:

- The **first syntactically-valid command** on either wire claims the session
  (`Usb` or `Uart`). Only valid commands activate a channel — parse garbage is
  dropped before the gate.
- Commands arriving on the **other** wire are refused: the log reads
  `Ignoring artisan command on <channel>, active channel is <channel>`, and the
  active host receives `ERR command_ignored_inactive_channel` (coalesced to at
  most once per second).
- After `IDLE_TIMEOUT_SECS` (60 s) without a command, the control loop resets
  the active channel to `None`; the next valid command on either wire claims
  the session again. This is the anti-hijack recovery window.
- Output (READ/STATUS responses, `#` telemetry, `ERR` lines) is routed **only**
  to the active transport. While no channel is active, output goes nowhere.

## Tests to Run on Hardware

### Test 1: Basic USB Connection
```
1. Flash firmware to ESP32-C3
2. Connect USB cable to native USB port
3. Open terminal: minicom -D /dev/ttyACM0 -b 115200
4. Send: READ\r
5. Expected: AMB,ET,BT,0.0,0.0 (5 TC4 fields; values vary)
   e.g. 25.0,120.3,150.5,0.0,0.0
```
Note: with the firmware PID active (`PID;ON`) READ returns the 8-field TC4
form `AMB,ET,BT,0.0,0.0,heater,fan,SV` (heater/fan are percentages, never
°F-converted; AMB is always the cold-junction temperature in °C).

### Test 2: UART Connection
```
1. Connect USB-UART adapter to GPIO20 (TX) and GPIO21 (RX)
2. Open terminal to the UART port (e.g., /dev/ttyUSB0)
3. Send: READ\r
4. Expected: Same response as USB
```

### Test 3: Channel Priority (first valid command claims the session)
```
1. Connect to USB, send READ → works; USB now owns the session
2. Send a valid command on UART (e.g. READ)
3. Check logs: "Ignoring artisan command on Uart, active channel is Usb"
4. The UART command is refused; the active host may see
   "ERR command_ignored_inactive_channel" at most once per second
```

### Test 4: Session Activation from an Idle Wire
```
1. Connect only to UART, send READ → works
2. Logs show: "Artisan command received on Uart, switching active channel to Uart"
3. Responses/telemetry are routed to UART only
```

### Test 5: Idle Release and Re-claim (60 seconds)
```
1. Send command via USB
2. Wait 60 seconds without sending anything
3. The control loop releases the session and logs:
   "No artisan commands for 60s, switching active channel to None"
4. Send READ again — on either USB or UART. That transport re-claims the
   session ("Artisan command received on ...") and READ works.
```

## Expected Log Output

```
[INFO] Artisan command received on Usb, switching active channel to Usb
[INFO] No artisan commands for 60s, switching active channel to None
[INFO] Ignoring artisan command on Uart, active channel is Usb
```

## Command Reference

| Command | Action | Expected Response |
|---------|--------|-------------------|
| READ | Read temperatures | `AMB,ET,BT,0.0,0.0` (8 fields when the PID is on: + `heater,fan,SV`) |
| START | Start the roast / hand control to the PID | No response; does **not** enable the `#` stream (use `STREAM;ON`) |
| STREAM;ON | Enable the spontaneous `#` telemetry stream | `#OK` |
| OT1 50 | Heater 50% | No response |
| IO3 30 | Fan 30% | No response |
| STOP | Emergency stop (heater 0, fan 100%) | No response |
| BOGUS | Unknown | `ERR unknown_command unknown_command` |

## Artisan Software Setup

1. Open Artisan
2. Config → Device
3. Type: Arduino/TC4
4. Port: /dev/ttyACM0 (USB) or /dev/ttyUSBx (UART)
5. Baudrate: 115200
6. Connect
7. Verify temperatures appear

## Troubleshooting

### Device not appearing
- Check cable (some are charge-only)
- Try different USB port
- Check `dmesg | tail` for USB enumeration

### Permission denied
- `sudo usermod -a -G dialout $USER`
- Log out and back in

### No response to commands
- Verify CR (\\r) is sent, not just LF
- Check baudrate is 115200
- Check if another app is using the port
- If the other transport already owns the session, either keep using it or
  wait out the 60 s idle window so your wire can claim the session
