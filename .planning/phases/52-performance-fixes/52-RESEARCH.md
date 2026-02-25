# Phase 52: Performance Fixes - Research

**Researched:** 2026-02-18
**Domain:** ESP32-C3 embedded systems, async I/O, LEDC timer configuration
**Confidence:** HIGH

## Summary

This phase addresses two performance issues in the LibreRoaster ESP32-C3 firmware:

1. **MAX31856 Blocking I/O**: The original implementation used a spin loop for the 160ms temperature conversion delay, blocking the async executor. The fix uses `embassy-time::Timer` for non-blocking async delays.

2. **LEDC Timer Conflicts**: SSR (1Hz zero-crossing) and Fan (25kHz) were sharing a timer, causing PWM frequency conflicts. The fix assigns Timer0 to SSR and Timer1 to Fan.

The implementation follows the decisions in CONTEXT.md exactly: async token pattern with `Result<f32, Error>`, one-shot reads without cancellation, in-place temperature reading, separate timer assignments, and retry logic with 2 retries (3 total attempts) using fixed duration delays.

**Primary recommendation:** Use the existing `embassy-time::Timer` for async delays and ensure Timer0 (SSR) and Timer1 (Fan) remain separate in LEDC configuration.

## Standard Stack

The established libraries/tools for this embedded ESP32-C3 project:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| embassy-time | 0.5.0 | Async delay/timer functionality | Embassy ecosystem standard for embedded async |
| esp-hal | ~1.0 | ESP32-C3 hardware abstraction | Official ESP-IDF HAL |
| embedded-hal | 1.0.0 | Hardware traits | Cross-platform embedded standard |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| esp32c3 | 0.31.0 | ESP32-C3 target | RISC-V embedded target |
| embassy-executor | 0.9.1 | Async task executor | Running async main function |

### Implementation Details
| Component | Value | Purpose |
|-----------|-------|---------|
| SSR PWM Frequency | 1 Hz | Zero-crossing AC control |
| Fan PWM Frequency | 25 kHz | Silent operation (above audible range) |
| SSR LEDC Timer | Timer0 | Low frequency for zero-crossing |
| Fan LEDC Timer | Timer1 | High frequency for silent operation |
| Temperature Read Delay | 160 ms | MAX31856 conversion time |
| Retry Delay | 10 ms | Fixed duration between retries |
| Max Retries | 2 | Total 3 attempts before failure |

## Architecture Patterns

### Recommended Project Structure
```
src/
├── hardware/
│   ├── max31856.rs      # Thermocouple interface (async + blocking)
│   ├── ledc_bus.rs      # LEDC channel management
│   └── ...
├── control/
│   ├── traits.rs        # Thermometer, AsyncThermometer traits
│   └── ...
├── config/
│   └── constants.rs     # Timer assignments, frequencies
└── main.rs             # Timer configuration
```

### Pattern 1: Async Temperature Reading
**What:** Non-blocking temperature read using embassy-time Timer
**When to use:** When the async executor needs to run other tasks during I/O wait
**Implementation:**
```rust
// Source: src/hardware/max31856.rs (lines 84-115)
pub async fn read_temperature_async(&mut self) -> Result<f32, Max31856Error> {
    self.write_register(0x80, 0x80)?; // Set one-shot bit
    
    // Non-blocking delay - allows executor to run other tasks
    Timer::after(Duration::from_millis(160)).await;
    
    let temp_data = self.read_registers(0x0C, 3)?;
    // ... process temperature
}
```

### Pattern 2: Retry with Fixed Delay
**What:** Retry logic with fixed delay between attempts (not exponential backoff)
**When to use:** When transient failures are possible but recovery is quick
**Implementation:**
```rust
// Source: src/hardware/max31856.rs (lines 120-137)
pub async fn read_with_retry(&mut self, max_retries: u8) -> Result<f32, Max31856Error> {
    let mut last_error = Max31856Error::CommunicationError;
    
    for attempt in 0..=max_retries {
        match self.read_temperature_async().await {
            Ok(temp) => return Ok(temp),
            Err(e) => {
                last_error = e;
                if attempt < max_retries {
                    Timer::after(Duration::from_millis(10)).await;
                }
            }
        }
    }
    Err(last_error)
}
```

### Pattern 3: Separate LEDC Timer Assignment
**What:** Different LEDC timers for different PWM frequencies
**When to use:** When multiple PWM outputs require vastly different frequencies
**Implementation:**
```rust
// Source: src/main.rs (lines 93-119)
// Timer0 for SSR (~1Hz for zero-crossing)
let mut ssr_timer = ledc.timer(timer::Number::Timer0);
ssr_timer.configure(TimerConfig {
    duty: timer::config::Duty::Duty8Bit,
    clock_source: timer::LSClockSource::APBClk,
    frequency: esp_hal::time::Rate::from_hz(SSR_PWM_FREQUENCY_HZ), // 1 Hz
})?;

// Timer1 for Fan (25kHz for silent operation)
let mut fan_timer = ledc.timer(timer::Number::Timer1);
fan_timer.configure(TimerConfig {
    duty: timer::config::Duty::Duty8Bit,
    clock_source: timer::LSClockSource::APBClk,
    frequency: esp_hal::time::Rate::from_hz(FAN_PWM_FREQUENCY_HZ), // 25000 Hz
})?;
```

### Pattern 4: Async Thermometer Trait
**What:** Separate async trait from sync trait for embedded async
**When to use:** When async methods are needed but trait must remain dyn-compatible
**Source:** src/control/traits.rs
```rust
pub trait Thermometer: Send {
    fn read_temperature(&mut self) -> Result<f32, RoasterError>;
}

pub trait AsyncThermometer: Send {
    async fn read_temperature_async(&mut self) -> Result<f32, RoasterError>;
}
```

### Anti-Patterns to Avoid
- **Blocking spin loop in async context:** Using `for _ in 0..N { spin_loop() }` blocks the entire executor. Use `Timer::after()` instead.
- **Sharing LEDC timer for different frequencies:** Multiple channels sharing a timer must use the same frequency. SSR at 1Hz and Fan at 25kHz cannot share a timer.
- **Exponential backoff for deterministic I/O:** For hardware like MAX31856 where failure is typically transient (SPI noise), fixed delay is simpler and sufficient.

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Async delay | Custom busy-wait loop | embassy-time::Timer | Properly yields to executor, allows task switching |
| LEDC channel management | Manual hardware registers | esp-hal LEDC channel API | Type-safe, handles edge cases |
| Temperature conversion | Manual bit manipulation | Existing MAX31856 code | Handles negative temps, fault detection |
| Error type | Create new error enum variants | Map to RoasterError | Consistent error handling across codebase |

**Key insight:** In embedded systems, using the official HAL libraries (esp-hal, embassy-time) provides better integration with the runtime (executor, interrupts) and handles hardware edge cases that custom solutions miss.

## Common Pitfalls

### Pitfall 1: Spin Loop Blocking Executor
**What goes wrong:** Temperature reading blocks all other async tasks for 160ms
**Why it happens:** Using `core::hint::spin_loop()` in a tight loop prevents the async executor from running other tasks
**How to avoid:** Use `embassy_time::Timer::after()` which yields to the executor
**Warning signs:** Other async tasks not running, system appears unresponsive during temperature reads

### Pitfall 2: LEDC Timer Conflict
**What goes wrong:** SSR PWM at 1Hz and Fan PWM at 25kHz conflict when sharing a timer
**Why it happens:** LEDC channels sharing a timer must use the same frequency - there's only one frequency divider per timer
**How to avoid:** Assign separate timers (Timer0 for SSR, Timer1 for Fan)
**Warning signs:** Fan frequency drops to 1Hz (audible hum), or SSR toggles at 25kHz (relay chatter)

### Pitfall 3: Missing Fault Detection
**What goes wrong:** Invalid temperatures accepted without checking fault register
**Why it happens:** MAX31856 sets fault bits for open thermocouple, short circuit, etc.
**How to avoid:** Check fault register (0x0F) after reading temperature
**Warning signs:** Erratic temperature readings (-1 or very high values)

### Pitfall 4: Infinite Retry
**What goes wrong:** Retry logic loops forever on persistent hardware failure
**Why it happens:** No retry limit, transient failure becomes infinite wait
**How to avoid:** Limit retries (e.g., 2 retries = 3 total attempts), then fail
**Warning signs:** System hangs on sensor failure, no error reported to caller

## Code Examples

### MAX31856 Async Read with Retry
```rust
// Source: src/hardware/max31856.rs (lines 192-200)
impl<SPI> AsyncThermometer for Max31856<SPI>
where
    SPI: SpiDevice + Send,
{
    async fn read_temperature_async(&mut self) -> Result<f32, RoasterError> {
        // Use read_with_retry for reliability (max_retries=2 = 3 attempts)
        Self::read_with_retry(self, 2).await.map_err(|e| e.into())
    }
}
```

### LEDC Timer Configuration Constants
```rust
// Source: src/config/constants.rs (lines 16-21)
pub const FAN_PWM_FREQUENCY_HZ: u32 = 25000;     // 25 kHz
pub const SSR_PWM_FREQUENCY_HZ: u32 = 1;          // 1 Hz
pub const FAN_LEDC_TIMER: u8 = 1;                 // Timer1
pub const SSR_LEDC_TIMER: u8 = 0;                 // Timer0
```

### Error Type Mapping
```rust
// Source: src/hardware/max31856.rs (lines 13-21)
impl From<Max31856Error> for RoasterError {
    fn from(e: Max31856Error) -> Self {
        match e {
            Max31856Error::CommunicationError => RoasterError::SensorFault,
            Max31856Error::FaultDetected => RoasterError::SensorFault,
            Max31856Error::InvalidTemperature => RoasterError::TemperatureOutOfRange,
        }
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Blocking spin loop (160ms) | embassy-time::Timer async | Phase 52 | Non-blocking I/O, system responsiveness |
| Shared LEDC timer | Separate Timer0/Timer1 | Phase 52 | SSR at 1Hz, Fan at 25kHz |
| Single read attempt | 3 attempts with retry | Phase 52 | Improved reliability |
| No fault checking | Fault register check | Pre-existing | Validates temperature validity |

**Deprecated/outdated:**
- Spin loop delays: Replaced with `embassy_time::Timer` for proper async behavior

## Open Questions

1. **Cancellation Support**
   - What we know: Current implementation is one-shot without cancellation token
   - What's unclear: Whether cancellation is needed for longer operations (e.g., multi-sample reads)
   - Recommendation: Current design is sufficient for single temperature reads; add cancellation if needed for future features

2. **Timer Hardware Constraints**
   - What we know: ESP32-C3 has 4 LEDC timers (Timer0-3), using Timer0 and Timer1
   - What's unclear: Whether other timers are needed for additional PWM outputs
   - Recommendation: Reserve Timer2-3 for future expansion if needed

3. **Error Type Details**
   - What we know: Max31856Error maps to RoasterError variants
   - What's unclear: Whether more specific error details should be preserved
   - Recommendation: Current mapping is adequate; consider enriched errors if debugging requires more detail

## Sources

### Primary (HIGH confidence)
- src/hardware/max31856.rs - Implementation of async temperature reading
- src/main.rs - LEDC timer configuration
- src/config/constants.rs - Timer and frequency constants
- src/control/traits.rs - Thermometer trait definitions
- Cargo.toml - Dependency versions

### Secondary (MEDIUM confidence)
- ESP32-C3 LEDC documentation via esp-hal API (type-safe configuration)
- embassy-time documentation (async Timer pattern)

### Tertiary (LOW confidence)
- MAX31856 datasheet for register details (assumed from implementation)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Verified from Cargo.toml and codebase
- Architecture: HIGH - Verified from source code implementation
- Pitfalls: HIGH - Based on actual issues fixed in this phase

**Research date:** 2026-02-18
**Valid until:** 90 days (embedded configuration is stable)
