# Connection Types: USB vs UART

**Last updated:** 2026-10-22 — verified on real ESP32-C3 hardware.

LibreRoaster accepts the same TC4-style protocol over native USB CDC and UART0. This document
explains the transport trade-offs and the GPIO9 boot constraint. Artisan device settings and the
startup handshake are owned by [`ARTISAN_CONNECTION.md`](ARTISAN_CONNECTION.md); flashing commands
are owned by [`DEVELOPMENT.md`](DEVELOPMENT.md).

---

## 1. Quick Decision

| You want to… | Use |
|---|---|
| Plug and play, no extra hardware | **Native USB** |
| Use an existing 3.3 V USB-UART adapter | **UART0** |
| Flash without a separate adapter | **Native USB** |
| Keep the console and Artisan separate | **USB for Artisan, UART for logs** (one active protocol channel at a time) |
| Build a custom board or use a bare module | Either, plus a **10 kΩ GPIO9 pull-up** |

Native USB is the recommended default. UART remains useful for bring-up, debug, and boards where
USB CDC is unavailable or isolated.

## 2. Native USB (Recommended)

The ESP32-C3 has a built-in USB Serial/JTAG peripheral that appears as a standard CDC ACM serial
port.

| Property | Value |
|---|---|
| Port (Linux) | `/dev/ttyACM0` |
| Port (macOS) | `/dev/cu.usbmodem*` |
| Port (Windows) | `COMx` |
| Baud | 115200 (virtual; USB CDC ignores the rate) |
| Cable | USB-C to USB-A or USB-C to USB-C |
| Extra hardware | None |
| Data pins | Internal D+/D−; no external GPIO |

Verified boot on real hardware:

```text
rst:0x15 (USB_UART_CHIP_RESET), boot:0xc (SPI_FAST_FLASH_BOOT)
```

The board boots from flash and answers Artisan commands over USB CDC.

Flashing over USB uses `espflash` with the `embedded` feature; see
[`DEVELOPMENT.md`](DEVELOPMENT.md).

## 3. UART0 (via CH340 / CP2102)

UART0 is available on GPIO20 (RX) and GPIO21 (TX).

| Property | Value |
|---|---|
| Port (Linux) | `/dev/ttyUSB0` |
| Port (macOS) | `/dev/cu.usbserial-*` |
| Port (Windows) | `COMx` |
| Baud | 115200, 8N1 |
| Adapter | CH340, CP2102, or similar **3.3 V** USB-UART |
| Extra hardware | Adapter plus wiring |

Wiring: adapter TX → GPIO20, adapter RX ← GPIO21, GND ↔ GND. The ESP32-C3 side is 3.3 V; do not
connect a 5 V adapter without level shifting. See [`pinout.md`](pinout.md) for connector-level
details.

### Observed behavior

Boards with native USB only (ESP32-C3-DevKitC-02, RUST-1): GPIO9 relies on the internal 45 kΩ weak
pull-up and boots deterministically when truly floating.

```text
# Floating (no external load) → HIGH via internal 45 kΩ → boot from flash
rst:0x1 (POWERON), boot:0xc (SPI_FAST_FLASH_BOOT)

# Externally driven LOW at reset → download mode
rst:0x1 (POWERON), boot:0x4 (DOWNLOAD(USB/UART0/1))
waiting for download
```

Boards with a CH9102/CH340 auto-program circuit (AI-C3 and similar): the USB-serial chip's RTS
line drives GPIO9 through the auto-program transistor. If RTS is asserted at reset, GPIO9 is held
LOW and the board enters download mode consistently.

```text
# Always LOW → never boots from flash
rst:0x15 (USB_UART_CHIP_RESET), boot:0x4 (DOWNLOAD(USB/UART0/1))
waiting for download
```

Verified: 10/10 consecutive resets showed `boot:0x4` on an AI-C3. Only a physical power cycle
(disconnect and reconnect USB) allowed a single flash boot, because POR resets the GPIO pad to its
default input with weak pull-up.

Even in download mode, `espflash` can connect and flash over UART0.

## 4. GPIO9: The Boot Strap

The ROM samples GPIO9 at **every reset** (power-on, USB DTR, watchdog, and so on):

| GPIO9 at reset | Result |
|---|---|
| HIGH (≈3.3 V) | Boot from flash |
| LOW (≈0 V, driven low) | Download mode |

### Why it matters

The ESP32-C3 has a **45 kΩ internal weak pull-up** on GPIO9. A truly unconnected pin latches HIGH
and boots from flash deterministically; a floating pin does not produce random boot outcomes.

The real risk is an **external circuit driving GPIO9 LOW at reset**, most commonly:

- a fan-driver MOSFET gate that holds the line low during power-on;
- a USB-UART auto-program circuit (for example CH9102 RTS → transistor) that asserts download mode
  at the wrong moment.

The internal 45 kΩ pull-up cannot overcome a transistor pull-down. Those boards are not “random”;
they are deterministically blocked from booting.

### The fix

```text
GPIO9 ──── 10 kΩ ──── 3.3 V
```

This holds GPIO9 at a defined HIGH level during reset. Use **10 kΩ** (brown-black-orange), not
10 Ω; 10 Ω would draw about 330 mA and can damage the pad.

For LibreRoaster specifically, the external pull-up is required **and** the fan driver must be
high-impedance during boot so it does not override the strap.

### Official dev boards vs custom boards

| Board | GPIO9 pull-up | Extra resistor |
|---|---|---|
| ESP32-C3-DevKitC-02 | Built-in | Not needed |
| ESP32-C3-DevKitM-1 | Built-in | Not needed |
| ESP32-C3-DevKit-RUST-1 / RUST-2 | Built-in | Not needed |
| AI-C3 (ESP32-C3-MINI-1 + CH9102) | CH9102 auto-program circuit pulls GPIO9 LOW | **10 kΩ to 3.3 V** |
| Bare ESP32-C3 module | Internal 45 kΩ only | **10 kΩ to 3.3 V** recommended |
| LibreRoaster custom board | Fan driver must not pull low at reset | **10 kΩ to 3.3 V** recommended |

Official dev boards use a module whose substrate includes an additional GPIO9 pull-up. A bare
module or custom PCB does not have it.

### How to check your board

1. Read the board documentation for the strapping pin.
2. Measure GPIO9 at power-on: 3.3 V suggests a built-in pull-up.
3. Flash firmware with monitoring enabled and check the boot log:
   - `boot:0xc (SPI_FAST_FLASH_BOOT)` consistently → no extra resistor needed;
   - `boot:0x4 (DOWNLOAD)` consistently → add the pull-up;
   - varying boot mode → add the pull-up.

## 5. Why USB Is the Default

| Advantage | USB | UART |
|---|---|---|
| Cables | One USB-C cable | USB-UART adapter plus USB power |
| Baud configuration | Automatic | Must match 115200 |
| Flashing | Works even in download mode | Works even in download mode |
| Power delivery | 5 V from host → internal LDO → 3.3 V | Depends on adapter |
| DTR reset | Built into USB Serial/JTAG | May need EN wiring |

UART remains the better choice when USB CDC is unavailable, when a separate console is useful, or
when the board's USB path is damaged.

## 6. Both Transports Listened Concurrently

The firmware runs USB CDC and UART transport tasks concurrently. Both cables may be connected. Both
are listened to, but commands are accepted only on the latched channel: the multiplexer latches
the first channel with valid traffic and resets after 60 s of inactivity. Formatted output goes to
the active transport.

```text
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

Do not use UART0 for Artisan while verbose logging is enabled: `esp_println` shares the same port.

## Related Documents

- [`ARTISAN_CONNECTION.md`](ARTISAN_CONNECTION.md) — Artisan device settings and handshake.
- [`HARDWARE.md`](HARDWARE.md) — firmware pin map and electrical constraints.
- [`pinout.md`](pinout.md) — physical wiring and board safety.
- [`DEVELOPMENT.md`](DEVELOPMENT.md) — build, flash, and test commands.
- [`PROTOCOL.md`](PROTOCOL.md) — serial command grammar.

---

*Last updated: 2026-10-22.*
