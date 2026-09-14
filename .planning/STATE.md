# Project State: LibreRoaster v0.1

## Project Reference

See: .planning/PROJECT.md

**Core value:** Artisan can read temperatures and control heater/fan during a roast session via serial connection.
**Current focus:** v0.1 — First working version

## Current Position

Status: **v0.1 released** — Compiles, flashes, and runs on ESP32-C3 hardware
Last activity: 2026-04-30 — USB CDC communication verified, READ command responds

> ⚠️ **Snapshot of 2026-04-30 (v0.1 release).** Since then the V2-series / Bug B1-B36 / p1-p12 hardening arc (2026-07-22 → 08-03) shipped. For the current state read `CONTEXT.md` (repo root) or `docs/`.

Progress: [██████████] 100%

## Performance Metrics

**Verification:**
- ✅ Firmware compiles for both `riscv32imc-unknown-none-elf` (embedded) and `x86_64-unknown-linux-gnu` (host test)
- ✅ Flashes successfully via CH340 UART adapter on `/dev/ttyUSB0`
- ✅ Boots without panics or watchdog resets
- ✅ All hardware inits: SPI, MAX31856×2 thermocouples, SSR (5 Hz zero-cross), Fan (25kHz LEDC), RTC WDT
- ✅ Control loop ticks at ~310-330ms per cycle (100 ms timer + 210 ms MAX31856 conversion wait)
- ✅ USB CDC (ttyACM0) responds to Artisan `READ` with TC4 format: `AMB,ET,BT,0.0,0.0`
- ✅ UART driver (GPIO20/21) properly implemented with `StaticCell` + async split
- ✅ Async USB CDC driver (non-blocking, co-operative executor)
- ✅ No `enter_safe_shutdown` false triggers after task spawn

*Updated: 2026-08-04*

## Accumulated Context

### Decisions

- [v0.1]: Original firmware bootstrap and hardware bring-up
- [v0.1]: Fix 6 boot-time bugs: heap allocator, RTOS scheduler, logger, LEDC timer frequency, LEDC channel config, embassy-time stale `current_time`
- [v0.1]: Proper UART0 driver with `Uart::new().with_rx().with_tx().into_async().split()` stored in `StaticCell` + raw pointer
- [v0.1]: USB CDC driver converted from `Blocking` to `Async` mode to prevent executor lock-up
- [v0.1]: `esp-println` auto-detection produces SOF conflict on USB Serial/JTAG — Artisan uses USB CDC
- [v0.1]: GPIO types resolved to `esp_hal::peripherals::GPIO20`/`GPIO21` (not `esp_hal::gpio::Gpio20`/`Gpio21`)

### Pending

- Real-hardware validation: thermocouples, SSR heater, fan, and a full roast with real Artisan (hardware-validation milestone)

### Blockers/Concerns

- None for the shipped firmware; the project-wide safety warning applies (do not run a live heater without independent thermal protection)
