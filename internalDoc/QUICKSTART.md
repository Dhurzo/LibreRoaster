# LibreRoaster Quick Start Guide

## What You Need

- [ ] ESP32-C3 development board
- [ ] USB-C cable (data-capable)
- [ ] Computer (Windows/Mac/Linux)
- [ ] Artisan software (download from artisan-roasterscope.org)

## Step 1: Connect

Plug the ESP32-C3 into your computer's USB port.

The board's LED should light up (pattern varies by board).

## Step 2: Flash

**Download the firmware:**

Get `libreroaster.bin` from the releases page.

**Using espflash GUI (recommended):**

1. Download espflash from https://github.com/esp-rs/espflash/releases
2. Open espflash
3. Select your USB port
4. Click "Flash" and select `libreroaster.bin`

**Using command line:**

```bash
espflash flash libreroaster.bin --monitor
```

## Step 3: Verify

After flashing, you should see:
- Board LED indicating activity
- Serial output showing "LibreRoaster started"

To monitor output:
```bash
espflash monitor --baud 115200
```

Press RESET on the board to see boot messages.

## Step 4: Connect to Artisan

1. Open Artisan
2. Go to **Config → Device**
3. Select your USB port (e.g., COM3 on Windows, /dev/ttyUSB0 on Linux)
4. Set **Baud Rate**: 115200
5. Enable **Arduino/RPi** mode
6. Click **Connect**

## What You'll See

After connecting, Artisan should show:
- Real-time temperature graphs (ET, BT)
- Controls for heater (OT1) and fan (IO3)
- Event buttons (START, STOP)

[IMAGE PLACEHOLDER: Artisan connected to LibreRoaster]

## Commands Reference

| Command | Action |
|---------|--------|
| READ | Get current temperatures and readings |
| OT1 XX | Set heater to XX% (0-100) |
| IO3 XX | Set fan to XX% (0-100) |
| START | Start roast (motor on) |
| STOP | Stop roast (motor off) |

## Problems?

- **Board not detected?** Try a different USB cable or port
- **Flash failed?** Hold BOOT + press RESET, then try again
- **Artisan won't connect?** Verify baud rate is 115200

See FLASH_GUIDE.md for detailed troubleshooting.
