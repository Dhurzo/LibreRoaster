# Codebase Concerns

**Analysis Date:** 2026-02-04

## Tech Debt

### Duplicate SSR Control Implementations

**Issue:** Two nearly identical SSR control implementations exist that share significant code
- **Files:** `src/hardware/ssr.rs`
- **Impact:** Maintenance burden, potential for inconsistencies, code bloat (354 lines total)
- **Fix approach:** Consolidate `SsrControl` and `SsrControlSimple` into a single implementation with optional pin control via generics or builder pattern

### Static Mutable State Patterns

**Issue:** Multiple `static mut` variables used for hardware state sharing
- **Files:**
  - `src/hardware/fan.rs` (lines 23, 71-76, 90-108): `PWM_CHANNEL_STATE`
  - `src/hardware/uart/driver.rs` (line 48): `UART_INSTANCE`
- **Impact:** Violates Rust safety principles, potential for data races, harder to reason about
- **Fix approach:** Replace with proper `static_cell` patterns or ` embassy_sync` primitives (Mutex, etc.)

### Unsafe Lifetime Extension via transmute

**Issue:** UART driver uses `core::mem::transmute` to extend lifetimes for static storage
- **File:** `src/hardware/uart/driver.rs` (lines 58-63)
- **Impact:** Undefined behavior if lifetimes are mismanaged, difficult to audit
- **Fix approach:** Use proper owned types or reference-counted wrappers instead of transmute

### Redundant Commented-Out Code

**Issue:** Multiple TODO/FIXME-style comments indicating incomplete refactoring
- **Examples:**
  - `src/control/handlers.rs` (line 25): "Nota: with_ssr ha sido eliminado"
  - `src/control/roaster_refactored.rs` (line 185): Comments about previous ventilation control
- **Impact:** Clutter, confusion about current architecture
- **Fix approach:** Remove outdated comments, update architecture documentation

### Inconsistent Error Handling

**Issue:** Mix of Result-based errors and `unwrap`/`expect` in initialization paths
- **Files:** `src/main.rs` (lines 67, 85, 103-104, 127, 151, 154)
- **Impact:** Panics during hardware initialization if any step fails
- **Fix approach:** Propagate all initialization errors through proper error channels, allow graceful degradation

## Known Bugs

### SPI Device Sharing Without Chip Select Coordination

**Issue:** Both temperature sensors share the same SPI bus via `SpiDeviceWithCs`, but chip select coordination depends on proper usage
- **File:** `src/main.rs` (lines 96-104)
- **Impact:** Potential for concurrent SPI transactions causing data corruption
- **Trigger:** If async tasks access both sensors simultaneously
- **Workaround:** Ensure sequential access through the mutex-protected SPI bus

### Heat Detection Pin Not Protected in Emergency

**Issue:** Heat detection pin is passed to SSR control but emergency shutdown doesn't re-verify hardware status
- **File:** `src/control/roaster_refactored.rs` (lines 128-138)
- **Trigger:** Emergency shutdown triggered while SSR was marked as available
- **Workaround:** Hardware heat detection runs periodically; SSR status should be re-checked on emergency

### Fan Emergency Logic Assumes Cooling

**Issue:** Emergency shutdown sets fan to 100% for cooling, but this assumption may not be valid
- **File:** `src/control/roaster_refactored.rs` (line 135)
- **Impact:** May not be appropriate for all emergency scenarios (e.g., electrical fire)
- **Workaround:** Document this behavior clearly; consider adding emergency type differentiation

## Security Considerations

### No Input Validation for Artisan Commands

**Issue:** Artisan command values (heater %, fan %) are cast directly without range validation
- **File:** `src/control/handlers.rs` (lines 104-105, 225, 233-234)
- **Current mitigation:** Values are clamped in hardware abstraction layer
- **Recommendations:** Add explicit range validation at command handler level with proper error responses

### Hardcoded PWM Frequency

**Issue:** PWM frequency and duty cycle calculations have hardcoded assumptions
- **Files:**
  - `src/hardware/ssr.rs` (line 140): 8-bit PWM resolution assumption
  - `src/hardware/fan.rs` (line 86): 8-bit duty cycle (0-255)
- **Risk:** Changing hardware requirements requires code modifications
- **Recommendations:** Move to configuration constants

### No Authentication on Serial Interface

**Issue:** Artisan protocol communication has no authentication mechanism
- **Current mitigation:** Assumed isolated embedded system
- **Risk:** Any device connected to serial bus could send commands
- **Recommendations:** If network connectivity is added, implement command signing

## Performance Bottlenecks

### Blocking SPI Reads

**Issue:** Temperature sensor reads may block during SPI transactions
- **File:** `src/hardware/max31856.rs`
- **Cause:** Synchronous SPI operations without timeout awareness
- **Improvement path:** Add async SPI with proper timeout handling

### No Rate Limiting on Temperature Reads

**Issue:** Temperature reading frequency depends on task scheduling without explicit rate limits
- **File:** `src/control/roaster_refactored.rs`
- **Impact:** Potential for sensor reading starvation or overload
- **Improvement path:** Add explicit sampling period configuration

### String Allocation in Hot Paths

**Issue:** `heapless::String` allocations occur in output formatting
- **Files:**
  - `src/output/serial.rs` (line 128)
  - `src/output/uart.rs` (line 125)
- **Impact:** Stack/heap pressure in embedded context
- **Improvement path:** Use stack-based buffers or pre-allocated format strings

## Fragile Areas

### Command Handler Chain

**Files:** `src/control/handlers.rs`, `src/control/roaster_refactored.rs`
**Why fragile:** Handler dispatch uses dynamic dispatch through trait objects in array; order matters for safety priority
**Safe modification:** When adding new handlers, always append before Artisan/System handlers
**Test coverage:** Unit tests exist for handler priority, but integration tests missing

### SPI Bus Sharing Architecture

**File:** `src/hardware/shared_spi.rs`
**Why fragile:** Manual chip select coordination required; no compile-time enforcement
**Safe modification:** Always acquire mutex before any SPI operation, release after complete transaction
**Test coverage:** No hardware integration tests

### State Machine Transitions

**Files:** `src/control/roaster_refactored.rs`, `src/config/constants.rs`
**Why fragile:** State transitions not fully validated; some states may be unreachable
**Safe modification:** Add state transition validation matrix
**Test coverage:** Partial - only handler tests, not state machine tests

## Dependencies at Risk

### esp-hal with "unstable" Feature

**Package:** `esp-hal` with `unstable` feature flag
- **Risk:** Unstable APIs may change between minor versions
- **Impact:** Breaking changes on esp-hal updates
- **Migration plan:** Pin to specific commit hash or wait for stable 1.0 release

### embassy-time Version Mismatch

**Package:** `embassy-time` listed twice in Cargo.toml (lines 30 and 66)
- **Risk:** Potential version conflict or feature inconsistency
- **Impact:** Subtle timing behavior differences
- **Migration plan:** Consolidate to single dependency entry

### heapless 0.8.0 Limitations

**Package:** `heapless` 0.8.0
- **Risk:** Fixed-capacity collections may overflow
- **Impact:** String truncation or data loss in error messages
- **Migration plan:** Audit all heapless usage for capacity requirements

## Missing Critical Features

### Roast Profile Persistence

**Problem:** No way to save/load roast profiles
**Blocks:** Recipe-based roasting automation

### Data Logging and Export

**Problem:** Temperature and event data not persisted
**Blocks:** Roast analysis and quality control

### Watchdog Timer

**Problem:** No software watchdog for system recovery
**Blocks:** Reliability in unattended operation

### Calibration Interface

**Problem:** Thermocouple offsets are compile-time constants
**Blocks:** Field calibration without firmware updates

## Test Coverage Gaps

### Untested Hardware Paths

- **What's not tested:** SPI sensor initialization failures, UART receive timeouts
- **Files:** `src/hardware/max31856.rs`, `src/hardware/uart/driver.rs`
- **Risk:** Hardware failure modes undetected until deployment
- **Priority:** High - critical for reliability

### PID Controller Not Tested

- **What's not tested:** PID computation, tuning parameters, windup prevention
- **File:** `src/control/pid.rs`
- **Risk:** Incorrect temperature control during roast
- **Priority:** High - core functionality

### Integration Tests Missing

- **What's not tested:** Full roast cycle from start to cool-down
- **Files:** No integration test directory
- **Risk:** Component interactions not validated
- **Priority:** Medium

### Error Recovery Paths Not Tested

- **What's not tested:** Sensor timeout recovery, emergency restart
- **Files:** `src/error/app_error.rs`, `src/control/roaster_refactored.rs`
- **Risk:** System may not recover from error states gracefully
- **Priority:** Medium

---

*Concerns audit: 2026-02-04*
