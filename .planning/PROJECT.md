# LibreRoaster

## What This Is

ESP32-C3 firmware for coffee roaster control with ARTISAN+ serial protocol compatibility. Allows Artisan coffee roasting software to read temperature data and control heater/fan output via UART or USB CDC.

## Core Value

Artisan can read temperatures and control heater/fan during a roast session via serial connection.

## Current Milestone: v5.0 Auditoria integral de calidad Rust

**Goal:** Auditar y refactorizar de forma segura el firmware para eliminar codigo muerto, mejorar practicas Rust/SOLID y confirmar que el control de tostadora real via Artisan Scope sigue siendo viable.

**Target features:**
- Analisis completo del codigo para detectar y eliminar codigo muerto sin romper funcionalidad.
- Refuerzo de buenas practicas de Rust en arquitectura, errores, ownership y limites de modulos.
- Revision y ajuste de diseno para cumplir SOLID de manera razonable en un firmware embebido.
- Verificacion del flujo de control real (Artisan Scope -> firmware -> actuadores) con criterios observables.

## Current State

v4.5 Alta Prioridad esta en tramo final (fases 77-80), con SSR deduplication, test stubs compartidos y optimizacion de memoria ya entregados, y la migracion completa del handler pattern en cierre.

- Fases 77-79 completadas y verificadas en `.planning/ROADMAP.md`.
- Fase 80 en progreso, manteniendo compatibilidad de comandos Artisan existentes.
- Base lista para iniciar el siguiente milestone de calidad integral (v5.0).

<details>
<summary>Previous project context</summary>

## Current Milestone: v4.5 Alta Prioridad

**Goal:** Complete SSR deduplication, unify test stubs, reduce allocations in formatter, and refactor process_artisan_command to use handler pattern.

**Target features:**
- Extract detect_heat_source() to SsrControlBase (trait default method or base implementation)
- Migrate local test stubs from command_idempotence.rs, fan_serialization.rs, mock_uart_integration.rs to crate::common::*
- Replace Vec<f32> BT history with heapless::Deque<f32, 5> and alloc::format! with core::write! in heapless::String
- Refactor process_artisan_command() match (~100 lines) to delegate to ArtisanCommandHandler

**Status:** In progress

---

## Current State

v4.4 SSR Refactoring & Test Stubs is shipped. SsrControlBase extracted with ~90 lines of duplicated code eliminated, both SSR types now implement focused traits (HeatSourceDetector, PeriodicCheck, StatusGetters), and shared test stubs module created in tests/common/mod.rs.

- Verified SSR-01, SSR-02, SSR-03, SSR-04, SSR-05, TEST-01, TEST-02, TEST-03, TEST-04, TEST-05 requirements via the SSR refactoring and test infrastructure efforts.

## Next Milestone Goals

- Initiate `/gsd-new-milestone` to define requirements for v4.5.
- Plan and scope phases for v4.5 based on current project state and identified opportunities.

---

## Current State

v4.3 Code Cleanup is shipped. Documentation cleanup, instrumentation automation, and API alignment now keep README/build instructions, automation hooks, and safety telemetry consistent with the source code.

- Verified CLEAN-01, CLEAN-02, ARCH-01, REFR-01, REFR-02, REFR-03 requirements via the code cleanup and consolidation efforts.

## Next Milestone Goals

- Initiate `/gsd-new-milestone` to define requirements for v4.4 (e.g., further performance tuning, new safety features).
- Plan and scope phases for v4.4 based on current project state and identified opportunities.

## Requirements

### Validated

- ✓ ARTISAN+ command parsing (OT1, IO3) — v1.0
- ✓ Parser boundary value handling (0, 100) — v1.0
- ✓ ArtisanFormatter READ response format — v1.0
- ✓ MutableArtisanFormatter CSV output — v1.0
- ✓ ROR calculation from BT history — v1.0
- ✓ Integration test infrastructure — v1.0
- ✓ Mock UART driver — v1.0
- ✓ Example file with correct API usage — v1.0
- ✓ Unused output modules removed — v1.1 cleanup
- ✓ Unused control modules removed — v1.1 cleanup
- ✓ OutputManager trait consolidated — v1.1 cleanup
- ✓ Build verified after cleanup — v1.1 cleanup
- ✓ Core command hardening with explicit ERR handling — v1.2
- ✓ Deterministic formatter outputs and ERR schema — v1.2
- ✓ Mock UART end-to-end integration tests — v1.2
- ✓ Dual-channel Artisan support (USB CDC + UART0) — v1.3
- ✓ Command multiplexer with 60s timeout — v1.3
- ✓ USB CDC port appears and Artisan can connect — v1.3
- ✓ Initialization handshake (CHAN→UNITS→FILT) — v1.5
- ✓ READ command with 7-value telemetry — v1.5
- ✓ UP/DOWN incremental heater control — v1.5
- ✓ Comprehensive error handling (ERR format) — v1.5
- ✓ Parser recovery for partial commands — v1.5
- ✓ Complete documentation update — v1.6
- ✓ Non-blocking logging infrastructure — v1.7
- ✓ Defmt + bbqueue foundation — v1.7
- ✓ UART drain task for async logging — v1.7
- ✓ USB traffic sniffing with log_channel! macro — v1.7
- ✓ Flash instructions for ESP32-C3 — v1.8
- ✓ Artisan connection setup guide — v1.8
- ✓ Command reference for end users — v1.8
- ✓ Troubleshooting common issues — v1.8
- ✓ Quick start reference card — v1.8
- ✓ Clippy configuration for embedded Rust — v2.0
- ✓ cargo-geiger unsafe code baseline (22 blocks) — v2.0
- ✓ Code quality issues inventory (31 issues) — v2.0
- ✓ Severity classification and remediation priorities — v2.0
- ✓ Comment rationale cleanup — v2.1
- ✓ OT2 command parsing with safety measures — v2.2
- ✓ READ telemetry with CSV format — v2.2
- ✓ BT2/ET2 disabled channel documentation — v2.2
- ✓ UNITS temperature scale parsing — v2.2
- ✓ ARCHITECTURE.md v2.2 command flows documented — v2.3
- ✓ PROTOCOL.md complete Artisan specification — v2.3
- ✓ CODE_QUALITY_ISSUES.md corrected (24 unsafe blocks) — v2.3
- ✓ hardware.md v2.2 specifications verified — v2.3
- ✓ Documentation cross-references validated — v2.3
- ✓ PROT-01: READ response terminates with exactly one CRLF — v2.5
- ✓ PROT-02: READ response is a 4-value CSV with one-decimal precision — v2.5
- ✓ ROR-01: delta_bt updates last_bt so ROR becomes non-zero after the second BT sample — v2.5
- ✓ ARCH-01: A centralized terminator policy appends CRLF at a single output boundary — v2.5
- ✓ TEST-01: Tests cover READ terminator and ROR update behavior — v2.5
- ✓ SSR-01: Saturating SSR duty conversion 0-100 → LEDC 0-255 — v2.6
- ✓ SSR-02: SSR cycle guard (≥1s) enforcement — v2.6
- ✓ SSR-03: LEDC drift monitoring (±2 ticks) with retry — v2.6
- ✓ FAN-01: FanController writes LEDC duty directly — v2.6
- ✓ FAN-02: Fan/SSR LEDC writes serialized via LedcBus — v2.6
- ✓ IO-01: Async UART with embassy traits and event queues — v2.6
- ✓ IO-02: USB CDC back-pressure handling — v2.6
- ✓ IO-03: CommandQueue FIFO with reject-on-full — v2.6
- ✓ TEST-02: Transport flood tests — v2.6
- ✓ SAFE-01: Replace make_static Use-After-Free in main.rs with StaticCell — v3.0
- ✓ SAFE-02: Replace mutable static in USB CDC driver with StaticCell — v3.0
- ✓ SAFE-03: Replace mutable static in UART driver with StaticCell — v3.0
- ✓ SAFE-04: Replace ServiceContainer::get_instance() static mut with StaticCell — v3.0
- ✓ TEST-01: Fix test_parse_ot2_partial_command - returns InvalidValue — v3.0
- ✓ DOCS-01: Update README.md to 4-value format (ET,BT,HEATER,FAN) — v3.0
- ✓ PERF-01: Async MAX31856 temperature reading with embassy-time Timer — v3.0
- ✓ PERF-02: Separate SSR and Fan LEDC timers — v3.0
- ✓ CLN-01: Remove outdated Artisan command information from README.md — v4.1 (Phase 62 documentation cleanup)
- ✓ CLN-02: Remove outdated pinout or hardware information from README.md — v4.1 (Phase 62 documentation cleanup)
- ✓ CLN-03: Ensure documentation accurately reflects the current codebase state — v4.1 (Phase 62 documentation cleanup)
- ✓ BLD-01: Add clear, step-by-step instructions for building the firmware — v4.1 (Phase 63 build/test documentation)
- ✓ BLD-02: Add instructions for running the test suite, including host integration tests — v4.1 (Phase 63 build/test documentation)
- ✓ BLD-03: Document the specific commands and flags needed for development (`async-lock-depth-metrics`) — v4.1 (Phase 63 build/test documentation)
- ✓ WDT-01: Feed the Task Watchdog each 100 ms control loop tick, record the last feed result, and expose failure reasons through SystemStatus — v4.1 (Phase 65 watchdog safety)
- ✓ WDT-02: Add an LEDC guard timeout that aborts stalled fades, logs guard events, and frees hardware before the watchdog fires, then document the guard hits for auditing — v4.1 (Phase 65 watchdog safety)
- ✓ WDT-03: Provide an over-temperature regression runner that drives the system into a safe shutdown, reports `SAFETY OT-REGRESSION` telemetry, and verifies the watchdog/guard stack stayed healthy before and after the event — v4.1 (Phase 65 watchdog safety)
- ✓ OBS-01: Provide a machine-readable instrumentation snapshot that exposes watchdog feed health, last failure reason, LEDC guard timeout counts, and regression activity alongside the existing sensor outputs — v4.1 (Phase 66 instrumentation observability)
- ✓ OBS-02: Expose the instrumentation snapshot through a new Artisan command so automation and auditors can poll it without changing the standard `READ` response — v4.1 (Phase 66 instrumentation observability)
- ✓ OBS-03: Document the `STATUS` payload and parsing expectations so instrumentation automation scripts and auditors know how to interpret watchdog and guard telemetry — v4.1 (Phase 66 instrumentation observability)
- ✓ CLEAN-01: Remove unused `sample_sync()`, `read_bean_sync()`, `read_env_sync()`, and `read_sensor_sync()` methods from sensors/conversion.rs so production binary contains only the async variant. — v4.3
- ✓ CLEAN-02: Verify all tests pass after removing sync methods; if tests depend on sync behavior, migrate them to use async with `block_on` or gate behind `#[cfg(test)]`. — v4.3
- ✓ ARCH-01: Document the decision to keep `log + esp-println` over `defmt` in PROJECT.md with rationale (adequate for 100ms loop, avoids tooling complexity). — v4.3
- ✓ REFR-01: Consolidate duplicate `SyncCell<T>` wrapper from uart/tasks.rs and usb_cdc/tasks.rs into a shared module using `static_cell::StaticCell`. — v4.3
- ✓ REFR-02: Update uart/tasks.rs and usb_cdc/tasks.rs to import the consolidated SyncCell from the shared module. — v4.3
- ✓ REFR-03: Verify both UART and USB CDC communication paths function correctly after SyncCell consolidation. — v4.3
- ✓ SSR-01: Extract common state into `SsrControlBase` struct with fields shared between `SsrControl` and `SsrControlSimple` — v4.4
- ✓ SSR-02: Define `SsrControlTrait` with default implementations for common methods — v4.4
- ✓ SSR-03: Refactor `SsrControl` to embed `SsrControlBase` and implement `SsrControlTrait` — v4.4
- ✓ SSR-04: Refactor `SsrControlSimple` to embed `SsrControlBase` and implement `SsrControlTrait` — v4.4
- ✓ SSR-05: Verify all existing tests pass after refactoring — v4.4
- ✓ TEST-01: Create `tests/common/mod.rs` module with module-level helper functions — v4.4
- ✓ TEST-02: Create `StubHeater` struct implementing `control::traits::Heater` with call history tracking — v4.4
- ✓ TEST-03: Create `StubFan` struct implementing `control::traits::Fan` with call history tracking — v4.4
- ✓ TEST-04: Create `StubThermometer` struct implementing `control::traits::Thermometer` with configurable temperature returns — v4.4
- ✓ TEST-05: Implement `reset_channels()` and `collect_output()` helper functions for test isolation — v4.4

### Active

- [ ] QUAL-01: Identificar y catalogar codigo muerto por modulo con evidencia de uso/no-uso antes de eliminar
- [ ] QUAL-02: Eliminar codigo muerto y rutas obsoletas manteniendo compatibilidad funcional y cobertura de pruebas
- [ ] RUST-01: Aplicar mejoras de buenas practicas Rust en ownership, errores, API boundaries y consistencia en modulos
- [ ] SOLID-01: Evaluar y ajustar componentes clave para cumplir SOLID de manera razonable en contexto embebido
- [ ] HW-01: Verificar que Artisan Scope puede controlar una tostadora real usando este firmware y documentar evidencia

### Out of Scope

- Hardware testing (actual ESP32 + roaster) — requires physical hardware
- PID control implementation
- Roast profile automation
- WiFi/Web UI
- Telemetry channel for SSR/fan duty versus Artisan commands — deferred until hardware reliability proves stable
- Dynamic PWM frequency reconfiguration across board variants — future milestone

## Context

Brownfield ESP32-C3 Rust embedded project using embassy-rs framework.

<details>
<summary>Previous state</summary>

## Current Milestone: v4.3 Code Cleanup

**Goal:** Remove dead sync code, consolidate duplicate SyncCell wrappers, and evaluate defmt for lower overhead in the 100ms control loop.

**Target features:**
- Remove unused `read_temperature()` sync variant with 160ms spin-loop
- Consolidate duplicate SyncCell<T> from uart/tasks.rs and usb_cdc/tasks.rs into shared module
- Evaluate defmt vs log+esp-println for embedded logging overhead

**Status: SHIPPED** ✓

**v4.3 shipped:** Removed dead sync code, documented logging architecture decision, and consolidated SyncCell wrappers.

<details>
<summary>Previous state</summary>

## Current Milestone: v4.2 Anti-windup integral

**Goal:** Harden the 100 ms control loop with anti-windup, derivative-on-measurement, and deterministic sampling so safety telemetry stays aligned with heater/fan outputs.

**Target features:**
- Anti-windup integral inside the PID stack so heater commands never let integrators grow beyond actuator saturation.
- Derivative-on-measurement computed from the shared sensor stream to boost responsiveness without noise-induced jitter.
- Control loop cadence aligned to the 100 ms timer so watchdog feeding, telemetry, and instrumentation remain deterministic.
- Centralized `MAX31856` conversion helper plus comprehensive unit tests for every control/safety component (including the conversion path).

**Status: SHIPPED** ✓

**v4.2 shipped:** All 9 requirements complete - anti-windup integral guard, derivative-on-measurement, centralized SensorConversionHub, and regression harness behind feature flag.

<details>
<summary>Previous state</summary>

## Current Milestone: v4.1 Documentation Update

**Goal:** Update readme with new code status and functionality. Clean all the information outdated and update it.

**Target features:**
- Cleanup outdated info
- Recent changes (async changes, transport resilience)
- Build/Test instructions
- Documentation consistency (binary paths, target name, macOS ports)

**Status: SHIPPED** ✓

**v4.1 shipped:** All documentation updated - README with build/test instructions, FLASH_GUIDE with correct binary paths, macOS port references added.

<details>
<summary>Previous state</summary>

## Current Milestone: v4.0 Async Sensor Race Condition Fix

**Goal:** Resolve race condition in roaster_async_sensor_read by replacing take/replace pattern with embassy_sync::Mutex for safe async access to RoasterControl.

**Target features:**
- Replace take/replace pattern with embassy_sync::Mutex in ServiceContainer
- Ensure safe concurrent async access to RoasterControl
- Verify no race conditions under concurrent sensor reads

## Last Shipped: v3.0 Critical Safety Fixes (2026-02-19)

v3.0 fixed critical safety issues: Use-After-Free bug, unsafe statics (replaced with StaticCell), OT2 parser test, README documentation, async MAX31856 temperature reading, and separate LEDC timers for SSR/Fan.

## Next Milestone

v4.0 — Async Sensor Race Condition Fix

## Current State

v3.0 shipped: StaticCell patterns eliminate unsafe statics, async temperature reading, separate LEDC timers, fan telemetry fixed. Ready for next milestone.

<details>
<summary>Previous State</summary>

v2.0 Code Quality Audit — Complete. Technical debt inventory finished with 31 issues identified (1 High, 7 Medium, 23 Low).

</details>

</details>

</details>

</details>

## Key Decisions

| Decision | Rationale | Outcome |
|---|---|---|
| Dual clippy config (Cargo.toml + clippy.toml) | Portability + project-specific thresholds | ✓ Configured |
| allow-unwrap-in-tests=true | Tests can use unwrap for test logic | ✓ Configured |
| Grep-based unsafe analysis | cargo-geiger embedded feature complexity | ✓ Documented 22 blocks |
| cargo unsafe-check alias | Avoid cargo-geiger shadowing | ✓ Working |
| UART for Artisan communication | Standard approach for ESP32 artisan integration | ✓ Verified |
| USB CDC as primary channel | Native USB, no external adapter needed | ✓ Implemented |
| Multiplexer with timeout | Graceful channel switching | ✓ Implemented |
| First command wins priority | Simple, predictable behavior | ✓ Implemented |
| USB + UART dual support | Maximum flexibility for users | ✓ Implemented |
| UP/DOWN clamping | No error at boundaries, just clamp | ✓ Implemented |
| Unused READ channels = -1 | Per Artisan spec | ✓ Implemented |
| OT2 decimal rounding | Round to nearest integer (50.5 → 51) | ✓ Implemented v2.2 |
| OT2 heater stop on out-of-range | Safety measure for invalid fan values | ✓ Implemented v2.2 |
| READ one-decimal format | Consistent with Artisan spec (75.0) | ✓ Implemented v2.2 |
| UNITS parse only, no conversion | Temperatures stay Celsius internally | ✓ Implemented v2.2 |
| Centralized CRLF termination at output boundary | Prevent double terminators across USB CDC/UART | ✓ Implemented v2.5 |
| Reset formatter on START/STOP transitions | Avoid stale ROR state across sessions | ✓ Implemented v2.5 |
| Saturating SSR duty conversion | Fix double-division, clamp to LEDC 0-255 | ✓ Implemented v2.6 |
| Shared LedcBus with serialization | SSR and Fan share timer via atomic guard | ✓ Implemented v2.6 |
| Embassy async UART/USB transports | Non-blocking with back-pressure | ✓ Implemented v2.6 |
| Dual mutex pattern for ServiceContainer | Keep async access safe while preserving ISR sync helpers | ✓ Implemented with EmbassyMutex + roaster_sync |
| Feature-gated async lock depth telemetry | Keep instrumentation out of release builds while proving ASYNC-06 | ✓ Implemented via `async-lock-depth-metrics` feature |
| Reset lock-depth metrics between runs | Ensure reproducible instrumentation for auditors | ✓ Documented in README and harness |
| USB instrumentation harness handles `process_usb_command_data_test` | Exercised unused export while keeping production tasks untouched | ✓ Wired with riscv32-only runner and documentation |
| Deterministic STATUS CSV layout | Keep automation parsing stable | ✓ Implemented (v4.1) |
| README links to REG/STATUS/STAT automation hooks | Make automation readers discover instrumentation without digging through internalDoc | ✓ Implemented (v4.1) |
| Privitized regression helper exports | Align the API surface with actual callers (regression_task/request_regression) | ✓ Implemented (v4.1) |
| log + esp-println over defmt | esp_println provides direct UART0 output without complex RTT integration, no buffering or async drain task needed, works reliably at 115200 baud for debugging/development | ✓ Implemented (v4.3) |
| Trait delegation pattern | impl Trait for Type { fn method(...) { Type::inherent(...) } } for consistency with existing code | ✓ Implemented (v4.4) |
| RefCell for interior mutability in test stubs | Per accumulated decisions from STATE.md | ✓ Implemented (v4.4) |
| Composition over inheritance for SSR base struct | Better for embedded Rust with static dispatch | ✓ Implemented (v4.4) |

## Constraints

- **Protocol**: ARTISAN+ standard serial protocol
- **Baud rate**: 115200 (typical for Artisan)
- **Pins**: UART_TX=20, UART_RX=21
- **Commands**: READ, START, STOP, OT1 (0-100), IO3 (0-100), UP, DOWN
- **USB**: Native USB CDC (USB Serial JTAG)
- **LEDC**: 25 kHz, 8-bit timers shared between SSR and fan with serialized access

---

*Last updated: 2026-03-07 — v5.0 milestone started*

</details>
