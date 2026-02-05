# LibreRoaster Troubleshooting Guide

Quick reference for diagnosing and resolving common connection issues between LibreRoaster and Artisan.

## USB Connection Issues

### Device Not Detected

**Symptoms:**
- Artisan shows "No device found" or "Device not responding"
- Device doesn't appear in Artisan's port dropdown
- USB CDC virtual COM port missing from system

**Diagnosis Steps:**
1. Check if device is plugged in and powered on (LED indicators)
2. Open Device Manager (Windows) or System Information (macOS/Linux)
3. Look for USB Serial Device or similar under Ports (COM & LPT)
4. Try a different USB cable (some cables are charge-only)

**Solutions:**
- Install CDC driver: On Windows, ensure USB Serial CDC driver is installed
- Try different USB port (direct to motherboard, not through hub)
- Press reset button on device after connecting
- Check CDC settings in firmware configuration

### COM Port Conflicts

**Symptoms:**
- Device appears but Artisan can't connect
- "Port already in use" error
- Intermittent disconnections

**Diagnosis Steps:**
1. Open Device Manager (Windows) or list `/dev/tty*` devices (Linux/macOS)
2. Check which applications have the port open
3. Note the specific COM port number

**Solutions:**
- Close other applications using the port (serial monitors, IDEs)
- Disconnect and reconnect the device to get a new COM port
- Use `mode` command (Windows) to release the port
- Restart Artisan after closing other serial applications

### Baud Rate Mismatch

**Symptoms:**
- Artisan connects but shows no data
- Garbage characters in serial terminal
- Connection timeout errors

**Diagnosis Steps:**
1. Verify Artisan baud rate setting matches device configuration
2. Default LibreRoaster baud rate: 115200
3. Check UART_LOGGING_GUIDE.md for device baud rate configuration

**Solutions:**
- Set Artisan baud rate to 115200 (default)
- If device was configured differently, update Artisan settings
- Verify device firmware configuration hasn't been changed

---

## UART Connection Issues

### Resource Conflicts

**Symptoms:**
- "Permission denied" when accessing serial port
- "Device or resource busy" error
- Can't open port after previous session didn't close properly

**Diagnosis Steps:**
1. Check if another process is using the port: `lsof | grep tty` (Linux/macOS)
2. Check Device Manager for phantom COM ports
3. Verify user has dialout/tty group permissions (Linux)

**Solutions:**
- Kill any lingering processes using the port
- Add user to dialout group: `sudo usermod -a -G dialout $USER`
- Log out and log back in for group changes to take effect
- Use `fuser -k /dev/ttyUSB0` to release locked port (use with caution)

---

## Artisan Communication Issues

### Connection Drops

**Symptoms:**
- Artisan shows "Device disconnected" during roast
- Readings freeze or become stale
- Intermittent "not responding" status

**Diagnosis Steps:**
1. Monitor UART logs for connection state messages
2. Check for `[SYSTEM] USB connected` and `[SYSTEM] USB disconnected` events
3. Verify Artisan Heartbeat setting is enabled
4. Check USB cable quality and connection firmness

**Solutions:**
- Use high-quality USB cable with data lines (not charge-only)
- Reduce cable length if possible
- Disable USB power saving in system BIOS
- Increase Artisan timeout settings if needed
- Keep USB hub usage to minimum
- See UART_LOGGING_GUIDE.md for detailed log analysis

### Event Sync Problems

**Symptoms:**
- Roast events (FC, SC) not registering correctly
- Button presses not reflected in Artisan
- State changes not synchronized between device and Artisan

**Diagnosis Steps:**
1. Check UART logs for event messages during roast
2. Verify Artisan event mapping configuration
3. Confirm device firmware supports event sync

**Solutions:**
- Configure Artisan event buttons to match device capabilities
- Check event mapping in Artisan → Config → Events
- Review UART logs for event transmission confirmation
- See ARTISAN_CONNECTION.md for proper event configuration

### Configuration Mismatch

**Symptoms:**
- Artisan receives unexpected characters or values
- Temperature readings appear as `-273.2` (disconnected sensor)
- SET commands don't change heater/fan values
- Protocol errors in Artisan

**Diagnosis Steps:**
1. Compare Artisan serial settings with device defaults
2. Check for extra characters in log output
3. Verify baud rate, data bits, stop bits, parity match
4. Review `[USB]` channel logs for malformed commands

**Solutions:**
- Set Artisan serial port to: 115200 baud, 8N1 (8 data bits, no parity, 1 stop bit)
- Disable additional serial options (flow control, etc.)
- Match protocol version between Artisan and device
- Check Artisan Extra Characters field is empty
- Review ARTISAN_CONNECTION.md for complete configuration guide
- See UART_LOGGING_GUIDE.md for interpreting USB channel messages

---

## Quick Diagnostic Steps

1. **Start with UART logs** - Connect serial terminal at 115200 baud
2. **Check for `[SYSTEM]` messages** - Confirm device is operational
3. **Look for `[USB]` traffic** - Verify Artisan commands are arriving
4. **Monitor `[ERROR]` messages** - Identify specific failure points
5. **Compare working vs. failing** - Note differences in configuration

For detailed log interpretation, see `UART_LOGGING_GUIDE.md`.

---

## Common Error Patterns

| Error Pattern | Likely Cause | Check |
|---------------|--------------|-------|
| No COM port appears | Driver issue or bad cable | Device Manager, try different cable |
| Connects but no data | Wrong baud rate | Set to 115200 |
| Stale readings | USB connection unstable | Check cable quality |
| -273.2°C readings | Sensor disconnected | Physical inspection |
| Garbage characters | Baud rate mismatch | Verify 115200 |
| Permission denied | Group membership | Add to dialout group |

---

*For firmware flashing instructions, see FLASH_GUIDE.md.*
*For complete Artisan setup, see ARTISAN_CONNECTION.md.*
