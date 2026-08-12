# LibreRoaster — Test Plan & Status

**Created:** 2026-08-10  
**Purpose:** Document what HIL tests have been executed on bare ESP32-C3 board, and what remains pending when thermocouples/heater hardware arrive.

---

## ✅ COMPLETED — Bare Board (No Thermocouples, No SSR, No Fan, No Load)

All tests executed on **ESP32-C3 rev v0.4** via `/dev/ttyUSB0` (CH340 → UART0: GPIO20/21).  
**Firmware:** `--features embedded,simulated-sensors` (synthetic roast curve active).  
**Native USB (USB-Serial-JTAG): NOT AVAILABLE** — pin conflict (USB needs GPIO19/20, UART0 uses GPIO20/21; ESP32-C3 pins are fixed).

### Quality Gates (Host)
| Check | Result |
|-------|--------|
| `cargo fmt --all -- --check` | ✅ PASS |
| `cargo clippy --locked --all-targets -- -W clippy::unwrap_used -W clippy::expect_used -W clippy::panic` | ✅ PASS |
| Host tests (`cargo test --target x86_64-unknown-linux-gnu --features test`) | ✅ **680 passed, 0 failed** (2026-08-12, tras auditoría A-TC4: +8 — replay de transcripciones Artisan, pipeline soak, T-B4 interleave, perfiles degenerados) |
| Regression gate (`--features "test,regression"`) | ✅ **740 passed, 0 failed** (2026-08-12) |
| Embedded build (`cargo build --release --target riscv32imc-unknown-none-elf --features embedded`) | ✅ 0 warnings |

---

### Tier 2: Test Firmware (examples/)

| Example | Tests | Result | Notes |
|---------|-------|--------|-------|
| **hil_c1** | 5 | ✅ **5/5 PASS** | C1 fix validation: Config DUTY sync @ +1ms, DUTY_R lag @ +250ms, 1%→60% ramp, safe zero. **Drives GPIO10 non-zero — disconnect SSR power.** |
| **hil_fan** | 7 | ✅ **7/7 PASS** | 8-bit 25kHz PWM sweep 0/25/50/75/100/0%. L7 off-by-one fixed (delta=0 all points). |
| **hil_gpio** | 3 | ✅ **3/3 PASS** | GPIO1 pull-up HIGH, 10/10 reads consistent. |
| **hil_ssr** | 4 | ✅ **4/4 PASS** | SAFE ONLY (0% duty). LEDC init, zero-duty readback, timer config, idempotency. |
| **hil_tc** | 6 | ⏭️ **SKIPPED** | Requires MAX31856 SPI bus — no thermocouples attached. |

---

### Tier 1: Hardware Scenarios via `validation_runner.py --hardware-mode`

All scenarios use main firmware with `simulated-sensors`. Telemetry CSVs + metadata in `tests/hardware/runs/<SCENARIO>/<TIMESTAMP>/`.

| Scenario | Category | Commands Executed | Result |
|----------|----------|-------------------|--------|
| **FAN-01** | Fan | `IO3 0` → `STATUS` | ✅ PASS — fan=0.0 |
| **FAN-02** | Fan | `IO3 0` → `IO3 50` → `STATUS` | ✅ PASS — fan=50.0 |
| **FAN-03** | Fan | Sweep `IO3 0/25/50/75/100/0` + `STATUS` each | ✅ PASS — all steps confirmed |
| **SSR-01** | SSR | `OT1 0` → `STATUS` | ✅ PASS — heater=0.0 |
| **SSR-02** | SSR | `STATUS` (boot) → `OT1 0` → `STATUS` | ✅ PASS — heater=0.0 at boot |
| **SSR-03** | SSR | 2× `OT1 0` + `STATUS` | ✅ PASS — heater=0.0 always |
| **GPIO-01** | GPIO | `OT1 0` → `STATUS` | ✅ PASS — heat detect pin HIGH (pull-up) |
| **GPIO-02** | GPIO | 3× `STATUS` | ✅ PASS — consistent heater/fan/guard_timeouts |
| **TC-01**..**TC-04** | Thermocouple | — | ⏭️ **SKIPPED** — no MAX31856 hardware |

---

### Synthetic Curve (Continuous Telemetry)
- **Firmware:** Main + `--features embedded,simulated-sensors`
- **Profile:** `RoastCurve::default_medium_roast()` — 25°C → 250°C ET / 225°C BT over 600s
- **Verified:**
  - `READ` returns AMB/ET/BT progressing along curve
  - `STATUS` shows ET/BT, watchdog=1, fault=0, uptime increasing
  - `START` enables continuous telemetry (`#timestamp,ET,BT,heater,fan`)
  - Safety guards active: **BT RoR (0.5°C/s) → emergency shutdown**, **Fan floor (20% min when heater>0)**
  - Emergency recovery: `STOP` + `OFF` clears emergency, heater stays 0%

---

## ❌ PENDING — Requires Hardware

| Test | Blocked By | Hardware Needed |
|------|------------|-----------------|
| **TC-01** READ returns plausible ambient | No thermocouples | 2× MAX31856 + Type-K thermocouples |
| **TC-02** Dual-channel valid, non-identical temps | No thermocouples | 2× MAX31856 at different locations |
| **TC-03** Fault detection on sensor disconnect | No thermocouples | Ability to disconnect TC at runtime |
| **TC-04** Temperature stability over 10s | No thermocouples | Stable thermal environment |
| **SSR with real heater** | No SSR/load | SSR module + heater element + **load power** |
| **Native USB (USB-Serial-JTAG)** | Pin conflict | **Cannot fix on ESP32-C3** — UART0 and USB-Serial-JTAG share GPIO20. Use CH340 only. |
| **PID tuning / C1 full validation** | No thermal loop | Thermocouples + SSR + heater + fan |
| **Full roast (C2)** | All of above | Complete roaster hardware |

---

## 📋 Resumption Checklist (When Thermocouples Arrive)

1. **Wire MAX31856 boards:**
   - ET (GPIO3 CS, shared SPI: SCK=GPIO6, MOSI=GPIO7, MISO=GPIO5)
   - BT (GPIO4 CS, shared SPI)

2. **Flash main firmware (real sensors):**
   ```bash
   cargo espflash flash --release --target riscv32imc-unknown-none-elf --features embedded --port /dev/ttyUSB0
   ```

3. **Run Tier 2 thermocouple tests:**
   ```bash
   python3 tests/hardware/hardware_test_runner.py --port /dev/ttyUSB0 --example hil_tc
   ```

4. **Run Tier 1 thermocouple scenarios:**
   ```bash
   python3 tests/hardware/validation_runner.py --port /dev/ttyUSB0 --scenario TC-01 --hardware-mode
   python3 tests/hardware/validation_runner.py --port /dev/ttyUSB0 --scenario TC-02 --hardware-mode
   python3 tests/hardware/validation_runner.py --port /dev/ttyUSB0 --scenario TC-03 --hardware-mode --pause-for-manual
   python3 tests/hardware/validation_runner.py --port /dev/ttyUSB0 --scenario TC-04 --hardware-mode
   ```

5. **SSR + Heater (SAFETY FIRST — disconnect load power for initial tests):**
   - `OT1 0` → `STATUS` confirm heater=0
   - `OT1 50` → `STATUS` confirm heater=50 (with fan ≥20%)
   - Verify `hil_c1` DUTY_R measurements match (config DUTY sync + DUTY_R lag)

6. **PID / Safety validation:**
   - `PID;SV;150` → `START` → monitor telemetry
   - Verify overtemp cutoff, RoR guard, fan floor, heat-source detection
   - `OFF` → verify safe shutdown

7. **Promote golden artifacts** (per `HIL-PLAYBOOK.md`):
   ```bash
   cp tests/hardware/runs/TC-01/<LATEST>/telemetry.csv tests/hardware/goldens/TC-01.csv
   tar -czf tests/hardware/goldens/WG-HW03-TC-01-<TIMESTAMP>.tar.gz ...
   ```

---

## 🐛 Known Issues (Test Infra Only)

| Issue | File | Fix |
|-------|------|-----|
| `validation_runner.py` STATUS parser: `datetime.utcnow()` deprecation | `tests/hardware/validation_runner.py` | Use `datetime.now(timezone.utc)` |
| HIL scripts: `datetime.utcnow()` deprecation warnings | Multiple | Same fix |

---

## 📁 Artifacts Created (This Session)

```
tests/hardware/runs/
├── FAN-01/20260810T205148Z/  (telemetry.csv, telemetry.csv.json, read_telemetry.csv)
├── FAN-02/20260810T205200Z/
├── FAN-03/20260810T205214Z/
├── SSR-01/20260810T205235Z/
├── SSR-02/20260810T205246Z/
├── SSR-03/20260810T205256Z/
├── GPIO-01/20260810T205308Z/
└── GPIO-02/20260810T205319Z/
```

---

## 🔧 Code Changes Committed (develop → 2c65f2b)

| File | Change |
|------|--------|
| `src/hardware/ledc_bus.rs` | L7 fix: cap computed ticks at `max_duty()` in `set_duty()` + `start_duty_fade()` |
| `examples/hil_fan.rs` | Corrected 75%→192, 100%→256 expectations (match esp-hal `2^bits * pct / 100`) |
| `tests/hardware/validation_runner.py` | Filter `#...` telemetry lines in `send_and_read()` |
| `Cargo.toml` | Added `hil_c1` example |
| `tests/hardware/hardware_test_runner.py` | Registered `hil_c1` in EXAMPLES |
| `tests/hardware/HARDWARE-TEST-PLAN.md` | Documented `hil_c1` test suite |
| `tests/hardware/HIL-PLAYBOOK.md` | Added `hil_c1` to available test firmware table |
| `examples/hil_c1.rs` | **NEW** — C1 DUTY_R latency validation (5 tests) |

---

## 📋 PENDING — Artisan Desktop Session (V2, requires a computer running Artisan)

Host-side replay tests (`tests/artisan_transcript_replay.rs`) now pin the
wire contract against byte transcripts of Artisan's session. The remaining
gap is a LIVE session against the real desktop app:

1. Flash `--features embedded,simulated-sensors` on the ESP32-C3.
2. In Artisan: Device = ArduinoTC4 (id 19), port = `/dev/ttyUSB0`,
   baud 115200, 8N1.
3. Observe: handshake completes (`CHAN`/`UNITS`/`FILT` get `#`-prefixed
   acks), `READ` polling draws a clean ET/BT curve, no "Arduino could not
   set channels/units/filters" dialogs.
4. Move heater/fan sliders (`OT1;n`/`IO3;n`) and confirm the curve and
   slider feedback stay consistent while the 1 Hz `#`-telemetry interleaves
   with READ responses (analysis says benign — confirm live).
5. Roast end: set slider to 0 / stop the roast in Artisan; verify the heater
   lands at 0 and no safety latch arms.
6. Reconnect mid-roast while a fault is latched (e.g. pull the probe flat
   with heat on to trip probe-stuck): the reconnect handshake must succeed
   and `ERR safety_fault ...` must be visible in Artisan's log.

---

*End of TEST-PLAN.md.*