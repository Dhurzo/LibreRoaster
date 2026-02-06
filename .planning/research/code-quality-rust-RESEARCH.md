# Code Quality Patterns for Embedded Rust - Research

**Researched:** 2026-02-05
**Domain:** Embedded Rust code quality, embassy-rs framework
**Confidence:** HIGH

## Summary

This research covers code quality patterns for embedded Rust applications, specifically for embassy-rs projects like LibreRoaster. The embedded Rust ecosystem has mature solutions for error handling, testing, and linting that differ significantly from std-based Rust development. The key insight is that embedded Rust requires intentional design choices around no_std compatibility, panic handling, and async abstractions that aren't present in traditional embedded development.

For LibreRoaster's 39 flagged unwrap/panic/unsafe issues, the research identifies a clear path: replace anyhow with thiserror or snafu for error types, use embedded-hal-mock for peripheral testing, and apply cargo-clippy with embedded-specific lint configurations. The embassy-rs ecosystem provides embassy-embedded-hal for hardware abstraction patterns that enable testable code without hardware dependencies.

**Primary recommendation:** Adopt thiserror for error types, embedded-hal-mock for peripheral testing, and configure cargo-clippy with panic-related lint rules to systematically eliminate production panics from embedded code.

## Standard Stack

The established tools and libraries for embedded Rust code quality:

### Linting and Static Analysis

| Tool | Purpose | Why Standard |
|------|---------|-------------|
| cargo-clippy | 750+ lints for Rust code quality | Official Rust project linter, catches common mistakes and performance issues |
| cargo-geiger | Detects unsafe Rust usage | Essential for embedded where unsafe is higher risk |
| cargo-audit | Security vulnerability scanning | Checks RustSec advisory database for known vulnerabilities |
| cargo-deny | Dependency graph linting | Validates licenses, checks for duplicate dependencies |
| cargo-miri | Undefined behavior detection | Runs code in Miri interpreter to catch UB |

**Installation:**
```bash
cargo install cargo-clippy cargo-geiger cargo-audit cargo-deny
rustup component add miri
```

### Testing Frameworks

| Library | Purpose | When to Use |
|---------|---------|-------------|
| embedded-hal-mock | Mocking embedded-hal traits | Testing drivers without hardware, unit tests for peripheral access |
| embassy-mock | Embassy-specific test utilities | Testing embassy executor tasks, async patterns |
| embedded-test | Embedded test harness | Running tests on target hardware with defmt/log output |
| snafu | Error type derive macro | no_std compatible error handling with context |

**Installation:**
```toml
[dev-dependencies]
embedded-hal-mock = "0.11"
embassy-mock = "0.5"
embedded-test = "0.7"
snafu = "0.8"
```

### Error Handling Crates (no_std compatible)

| Crate | no_std Support | Purpose | Why Standard |
|-------|---------------|---------|--------------|
| thiserror | Yes (v2.0+) | Derive Error trait implementations | Minimal macro overhead, std optional |
| snafu | Yes | Error handling with context | Popular in embedded community |
| anyhow | No | Application error handling | Requires std, not for embedded |
| embedded-error-chain | Yes | Error chaining for embedded | Specialized for embedded workflows |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| thiserror | Manual enum implementation | Manual is more control but more boilerplate |
| embedded-hal-mock | Stub implementations | More work but no extra dependency |
| cargo-clippy | rustc warnings only | Missing 700+ specialized lints |

## Architecture Patterns

### Recommended Project Structure for embassy-rs

```
src/
├── hal/                    # Hardware Abstraction Layer
│   ├── mod.rs
│   ├── uart.rs            # UART trait + implementations
│   ├── gpio.rs            # GPIO abstractions
│   └── spi.rs             # SPI bus abstractions
├── peripherals/            # Peripheral configuration
│   ├── mod.rs
│   ├── init.rs            # Peripheral takeover pattern
│   └── config.rs          # Peripheral configuration structs
├── protocol/              # Protocol implementation
│   ├── mod.rs
│   ├── artisan.rs         # ARTISAN+ protocol
│   └── parser.rs          # Message parsing
├── application/           # Business logic
│   ├── mod.rs
│   ├── roaster_control.rs
│   └── temperature_manager.rs
├── error/                 # Error types (centralized)
│   ├── mod.rs
│   └── types.rs
└── main.rs
```

### Pattern 1: Error Handling Chain for no_std

**What:** Centralized error enum with thiserror derive, wrapped in embassy-futures::io::Error or custom error type

**When to use:** All error-producing operations that need to propagate through async boundaries

**Example:**
```rust
// src/error/types.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RoasterError {
    #[error("UART communication error: {source}")]
    Uart {
        #[from]
        source: embassy_hal_common::PeripheralError,
    },
    
    #[error("ARTISAN protocol parse error at byte {position}")]
    Parse {
        position: usize,
        #[source]
        source: core::num::TryFromIntError,
    },
    
    #[error("Temperature sensor timeout")]
    SensorTimeout,
    
    #[error("Invalid message checksum")]
    ChecksumMismatch {
        expected: u8,
        received: u8,
    },
}

impl From<nb::Error<embassy_stm32::usart::Error>> for RoasterError {
    fn from(e: nb::Error<embassy_stm32::usart::Error>) -> Self {
        match e {
            nb::Error::Other(e) => RoasterError::Uart { source: e },
            nb::Error::WouldBlock => RoasterError::SensorTimeout,
        }
    }
}

// Application-level Result type
pub type Result<T> = core::result::Result<T, RoasterError>;
```

### Pattern 2: Trait-Based Hardware Abstraction

**What:** Define traits for hardware operations, implement for real and mock types

**When to use:** Any code that accesses peripherals and needs testing without hardware

**Example:**
```rust
// src/hal/uart.rs
use embedded_hal::serial::Read;
use embedded_hal_nb::serial::Write;

pub trait RoasterUart:
    Read<Error = embassy_stm32::usart::Error> 
    + Write<Error = embassy_stm32::usart::Error>
{
    fn flush(&mut self) -> nb::Result<(), embassy_stm32::usart::Error>;
}

impl RoasterUart for embassy_stm32::usart::Uart<'_, embassy_stm32::peripherals::UART0, ()> {
    fn flush(&mut self) -> nb::Result<(), embassy_stm32::usart::Error> {
        // Implementation
        Ok(())
    }
}

// In tests/uart_mock.rs - for testing without hardware
#[cfg(test)]
pub mod mock {
    use embedded_hal_mock::{MockPinState, SerialConnection};
    
    pub struct MockUart {
        connection: SerialConnection<u8, u8>,
        state: MockPinState,
    }
    
    impl embedded_hal::serial::Read<u8> for MockUart {
        type Error = <SerialConnection as embedded_hal_mock::Serial>::Error;
        fn read(&mut self) -> nb::Result<u8, Self::Error> {
            self.connection.read()
        }
    }
    
    // Implement Write and flush similarly
}
```

### Pattern 3: Async Task Boundary with Error Propagation

**What:** Use ? operator within async tasks, convert HAL errors to app errors at task boundaries

**When to use:** Any embassy spawn_fn or task that calls HAL methods

**Example:**
```rust
// src/application/temperature_reader.rs
use crate::error::{RoasterError, Result};

#[embassy_executor::task]
async fn temperature_reader(
    mut uart: impl RoasterUart + Send + 'static,
    tx: Signal<Option<u16>>,
) -> Result<!> {
    loop {
        // Read temperature from roaster via UART
        let response = read_temperature_response(&mut uart).await?;
        
        // Parse and send
        let temp = parse_temperature(response)?;
        tx.signal(Some(temp));
        
        // Handle errors from HAL - convert to app errors
        embassy_time::Timer::after_secs(1).await;
    }
}

async fn read_temperature_response(uart: &mut impl RoasterUart) -> Result<[u8; 64]> {
    let mut buffer = [0u8; 64];
    let bytes_read = uart.read(&mut buffer).await
        .map_err(|e| RoasterError::from(e))?;
    Ok(buffer)
}
```

### Anti-Patterns to Avoid

- **anyhow in no_std:** anyhow requires std and cannot be used in embassy-rs projects
- **Panic in async tasks:** A panic in one embassy task crashes the entire executor
- **Unconditional unwrap in HAL calls:** unwrap() on Result from peripheral methods will panic if hardware is busy
- **Blocking in async context:** Using blocking HAL methods inside async tasks blocks the entire executor

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Error enum with Display | Manual implementation | thiserror | Derives Debug, Display, From automatically; zero runtime cost |
| HAL trait mocking | Custom stub types | embedded-hal-mock | Provides ready-made mocks for all embedded-hal traits |
| Dependency vulnerability checking | Manual crate review | cargo-audit | Checks RustSec database; runs automatically in CI |
| Panic-free compilation | Manual code review | #![no_panic] attribute + cargo no-panic | Compile-time guarantee no panic! calls |
| Peripheral initialization | Singleton pattern | embassy-take pattern | Proper ownership transfer, prevents double-init |

### Key insight: thiserror in no_std

Unlike anyhow which requires std, thiserror v2.0+ supports no_std and is the standard solution for embedded error types. The library generates only the Error trait implementation without any std dependencies.

```toml
[dependencies]
thiserror = { version = "2.0", features = ["no-std-compat"] }
```

### Key insight: embedded-hal-mock ecosystem

The embedded-hal-mock crate provides mock implementations for the embedded-hal traits (v0.2, v1.0, and async variants). This is the standard approach for testing embedded drivers without requiring actual hardware. It supports:

- Mock UART, SPI, I2C connections
- Configurable expectations and behavior
- Integration with embedded-hal-async traits

## Common Pitfalls

### Pitfall 1: unwrap() in Production Embedded Code

**What goes wrong:** The Cloudflare outage on November 18, 2025 was caused by an unhandled unwrap() on a Result that unexpectedly contained an error. In embedded context, this crashes the entire application.

**Why it happens:** Developers use unwrap() during prototyping and forget to replace it with proper error handling. The "happy path" works, but edge cases (communication timeouts, hardware busy states) trigger panics.

**How to avoid:**
1. Configure clippy to warn on unwrap usage:
   ```toml
   # clippy.toml
   disallow = ["unwrap_used", "expect_used", "panic"]
   ```
2. Use the no-panic compiler plugin:
   ```toml
   [dependencies]
   no_panic = "0.2"
   ```
3. Create a custom Result type that forces handling:
   ```rust
   pub type Result<T> = core::result::Result<T, RoasterError>;
   // Methods that return Result must be handled
   ```

**Warning signs:**
- Code contains `unwrap()`, `expect()`, `panic!()` calls
- HAL methods are called without error conversion
- Error types exist but aren't propagated

### Pitfall 2: panic!() in Async Tasks

**What goes wrong:** A panic in any embassy task crashes the entire executor. Other tasks stop executing, hardware may be left in undefined state.

**Why it happens:** Embassy tasks are not separate processes; they're state machines managed by a single executor. Unwinding is not always available or safe on embedded.

**How to avoid:**
1. Never call panic!() in task code
2. Replace panic!() with error logging and state machine transition:
   ```rust
   // Instead of:
   panic!("Sensor disconnected");
   
   // Use:
   error!("Sensor disconnected, entering safe mode");
   self.state = RoasterState::Error(ErrorState::SensorDisconnected);
   ```
3. Use defmt::panic!() or embassy's panic handler if panic is truly unrecoverable

**Warning signs:**
- Task code contains panic macros
- No error state handling in state machines
- Critical errors cause immediate termination

### Pitfall 3: Blocking in Embassy Executors

**What goes wrong:** Blocking HAL calls (blocking UART read, busy-wait loops) block the entire embassy executor, preventing other tasks from running.

**Why it happens:** Embassy uses cooperative multitasking. If one task blocks, no other task can make progress.

**How to avoid:**
1. Use async HAL methods (embassy-time, async UART read/write)
2. Never call `.wait()` or block_on() from within a task
3. For blocking operations, spawn them on a dedicated thread if available, or use critical sections appropriately

**Warning signs:**
- Code calls `.blocking_read()` or similar blocking methods in async context
- Uses `core::hint::spin_loop()` for timing
- No yields during long operations

### Pitfall 4: Heap Allocations in no_std

**What goes wrong:** Using Vec, Box, String, or other heap-allocating types in no_std causes compilation errors or undefined behavior at runtime.

**Why it happens:** Standard library types default to heap allocation. In no_std, you must use `alloc::` types explicitly and configure a global allocator.

**How to avoid:**
1. Use stack-allocated arrays for fixed-size buffers
2. Use `heapless` crate for bounded collections:
   ```rust
   use heapless::Vec;
   let mut buffer: Vec<u8, 64> = Vec::new();
   ```
3. Configure global allocator if heap is needed:
   ```rust
   #[global_allocator]
   static ALLOC: embedded_alloc::Heap = embedded_alloc::Heap::empty();
   ```

**Warning signs:**
- Uses `Vec`, `Box`, `String` without `alloc` feature
- Collects iterators into Vec
- Dynamic sizing based on runtime values

## Code Examples

### Safe Error Handling Pattern for embassy-rs

```rust
// src/error/mod.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Peripheral error: {0}")]
    Peripheral(#[from] embassy_stm32::usart::Error),
    
    #[error("Protocol timeout after {ms}ms")]
    Timeout { ms: u32 },
    
    #[error("Invalid frame checksum")]
    Checksum,
    
    #[error("Buffer overflow: needed {needed}, had {had}")]
    Overflow { needed: usize, had: usize },
}

pub type Result<T> = core::result::Result<T, Error>;

// src/hal/artisan_protocol.rs
use crate::error::{Error, Result};

pub struct ArtisanProtocol<U: embedded_hal_nb::serial::Read + embedded_hal_nb::serial::Write> {
    uart: U,
    buffer: heapless::Vec<u8, 128>,
}

impl<U: embedded_hal_nb::serial::Read + embedded_hal_nb::serial::Write> ArtisanProtocol<U> {
    pub async fn read_frame(&mut self) -> Result<heapless::Vec<u8, 128>> {
        self.buffer.clear();
        
        loop {
            // Read one byte with timeout awareness
            let byte = self.uart.read().await
                .map_err(|e| Error::Peripheral(e.into()))?;
            
            self.buffer.push(byte)
                .map_err(|_| Error::Overflow { 
                    needed: self.buffer.len() + 1, 
                    had: self.buffer.capacity() 
                })?;
            
            // Check for frame delimiter or buffer full
            if byte == 0x0A && self.buffer.last() == Some(&0x0D) {
                break;
            }
        }
        
        Ok(self.buffer.clone())
    }
}
```

### Unit Test Structure for embassy-rs

```rust
// tests/artisan_protocol.rs
#[cfg(test)]
mod tests {
    use embedded_hal_mock::{
        MockPinState, SerialConnection, Transaction,
        common::摸拟::Mock,
    };
    use crate::artisan_protocol::ArtisanProtocol;
    
    fn setup() -> (SerialConnection<u8, u8>, ArtisanProtocol<Mock>) {
        let serial = SerialConnection::new(
            Mock::new(&[
                Transaction::read(0xAA),
                Transaction::read(0xBB),
                Transaction::read(0x0D),
                Transaction::read(0x0A),
            ]),
            MockPinState::High,
        );
        let protocol = ArtisanProtocol::new(serial);
        (serial, protocol)
    }
    
    #[test]
    fn test_frame_parsing() {
        let (mut serial, mut protocol) = setup();
        
        let frame = embassy_executor::block_on(protocol.read_frame());
        
        assert_eq!(frame.unwrap(), [0xAA, 0xBB, 0x0D, 0x0A]);
    }
    
    #[test]
    fn test_checksum_error() {
        // Test error handling path
    }
}
```

### Mocking UART/Peripheral Patterns

```rust
// tests/mocks/uart.rs
use embedded_hal_mock::SerialConnection;
use heapless::Vec;

pub struct MockUart {
    read_buffer: Vec<u8, 256>,
    write_buffer: Vec<u8, 256>,
    read_index: usize,
}

impl MockUart {
    pub fn new(data: &[u8]) -> Self {
        Self {
            read_buffer: Vec::from_slice(data).unwrap(),
            write_buffer: Vec::new(),
            read_index: 0,
        }
    }
    
    pub fn written(&self) -> &[u8] {
        &self.write_buffer
    }
}

impl embedded_hal_nb::serial::Read<u8> for MockUart {
    type Error = core::convert::Infallible;
    
    fn read(&mut self) -> nb::Result<u8, Self::Error> {
        if self.read_index < self.read_buffer.len() {
            let byte = self.read_buffer[self.read_index];
            self.read_index += 1;
            Ok(byte)
        } else {
            Err(nb::Error::WouldBlock)
        }
    }
}

impl embedded_hal_nb::serial::Write<u8> for MockUart {
    type Error = core::convert::Infallible;
    
    fn write(&mut self, byte: u8) -> nb::Result<(), Self::Error> {
        self.write_buffer.push(byte).map_err(|_| nb::Error::Other(()))
    }
    
    fn flush(&mut self) -> nb::Result<(), Self::Error> {
        Ok(())
    }
}
```

## Code Quality Audit Approach

### Systematic Audit of 44 Files

1. **Phase 1: Inventory Collection**
   ```bash
   # Find all unwrap/panic/unsafe usages
   grep -rn "unwrap()" src/ --include="*.rs" | wc -l
   grep -rn "panic!" src/ --include="*.rs" | wc -l
   grep -rn "unsafe" src/ --include="*.rs" | wc -l
   ```

2. **Phase 2: Classification**
   - Critical: Any unwrap in task code that could receive unexpected hardware state
   - High: panic! calls that could be error state transitions
   - Medium: unwrap in initialization (usually safe if init is checked)
   - Low: unwrap in test code (acceptable in #[cfg(test)])

3. **Phase 3: Remediation Priority**
   ```
   Priority 1: unwrap in async tasks → Convert to proper Result handling
   Priority 2: panic! calls → Replace with error logging + state transition
   Priority 3: unsafe blocks → Review necessity, add safety comments
   Priority 4: expect calls → Document why this case shouldn't happen
   ```

### Prioritization Strategy for 39 Flagged Issues

1. **Categorize by impact:**
   - Production-critical: Any issue in task code that affects roaster control
   - Error-handling gaps: Missing error propagation paths
   - Development-only: Issues in test code only

2. **Group by module:**
   - HAL/peripherals: Fix unwrap → Result conversion
   - Protocol: Add checksum validation errors
   - Application: Add state machine error transitions
   - Main: Fix panic handler, ensure graceful shutdown

3. **Verification approach:**
   - Run cargo clippy before and after to measure improvement
   - Add clippy lint checks to CI pipeline
   - Run cargo-geiger to ensure unsafe is justified
   - Add cargo deny checks for dependency security

### Verification That Refactoring Doesn't Break Functionality

1. **Before refactoring:**
   - Capture current behavior with integration tests
   - Document expected error cases
   - Create protocol frame examples for ARTISAN+

2. **During refactoring:**
   - Run unit tests after each module fix
   - Use embedded-hal-mock to verify protocol parsing
   - Test error paths with mock UART that returns error bytes

3. **After refactoring:**
   - Full integration test on hardware
   - Verify error cases trigger appropriate state transitions
   - Confirm no unwrap remains in production code paths

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|-----------------|--------------|--------|
| anyhow for errors | thiserror/snafu for no_std | 2024+ | Enables embedded usage |
| blocking HAL | embassy async HAL | 2022+ | Non-blocking, cooperative multitasking |
| manual unsafe for peripherals | embassy HAL abstractions | 2021+ | Memory safety guaranteed by compiler |
| panic on error | error state machines | 2023+ | Graceful degradation |

**Deprecated/outdated:**
- `nb` crate error handling: Moving to embedded-hal-async trait variants
- embassy-time v0.3: Replaced by embassy-time with TimerQueue
- embedded-storage v0.3: Async variants now preferred

## Open Questions

1. **ARTISAN+ protocol error recovery:**
   - What's the expected behavior on checksum failure? Retry, abort, safe mode?
   - Research needed: ARTISAN+ protocol specification for LibreRoaster

2. **embassy-esp32c3 specific patterns:**
   - ESP32-C3 embassy integration may have unique initialization patterns
   - Verify: embassy-esp32c3 vs embassy-executor integration

3. **Testing hardware interaction:**
   - embedded-hal-mock covers unit tests but integration tests still need hardware
   - Consider: defmt integration for on-device test output

## Sources

### Primary (HIGH confidence)
- [Embassy Book - Project Structure](https://embassy.dev/book/) - Official embassy-rs documentation on project organization
- [embedded-hal-mock crate](https://docs.rs/embedded-hal-mock/latest/embedded_hal_mock/) - Official documentation for mocking embedded-hal traits
- [thiserror crate - no_std support](https://github.com/dtolnay/thiserror/issues/196) - GitHub issue confirming no_std support
- [rust-clippy documentation](https://doc.rust-lang.org/stable/clippy/) - Official Rust linter documentation

### Secondary (MEDIUM confidence)
- [Cloudflare outage November 2025](https://hackaday.com/2025/11/20/how-one-uncaught-rust-exception-took-out-cloudflare/) - Production panic case study
- [embedded-hal-mock GitHub](https://github.com/dbrgn/embedded-hal-mock) - Community-maintained, widely used for embedded testing
- [cargo-deny documentation](https://embarkstudios.github.io/cargo-deny/) - Embark Studios maintained dependency linting

### Tertiary (LOW confidence)
- [embedded-test crate](https://docs.rs/embedded-test/latest/embedded_test/) - Emerging test harness, less established than alternatives
- [embassy-mock crate](https://docs.rs/embassy-mock/latest/embassy_mock/) - Unofficial embassy mocking, verify compatibility with embassy version

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Well-established tools in embedded Rust ecosystem
- Architecture patterns: HIGH - Embassy book provides authoritative guidance
- Pitfalls: HIGH - Well-documented production failures (Cloudflare case)
- Code examples: MEDIUM - Patterns adapted from embassy docs and embedded-hal-mock examples

**Research date:** 2026-02-05
**Valid until:** 2026-08-05 (6 months for fast-moving embedded ecosystem)
