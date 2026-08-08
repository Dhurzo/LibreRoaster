# LibreRoaster ESP32-C3 Pinout & Connection Diagram

## Overview

Pin assignments for the LibreRoaster coffee roaster firmware running on **ESP32-C3** (RISC-V). All pins are optimized for the ESP32-C3's capabilities and the coffee roaster application requirements.

## Pin Allocation Table

| GPIO  | Function              | Direction | Peripheral          | Notes / Warnings                                |
|-------|-----------------------|-----------|---------------------|-------------------------------------------------|
| 1     | Heat Detection        | Input     | SSR feedback        | Internal pull-up enabled. Reads LOW when SSR conducts. |
| 2     | *(not used)*          | —         | —                   | **Strapping pin (VDD_SPI). Must be avoided.** Conflicts with FSPIQ. |
| 3     | MAX31856 #1 CS        | Output    | Thermocouple ET     | Chip Select for Environment Temperature sensor. Shared SPI bus. |
| 4     | MAX31856 #2 CS        | Output    | Thermocouple BT     | Chip Select for Bean Temperature sensor. Shared SPI bus. |
| 5     | SPI MISO              | Input     | SPI (GPIO Matrix)   | Routed via GPIO Matrix because GPIO2 (native FSPIQ) is a strapping pin. |
| 6     | SPI SCLK              | Output    | SPI (FSPICLK)       | Serial Clock — shared between both MAX31856 chips. |
| 7     | SPI MOSI              | Output    | SPI (FSPID)         | Master Out Slave In — shared between both MAX31856 chips. |
| 8     | Status LED            | Output    | Onboard indicator   | Push-pull output only, never open-drain. Not sampled for normal flash boot. |
| 9     | **Fan PWM**           | Output    | LEDC (25 kHz)       | **⚠️ STRAPPING PIN (boot mode select).** Internal pull-up; see warning below. |
| 10    | SSR PWM               | Output    | LEDC (5 Hz, zero-cross compatible) | Heater control via Solid State Relay. |
| 20    | UART RX               | Input     | UART0               | Receives Artisan commands. Connect to CH341 TX. |
| 21    | UART TX               | Output    | UART0               | Sends Artisan telemetry. Connect to CH341 RX. |
| —     | USB D+ / D−           | Bidir     | USB Serial/JTAG     | Internal to ESP32-C3. Native USB CDC for Artisan (alternative to UART). No external pins. |

## Connection Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                          ESP32-C3                                   │
│                                                                     │
│  ┌──────┐  ┌──────┐  ┌──────┐  ┌──────┐  ┌──────┐  ┌──────┐       │
│  │ GP1  │  │ GP3  │  │ GP4  │  │ GP5  │  │ GP6  │  │ GP7  │       │
│  │ DET  │  │ ET_CS│  │ BT_CS│  │MISO  │  │SCLK  │  │MOSI  │       │
│  └──┬───┘  └──┬───┘  └──┬───┘  └──┬───┘  └──┬───┘  └──┬───┘       │
│     │         │         │         │         │         │            │
│     │    ┌────┴─────────┴─────────┴─────────┴─────────┘            │
│     │    │              Shared SPI Bus                              │
│     │    │  ┌──────────────────────────────────┐                    │
│     │    │  │  MAX31856 #1 (ET)    MAX31856 #2 (BT)                │
│     │    │  │  CS=GP3              CS=GP4                          │
│     │    │  └──────────────────────────────────┘                    │
│     │    │         ▲                               ▲               │
│     │    │         │                               │               │
│     │    │    ┌────┴────┐                   ┌──────┴────┐          │
│     │    │    │ Type-K  │                   │  Type-K   │          │
│     │    │    │Thermo-  │                   │ Thermo-   │          │
│     │    │    │couple   │                   │ couple    │          │
│     │    │    │(ET)     │                   │ (BT)      │          │
│     │    │    └─────────┘                   └───────────┘          │
│     │    │                                                         │
│  ┌──┴────┴─────────────────────────────────────────────────────┐   │
│  │                    Power / Load Side                         │   │
│  │                                                              │   │
│  │  ┌──────┐  ┌──────┐  ┌──────┐  ┌──────┐                     │   │
│  │  │ GP9  │  │ GP10 │  │ GP1  │  │ GP8  │                     │   │
│  │  │FAN   │  │ SSR  │  │ HT   │  │ LED  │                     │   │
│  │  │PWM   │  │ PWM  │  │ DET  │  │ STATUS│                    │   │
│  │  └──┬───┘  └──┬───┘  └──┬───┘  └──────┘                     │   │
│  │     │         │         │                                    │   │
│  │     ▼         ▼         │                                    │   │
│  │  ╔═══╗   ┌─────────┐   │   ╔══════════════╗                 │   │
│  │  ║FAN║   │  SSR    │   └───║ SSR feedback  ║                 │   │
│  │  ║MOTOR║  │(Solid   │       ║(load detect) ║                 │   │
│  │  ║  ~  ║  │ State   │       ╚══════════════╝                 │   │
│  │  ╚═══╝   │ Relay)  │                                         │   │
│  │          │         │                                         │   │
│  │          │   ╔═╗   │       ╔══════════════╗                  │   │
│  │          │   ║ ║   │       ║ Heater       ║                  │   │
│  │          │   ║ ║ ←─┼───────║ Element      ║                  │   │
│  │          │   ╚═╝   │       ║  (AC Load)   ║                  │   │
│  │          └─────────┘       ╚══════════════╝                  │   │
│  │                                                              │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                  Communication Side                          │   │
│  │                                                              │   │
│  │  ┌──────┐  ┌──────┐                                         │   │
│  │  │ GP20 │  │ GP21 │     ┌──────────────────┐                │   │
│  │  │UART  │  │UART  │     │  USB (native)    │                │   │
│  │  │ RX   │  │ TX   │     │  D+ / D−         │                │   │
│  │  └──┬───┘  └──┬───┘     │  (internal)      │                │   │
│  │     │         │         └──────────────────┘                │   │
│  │     │         │                                              │   │
│  │     ▼         ▼                                              │   │
│  │  ┌───────────────┐    ┌───────────────┐                     │   │
│  │  │  CH341 TX     │    │  CH341 RX     │                     │   │
│  │  │  (USB→UART)   │    │  (USB→UART)   │                     │   │
│  │  └───────────────┘    └───────────────┘                     │   │
│  │                                                              │   │
│  │    Artisan+ can connect via EITHER:                          │   │
│  │      • UART0 (GPIO20/21) — needs external USB-UART adapter  │   │
│  │      • USB CDC (native) — direct USB cable, no adapter      │   │
│  │                                                              │   │
│  └──────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

## Special Pin Warnings

### ⚠️ GPIO9 — Fan PWM (Strapping Pin: Boot Mode Select)

**This is the most critical pin on the board.**

| Property          | Value                        |
|-------------------|------------------------------|
| Strapping Role    | **Boot mode / download select** (Espressif: GPIO9 = boot-mode selector; “ULW” is not a real ESP32-C3 function name) |
| Boot Effect       | Pulled LOW → ESP32-C3 enters **download/bootstrap mode** |
| Normal Operation  | LEDC PWM output (25 kHz) for fan speed control |
| Risk              | If the fan driver circuit pulls GPIO9 low **during power-on reset**, the chip will not boot. |
| Internal pull-up  | GPIO9 has a **45 kΩ internal weak pull-up**; left unconnected it latches HIGH and the chip boots normally. The external 10 kΩ pull-up below is a robustness measure, not a mandatory boot requirement. |

**Required mitigations:**
1. **External pull-up resistor (10 kΩ to 3.3V)** on the GPIO9 line — mandatory.
2. The fan driver (e.g., MOSFET gate driver, motor controller) must be **high-impedance or tri-stated during ESP32-C3 reset**.
3. If using an N-channel MOSFET fan driver, add a pull-up between GPIO9 and the MOSFET gate, or use a series resistor + pull-up to ensure the gate is not floating low during boot.
4. Do **not** use a capacitive load >50 pF on this pin — it can delay the strapping level sampling.

**Failure symptom:** ESP32-C3 does not boot; enters serial download mode instead. Monitor with:
```bash
cargo espflash monitor
```
If you see "waiting for download" instead of "LibreRoaster v0.1 starting...", GPIO9 is being pulled low at boot.

---

### ⚠️ GPIO8 — Status LED (Not a Boot-Strapping Pin)

GPIO8 is **not** sampled during normal flash boot — the boot mode control
ignores it in SPI-boot mode, so a LOW level on GPIO8 cannot stop the firmware
from running. It only matters in **download mode**, where the combination
GPIO8=HIGH + GPIO9=LOW is used; GPIO8=0 + GPIO9=0 is invalid.

| Property          | Value                        |
|-------------------|------------------------------|
| Strapping Role    | None for normal flash boot (only the download-mode combo uses it) |
| Boot Effect       | Ignored in SPI boot mode; no “JTAG gear mode” behavior exists on the C3 |
| Normal Operation  | Output driving status LED    |

**Required mitigation:**
- Configure as **push-pull output** only. Never use open-drain.
- The LED circuit must **not** pull this pin low during boot. If the LED is connected between GPIO8 and GND (with series resistor), this is safe because `Output::new()` drives HIGH after initialization.
- If connecting an external transistor driver, ensure it is off (high-Z) during power-up.

---

### ⚠️ GPIO2 — FSPIQ (Strapping Pin — NOT USED)

| Property          | Value                        |
|-------------------|------------------------------|
| Strapping Role    | **VDD_SPI** voltage selection |
| Boot Effect       | Determines SPI flash voltage (1.8V vs 3.3V) |
| Status            | **Deliberately avoided** in this design |

SPI MISO is routed through **GPIO5 via the GPIO Matrix** instead of using the native FSPIQ (GPIO2). This avoids the strapping conflict entirely. Do not connect anything to GPIO2.

---

### ⚠️ GPIO1 — Heat Detection (SSR Feedback)

| Property          | Value                        |
|-------------------|------------------------------|
| Pull Configuration| Internal **pull-up enabled** |
| Normal Operation  | Reads **LOW** when SSR is conducting (heater ON) |
|                   | Reads **HIGH** when SSR is off (heater OFF) |

**Notes:**
- The internal pull-up ensures a known state when the SSR is disconnected.
- If the SSR feedback output is open-collector, no external pull-up is needed.
- If using an optocoupler for feedback, connect the transistor output between GPIO1 and GND.

---

### SPI Bus Notes (GPIO5, GPIO6, GPIO7)

The SPI bus is shared between two MAX31856 thermocouple amplifiers:

| Signal | GPIO | MAX31856 Pin |
|--------|------|--------------|
| SCLK   | 6    | SCK          |
| MOSI   | 7    | SDI          |
| MISO   | 5    | SDO          |
| CS_ET  | 3    | CS           |
| CS_BT  | 4    | CS           |

- **GPIO5 (MISO)** is used instead of GPIO2 (native FSPIQ) because GPIO2 is a strapping pin.
- The SPI runs at **1 MHz** in Mode 1 (CPOL=0, CPHA=1).
- Chip selects (GPIO3, GPIO4) are driven by regular GPIO, not hardware CS — ensure both default HIGH after boot.

---

### Communication Pins (GPIO20, GPIO21)

| Signal | GPIO | Direction | Connection |
|--------|------|-----------|------------|
| UART RX | 20  | Input     | CH341 TX   |
| UART TX | 21  | Output    | CH341 RX   |

- Baud rate: **115200**
- Both UART and native USB CDC are available. Artisan can connect through either path.
- No pull-up/pull-down resistors needed — the CH341 and ESP32-C3 handle line levels.

---

## Pin Group Summary

```
SPI Bus (MAX31856 thermocouples)
  GPIO6  ── SCLK
  GPIO7  ── MOSI
  GPIO5  ── MISO
  GPIO3  ── CS (ET sensor)
  GPIO4  ── CS (BT sensor)

PWM Outputs (LEDC)
  GPIO9  ── Fan PWM  (25 kHz)  ⚠️ STRAPPING PIN
  GPIO10 ── SSR 5 Hz (zero-cross compatible)

Feedback / Detection
  GPIO1  ── SSR heat detection (input, pull-up)

Status
  GPIO8  ── Status LED (output, push-pull) — not sampled for normal boot

Communication
  GPIO20 ── UART RX (Artisan in)
  GPIO21 ── UART TX (Artisan out)
  USB    ── USB CDC (native, alternative to UART)

NOT USED (avoid connecting anything)
  GPIO2  ── FSPIQ strapping pin — do not use
```

## Power Supply Notes

| Rail   | Voltage | Typical Current | Notes |
|--------|---------|-----------------|-------|
| VDD    | 3.3V    | 200–500 mA      | Regulated from USB or external supply |
| Heater | 120/230V AC | 5–15 A     | **DANGER: High voltage.** Always disconnect when modifying wiring. |
| Fan    | 12V DC  | 0.1–0.5 A       | Powered from separate supply, switched via MOSFET driven by GPIO9 |

- The ESP32-C3, MAX31856, and CH341 run on **3.3V**.
- The SSR heater and fan motor require **separate, isolated power supplies**.
- Ensure the 3.3V supply can deliver at least 200 mA to cover the ESP32-C3 + both MAX31856 chips + CH341.

## Safety Warnings

### High Voltage ⚡
- The SSR controls **mains AC voltage** (120V/230V). Always disconnect power before working on wiring.
- The heater element can exceed **260°C**. Risk of fire and severe burns.
- Never leave the roaster unattended while operating.

### GPIO9 Strapping Pin
- **CRITICAL:** GPIO9 is sampled on reset. If pulled low, the chip will **not boot**.
- An **external 10 kΩ pull-up to 3.3V** on the GPIO9 line is **mandatory**.
- Verify with an oscilloscope that GPIO9 is >2.5V during the first 1 ms after power-on.

### Fan Driver Compatibility
- If using a MOSFET to drive the fan, ensure the gate has a pull-down resistor (to keep fan OFF during ESP32-C3 boot) BUT the GPIO9 line itself must be pulled UP.
- **Solution:** Add both a pull-up (10 kΩ to 3.3V) on the GPIO9 line **and** a **weak** pull-down (100 kΩ to GND) on the MOSFET gate side, separated by a series resistor (1 kΩ) from GPIO9 to the gate.

```
        ┌──────────────────────┐
GPIO9 ──┤ 1kΩ                 │──── MOSFET Gate
        │         10kΩ         │
        │ ┌─────/\/\/\/── 3.3V │  (pull-up ensures boot OK)
        │ │                    │
        │ └─────/\/\/\/── GND  │  (pull-down keeps fan OFF during boot)
        └──────────────────────┘
```

- **Why 100 kΩ (weak):** a 10 kΩ gate pull-down would load the GPIO9 strap divider down to ≈1.9 V — below the ≈2.5 V boot threshold; 100 kΩ keeps GPIO9 ≈3.0 V at reset while still holding the gate near GND.

### Thermocouple Polarity
- Type-K thermocouples are polarity-sensitive. Reversing leads produces inverted (negative) temperature readings.
- Yellow = positive (+), Red = negative (−) for Type-K connectors.

---

## ⚠️ Classic Ways to Destroy Your ESP32-C3

Below are the most common ways to permanently damage an ESP32-C3 in a roaster project. Read these **before** wiring.

### 1. Overvoltage on GPIO Pins (Instant Death)

| Violation | Limit | Consequence |
|-----------|-------|-------------|
| Input voltage > 3.6V on any GPIO | Absolute max: **3.6V** | **Instant permanent damage** to the IO pad |
| Input voltage < −0.3V on any GPIO | Absolute min: **−0.3V** | Latch-up, chip destruction |

**How it happens in this project:**
- The SSR feedback line (GPIO1) is touched by AC mains voltage — the detection pin expects 3.3V logic, not 120V/230V.
- A 5V USB-UART (CH341) is connected directly to GPIO20/21 **without a level shifter**. Many CH341 clones run at 5V and will slowly kill or instantly fry the ESP32-C3's RX pin.
- A 5V fan tachometer feedback pin is connected directly to a GPIO.

**Mitigation:**
```text
CH341 TX ──┬── 1kΩ ──── GPIO20 (UART RX)
           │
           └── 3.3V Zener ── GND   (clamp to 3.3V)
```

Use a **10 kΩ series resistor** on any GPIO connected to an unknown external signal. This limits fault current if the pin is accidentally exposed to overvoltage.

---

### 2. 5V Power on 3.3V Rail (Catastrophic)

| Violation | Consequence |
|-----------|-------------|
| Feeding 5V into the 3.3V rail | **Chip dies instantly.** The ESP32-C3 is NOT 5V-tolerant on VDD. |

**How it happens:**
- Connecting a 5V USB-UART adapter's VCC pin to the ESP32-C3 3.3V rail.
- Using a 5V power supply without a regulator.
- A 12V fan supply leaks back through a faulty MOSFET drain-gate short into GPIO9 and then through the ESD diode into the 3.3V rail.

**Mitigation:**
- Always measure voltage rails with a multimeter before connecting.
- Use a **3.3V LDO regulator** (e.g., AMS1117-3.3) between the input supply and the ESP32-C3.
- The 3.3V rail should be **3.3V ±5%** (3.135V – 3.465V).

---

### 3. Backpowering via GPIO Protection Diodes (Slow Death)

Each ESP32-C3 GPIO has an internal ESD protection diode to VDD. If you apply 3.3V to a GPIO **while the chip is unpowered**, current flows backward through the diode into the VDD rail, partially powering the chip through the GPIO pin.

**Why this destroys the chip:**
- The chip powers up in an **undefined state** — the internal POR (Power-On Reset) circuit does not trigger correctly.
- The protection diode is **not designed for continuous current** — it overheats and fails short.
- The flash memory may corrupt during this brownout-like condition.

**How it happens:**
- The CH341 USB-UART is powered (5V or 3.3V) while the ESP32-C3 is not powered. Voltage flows from CH341 TX → GPIO20 → internal diode → 3.3V rail.
- The SSR's pull-up on GPIO1 back-powers the chip when the main supply is off.

**Mitigation:**
```text
GPIO signal ──── 10 kΩ ──── External device
                          (limits backpowering current to <330 µA)
```
Or add a series Schottky diode (e.g., BAT54) on any external signal that may be active when the ESP32-C3 is off.

---

### 4. Exceeding GPIO Output Current (Silent Death)

| Parameter | Limit |
|-----------|-------|
| Max sink/source current per GPIO | **40 mA** (absolute max) |
| Recommended continuous current | **20 mA** |
| Total current across all GPIOs | **200 mA** |

**How it happens:**
- Driving an LED directly without a current-limiting resistor (a 3.3V GPIO driving a blue LED with no resistor → easily 50+ mA → damages the GPIO pad over time).
- Driving a MOSFET gate directly without a series resistor. The gate capacitance causes a momentary short-circuit current spike during switching that exceeds 40 mA.
- Powering a relay coil or buzzer directly from a GPIO.

**Mitigation:**
- Always use series resistors: **330 Ω for LEDs**, **100–220 Ω for MOSFET gates**.
- Never drive loads >20 mA from a GPIO.
- Use a transistor/MOSFET driver for any load >20 mA.

---

### 5. Inductive Kickback from Fan Motor (Latch-up)

The fan on GPIO9 is a **motor** (inductive load). When the PWM signal switches off, the motor winding generates a reverse voltage spike that can be **>20V** — enough to punch through the GPIO's ESD protection.

**How it destroys:**
- The high-voltage spike enters GPIO9 → jumps the ESD diode → injects carriers into the silicon substrate → **latch-up** (a short circuit condition that draws unlimited current until the chip melts internally).
- The chip draws 1A+ from the supply and heats up until it physically cracks.

**Mitigation (MANDATORY):**
```text
GPIO9 ──┬── 1kΩ ──── MOSFET Gate
        │
        └── BAT54 ── 3.3V      (flyback catch diode to 3.3V)
        
Add a flyback diode across the fan motor terminals:
    Fan (+) ──┬── DIODE (1N4007 or Schottky) ──── Fan (−)
              │              cathode ◄── anode
              └───────────────────────────────────┘
              (diode cathode to Fan +, anode to Fan −)
```

- Always place a **flyback diode** across the fan motor terminals.
- For MOSFET-driven fans, add a **Zener clamp (3.6V)** from gate to source to prevent gate overvoltage.

---

### 6. ESD (Electrostatic Discharge) — Cumulative Damage

The ESP32-C3 GPIOs have built-in ESD protection rated for **HBM (Human Body Model) ±2000V**, but:

- In a dry workshop, you can easily generate **>10 kV** by walking across a carpet.
- Each ESD event **weakens** the protection diode — it may survive 5–10 hits before failing short.
- Damage is **cumulative and invisible** — the chip works for weeks then suddenly dies.

**Mitigation:**
- **Ground yourself** before touching the board (use a wrist strap or touch a grounded metal surface).
- Add **TVS diodes (5V bidirectionals)** on all external connectors (GPIO20/21, USB, SSR control lines).
- In humid environments (>40% RH), ESD risk is lower — but don't rely on it.

---

### 7. Soldering Iron Damage (Thermal Shock)

| Hazard | Detail |
|--------|--------|
| Soldering temperature | Typical iron: 350°C – 400°C |
| ESP32-C3 max junction temp | **125°C** |
| Time to damage at 350°C | **< 3 seconds** on a pin |

**How it happens:**
- Holding the iron on a GPIO pin for >5 seconds — the heat travels through the lead frame into the silicon die.
- Using a **30W unregulated iron** that doesn't temperature-control — tip temperature can reach 500°C.
- Soldering the ESP32-C3 module with a large tip that heats multiple pins simultaneously.

**Mitigation:**
- Use a **temperature-controlled iron at 320°C** (no higher).
- Limit contact time to **2–3 seconds per pin**.
- Let the board cool for 10 seconds between pins.
- Use flux — it improves heat transfer, meaning less time needed.
- For long soldering sessions, use a **hot air station at 300°C** or a **pre-heater plate at 100°C**.

---

### 8. Strapping Pin Misconfiguration (Boot Failure ≠ Dead, but Bricked)

While not lethal to the chip itself, misconfiguring strapping pins can make the ESP32-C3 **unbootable**, giving the same symptoms as a dead chip:

| Pin | Strapping Function | If Wrong at Boot |
|-----|-------------------|------------------|
| GPIO2 | VDD_SPI voltage | Flash operates at wrong voltage → CRC errors → no boot |
| GPIO8 | None for normal boot (only the download-mode combo GPIO8=HIGH + GPIO9=LOW) | No effect on flash boot |
| GPIO9 | Boot mode select | Chip enters download mode → waits for USB serial |

**As covered in the GPIO9 section above** — always ensure proper pull states during reset.

---

### 9. Power Supply Brownout (Undefined Behavior)

The ESP32-C3 requires a **clean 3.3V supply**:

| Condition | Voltage | Effect |
|-----------|---------|--------|
| Normal operation | 3.0V – 3.6V | Stable |
| Brownout warning | 2.7V – 3.0V | Flash corruption risk, random crashes |
| Brownout reset | < 2.7V | Chip resets, possible EEPROM corruption |
| Deep brownout | < 2.1V | Flash memory can corrupt during writes |

**How it happens:**
- Powering the ESP32-C3 from a linear regulator without enough input capacitance.
- The SSR heater and fan motor draw large transient currents that momentarily drop the 3.3V rail below 2.7V.
- Using thin/long wires for power (voltage drop under load).

**Mitigation:**
```text
3.3V rail ──── 100 µF electrolytic ──── GND
            ──── 10 µF ceramic      ──── GND    (placed near ESP32-C3)
            ──── 0.1 µF ceramic     ──── GND    (one per IC: ESP32, MAX31856×2, CH341)
```
- Route power and ground with **thick traces/wires** (≥AWG 20 for the main supply).
- Place the LDO regulator **close to the ESP32-C3**.

---

### 10. Shared SPI Bus Contention (Not Fatal but Corrupts Data)

Both MAX31856 chips share the SPI bus (GPIO5/6/7). If both chip selects are driven LOW simultaneously (firmware bug or glitch during initialization), both chips drive MISO simultaneously → **bus contention**.

While this doesn't destroy the ESP32-C3 (the MAX31856 outputs are typically weak enough), it produces **garbage temperature readings** that can cause:
- False over-temperature detection → emergency shutdown
- Phantom readings that trigger PID runaway
- Sensor fault cascade

**Mitigation:**
- The firmware already handles this via critical-section SPI access.
- Never manually toggle CS pins — always use `SpiDeviceWithCs`.
- Verify with an oscilloscope that CS signals are never both low simultaneously.

---

### Quick Reference: Pin Survival Checklist

Before applying power, verify every pin on this table:

| GPIO | Function | Check |
|------|----------|-------|
| 1 | Heat detection | Input only, pull-up enabled, < 3.3V expected |
| 3 | ET CS | Output, driven high/low only |
| 4 | BT CS | Output, driven high/low only |
| 5 | MISO | Input only, no external drive conflicts |
| 6 | SCLK | Output only |
| 7 | MOSI | Output only |
| 8 | LED | Push-pull output, series resistor ≥ 330 Ω |
| **9** | **Fan PWM** | **⚠️ External pull-up required! No inductive kickback!** |
| 10 | SSR PWM | Output only |
| 20 | UART RX | Input, **must not exceed 3.6V**, series resistor recommended |
| 21 | UART TX | Output only |
| VDD | 3.3V | **3.3V ±5%, never 5V** |
| GND | Ground | All grounds common, no ground loops |

---

### Summary: The Golden Rules

1. **Never exceed 3.6V on any GPIO.** Use level shifters for 5V signals.
2. **Never power a GPIO when the chip is off.** Sequence power or add series resistors.
3. **Never drive inductive loads directly.** Always use a flyback diode.
4. **Never skip the GPIO9 pull-up.** The chip won't boot without it.
5. **Always use current-limiting resistors.** 10 kΩ on signals, 330 Ω on LEDs.
6. **Always decouple the power supply.** 100 µF + 0.1 µF ceramic near the chip.
7. **Never trust CH341 clones.** Many run at 5V — verify with a multimeter.
8. **Measure twice, power once.** Check all voltages with a DMM before connecting power.
