# Project Milestones: LibreRoaster

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

*Last updated: 2026-02-05 during v1.6 milestone shipped*
