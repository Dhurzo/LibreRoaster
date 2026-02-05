# LibreRoaster Artisan Connection Guide

> **Analysis (2026-02-05):** Key connection facts for roasters.
> - USB CDC: Native ESP32-C3 USB, appears as /dev/ttyACM0 (Linux) or COM port (Windows)
> - UART0: GPIO20 TX → Host RX, GPIO21 RX → Host TX, 115200 baud, 8N1
> - Channel multiplexer: First command wins, 60-second timeout
> - READ response: ET,BT,ET2,BT2,ambient,fan,heater (7 values)
> - Handshake disabled: Commands work immediately after connection

## Overview

This guide explains how to connect your flashed LibreRoaster ESP32-C3 board to Artisan coffee roasting software. After following this guide, you'll have a stable connection for reading temperatures and controlling your roaster during roast sessions.

**Time required:** 5-10 minutes

**What you'll achieve:**
- Stable bidirectional communication with Artisan
- Real-time temperature display (ET and BT)
- Control over heater and fan output

---

## Two Ways to Connect

LibreRoaster supports two connection methods to Artisan:

### USB CDC (Native USB) — Recommended for Most Users

| Pros | Cons |
|------|------|
| No external adapter needed | May require USB driver installation |
| Simple single-cable connection | Some boards have driver issues |
| Powers the board directly | |

### UART0 (GPIO20/21) — For Advanced Users

| Pros | Cons |
|------|------|
| Reliable standard serial connection | Requires USB-to-UART adapter |
| Works on all computers | Additional wiring required |
| No driver issues | Extra cable and power needed |

---

## USB CDC Connection

### Step 1: Identify the Port

**Windows:**
1. Open Device Manager (right-click Start → Device Manager)
2. Expand "Ports (COM & LPT)"
3. Look for "USB Serial Port (COMx)" — remember the COM number

[IMAGE PLACEHOLDER: Device Manager showing USB Serial Port]

**Mac:**
1. Open System Information (⌘ + Space → "System Information")
2. Select "USB" from the sidebar
3. Look for "USB Serial JTAG" or similar device

**Linux:**
```bash
ls /dev/ttyACM*
# Should show something like /dev/ttyACM0
```

### Step 2: Configure Artisan

[IMAGE PLACEHOLDER: Artisan Device Configuration dialog]

1. Open Artisan
2. Go to **Config → Device**
3. Select your USB port (e.g., COM3 on Windows, /dev/ttyACM0 on Linux)
4. Set **Baud Rate**: 115200
5. Enable **Arduino/RPi** mode
6. Optional: Enable **Extra: "机电"** if using certain Artisan modes
7. Click **Connect**

### Step 3: Verify Connection

After connecting, Artisan should:
- Show "Connected" in the status bar
- Display real-time temperature updates
- Enable control sliders

---

## UART0 Connection

### Required Hardware

- USB-to-UART adapter (common options: CH340, CP2102, FTDI)
- 3 jumper wires
- Breadboard or soldering equipment

### Wiring Diagram

```
USB-to-UART Adapter          ESP32-C3 Board
┌─────────────┐              ┌─────────────┐
│     TX      │─────────────►│  GPIO 21    │  (RX)
│     RX      │◄─────────────│  GPIO 20    │  (TX)
│     GND     │─────────────►│   GND       │
└─────────────┘              └─────────────┘
```

### Step 1: Connect the Wires

1. Connect Adapter TX → ESP32-C3 GPIO 21
2. Connect Adapter RX → ESP32-C3 GPIO 20
3. Connect Adapter GND → ESP32-C3 GND

**Important:** Do NOT connect VCC (power) — the ESP32-C3 should be powered separately via USB-C.

### Step 2: Identify the Adapter Port

**Windows:** Check Device Manager → Ports (COM & LPT) → "USB Serial Port (COMx)"

**Mac:** Check System Information → USB → look for adapter name

**Linux:**
```bash
ls /dev/ttyUSB*
# or
ls /dev/ttyUSB*
```

### Step 3: Configure Artisan

1. Open Artisan
2. Go to **Config → Device**
3. Select your adapter's port (e.g., COM3, /dev/ttyUSB0)
4. Set **Baud Rate**: 115200
5. Enable **Arduino/RPi** mode
6. Click **Connect**

---

## Verifying Connection

### What You Should See

After successfully connecting:

1. **Artisan Status Bar** — Shows "Connected" and the active port

2. **Scope Area** — Real-time temperature curves should appear:
   - **ET** (Environment Temperature) — Your roaster's temperature sensor
   - **BT** (Bean Temperature) — The beans' temperature

[IMAGE PLACEHOLDER: Artisan scope showing ET and BT curves]

3. **Control Panel** — Sliders for:
   - **OT1** — Heater output percentage
   - **IO3** — Fan speed percentage

### Testing with READ Command

Send a READ command to verify bidirectional communication:

1. In Artisan, find the command terminal or send raw commands
2. Type: `READ\r`
3. You should receive a response like:
   ```
   185.2,192.3,-1,-1,24.5,45,75
   ```

**Response format:**
| Field | Value | Description |
|-------|-------|-------------|
| ET | 185.2 | Environment temperature (°C) |
| BT | 192.3 | Bean temperature (°C) |
| ET2 | -1 | Extra channel (unused) |
| BT2 | -1 | Extra channel (unused) |
| ambient | 24.5 | Ambient temperature |
| fan | 45 | Fan output % |
| heater | 75 | Heater output % |

### Visual Indicators in Artisan

| Indicator | Meaning |
|-----------|---------|
| Connected (green) | Stable connection established |
| Disconnected (red) | Connection lost or failed |
| Flashing values | Active data streaming |
| Grayed-out controls | Not in roasting mode (send START) |

---

## Troubleshooting

### "Port Not Appearing"

**USB CDC:**
1. Try a different USB cable (data-capable, not charge-only)
2. Try a different USB port on your computer
3. Install USB drivers:
   - **CP2102**: Download from Silicon Labs
   - **CH9102**: Download from manufacturer
4. Press the RESET button on the ESP32-C3

**UART0:**
1. Verify adapter is recognized in Device Manager/System Information
2. Try a different USB port for the adapter
3. Check all wiring connections
4. Verify TX/RX are not swapped

### "Artisan Can't Connect"

1. Close Artisan completely, then reopen
2. Verify no other application is using the port
3. Check baud rate is set to 115200
4. Ensure Arduino/RPi mode is enabled
5. Try a different port

### "Connection Works But Data Looks Wrong"

1. Verify baud rate is 115200
2. Check for extra characters in serial monitor
3. Ensure no extra spaces or newlines in commands
4. Try sending commands manually

### "Dropped Connections"

1. USB CDC: May indicate power issues — use powered USB hub
2. UART0: Check all wire connections
3. Ensure 60-second timeout isn't triggering
4. Reduce cable length for UART connections

### Channel Multiplexer Issues

LibreRoaster uses a channel multiplexer that:
- Accepts commands from USB CDC and UART0 independently
- Ignores commands on inactive channels
- Resets to "none" after 60 seconds of no commands

**Symptoms:**
- Commands sent on one channel don't respond
- Connection appears to stop after 60 seconds

**Solution:**
1. Send a command on the active channel
2. Or wait for Artisan's keepalive to maintain connection
3. Artisan typically sends periodic commands during roasting

---

## Next Steps

Now that you're connected, learn how to control your roaster:

- **READ** — Get current temperatures
- **OT1 XX** — Set heater to XX%
- **IO3 XX** — Set fan to XX%
- **START** — Begin roasting (enables continuous output)
- **STOP** — Emergency stop (disables outputs)

**See also:** [Command Reference](PROTOCOL.md) for complete command documentation.

---

## Reference

| Setting | Value |
|---------|-------|
| Connection Type | USB CDC (native) or UART0 |
| Baud Rate | 115200 |
| Data Bits | 8 |
| Stop Bits | 1 |
| Parity | None |
| Flow Control | None |
| Commands | ASCII terminated with CR (\\r) |

---

*See also: [FLASH_GUIDE.md](FLASH_GUIDE.md) for flashing instructions*
