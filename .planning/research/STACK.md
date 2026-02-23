# Stack Research

**Domain:** LibreRoaster ESP32-C3 safety stack (Watchdog + over-temp guard)
**Researched:** 2026-02-23
**Confidence:** MEDIUM

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| `esp-hal` (esp32c3 + `ledc`, `tsens`, `system`) | 1.0.0 | Access the on-chip temperature sensor, LED/PWM hardware, and system control registers without unsafe bindings | `esp_hal::tsens::TemperatureSensor` exposes `get_temperature`/`to_celsius` for over-temperature regression and `ledc` controls the fan/SSR channels; using the HAL keeps C register gymnastics under Rust safety (see esp-hal tsens module docs). |
| `esp_bootloader_esp_idf` | 0.4.0 | Link ESP-IDF’s system API so we can call TWDT helpers (`esp_task_wdt_*`) directly from Rust | The IDF Watchdog documentation shows that `esp_task_wdt_init`, `esp_task_wdt_add_user`, and `esp_task_wdt_reset_user` are the sanctioned APIs for feeding TWDT and printing the triggered tasks, so the bootloader shim must remain part of the stack to provide those symbols. |
| `embassy-time` | 0.5.0 | Drive the existing 100 ms control loop that feeds TWDT and samples temperature | `embassy_time::Timer`/`Ticker` already orchestrates the 100 ms control cycle; keeping this crate lets the watchdog feed stay in sync with the async loop that already updates heater outputs and instrumentation macros, ensuring the feed happens deterministically. |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `libreroaster::hardware::ledc_bus::LedcGuard` | current tree | Serialize LEDC calls and enforce the guard token drop before handing control back | Always around instrumentation macros that talk to the LEDC bus (fan + SSR). The guard uses an `AtomicBool` so only one caller can update the LEDC registers at a time, preventing the `ledc_set_duty`/`ledc_update_duty` re-entry failure noted in the official LEDC driver doc. |
| `esp-hal::tsens` | 1.0.0 (already part of esp-hal) | Sample raw temperature values for the over-temperature regression test | Use `TemperatureSensor::new(...).get_temperature()` when the test runs to confirm HW temperature matches expected thermal curves; `Temperature::to_celsius` does the conversion doc describes. |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| `cargo` | Manage embedded dependencies | The existing toolchain (Rust 1.88 + `riscv32imc-unknown-none-elf`) already satisfies all crates above; no new tooling needed. |

## Installation

```bash
# Keep the safety stack aligned
cargo add esp-hal@1.0 --features esp32c3,unstable,log-04
cargo add esp_bootloader_esp_idf@0.4 --features esp32c3,log-04
cargo add embassy-time@0.5
```

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| Task WDT (`esp_task_wdt_*`) | Interrupt WDT (IWDT) | Only when ISR deadlocks/critical sections are the dominant risk. Task WDT lets us watch the 100 ms control task directly, which is what needs to keep moving to prevent heater hang-ups. |
| `LedcGuard` (single mutex) | Hardware fade interrupts + `ledc_set_duty_and_update` | Use the guard when multiple instrumentation macros run on the same LEDC channel; fall back to the thread-safe API only if you need per-channel interrupts (none of our macros do). |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| Calling `ledc_set_duty`/`ledc_update_duty` from multiple tasks | The LECD peripheral is not thread-safe, and the driver explicitly warns against this pattern (it triggers spinlock deadlocks). | Hold `LedcGuard::lock(...)` around each update so the `AtomicBool` serializes access before calling the non-thread-safe APIs. |
| Spin-locking the 100 ms control loop just to feed the TWDT | Blocking the loop increases latency for the safety guard itself and defeats the async/embassy timing guarantees; the TWDT is satisfied with a periodic async reset. | Use `embassy_time::Ticker` to drive the 100 ms work and call `esp_task_wdt_reset_user` once the guard/measurements complete. |

## Stack Patterns by Variant

**If the heater control loop is running (100 ms periodic task):**
- Schedule the same `embassy_time::Ticker` that already steps heaters to also call `esp_task_wdt_reset_user(twdt_handle)` before instrumentation macros run. This ensures the Task WDT always gets fed from the long-lived task that would otherwise starve the heater if something stuck. The doc states `esp_task_wdt_reset_user` must be called via the user handle returned by `esp_task_wdt_add_user` so we keep that handle in the control loop context.

**If LED instrumentation must update the LEDC bus:**
- Acquire `LedcGuard::lock("fan")` before calling any `ledc_set_*` or `ledc_update_duty` pair and drop the guard (via `LedcGuardToken`) once the hardware write completes. Because `esp-idf` warns the LEDC APIs are not thread-safe, the guard replaces the spinlock deadlock path while still allowing `LedcDutyReader` to work inside the guard scope. |

## Version Compatibility

| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| `esp-hal@1.0` | `esp32c3@0.31`, `esp_bootloader_esp_idf@0.4` | `esp-hal` 1.0 is the documented release for these peripherals; it gates `tsens` under `soc_has_tsens` so no extra features are needed. |
| `esp_bootloader_esp_idf@0.4` | `esp_task_wdt` IDF v5.2.3 APIs | This version targets the same IDF release we vendor (v5.2.3), so the exported `esp_task_wdt_*` functions match the docs used for this research. |
| `embassy-time@0.5` | `embassy-executor@0.9`, `embassy-sync@0.7` | Embassy’s 0.5 tick driver is compatible with the existing executor crates already in Cargo.toml; the periodic `Ticker` API is what runs the 100 ms cooperative loop. |

## Sources

- https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-reference/system/wdts.html — Task Watchdog API (`esp_task_wdt_init`, `esp_task_wdt_add_user`, `esp_task_wdt_reset_user`, `esp_task_wdt_print_triggered_tasks`) and configuration options (HIGH confidence)
- https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-reference/peripherals/ledc.html — LEDC API warns `ledc_set_duty`/`ledc_update_duty` are not thread-safe, illustrates `ledc_timer_config`/`ledc_channel_config`, justifying the guard (HIGH confidence)
- /home/juan/Repos/LibreRoaster/target/riscv32imc-unknown-none-elf/doc/esp_hal/tsens/index.html — `esp_hal::tsens::TemperatureSensor` and `Temperature::to_celsius` (HIGH confidence)
- /home/juan/Repos/LibreRoaster/target/riscv32imc-unknown-none-elf/doc/libreroaster/hardware/ledc_bus/index.html — `LedcGuard` ensures atomic LEDC access via `LedcGuardToken` drop (MEDIUM confidence)
- https://docs.rs/embassy-time/0.5.0/embassy_time/ — `embassy_time::Ticker`/`Timer` provide the periodic 100 ms driver that feeds the TWDT while keeping async tasks scheduled (MEDIUM confidence)

---
*Stack research for: LibreRoaster v4.2 “Watchdog Timer” safety features milestone*
*Researched: 2026-02-23*
