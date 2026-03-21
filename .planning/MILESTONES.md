# Project Milestones: LibreRoaster

## v5.2 Architecture Hardening & Validation (Shipped: 2026-03-20)

**Delivered:** Fixed the embedded build pipeline, normalized the error taxonomy from hardware through AppError, captured TRACE traceability evidence, and proved diagnostics + HIL validation artifacts so the next phase can focus on safe-shutdown replay automation.

**Phases completed:** 95-103 (21 plans total)

**Key accomplishments:**
- Removed the duplicate embassy-time stubs so `cargo build --release --target riscv32imc-unknown-none-elf --features embedded` produces a flashable `.bin` and documented the audited command.
- Implemented `RoasterError`/`AppError` conversions, hardware mocks, and expanded AppError tests so diagnostics now carry `error_category`/`error_source` from sensor → control → guard.
- Instrumented the full TRACE flow, updated the parser, and documented TraceId-driven regression playback for queue → actuation → telemetry → guard.
- Built manifest-aware HIL validation runs with analysis/playbook, then archived every artifact with the safe-shutdown log + metadata for auditors.
- Anchored DIAG-01 by packaging `safe-shutdown-replay.zip`, regenerating `traceability-replay.csv`, and emitting `replay-report.json` for guard metadata verification.

**Stats:**
- ~70 files touched (firmware, instrumentation, docs, automation)
- 9 phases, 21 plans, ~30+ tasks (control + diagnostics + docs)
- 5 weeks of cross-team work culminating in the verification artifacts.

**Git range:** `feat(95-01)` → `feat(103-01)`

**What’s next:** `/gsd-new-milestone` to plan the safe-shutdown replay automation + next diagnostics chapter.

---

## v4.1 Documentation Update (Shipped: 2026-02-23)

**Delivered:** Finalized the documentation cleanup, closed audit gaps, shipped Watchdog/LEDC/regression instrumentation observability, published the STATUS automation snapshot, linked REG/STATUS hooks, and aligned the regression helper API with its actual consumers.

**Phases completed:** 62-69 (11 plans total)

**Key accomplishments:**
- Rewrote the README/FLASH_GUIDE with correct build/test instructions, binary paths, and macOS guidance so the docs now match the live code.
- Captured Phase 64 verification evidence and closed the audit gap for every documentation inconsistency.
- Delivered the WatchdogFeeder, LEDC guard, and regression runner instrumentation and surfaced the data through `SystemStatus` and the deterministic STATUS command.
- Documented REG/STATUS/STAT automation hooks and linked readers directly to `internalDoc/INSTRUMENTATION_README.MD` so automation engineers can find the payload spec without digging.
- Privatized the regression helper export while keeping `regression_task`/`request_regression` public so the API surface reflects actual consumers.

**Stats:**
- ~25 files created/modified (docs, instrumentation, safety, README entries)
- 8 phases, 11 plans, ~20+ tasks
- 5 days from start to ship (2026-02-18 → 2026-02-23)

**Git range:** `2293b65` → `6baf634`

**What's next:** `/gsd-plan-phase 70` to turn the v4.2 requirements into phase plans.

---

## v4.2 Anti-windup integral (Planning)

**Goal:** Harden the 100 ms control loop with anti-windup, derivative-on-measurement, deterministic sampling, and centralized MAX31856 telemetry/tests so automation sees a stable safety envelope.

**Phases:** 70+ (roadmap still to be defined)

**Target features:**
- Anti-windup integral guard that clamps integrator growth when LEDC outputs saturate.
- Derivative-on-measurement tied to the centralized MAX31856 conversion helper.
- Timer-aligned 100 ms loop cadence that feeds watchdog, telemetry, and instrumentation before each tick.
- Centralized MAX31856 conversion pipeline plus regression/unit tests for every control/safety component.

**What’s next:** Define requirements and phases for v4.2 (phase 70 onward) and head into `/gsd-plan-phase 70` once the roadmap is ready.

---

## v4.0 Async Sensor Race Condition Fix (Shipped: 2026-02-20)

**Delivered:** Replaced the take/replace sensor access pattern with an Embassy mutex-backed API, proved ASYNC-06 via a concurrent sensor read harness with lock-depth telemetry, and wired the USB instrumentation helper so the previously unused export is exercised and documented.

**Phases completed:** 58, 60-61 (7 plans total)

**Key accomplishments:**
- Dual `ServiceContainer` mutex and async helpers eliminate the unsafe take/replace window while keeping ISR access intact.
- Concurrent host harness plus `async-lock-depth-metrics` instrumentation proves ASYNC-06 and is documented for auditors.
- `process_usb_command_data_test` now runs inside a riscv32 harness that resets the queue and documents the wiring in internalDoc/INSTRUMENTATION_README.MD.
- README describes how to run the concurrent sensor harness and interpret the instrumentation metrics.

**Stats:**
- ~15 files created/modified (mutex migration, instrumentation tests, README, USB harness)
- 3 phases, 7 plans, ~12 tasks
- 1 day from start to ship (2026-02-19 → 2026-02-20)

**Git range:** `00ca7c1` → `2293b65`

**What's next:** `/gsd-new-milestone` to define requirements for the next focus (observability, automation, etc.).

---

## v2.5 Artisan Edge Cases (Shipped: 2026-02-17)

**Delivered:** Fixed Artisan READ framing edge cases, corrected ROR timing/reset behavior, and added regression tests.

**Phases completed:** 44-45 (3 plans total)

**Key accomplishments:**
- Normalized READ CSV output and added invalid-value regression coverage
- Centralized CRLF termination at the dual output boundary (USB CDC/UART)
- Routed raw response payloads through the shared output channel to prevent double terminators
- Added regression tests for ROR timing/reset and READ framing with host test stubs

**Stats:**
- 21 files created/modified
- 71,036 lines of Rust (repo total)
- 2 phases, 3 plans, 6 tasks
- 0 days from start to ship (2026-02-17)

**Git range:** `96ff8eb` → `dd575e7`

**What's next:** Define v2.6 hardware reliability roadmap (phases 46-48) while requirements are still being documented

---

## v2.6 Problemas que impiden funcionamiento real en hardware (Shipped: 2026-02-18)

**Delivered:** Fixed SSR duty scaling, made FanController drive physical LEDC channels, and implemented async UART/USB CDC transports with back-pressure handling so hardware stays responsive under load.

**Phases completed:** 46-48 (10 plans total)

**Key accomplishments:**
- SSR saturating duty math (0-100% → LEDC 0-255) with cycle guard (≥1s) and ±2 tick drift monitoring
- FanController wired to shared LEDC bus with serialization, telemetry reflects applied duty
- Async UART with embassy embedded_io_async and buffered event queues
- USB CDC back-pressure handling with exponential backoff (1ms→10ms)
- CommandQueue with FIFO reject-on-full semantics
- Queue processor tasks wired to artisan_channel for full E2E command flow
- Transport flood tests proving no executor stalls

**Stats:**
- 43+ files created/modified
- 3 phases, 10 plans, ~25 tasks
- 1 day from start to ship (2026-02-17 to 2026-02-18)

**Git range:** `8776cee` → `2f19685`

**What's next:** v2.7 TBD — hardware reliability verified, ready for next features

---

## v2.7 (Planning)

---

## v2.3 Documentation Update (Shipped: 2026-02-08)

**Delivered:** Comprehensive documentation update ensuring all internalDoc files accurately reflect v2.2 implementation. ARCHITECTURE.md updated with command flows, PROTOCOL.md created with complete Artisan specification, CODE_QUALITY files corrected, hardware.md verified, and all cross-references validated.

**Phases completed:** 38-42 (6 plans total)

**Key accomplishments:**
- ARCHITECTURE.md updated with OT2, READ, UNITS command flows (4-value CSV format)
- PROTOCOL.md created with 9 Artisan commands documented and ASCII sequence diagram
- CODE_QUALITY_ISSUES.md corrected (24 unsafe blocks vs 22 documented)
- hardware.md corrected to show ET,BT,HEATER,FAN format
- All documentation cross-references validated with consistent v2.2/v2.3 versions
- Timestamps added to all modified files

**Stats:**
- 18 v1 requirements satisfied
- 6 plans, ~12+ tasks
- 2 days from start to ship (2026-02-07 to 2026-02-08)

**Git range:** `docs(v2.3-start)` → `docs(42-01): complete cross-reference validation`

**What's next:** Ready for /gsd-new-milestone — next features TBD

---

## v2.2 Comandos de Entrada (Shipped: 2026-02-07)

**Delivered:** Complete Artisan protocol support with OT2 fan control, READ telemetry response, and UNITS temperature scale parsing.

**Phases completed:** 35-37 (3 phases, 3 plans)

**Key accomplishments:**
- OT2 command parsing with decimal rounding (50.5 → 51) and clamping 0-100
- OT2 safety feature: heater stops when out-of-range values received
- READ telemetry with CSV format ET,BT,-1,-1,-1,FAN,HEATER (one decimal place)
- BT2/ET2 disabled channel comment: "store value for future et2 and bt2 support"
- UNITS command parsing with TemperatureScale enum and TemperatureSettings storage
- Default Celsius with no temperature conversion applied

**Stats:**
- 4 files modified (config/constants.rs, input/parser.rs, control/roaster_refactored.rs, output/artisan.rs)
- 192 insertions, 3 deletions
- 3 phases, 3 plans, 9 tasks
- 1 day from start to ship (2026-02-07)

**Git range:** `feat(35-01)` → `docs(audit): v2.2 milestone audit complete`

**What's next:** Ready for `/gsd-new-milestone` — next features TBD

---

## v2.0 Code Quality Audit (Shipped: 2026-02-05)

**Delivered:** Comprehensive code quality audit with clippy and cargo-geiger configuration, baseline unsafe code inventory, and 31-issue severity-classified findings document.

**Phases completed:** 31 (1 phase, 3 plans)

**Key accomplishments:**
- Clippy configuration for embedded Rust (dual config: Cargo.toml + clippy.toml)
- cargo-geiger unsafe code baseline (22 blocks, 15 LOW/7 MEDIUM risk)
- Code quality issues inventory (31 issues: 1 High, 7 Medium, 23 Low)
- Severity classification and remediation priorities documentation

**Stats:**
- 5 files created/modified (clippy.toml, geiger-report.md, CODE_QUALITY_ISSUES.md, CODE_QUALITY_REMEDIATION.md)
- 1 phase, 3 plans, 9 tasks
- Same-day ship (2026-02-05)

**Git range:** `feat(31-01)` → `docs(31): complete linting-audit phase`

**What's next:** Ready for `/gsd-new-milestone` — refactoring deferred to future milestone

---

## v1.8 Flash & Test Documentation (Shipped: 2026-02-05)

**Delivered:** Comprehensive flash, connection, and usage documentation for roasters covering ESP32-C3 flashing, Artisan protocol commands, and v1.7 UART logging features.

**Phases completed:** 26-30 (5 phases, 5 plans)

**Key accomplishments:**
- Flash instructions for ESP32-C3 via USB/JTAG (FLASH_GUIDE.md, QUICKSTART.md)
- Artisan connection setup guide with USB CDC and UART0 (ARTISAN_CONNECTION.md)
- Complete command reference for all Artisan protocol commands (COMMAND_REFERENCE.md)
- UART logging usage guide covering v1.7 esp_println and drain task features (UART_LOGGING_GUIDE.md)
- Troubleshooting guide and quick start reference (TROUBLESHOOTING_GUIDE.md, QUICKSTART.md)

**Stats:**
- 14+ documentation files created/modified
- 6 documentation guides delivered (DOCS-01 through DOCS-06)
- 5 phases, 5 plans, 10 tasks
- 1 day from start to ship

**Git range:** `docs(v1.8-start)` → `docs(30): complete troubleshooting & quick start phase`

**What's next:** v1.9 milestone — TBD

---

## v1.6 Documentation (Shipped: 2026-02-05)

**Delivered:** Comprehensive documentation update for v2.0 accuracy and developer experience.

**Phases completed:** 19-20 (2 phases, 2 plans)

**Key accomplishments:**
- Complete README.md rewrite with Artisan commands, Quick Start, Hardware Requirements, Pinout
- Created internalDoc/ARCHITECTURE.md with system overview, task structure, async model
- Created internalDoc/PROTOCOL.md with Artisan command reference and message formats
- Created internalDoc/HARDWARE.md with pinout and thermocouple wiring guide
- Created internalDoc/DEVELOPMENT.md with build, flash, test, and debugging guides

**Stats:**
- 1 file modified (README.md)
- 4 new documentation files created
- 2 phases, 2 plans, 4 tasks
- Same-day ship (2026-02-05)

**Git range:** `6bc05c8` → `1e34ea8`

---

## v1.4 UART Verification (In Progress)

**Goal:** Verify UART0 works as a reliable backup communication channel to USB CDC.

**Phases:** 14-16

**Target features:**
- UART0 initialization on GPIO20/21 at 115200 baud
- Artisan connectivity via UART (USB-serial adapter or direct)
- Channel multiplexer correctly routes commands between USB CDC and UART0
- Mock UART driver for host-side testing

**What's next:** Define requirements and roadmap for UART verification phases.

---

## v1.3 Artisan USB Verification (Shipped: 2026-02-04)

**Delivered:** USB CDC dual-channel implementation with command multiplexer and 26 unit tests.

**Phases completed:** 11-13 (3 phases, 3 plans)

**Key accomplishments:**
- USB CDC driver using `esp_hal::usb_serial_jtag::UsbSerialJtag`
- Command multiplexer with 60-second timeout and first-command-wins logic
- Dual output task for channel-aware response routing
- 41 unit tests for multiplexer logic and USB CDC driver
- Implementation documentation and testing guides

**Stats:**
- 12 files changed
- 892 insertions, 45 deletions (Rust)
- 3 phases, 3 plans, 6 tasks
- Same-day ship (2026-02-04)

**Git range:** `ba509bd` → current

---

## v1.2 Artisan integration polish (Shipped: 2026-02-04)

**Delivered:** Core Artisan command hardening, deterministic formatting, and mock UART end-to-end integration tests.

**Phases completed:** 8-10 (5 plans total)

**Key accomplishments:**
- Structured ERR responses and bounds enforcement for core commands
- Idempotent START/STOP with safe shutdown and bounded manual outputs
- Deterministic READ formatting and ERR schema standardization
- Host-friendly mock UART integration tests for success/error flows
- Embedded feature gating with host stubs to enable host-target tests

**Stats:**
- 36 files changed
- 2768 insertions, 157 deletions (Rust)
- 3 phases, 5 plans, 14 tasks
- Same-day ship (2026-02-04)

**Git range:** `cf6efbc` → `b1b1a9e`

---

*Last updated: 2026-02-18 — v2.6 milestone shipped*
