# Plan: Phase 21 - Logging Foundation

**Phase:** 21
**Goal:** Secure non-blocking logging infrastructure using `defmt` and `bbqueue`.
**Requirement:** LOG-06
**Date:** 2026-02-05

---

## Overview

This plan implements the logging foundation for LibreRoaster using the deferred logging pattern. The goal is to ensure that log calls are mere memory writes that do not block the Embassy executor or the 100ms PID control loop.

---

## Plan 01: Dependencies and Configuration

**Objective:** Add required dependencies and configure the build system for `defmt` and `bbqueue`.

### Tasks

```xml
<task>
<name>Add defmt and bbqueue dependencies</name>
<description>
Edit Cargo.toml to add:
- defmt = "0.3"
- bbqueue = "0.5"
- esp-println = { version = "0.11", features = ["defmt"] }

Ensure no_std compatibility is maintained.
</description>
<autonomous>true</autonomous>
<files_modified>
Cargo.toml
</files_modified>
</task>

<task>
<name>Configure defmt build targets</name>
<description>
Create or update .cargo/config.toml with:
- Build target: riscv32imc-unknown-none-elf
- Runner: cargo embed or probe-rs for defmt output

Verify defmt-rtt integration is enabled.
</description>
<autonomous>true</autonomous>
<files_modified>
.cargo/config.toml
</files_modified>
</task>

<task>
<name>Add memory configuration</name>
<description>
Create or update memory.x linker script if needed for BBQueue buffer allocation.

Recommend: 2KB static buffer for initial testing.
</description>
<autonomous>true</autonomous>
<files_modified>
memory.x
</files_modified>
</task>
```

### Verification
- [ ] `cargo build` succeeds with no warnings.
- [ ] Defmt format strings are recognized by the linker.
- [ ] Buffer size is verified in the linker map.

### Must Haves
- [ ] `defmt::info!` macros compile successfully.
- [ ] `bbqueue` compiles without std dependencies.
- [ ] Build target is correctly configured.

---

## Plan 02: Global Logging Infrastructure

**Objective:** Initialize the global BBQueue buffer and verify non-blocking behavior.

### Tasks

```xml
<task>
<name>Create logging module</name>
<description>
Create src/logging/mod.rs with:
- Global BBQueue static buffer (2KB)
- Producer and Consumer handles
- Initialization function with panic on failure
- Non-blocking write implementation

Pattern: Singleton with try_split() for producer/consumer.
</description>
<autonomous>true</autonomous>
<files_modified>
src/logging/mod.rs
src/lib.rs (add module export)
</files_modified>
</task>

<task>
<name>Verify non-blocking writes</name>
<description>
Write a unit test that:
- Creates 1000 log messages rapidly
- Measures total time elapsed
- Asserts total time < 10ms (arbitrary threshold for "non-blocking")

Run with: cargo test --release
</description>
<autonomous>true</autonomous>
<files_modified>
src/logging/mod.rs (add tests)
</files_modified>
</task>

<task>
<name>Document logging API</name>
<description>
Create src/logging/README.md with:
- How to use defmt macros
- Buffer overflow behavior
- Examples for common logging patterns
</description>
<autonomous>true</autonomous>
<files_modified>
src/logging/README.md
</files_modified>
</task>
```

### Verification
- [ ] Initialization panics if buffer allocation fails.
- [ ] Non-blocking test passes (elapsed time < threshold).
- [ ] Documentation covers basic usage.

### Must Haves
- [ ] Global BBQueue buffer is accessible from any task.
- [ ] Write operations return immediately.
- [ ] Initialization failure triggers panic.

---

## Plan 03: Integration and Verification

**Objective:** Integrate logging into the existing codebase and verify system stability.

### Tasks

```xml
<task>
<name>Integrate defmt-rtt</name>
<description>
Add defmt-rtt initialization to src/main.rs:
```rust
#[entry]
fn main() -> ! {
    // ... existing initialization ...
    defmt_rtt::init();
    // ... continue ...
}
```

Ensure defmt-rtt is initialized before any log calls.
</description>
<autonomous>true</autonomous>
<files_modified>
src/main.rs
</files_modified>
</task>

<task>
<name>Add test log statements</name>
<description>
Add sample log statements to existing tasks:
- usb_reader_task: Log "USB reader started"
- control task: Log PID loop timing

This validates the infrastructure end-to-end.
</description>
<autonomous>true</autonomous>
<files_modified>
src/hardware/usb_cdc/tasks.rs
src/control/mod.rs
</files_modified>
</task>

<task>
<name>Executor stability test</name>
<description>
Verify that logging does NOT block the Embassy executor:
1. Send 1000+ log messages rapidly from a test task
2. Verify the executor continues to function (other tasks can still run)
3. Measure maximum blocking time of a log call

Note: PID loop does not exist yet in LibreRoaster. This test verifies general non-blocking behavior.

Document findings in src/logging/PERFORMANCE.md
</description>
<autonomous>false</autonomous>
<files_modified>
src/logging/PERFORMANCE.md
</files_modified>
</task>

```

### Verification
- [ ] Application compiles and runs with logging active.
- [ ] Log output is visible via probe-rs or cargo-embed.
- [ ] Executor remains responsive under heavy logging load.

### Must Haves
- [ ] Logs appear in defmt-compatible viewer.
- [ ] No dropped log messages during normal operation.
- [ ] Executor stability verified (logging does not cause starvation).

---

## Dependencies Between Plans

```
Plan 01 (Dependencies)
    │
    ▼
Plan 02 (Infrastructure)
    │
    ▼
Plan 03 (Integration & Verification)
```

---

## Waves

| Wave | Plans | What it builds |
|------|-------|----------------|
| 1 | 01 | Build system ready for defmt + bbqueue |
| 2 | 02 | Global logging infrastructure working |
| 3 | 03 | Full integration and verification |

---

## Summary

This phase establishes the foundation for LibreRoaster's logging system. By the end of Phase 21:
- The project will have defmt and bbqueue integrated.
- Log calls will be non-blocking memory writes.
- The PID loop will be protected from logging overhead.

Phase 22 will add the async transport to drain logs to hardware.
Phase 23 will implement Artisan traffic sniffing.

---

*Plan created: 2026-02-05*
