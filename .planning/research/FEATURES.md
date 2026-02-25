# Feature Research: SSR Refactoring and Test Infrastructure

**Domain:** Embedded Rust coffee roaster control - SSR hardware control and testing infrastructure
**Researched:** 2026-02-24
**Project:** LibreRoaster - ESP32-C3 firmware with Artisan+ protocol compatibility
**Confidence:** HIGH (codebase verified) / MEDIUM (ecosystem patterns)

---

## Executive Summary

This document categorizes features for the SSR (Solid State Relay) refactoring milestone and shared test stubs infrastructure. Based on research of the existing codebase and embedded Rust ecosystems, features are organized into:

- **Table stakes** — features that must exist for the system to function properly
- **Differentiators** — features that provide competitive advantage  
- **Anti-features** — features to deliberately NOT build

The existing codebase already has significant SSR infrastructure (`SsrControl`, `SsrControlSimple`, `Heater` trait) and test mocks (`MockUsbCdcDriver`, `FakeDetectPin`). This research identifies gaps and recommended improvements for this milestone.

---

## Table Stakes

Features users expect. Missing or broken implementations cause system failure.

### SSR Control Features

| Feature | Why Expected | Complexity | Dependencies | Notes |
|---------|--------------|------------|--------------|-------|
| **SSR on/off control** | Basic heating element control via GPIO | Low | `OutputPin` trait, LEDC PWM | Implemented via `set_percentage(0)` or `set_percentage(100)` |
| **SSR PWM/phase control** | Variable heating power (not just on/off) | Medium | LEDC channel, duty cycle mapping | Implemented with `percentage_to_ledc_duty()` |
| **Heat source detection** | Verify SSR is actually heating | Medium | `InputPin` trait, detection circuit | `detect_heat_source()` in existing code |
| **Cycle guard** | Prevent SSR damage from rapid cycling | Low | Timer/state tracking | `SsrCycleGuard` already exists |
| **Duty readback verification** | Confirm PWM duty matches commanded | Medium | LEDC duty readback | `monitor_ledc_after_set()` with retry logic |
| **Error state handling** | Graceful degradation when hardware fails | Low | Error enum, hardware status | `SsrError`, `SsrHardwareStatus` enums |
| **Heater trait implementation** | Abstract heating control for testability | Low | `Heater` trait | Implemented for both `SsrControl` and `SsrControlSimple` |

### Test Infrastructure Features

| Feature | Why Expected | Complexity | Dependencies | Notes |
|---------|--------------|------------|--------------|-------|
| **Mock GPIO pins** | Test without hardware | Low | `embedded-hal-mock` | `FakeDetectPin` implements `InputPin` trait |
| **Mock PWM channels** | Test LEDC control without ESP32 | Medium | `ChannelIFace`, `LedcDutyReader` | `FakeLedcChannel` in `tests/ssr_monitor.rs` |
| **Mock USB CDC driver** | Test Artisan protocol without USB hardware | Medium | `UsbCdcDriver` trait | `MockUsbCdcDriver` exists (668 lines) |
| **Unit test support** | Test business logic in isolation | Low | `#[cfg(test)]` modules | In `src/hardware/ssr.rs` |
| **Integration test framework** | Test component interactions | Medium | `ServiceContainer`, channels | Existing test suite in `tests/` |
| **Shared mock location** | Reuse mocks across test files | Low | Module organization | Mocks currently in individual test files |

### Safety Features

| Feature | Why Expected | Complexity | Dependencies | Notes |
|---------|--------------|------------|--------------|-------|
| **Watchdog integration** | Prevent runaway heating | Low | `WatchdogFeeder` | Integrated in `RoasterControl` |
| **Temperature limits** | Prevent over-temp conditions | Low | Thermometer reads | Should block heating above threshold |
| **Fault detection and reporting** | Notify Artisan of problems | Medium | Artisan protocol, status codes | Part of `SsrHardwareStatus` |

---

## Differentiators

Features that set LibreRoaster apart from other coffee roaster firmware.

### Advanced SSR Features

| Feature | Value Proposition | Complexity | Dependencies | Notes |
|---------|-------------------|------------|--------------|-------|
| **PID temperature control** | Precise temperature tracking of roast profiles | High | PID algorithm, temperature sensor, SSR output | Has `RoasterControl` with PID |
| **SSR duty cycle logging** | Record heating patterns for analysis | Medium | Storage, Artisan logging | Could add to `SystemStatus` |
| **Dual SSR channel support** | Control multiple heating elements | High | Additional SSR channels | Future consideration |

### Testing Infrastructure Differentiators

| Feature | Value Proposition | Complexity | Dependencies | Notes |
|---------|-------------------|------------|--------------|-------|
| **Property-based testing** | Test SSR behavior across input ranges | Medium | `proptest` or custom | Not yet implemented |
| **Hardware-in-the-loop (HIL) tests** | Run tests on actual ESP32 | High | ESP32-C3, probe-rs | `embedded-test` crate available |
| **MockHeater test double** | Test PID without real SSR | Low | `Heater` trait | Could be created |
| **Cross-platform test CI** | Run tests on multiple architectures | Medium | GitHub Actions, QEMU | Uses `std` feature already |
| **Protocol fuzzing** | Find edge cases in Artisan parsing | Medium | Fuzzing framework | Could integrate |

---

## Anti-Features

Features to explicitly NOT build. Common mistakes in this domain.

### SSR Anti-Features

| Anti-Feature | Why Avoid | What To Do Instead |
|--------------|-----------|---------------------|
| **Blocking PWM calls in hot path** | Blocks async executor, timing issues | Use async-safe LEDC driver |
| **Direct hardware access in app code** | Breaks testability | Use `Heater` trait abstraction |
| **Ignoring SSR cycle time limits** | Can damage SSR | Use `SsrCycleGuard` |
| **Hardcoded GPIO pin numbers** | Makes hardware changes difficult | Use board configuration |
| **No duty readback** | Silent failures | Keep retry logic |

### Test Infrastructure Anti-Features

| Anti-Feature | Why Avoid | What To Do Instead |
|--------------|-----------|---------------------|
| **Mocking everything** | Loses integration test value | Use mocks for units, real for HIL |
| **Tests only run on hardware** | Blocks CI | Maintain `#[cfg(test)]` host tests |
| **Manual mocks for all peripherals** | Duplication | Use `embedded-hal-mock` crate |
| **Tests without assertions** | No value | Verify expected behavior |

### Architecture Anti-Features

| Anti-Feature | Why Avoid | What To Do Instead |
|--------------|-----------|---------------------|
| **Global mutable state** | Race conditions | Use `ServiceContainer` |
| **Panic on hardware errors** | No recovery | Return `Result<T, SsrError>` |
| **Synchronous blocking I/O** | Blocks system | Use async/embassy |

---

## Feature Dependencies

```
SSR Control Flow:
┌─────────────────┐
│ Artisan Command │ (OT1, IO3)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ RoasterControl  │ ←── Heater trait
└────────┬────────┘
         │
         ▼
┌─────────────────┐     ┌──────────────────┐
│ SsrCycleGuard   │────▶│ SsrControlSimple │──▶ LEDC PWM
└─────────────────┘     └────────┬─────────┘
                                 │
                                 ▼
                        ┌──────────────────┐
                        │ Heat Source      │
                        │ Detection (Input)│
                        └──────────────────┘

Test Infrastructure:
┌─────────────────┐
│ MockUSBDriver   │ ← used by → Integration Tests
└────────┬────────┘
         │
         ▼
┌─────────────────┐     ┌──────────────────┐
│ FakeLedcChannel │────▶│ SSR Monitor Test │
└────────┬────────┘     └──────────────────┘
         │
         ▼
┌─────────────────┐
│ MockUartDriver  │ ← used by → Protocol Tests
└─────────────────┘
```

---

## MVP Recommendation

For this milestone (SSR refactoring + shared test stubs), prioritize:

### Phase 1: Table Stakes (Must Have)

1. **SSR refactoring** — Consolidate and improve existing SSR logic
   - Ensure `Heater` trait is consistently used
   - Verify cycle guard is applied everywhere
   
2. **Shared test stubs** — Centralize mock implementations
   - Move `FakeDetectPin`, `FakeLedcChannel` to shared location
   - Create `MockHeater` implementing `Heater` trait for tests
   - Add module documentation for mock patterns

### Phase 2: Testing Improvements

3. **Add `MockHeater` test double** — For PID/controller unit tests
4. **Improve mock USB driver** — Add more error injection capabilities
5. **Property-based tests** for SSR percentage conversion

### Phase 3: Differentiators (Future)

- HIL test setup with probe-rs
- Duty cycle logging
- PID auto-tuning

---

## Existing Implementation Assessment

### Already Implemented (Table Stakes)

| Feature | Location | Status |
|---------|----------|--------|
| SSR PWM control | `src/hardware/ssr.rs` | ✅ Complete |
| Heat source detection | `src/hardware/ssr.rs` | ✅ Complete |
| Cycle guard | `src/control/ssr_scheduler.rs` | ✅ Complete |
| Duty readback with retry | `src/hardware/ssr.rs` | ✅ Complete |
| `Heater` trait | `src/control/traits.rs` | ✅ Complete |
| Mock USB driver | `tests/mock_usb_driver.rs` | ✅ Complete |
| Fake LEDC channel | `tests/ssr_monitor.rs` | ✅ Complete |
| Unit tests | `src/hardware/ssr.rs` | ✅ Complete |

### Gap Analysis

| Feature | Status | Recommendation |
|---------|--------|----------------|
| Shared mock location | ❌ Mocks in individual test files | Create `tests/mocks/` module |
| `MockHeater` test double | ❌ Not created | Implement for PID tests |
| Property-based tests | ❌ Not implemented | Add `proptest` for SSR math |
| HIL tests | ❌ Not implemented | Add `embedded-test` setup |

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| SSR table stakes | HIGH | Verified via code review |
| Test infrastructure | HIGH | Existing mocks work, `embedded-hal-mock` well-documented |
| Differentiators | MEDIUM | Based on research of coffee roaster systems |
| Anti-features | HIGH | Common embedded patterns verified |

---

## Sources

- **embedded-hal-mock crate**: https://docs.rs/embedded-hal-mock/0.11.1/
- **embedded-test crate**: https://docs.rs/embedded-test/0.7.0/
- **Artisan PID control**: https://artisan-roasterscope.blogspot.com/2016/11/pid-control.html
- **ESP32 LEDC PWM**: https://medium.com/@7086cmd/generating-pwm-signals-on-bare-metal-rust-esp32-ae4aaf23cf38
- **Coffee roaster SSR control**: https://github.com/AlexMunt/coffee-roaster-software
- **Embedded Rust testing**: https://barretts.club/posts/embedded-tests/
- **Project code**: `/home/juan/Repos/LibreRoaster/src/hardware/ssr.rs`
- **Existing tests**: `/home/juan/Repos/LibreRoaster/tests/ssr_monitor.rs`, `tests/mock_usb_driver.rs`

---

*Feature research for: SSR refactoring and shared test stubs milestone*
