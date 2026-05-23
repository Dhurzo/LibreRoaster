# Connection Types: USB vs UART

**Last updated:** 2026-05-23 — verified on real ESP32-C3 hardware

This document explains the two connection methods for communicating with LibreRoaster from Artisan, why **USB is the recommended default**, and how GPIO9 affects boot reliability.

---

## 1. Quick Decision

| You want to… | Use this |
|--------------|----------|
| Plug and play, no extra hardware | **USB (native)** — just a USB cable |
| Use an existing USB-UART adapter (CH340/CP2102) | **UART** |
| Guaranteed boot every time | Either (see §4.1: official dev board vs custom) |
| Use an official ESP32-C3 dev board (DevKitC-02, DevKitM-1, RUST-1) | **No extra resistor** — GPIO9 pull-up already on board |
| Build a custom board with a bare ESP32-C3 module | **Add 10kΩ pull-up on GPIO9** |
| Flash firmware via serial | **USB** (recommended) or **UART** |

---

## 2. Native USB (recommended)

The ESP32-C3 has a built-in **USB Serial/JTAG** peripheral that appears as a standard CDC ACM serial port on your PC.

| Property | Value |
|----------|-------|
| **Port (Linux)** | `/dev/ttyACM0` |
| **Port (Windows)** | `COM3` (or similar) |
| **Port (macOS)** | `/dev/cu.usbmodem*` |
| **Baud rate** | 115200 (virtual — USB CDC ignores baud) |
| **Cable** | USB-C to USB-A (or USB-C to USB-C) |
| **Extra hardware** | **None** |
| **Artisan config** | Serial port, TC4 protocol, 115200 baud |

### Verified on real hardware

```
rst:0x15 (USB_UART_CHIP_RESET), boot:0xc (SPI_FAST_FLASH_BOOT)
```

Boots from flash and responds to Artisan commands via USB CDC. Temperature telemetry, heater control (OT1), fan control (IO3), and PID toggling all confirmed working.

### Flashing over USB

```bash
cargo espflash flash --release --target riscv32imc-unknown-none-elf \
  --features embedded --port /dev/ttyACM0
```

Leave `--port` off to auto-detect — `espflash` prefers the native USB port.

---

## 3. UART (via CH340 / CP2102)

The ESP32-C3's UART0 is available on GPIO20 (RX) and GPIO21 (TX) for connection to an external USB-UART adapter.

| Property | Value |
|----------|-------|
| **Port (Linux)** | `/dev/ttyUSB0` |
| **Baud rate** | 115200 (real — UART uses this) |
| **Adapter** | CH340, CP2102, or similar **3.3V** USB-UART |
| **Extra hardware** | USB-UART adapter + wiring |

### Observed behavior (real hardware)

Without a pull-up resistor, GPIO9 is floating and its boot-time value is **unpredictable**:

```
# Sometimes HIGH → boot from flash ✅
rst:0x1 (POWERON), boot:0xc (SPI_FAST_FLASH_BOOT)

# Sometimes LOW → download mode ❌
rst:0x1 (POWERON), boot:0x4 (DOWNLOAD(USB/UART0/1))
waiting for download
```

### Flashing over UART

```bash
cargo espflash flash --release --target riscv32imc-unknown-none-elf \
  --features embedded --port /dev/ttyUSB0
```

Even if the chip boots into download mode, `espflash` can still connect and flash — the ROM download mode accepts flash commands over both UART0 and USB.

---

## 4. GPIO9: the key to reliable boot (both methods)

**GPIO9** is a strapping pin on the ESP32-C3. The ROM reads it at **every reset** (power-on, USB DTR, watchdog, etc.):

| GPIO9 at reset | Result |
|----------------|--------|
| HIGH ( ≈ 3.3V) | Boot from flash — normal operation |
| LOW ( ≈ 0V or floating) | Download mode — chip waits for serial flash |

### Why it matters

On a bare board with no pull-up or pull-down, GPIO9 is **floating** — a high-impedance input whose voltage depends on parasitic capacitance, leakage currents, and power-rail noise. The boot outcome is random.

We saw this in our tests:
- **USB test**: GPIO9 happened to float HIGH → `boot:0xc` → firmware ran ✅
- **UART test**: GPIO9 happened to float LOW → `boot:0x4` → download mode ❌

USB is **not** immune to this — it just happened to work during our tests because the pin floated HIGH at that moment.

### The fix: 10kΩ pull-up to 3.3V

```
GPIO9 ──── 10kΩ ──── 3.3V
```

This holds GPIO9 at a defined HIGH level during reset, making boot **deterministically reliable** with any connection method.

> ⚠️ **Not 10Ω!** A 10Ω resistor would draw 330mA and likely damage the GPIO pad. Use **10kΩ** (brown-black-orange).

### 4.1 Official dev boards vs custom boards

**Not all ESP32-C3 boards need an external pull-up.**

Official Espressif development boards already include a pull-up resistor on GPIO9:

| Board | GPIO9 pull-up | Extra resistor needed? |
|-------|---------------|----------------------|
| **ESP32-C3-DevKitC-02** | ✅ Built-in (via CP2102 + module) | ❌ No |
| **ESP32-C3-DevKitM-1** | ✅ Built-in (module-level) | ❌ No |
| **ESP32-C3-DevKit-RUST-1** | ✅ Built-in (module-level) | ❌ No |
| **ESP32-C3-DevKit-RUST-2** | ✅ Built-in (module-level) | ❌ No |
| **Bare ESP32-C3 module** (e.g. ESP32-C3-WROOM-02 on a custom PCB) | ❌ Floating | ✅ **10kΩ to 3.3V** |
| **LibreRoaster (custom board)** | ❌ Floating (fan on GPIO9) | ✅ **10kΩ to 3.3V** |

**Why the difference?** Official dev boards use an ESP32-C3 module (WROOM, MINI) whose substrate PCB includes the GPIO9 pull-up. When you buy a bare module or design a custom board, that pull-up isn't present — the pin is floating at reset.

**How to check your board:**
1. Look at the board documentation — most dev board guides mention the strapping pin
2. Measure GPIO9 with a multimeter at power-on — if you see 3.3V (high-Z), the pull-up is likely built-in
3. Run the boot test below: if boot is 100% reliable across many resets, you're fine without an extra resistor

> **For LibreRoaster specifically**, the external 10kΩ pull-up is required AND the fan driver circuit on GPIO9 must be high-impedance during boot so it doesn't override the strapping level.

### With the pull-up installed

| Connection | Boot |
|-----------|------|
| USB native | ✅ Always boots |
| UART (CH340) | ✅ Always boots |
| Either | ✅ Deterministic, no more "sometimes" |

---

## 5. Why USB is the recommended default

Even though both methods benefit from the pull-up resistor, USB has practical advantages:

| Advantage | USB | UART |
|-----------|-----|------|
| Cables needed | 1 (USB-C) | 2 (USB-UART + USB-C for power) |
| Baud rate config | Automatic | Must match 115200 |
| Flashing | Works even in download mode | Works even in download mode |
| Power delivery | 5V from PC → internal LDO → 3.3V | Depends on adapter |
| DTR reset | Built into USB Serial/JTAG | May need wiring to EN pin |

---

## 6. Both connections work simultaneously

The firmware runs USB CDC and UART transport tasks concurrently. You can connect both cables. Artisan will use whichever channel sends the first command. The `dual output task` routes formatted output to the active transport.

```
┌─────────────────────────────────────────────────┐
│                ESP32-C3                          │
│                                                  │
│  ┌─────────────┐         ┌──────────────────┐   │
│  │  USB CDC    │ ←────── │  PC (Artisan)    │   │
│  │  (native)   │         │  /dev/ttyACM0    │   │
│  └─────────────┘         └──────────────────┘   │
│                                                  │
│  ┌─────────────┐         ┌──────────────────┐   │
│  │  UART0      │ ←────── │  CH340 → PC      │   │
│  │  GPIO20/21  │         │  /dev/ttyUSB0    │   │
│  └─────────────┘         └──────────────────┘   │
└─────────────────────────────────────────────────┘
```

---

## 7. Artisan Configuration

### USB (recommended)

| Setting | Value |
|---------|-------|
| Connection | Serial port |
| Port | `/dev/ttyACM0` (Linux) / `COM3` (Windows) / `/dev/cu.usbmodem*` (macOS) |
| Baud rate | 115200 |
| Protocol | TC4 |
| DTR/RTS | Disabled (not needed — USB resets internally) |

### UART

| Setting | Value |
|---------|-------|
| Connection | Serial port |
| Port | `/dev/ttyUSB0` (Linux) / varies by adapter |
| Baud rate | 115200 |
| Protocol | TC4 |

---

## 8. Summary

| Feature | USB (native) | UART (CH340) |
|---------|-------------|--------------|
| Extra hardware | **None** | USB-UART adapter + wiring |
| GPIO9 pull-up needed (custom board)? | **Yes — 10kΩ to 3.3V** | **Yes — 10kΩ to 3.3V** |
| GPIO9 pull-up needed (official dev board)? | **No — already on board** | **No — already on board** |
| Boot without pull-up (custom board) | Sometimes works (floating = random) | Sometimes works (same) |
| Boot with pull-up | ✅ Always | ✅ Always |
| Flash firmware | ✅ | ✅ |
| Artisan communication | ✅ | ✅ |

### Your setup checklist

1. ✅ Flash firmware via USB: `cargo espflash flash --port /dev/ttyACM0`
2. ❓ **Check if you need the pull-up** — see §4.1 (official dev board: no resistor needed; custom board like LibreRoaster: add 10kΩ from GPIO9 to 3.3V)
3. ✅ Connect ESP32-C3 to PC via native USB
4. ✅ Configure Artisan: serial port → `/dev/ttyACM0` → 115200 → TC4
5. ☕ Roast
