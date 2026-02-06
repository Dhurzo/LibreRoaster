// Dual-channel Artisan communication verification
// This file documents what needs to be tested and provides test patterns
// that can be run when hardware is available.

# Dual-Channel Artisan Communication Test Plan

## Tests to Run on Hardware

### Test 1: Basic USB Connection
```
1. Flash firmware to ESP32-C3
2. Connect USB cable to native USB port
3. Open terminal: minicom -D /dev/ttyACM0 -b 115200
4. Send: READ\r
5. Expected: 120.3,150.5,75.0,25.0 (values may vary)
```

### Test 2: UART Connection
```
1. Connect USB-UART adapter to GPIO20 (TX) and GPIO21 (RX)
2. Open terminal to the UART port (e.g., /dev/ttyUSB0)
3. Send: READ\r
4. Expected: Same response as USB
```

### Test 3: Channel Priority
```
1. Connect to USB, send READ → works
2. Send command on UART
3. Check logs: "Ignoring artisan command on UART, active channel is USB"
```

### Test 4: Channel Switch
```
1. Connect only to UART, send READ → works
2. Logs show: "Artisan command received on UART, switching active channel to UART"
```

### Test 5: Timeout (60 seconds)
```
1. Send command via USB
2. Wait 60 seconds without sending anything
3. Check logs: "No artisan commands for 60s, switching active channel to None"
4. Send READ → should work again
```

## Expected Log Output

```
[INFO] Artisan command received on USB, switching active channel to USB
[INFO] No artisan commands for 60s, switching active channel to None
[INFO] Ignoring artisan command on UART, active channel is USB
```

## Command Reference

| Command | Action | Expected Response |
|---------|--------|-------------------|
| READ | Read temps | `ET,BT,Power,Fan` |
| START | Start PID | No response (enables streaming) |
| OT1 50 | Heater 50% | No response |
| IO3 30 | Fan 30% | No response |
| STOP | Emergency | No response |
| BOGUS | Unknown | `ERR unknown_command BOGUS` |

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
