# Artisan Connection Guide

This guide provides detailed instructions for connecting LibreRoaster to Artisan coffee roaster software.

## Connection Overview

LibreRoaster supports two connection methods to communicate with Artisan:

| Method | Description | Requirements |
|--------|-------------|--------------|
| **USB CDC** | Native ESP32-C3 USB (recommended) | USB cable, no adapter needed |
| **UART0** | GPIO20/21 at 115200 baud | USB-to-UART adapter |

## Connection Method 1: USB CDC (Recommended)

### Prerequisites

- ESP32-C3 board with flashed LibreRoaster firmware
- USB cable (data-capable, not power-only)
- Computer running Artisan

### Connection Steps

1. **Connect the Board**
   - Connect ESP32-C3 to your computer via USB
   - The board will enumerate as a CDC device

2. **Identify the Port**
   - **Linux:** `/dev/ttyACM0` or `/dev/ttyUSB0`
   - **macOS:** `/dev/cu.usbmodem-*` or `/dev/cu.usbserial-*`
   - **Windows:** `COM3`, `COM4`, etc. (check Device Manager)

3. **Verify Port Detection**
   ```bash
   # Linux
   ls -la /dev/ttyACM*
   
   # macOS
   ls /dev/cu.usbmodem-*
   
   # Or use espflash
   espflash list
   ```

## Connection Method 2: UART0

### Prerequisites

- ESP32-C3 board with flashed LibreRoaster firmware
- USB-to-UART adapter (e.g., FT232, CP2102)
- 3 jumper wires (TX, RX, GND)

### Pinout Connections

| ESP32-C3 Pin | Function | USB-UART Adapter |
|--------------|----------|------------------|
| GPIO20 | UART TX | RX |
| GPIO21 | UART RX | TX | 
| GND | Ground | GND |

### Connection Steps

1. **Wire the Connections**
   - Connect ESP32-C3 GPIO20 to adapter's RX
   - Connect ESP32-C3 GPIO21 to adapter's TX
   - Connect ESP32-C3 GND to adapter's GND

2. **Identify the Port**
   - Use the port assigned to your USB-to-UART adapter

## Artisan Configuration

### Basic Setup

1. **Open Artisan**
   - Launch Artisan on your computer

2. **Device Settings**
   - Go to **Config** → **Device**
   - Select device: **Arduino/RPi** (or similar USB-serial option)

3. **Port Configuration**
   - Select the identified port from the dropdown
   - Set baud rate: **115200**
   - Data bits: 8
   - Stop bits: 1
   - Parity: None

4. **Mode Selection**
   - Choose **Arduino/RPi** mode
   - This protocol is compatible with LibreRoaster's output format

### Advanced Settings (Optional)

#### Command Timing

| Setting | Recommended Value |
|---------|------------------|
| Read interval | 1-2 seconds |
| Command delay | 100ms |

#### Startup Commands

LibreRoaster supports Artisan's initialization sequence but works without it:

```
Optional: Artisan sends CHAN → UNITS → FILT
LibreRoaster responds with # acknowledgment
```

## Connection Verification

### Test the Connection

1. **Start Roasting Mode**
   - In Artisan, start a new roasting session
   
2. **Send READ Command**
   - Type `READ` in the command area (if available)
   - Or wait for periodic readings

3. **Expected Response**
   
   You should see temperature data in this format:
   
   ```
   ET,BT,HEATER,FAN
   ```
   
   Example: `185.3,201.4,45,80`
   
   | Field | Description | Unit |
   |-------|-------------|------|
   | ET | Exhaust Temperature | °C |
   | BT | Bean Temperature | °C |
   | HEATER | Heater PWM percentage | % |
   | FAN | Fan PWM percentage | % |

### Verify in Artisan UI

- Temperature readings should appear in the ET and BT curves
- Heater and fan sliders should respond
- Data should update at regular intervals

## Troubleshooting

### Connection Issues

#### Port Not Found

- **Linux:** Add user to dialout group: `sudo usermod -a -G dialout $USER`
- Check USB cable is data-capable
- Try a different USB port
- Verify device is detected: `ls /dev/ttyACM*`

#### Artisan Can't Connect

- Close other programs that might be using the port
- Check baud rate is set to 115200
- Verify correct port is selected
- Try disconnecting and reconnecting the device

### Data Issues

#### No Temperature Reading

- Verify firmware is flashed correctly
- Check thermocouple connections
- Use serial monitor to verify raw sensor data

#### Erratic Readings

- Check for loose connections
- Verify power supply is stable
- Check for electrical noise

#### Heater/Fan Not Responding

- Verify SSR and fan are properly connected
- Check GPIO pin connections
- Verify power to heater/fan

### Serial Monitor Debug

Use the serial monitor to verify communication:

```bash
# Using espflash
espflash monitor --speed 115200

# Or using cargo
cargo espflash monitor --speed 115200
```

You should see:
- Initialization messages
- READ command responses in CSV format

## Supported Artisan Commands

LibreRoaster supports these Artisan commands:

| Command | Description | Example |
|---------|-------------|---------|
| `READ` | Request telemetry | Returns: `ET,BT,HEATER,FAN` |
| `STATUS`/`STAT` | Automation telemetry snapshot | Returns 18‑field CSV with safety metrics |
| `REG` | Over‑temperature regression trigger | `REG` (logs `SAFETY OT-REGRESSION`) |
| `OT1 [0-100]` | Set heater % | `OT1 50` |
| `OT2 [0-100]` | Set fan % (auto-cuts heater) | `OT2 80` |
| `IO3 [0-100]` | Set fan % | `IO3 60` |
| `UP` | Increase heater 5% | `UP` |
| `DOWN` | Decrease heater 5% | `DOWN` |
| `START` | Begin roasting | `START` |
| `STOP` | Emergency stop | `STOP` |
| `CHAN [rate]` | Set rate (legacy) | `CHAN 1` |
| `UNITS [C/F]` | Set units | `UNITS C` |
| `FILT [value]` | Set filter (legacy) | `FILT 1` |

## Quick Reference

| Setting | Value |
|---------|-------|
| Baud rate | 115200 |
| Mode | Arduino/RPi |
| Protocol | CSV (ET,BT,HEATER,FAN) |
| Units | Celsius (configurable) |

---

For more information, see [README.md](../README.md).
