# LibreRoaster Flash Guide

> **Analysis (2026-02-05):** Original guide assumed developer knowledge. This version removes CLI complexity, adds visual placeholders, simplifies USB port identification, and prioritizes GUI options for roasters.

## Overview

This guide explains how to flash (install) the LibreRoaster firmware onto your ESP32-C3 development board. After flashing, your board will communicate with Artisan coffee roasting software.

**Time required:** 5-10 minutes

**What you'll need:**
- ESP32-C3 development board
- USB-C cable
- Computer (Windows, Mac, or Linux)

---

## Prerequisites

### Hardware

1. **ESP32-C3 Board** — Any ESP32-C3 development board works
2. **USB-C Cable** — Must support data transfer (not just charging)
3. **Computer** — With a USB port

### Software

**Option A: espflash GUI (Recommended for beginners)**

1. Download from: https://github.com/esp-rs/espflash/releases
2. Install the graphical version for your operating system
3. Open the application

**Option B: Command Line (for advanced users)**

If you have Rust installed:
```bash
cargo install espflash
```

### USB Drivers (if needed)

Some boards require USB drivers:
- **CP2102** — Common on many ESP32-C3 boards
- **CH9102** — Found on some newer boards

If your computer doesn't detect the board after connecting, install the appropriate driver from the manufacturer's website.

---

## Step-by-Step

### Step 1: Connect the Board

[IMAGE PLACEHOLDER: ESP32-C3 board connected via USB-C cable]

1. Plug the ESP32-C3 into your computer's USB port
2. The board's LED may light up (varies by board)
3. If prompted, allow the device to be recognized

### Step 2: Find Your USB Port

**Windows:**
1. Open Device Manager (right-click Start → Device Manager)
2. Look under "Ports (COM & LPT)"
3. You should see something like "USB Serial Port (COM3)"
4. Remember the COM number (e.g., COM3)

**Mac:**
1. Open System Information (⌘ + Space → "System Information")
2. Select "USB" from the sidebar
3. Look for "USB to UART" or similar device

**Linux:**
```bash
ls /dev/ttyUSB*
# or
ls /dev/ttyACM*
```

If no port appears, see Troubleshooting below.

### Step 3: Install Flashing Tool

**Using espflash GUI:**

[IMAGE PLACEHOLDER: espflash GUI main window]

1. Open espflash GUI
2. The application should auto-detect your board
3. If not, click "Select Device" and choose your USB port

**Using Command Line:**

```bash
espflash board-info
```

This verifies your computer can communicate with the board.

### Step 4: Flash the Firmware

**Using espflash GUI:**

[IMAGE PLACEHOLDER: espflash GUI with firmware selected]

1. Click "Flash Firmware"
2. Select the firmware file: `libreroaster.bin`
3. Click "Flash"
4. Wait for the process to complete

**Using Command Line:**

```bash
espflash flash libreroaster.bin --monitor
```

The `--monitor` flag opens the serial monitor after flashing, showing output from the board.

### Step 5: Verify It Worked

After flashing completes successfully, you should see:

1. **LED Indicators** — Board LEDs show activity (pattern varies by board)
2. **Serial Output** — Messages like "LibreRoaster started"
3. **Artisan Connection** — Board is ready to communicate

[IMAGE PLACEHOLDER: Serial monitor showing successful boot]

---

## Troubleshooting

### "Device Not Detected"

[IMAGE PLACEHOLDER: Device Manager showing unknown device]

**Check:**
- Try a different USB cable (some cables are charge-only)
- Try a different USB port on your computer
- Install USB drivers (see Prerequisites)

**Recovery:**
1. Hold the BOOT button on the board
2. While holding BOOT, press and release RESET
3. Release BOOT
4. The board should enter bootloader mode

### "Flash Failed"

**Try:**
1. Erase and reflash:
   ```
   espflash erase-flash
   espflash flash libreroaster.bin
   ```

2. Flash at slower speed:
   ```
   espflash flash libreroaster.bin --baud 115200
   ```

### "Nothing Happens After Flash"

1. Open the serial monitor:
   ```
   espflash monitor --baud 115200
   ```
2. Press the RESET button on the board
3. Watch for output
4. If no output appears, see "Device Not Detected"

---

## Next Steps

**Congratulations! Your LibreRoaster is flashed.**

1. **Connect to Artisan** — See the Artisan Connection Guide
2. **Configure Port** — Select the USB port where your board appears
3. **Set Baud Rate** — Use 115200
4. **Start Roasting** — Your board is ready

**Need help?** See the Troubleshooting section above or the main troubleshooting guide.

---

## Reference

| Setting | Value |
|---------|-------|
| Baud Rate | 115200 |
| Data Bits | 8 |
| Stop Bits | 1 |
| Parity | None |
| Flow Control | None |

---

*For developers compiling from source, see DEVELOPMENT.md*
