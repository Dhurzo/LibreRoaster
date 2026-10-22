# Artisan Connection Test (bare ESP32-C3 board)

> **Historical snapshot (2026-08-12, updated to `develop` @ `b344f08` on 2026-10-17).** This guide
> records a code-based test plan and observed expectations at that time; it was not yet validated on
> hardware. Current Artisan setup is owned by [`../ARTISAN_CONNECTION.md`](../ARTISAN_CONNECTION.md);
> current simulation instructions are owned by [`../SIMULATION.md`](../SIMULATION.md); current
> project state is summarized in [`../../CONTEXT.md`](../../CONTEXT.md).

**Goal:** verify that Artisan connects, plots ET/BT, and that the sliders reach the firmware. Use the
bare board: no thermocouples, no heater element, and no fan. Temperature comes from a simulated
curve.

**Status:** code-based analysis, updated to `develop` @ `b344f08` (2026-10-17). Not yet tested on
hardware.

> Changes from the previous version of this guide: the `no-heat-sense` feature **no longer exists**
> (heat-sense is now opt-in via `heat-sense`, so the default build does not block the heater),
> logs **no longer go over USB** (they go over UART0), `STOP` remains a latched stop, and `PID;ON`
> no longer clears the latch.

---

## 1. Build and Flash

```bash
cargo espflash flash --release --target riscv32imc-unknown-none-elf \
  --features "embedded,simulated-sensors"
```

| Feature | Why |
|---|---|
| `simulated-sensors` | Without MAX31856 chips the firmware **aborts at boot**. With chips but no thermocouples, they detect an “open thermocouple,” BT becomes NaN, and the safety latch trips. Simulation skips SPI and generates the curve. |
| *(do not add `heat-sense`)* | Without the GPIO1 circuit, the `heat-sense` feature would block the heater from 50 % upward. The default build does not interpret GPIO1. |

> ⚠️ **Do not connect the heater element during this phase.**

---

## 2. Before Opening Artisan

1. **Close `espflash monitor`**: if it stays open, Artisan reports “Unable to open serial port.”
2. Identify the port:
   ```bash
   ls /dev/ttyACM*
   ```
   - If `/dev/ttyACM0` **appears**, that is native USB. Use it. Firmware logs go over UART0, so they
     do **not** appear here.
   - If it does **not** appear, the board only has the USB-serial converter (UART0). Use
     `/dev/ttyUSB0`. In that case the firmware's `WARN` logs and the bootloader output **share the
     cable** with Artisan (see §7.1), and the board will likely reset every time Artisan opens the
     port.

---

## 3. Artisan Configuration

| Where | Value |
|---|---|
| Config → Device | **TC4** (Arduino TC4). ET = channel **1**, BT = channel **2**. No extra devices. |
| Config → Port | The port from step 2, **115200 8N1**. The default 0.4 s timeout is fine. |
| Config → Events (sliders) | Serial command `OT1;{}` for the heater and `IO3;{}` for the fan. |
| Extra button (recommended) | One that sends `PID;OFF`: the clean exit from any latch. |
| Config → Serial (log) | **Enable it**: it is the only way to see heater and fan values (see §6). |
| Artisan PID | If testing PID ON: set **Input = 2** (BT) in the PID dialog. With the default (1), Artisan makes the firmware regulate **ET**. Disable “PID ON at CHARGE.” |

---

## 4. Test Order

- [ ] 1. Flash (§1).
- [ ] 2. Close the monitor (§2).
- [ ] 3. Connect Artisan (ON button).
- [ ] 4. **Check the handshake:** “Arduino could not set channels/units/filters” must not appear.
- [ ] 5. **Check the curves:** ET and BT plot with coherent values (see §5).
- [ ] 6. **Move the sliders** `OT1` / `IO3` and confirm in the serial log that they arrive.
- [ ] 7. *(Optional)* Test **PID ON**, only after **2 minutes since the last boot** (see §7.2).
- [ ] 8. Finish with **`PID;OFF`**.

---

## 5. What You Should See

### The simulated curve (counts from board boot)

| Time since boot | BT (°C) | ET (°C) |
|---:|---:|---:|
| 0 s | 25 | 25 |
| 30 s | 80 | 100 |
| 60 s | 120 | 150 |
| 2 min | 150 | 180 |
| 4 min | 190 | 220 |
| 7 min | 215 | 240 |
| 9 min | 225 | 250 |
| ≥ 10 min | **225 (fixed)** | **250 (fixed)** |

### Did the board reset when the port opened?

- If Artisan **starts at ≈25 °C** on connect, the board **reset** when the port opened, and the curve
  starts from that moment.
- If it **starts mid-curve** or fixed at 225 °C, it **did not reset**.

### `READ` response

- Without PID: `AMB,ET,BT,0.0,0.0` (5 fields). In simulated mode `AMB` is `0.0` (there is no chip to
  read the cold junction from).
- With PID: `AMB,ET,BT,0.0,0.0,HEATER,FAN,SV` (8 fields).
- Expected latency is <120 ms: simulated mode has no conversion wait and the tick is ≈100-120 ms.

---

## 6. Heater and Fan Are Not Visible in Artisan (Normal)

The original TC4 responds `AMB,ET,BT,Heater,Fan,SV` (6 fields). LibreRoaster inserts two `0.0`
before them, so heater, fan, and SV are shifted to positions 5, 6, and 7 (`comm.py:7119-7126`).

- Without extra devices, Artisan reads only ET and BT, so this **does not affect the test**.
- If you add `+ArduinoTC4_56`, heater/fan appear **at 0**. If you add `+ArduinoTC4_78`, the “SV” is
  actually the heater percentage.
- **To see the real values, use Artisan's serial log**, which shows the full line.

---

## 7. Things That Look Like Failures but Are Not

### 7.1 Isolated `-1` samples
Only possible when Artisan uses **UART0** (`/dev/ttyUSB0`): there the `WARN` logs share the cable,
and one landing inside the `READ` window produces a `-1` sample. Native USB no longer has this
problem (logs go over UART0). It is not a connection problem: Artisan drains the buffer before each
`READ`.

### 7.2 Rate-of-rise (RoR) latch in the first 2 minutes
- The curve rises from 25 to 120 °C in 60 s (≈1.6 °C/s). If the RoR guard is armed, it latches in
  1-2 s.
- It is armed **only** in modes where the **firmware** controls the heater: PID ON (`PID;ON`),
  `START`, and sending an SV (`PID;SV`). With sliders (`OT1`) the guard is disarmed, including after
  `START`.
- **Rule:** wait **≥2 minutes since the last boot**, including a reset when the port opened, before
  touching PID/SV/START.

### 7.3 Latches in a long session

| Situation | What happens | When |
|---|---|---|
| `OT1 > 0` with the curve already fixed at 225 °C (≥10 min) | **Nothing**: flat BT 225 and ET 250 with BT > 60 °C count as thermal equilibrium (manual probe-stuck exemption) | — |
| Stop Artisan sampling with `OT1 > 0` | Communications latch (comms-idle) | 15 s |
| PID ON with an SV more than 5 °C above fixed BT | “Probe stuck” latch (firmware PID) | 2 min |
| Accumulated `OT1 > 0` in manual mode | `Maximum roast time exceeded` (manual budget) | 90 min |
| `START`/`PID;ON` + 30 min | `Maximum roast time exceeded` (roast budget) | 30 min from START |

### 7.4 `STOP`, `PID;OFF`, `PID;ON`, `START`
- **`STOP`** is an emergency stop with a latch: afterward it **rejects the sliders**.
- **`PID;OFF`** is the clean exit: it clears the latch and does not heat again.
- **`PID;ON`** no longer clears the latch (it rejects it with `ERR …fault_condition_active`).
- **`START`** clears the latch but **heats again** with the PID toward 225 °C (or toward the
  `PREHEAT` target if that was the previous state). Do not use it for recovery.
- After `PID;OFF` the **fan stays at 100 %** until BT drops below 60 °C **or until the next
  `OT1 > 0`**, which returns fan control. With the simulated BT (never below 60 °C), `IO3` appears
  dead until you move `OT1`.

---

## 8. If Something Really Fails

| Symptom | Likely cause | What to do |
|---|---|---|
| “Unable to open serial port” | `espflash monitor` is open, or the wrong port | Close the monitor; recheck §2 |
| “Arduino could not set channels…” in a loop | Lines without `#` arrive during the handshake (only plausible over UART0: logs or bootloader) | Enable the serial log and inspect incoming lines; try native USB |
| Everything is continuously `-1` | Wrong port or baud, or the board is resetting in a loop | Check `espflash monitor` (with Artisan closed) and confirm it boots |
| BT/ET swapped | Artisan channels are swapped | ET = 1, BT = 2 |
| `OT1` does nothing | A latch is active, or the PID latched before 2 minutes | Send `PID;OFF`; look for `ERR safety_fault …` in the serial log |
| `OT1 1–4` shows heater 0 | 1–4 % is not deliverable (minimum duty = half a mains cycle, 819 ticks) | Use ≥5 %. After R1, `OT1 5`/`UP` from 0 does deliver power |
| `OT1` ≥50 % returns to 0 | The build has `heat-sense` without a GPIO1 circuit | Reflash with the features in §1 |

---

## 9. Optional Pre-Check (without Artisan)

The `scripts/hw_virtual_roast.py` script performs a complete virtual roast over serial. Check the
port it expects in the script: it is designed for UART.
