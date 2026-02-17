# Project Milestones: LibreRoaster

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

**What's next:** Define v2.6 requirements (ROR reset, dual output routing tests, protocol fuzzing)

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

*Last updated: 2026-02-08 — v2.3 Documentation Update shipped*
