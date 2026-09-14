# LibreRoaster — Wiring Diagrams

Wiring diagrams verified against firmware constants (`src/config/constants.rs`) and hardware initialization (`src/hardware/init.rs`).

---

## Full Wiring Diagram

<img src="wiring-diagram.svg" alt="LibreRoaster complete wiring diagram (ESP32-C3)" width="100%">
*Fritzing:* <a href="wiring-diagram_fritzing.svg">wiring-diagram_fritzing.svg</a> — same circuit, Fritzing breadboard style

**File:** `docs/diagrams/wiring-diagram.svg` · **Fritzing:** `wiring-diagram_fritzing.svg`

Complete pin-to-pin wiring of all subsystems on one page. Pin mapping enforced by `assert_eq!` at boot. Use this as the build reference.

**Firmware-verified pinout:**

| GPIO | Signal | Notes |
|:---:|--------|-------|
| 1 | Heat detect | External current-sense circuit, internal pull-up; LOW = SSR conducting |
| 3 | CS ET | MAX31856 #1 chip select (environment) |
| 4 | CS BT | MAX31856 #2 chip select (bean) |
| 5 | SPI MISO | ← MAX31856 SDO (GPIO matrix, avoids FSPIQ strapping) |
| 6 | SPI SCLK | → MAX31856 SCK, 1 MHz Mode 1 |
| 7 | SPI MOSI | → MAX31856 SDI |
| 8 | Status LED | Push-pull output, NOT strapping for normal SPI-boot (see HARDWARE.md §8) |
| 9 | Fan PWM | LEDC Ch0 @ 25 kHz, ⚠ strapping pin — 10 kΩ → 3.3 V on custom boards |
| 10 | SSR control | LEDC Ch1 @ 5 Hz zero-cross, 14-bit |
| 20 | UART RX | ← USB-UART adapter TX, 3.3 V max (1 kΩ series recommended) |
| 21 | UART TX | → USB-UART adapter RX |

**SSR:** `SSR_CONTROL_CYCLE_HZ = 5` (zero-cross, fixed)
**Fan:** `FAN_PWM_FREQUENCY_HZ = 25000` (25 kHz, silent operation)

---

## Subsystem Diagrams

Assemble and verify one subsystem at a time. Each diagram isolates a single subsystem for clarity.

### 1. Thermocouples — SPI + MAX31856 ×2

<img src="libreroaster_sub_termopares_en.svg" alt="Thermocouple wiring diagram" width="100%">
*Fritzing:* <a href="libreroaster_sub_termopares_fritzing_en.svg">libreroaster_sub_termopares_fritzing_en.svg</a> — same circuit, breadboard view

**File:** `libreroaster_sub_termopares_en.svg` · **Fritzing:** `libreroaster_sub_termopares_fritzing_en.svg`

- Shared SPI2 bus: SCLK=GPIO6, MOSI=GPIO7, MISO=GPIO5
- Individual chip selects: BT=GPIO4, ET=GPIO3
- 1 MHz, Mode 1 (CPOL=0, CPHA=1), software CS (defaults HIGH)
- FAULT/DRDY unconnected (polled via SPI with 210 ms conversion wait)

### 2. Fan / Motor — MOSFET PWM

<img src="libreroaster_sub_motor_ventilador_en.svg" alt="Fan motor wiring diagram" width="100%">
*Fritzing:* <a href="libreroaster_sub_motor_ventilador_fritzing_en.svg">libreroaster_sub_motor_ventilador_fritzing_en.svg</a>

**File:** `libreroaster_sub_motor_ventilador_en.svg` · **Fritzing:** `libreroaster_sub_motor_ventilador_fritzing_en.svg`

- GPIO9 → LEDC Ch0 / Timer1 @ 25 kHz
- Logic-level MOSFET IRLZ44N, low-side switching
- ⚠ GPIO9 is a strapping pin: 10 kΩ pull-up to 3.3 V mandatory
- 100 kΩ gate pull-down (weak, does not load strapping)
- Flyback diode across motor terminals mandatory

### 3. SSR + Heater — AC Mains

<img src="libreroaster_sub_ssr_calentador_en.svg" alt="SSR heater wiring diagram" width="100%">
*Fritzing:* <a href="libreroaster_sub_ssr_calentador_fritzing_en.svg">libreroaster_sub_ssr_calentador_fritzing_en.svg</a>

**File:** `libreroaster_sub_ssr_calentador_en.svg` · **Fritzing:** `libreroaster_sub_ssr_calentador_fritzing_en.svg`

- GPIO10 → LEDC Ch1 / Timer0 @ 5 Hz (zero-cross time-proportioning)
- Compatible with zero-cross SSRs (SSR-25DA, etc.)
- GPIO1 ← external current-sense circuit (open-collector)
- ⚠ AC mains zone: physically isolate, disconnect to wire
- HW thermal cutoff (klixon/thermal fuse) in series with heater — mandatory
- PE to metal chassis mandatory

### 4. Power + Communication

<img src="libreroaster_sub_alimentacion_comms_en.svg" alt="Power and communication wiring diagram" width="100%">
*Fritzing:* <a href="libreroaster_sub_alimentacion_comms_fritzing_en.svg">libreroaster_sub_alimentacion_comms_fritzing_en.svg</a>

**File:** `libreroaster_sub_alimentacion_comms_en.svg` · **Fritzing:** `libreroaster_sub_alimentacion_comms_fritzing_en.svg`

- 3.3 V logic rail, 12 V fan power (separate supplies)
- Star ground — all grounds tied to a single point
- Native USB-C (CDC) for Artisan and flashing
- Optional USB-UART adapter at 3.3 V: RX←GPIO20, TX→GPIO21
- Both transports listened concurrently; commands accepted on the latched channel (multiplexer, 60 s idle reset)

---

## Assembly Order (Recommended)

1. Power + Communication → verify startup
2. Thermocouples → verify readings with `hil_tc`
3. Fan / Motor → verify PWM sweep with `hil_fan`
4. SSR + Heater → verify control with `hil_ssr` (safe mode, duty 0%)
5. Full wiring → verify with `cargo test --features test` and a `--dry-run` of the HIL scripts under `tests/hardware/` (no `preflight-check.sh` is shipped; pin-assignment validation is part of the regular test suite)
6. ⚠ AC mains last, with extinguisher nearby

---

---

## Detailed Diagrams (Per-Section, Component-Level)

Very detailed breakdowns — one per functional block, with resistor calculations, timing, scope points and failure modes.

### 5. Resistor Networks — All Passives Explained

> ⏳ Detailed schematic + Fritzing diagrams pending — not yet in the repo.

- GPIO9 divider: 10 kΩ → 3.3 V (boot) + 1 kΩ series + 100 kΩ weak gate → GND (calc 3.0 V vs 1.65 V failure)
- GPIO8 LED 330 Ω (3.9 mA), UART 1 kΩ series, ESD 10 kΩ, decoupling 100 µF + 10 µF + 0.1 µF
- Color codes, wattage, tolerance, quick DMM checklist before power

### 6. SPI Bus — Sharing, Timing & GPIO Matrix

> ⏳ Detailed schematic + Fritzing diagrams pending — not yet in the repo.

- Shared SCLK/MOSI/MISO rails, per-CS isolation, GPIO matrix MISO via GPIO5 (avoids GPIO2)
- Mode 1 timing diagram (CPOL=0/CPHA=1), 1 MHz period, code snippet `.with_sck(6).with_mosi(7).with_miso(5)`
- Pitfalls: CS contention, swapped probes, missing 0.1 µF/bulk caps, 210 ms one-shot wait

### 7. Fan Stage — MOSFET Ultra-Detail

> ⏳ Detailed schematic + Fritzing diagrams pending — not yet in the repo.

- 25 kHz PWM waveforms (25/50/75 %), IRLZ44N vs IRF520, flyback 1N4007, 1 kΩ gate stopper, 100 kΩ weak, 10 kΩ boot pull-up
- Waveform scope points, fade threshold 12 ticks, `FAN_MIN_SAFETY_PCT 20%`, hil_fan sweep verification

### 8. SSR & Heat Detection — AC + GPIO1 State Machine

> ⏳ Detailed schematic + Fritzing diagrams pending — not yet in the repo.

- DC 3.3 V → SSR-25DA DC+ (5 Hz 14-bit), AC mains L→fuse→SSR→klixon→heater, PE chassis, heatsink 1 W/A
- Two sense options (CT vs opto) → GPIO1 Pull::Up LOW=heat, debounce 5× @≥50% → NotDetected, 10× @0% → Error, rearm via OFF/START
- Min duty 820 ticks (10 ms) for zero-cross reliability, scope DMM checks

### 9. Power Rails & Star Ground — Decoupling & Back-Powering

> ⏳ Detailed schematic + Fritzing diagrams pending — not yet in the repo.

- 5 V → AMS1117-3.3 LDO → 3.3 V ±5% ≥200 mA, 12 V ≥5 A fan rail, star GND (no loops, AC isolated), 100 µF + 10 µF + 0.1 µF per IC &lt;5 mm
- Native USB-C vs UART 115200 multiplexer (60 s latch), CH341 5 V clone warning, back-power BAT54 option

### 10. Status LED — GPIO8 & Safe-Shutdown Blink

> ⏳ Detailed schematic + Fritzing diagrams pending — not yet in the repo.

- GPIO8 → 330 Ω → LED → GND, push-pull, active-high, resistor calc (Vf 2.0 V → 3.9 mA), 1 Hz/4 Hz patterns, phase-locked `led_on(elapsed_ms)`, `Peripherals::steal()` 3×200 ms blink on init failure

---

*Diagrams verified against `src/config/constants.rs` (`SSR_CONTROL_CYCLE_HZ=5`, `FAN_PWM_FREQUENCY_HZ=25000`) and `src/hardware/init.rs`. Last updated 2026-09-07. The 6 per-section detailed diagrams (resistors, SPI, fan ultra, SSR/heat, power/star-GND, LED) are pending.*
