# Bug Fix Research: ESP32-C3 Safety Issues

**Project:** LibreRoaster v3.0 Critical Safety Fixes  
**Domain:** Embedded Rust / ESP32-C3 Firmware  
**Researched:** 2026-02-18  
**Confidence:** HIGH

## Summary

This document researches the correct fix approaches for 8 critical safety bugs in the ESP32-C3 firmware. Each bug is analyzed for root cause, expected fix approach, and verification strategy.

---

## Bug A: make_static Use-After-Free in main.rs

### Current Problem

The `make_static` function at line 46-49 of `main.rs`:

```rust
#[cfg(target_arch = "riscv32")]
unsafe fn make_static<T>(mut value: T) -> &'static mut T {
    let ptr = &mut value as *mut T;
    &mut *ptr
}
```

**Issue:** This function takes ownership of `value`, creates a pointer to it, then returns a reference. However, `value` is dropped when it goes out of scope at the end of the function, leaving the returned reference as a dangling pointer (Use-After-Free).

The function is used at lines 219-220:

```rust
let static_ssr = unsafe { make_static(real_ssr) };
let static_fan = unsafe { make_static(fan_controller) };
```

### Root Cause

The `make_static` pattern is fundamentally broken because:

1. The local variable `value` is moved into the function
2. A pointer to `value` is created
3. `value` is dropped when the function returns
4. The returned reference points to freed memory

This is undefined behavior that may cause immediate crashes, memory corruption, or silent data corruption.

### Correct Fix Approach

**Option 1: Use StaticCell (Recommended)**

The `static_cell` crate provides a safe way to create static references:

```rust
use static_cell::StaticCell;

static SSR_CONTROLLER: StaticCell<SsrControlSimple> = StaticCell::new();
static FAN_CONTROLLER: StaticCell<FanController> = StaticCell::new();

// In main():
let static_ssr: &'static mut SsrControlSimple = SSR_CONTROLLER.init(real_ssr);
let static_fan: &'static mut FanController = FAN_CONTROLLER.init(fan_controller);
```

**Option 2: Use `mk_static` macro**

The `static_cell` crate provides a convenience macro:

```rust
use static_cell::make_static;

let static_ssr = make_static!(SsrControlSimple, real_ssr);
let static_fan = make_static!(FanController, fan_controller);
```

### Expected Behavior After Fix

- SSR and FanController are properly stored in static memory
- References remain valid for the program's lifetime
- No memory corruption or use-after-free
- StaticCell can only be initialized once (panics on double init)

---

## Bug C: test_parse_ot2_partial_command Failure

### Current Problem

The test at `src/input/parser.rs:461-464`:

```rust
#[test]
fn test_parse_ot2_partial_command() {
    let result = parse_artisan_command("OT2");
    assert!(matches!(result, Err(ParseError::InvalidValue)));
}
```

Looking at the parsing logic at lines 78-80:

```rust
["OT2" | "ot2"] => Ok(ArtisanCommand::SetFanSpeed(0, false)),
["OT2" | "ot2", value_str] => {
    // ... parse value
```

**Issue:** When `"OT2"` is parsed without a value, it matches the first pattern and returns `Ok(ArtisanCommand::SetFanSpeed(0, false))` — not an error!

### Root Cause

The parser incorrectly treats `"OT2"` (without value) as a valid command that sets fan to 0%, rather than rejecting it as malformed.

### Correct Fix Approach

The test expectation is correct: `"OT2"` without a value should return `InvalidValue`. The fix is in the parser:

```rust
// Change from:
["OT2" | "ot2"] => Ok(ArtisanCommand::SetFanSpeed(0, false)),

// To require a value - the second pattern already handles valid values
// Simply remove or comment out the first pattern, or change to:
["OT2" | "ot2"] => Err(ParseError::InvalidValue),  // Require value
["OT2" | "ot2", value_str] => {
    // Existing parsing logic
}
```

### Expected Behavior After Fix

- `parse_artisan_command("OT2")` returns `Err(ParseError::InvalidValue)`
- `parse_artisan_command("OT2 50")` returns `Ok(SetFanSpeed(50))`
- Test passes

---

## Bug D: Mutable Statics Without Protection in driver.rs

### Current Problem

At `src/hardware/uart/driver.rs:57`:

```rust
static mut UART_INSTANCE: Option<UartDriver> = None;
```

This is a mutable static without any synchronization. The code does use `critical_section::with` for access (line 78-80), but:

1. The raw `static mut` is deprecated/disallowed in Rust 2024
2. Direct mutable static access without synchronization is undefined behavior
3. The `#[allow(static_mut_refs)]` at line 87 is a code smell

### Root Cause

Using `static mut` directly violates Rust's aliasing rules and is being phased out.

### Correct Fix Approach

**Option 1: Use StaticCell (Recommended)**

```rust
use static_cell::StaticCell;

static UART_INSTANCE: StaticCell<Option<UartDriver>> = StaticCell::new();

pub fn init_uart(...) -> Result<(), UartError> {
    // ... create driver ...
    UART_INSTANCE.init(Some(driver));
    Ok(())
}

pub fn get_uart_driver() -> Option<&'static mut UartDriver> {
    unsafe { UART_INSTANCE.as_mut()?.as_mut() }
}
```

**Option 2: Use Mutex (If Reinitialization Needed)**

```rust
use critical_section::Mutex;
use core::cell::RefCell;

static UART_INSTANCE: Mutex<RefCell<Option<UartDriver>>> = 
    Mutex::new(RefCell::new(None));

pub fn get_uart_driver() -> Option<&'static mut UartDriver> {
    critical_section::with(|cs| {
        UART_INSTANCE.borrow(cs).borrow_mut().as_mut()
    })
}
```

### Expected Behavior After Fix

- Code compiles without `static_mut_refs` warnings
- No data races during concurrent access
- Existing functionality preserved

---

## Bug E: ServiceContainer::get_instance() Unsafe

### Current Problem

At `src/application/service_container.rs:41-44`:

```rust
pub fn get_instance() -> &'static mut Self {
    static mut INSTANCE: ServiceContainer = ServiceContainer::new();
    unsafe { &mut *core::ptr::addr_of_mut!(INSTANCE) }
}
```

**Issues:**

1. Uses `static mut` which is deprecated
2. Returns `&'static mut` without synchronization
3. Multiple callers can get mutable aliasing access (violates Rust's aliasing rules)
4. The `unsafe` block doesn't provide any safety documentation

### Root Cause

The singleton pattern is implemented incorrectly for Rust's memory safety guarantees.

### Correct Fix Approach

**Option 1: Use StaticCell (Simplest)**

```rust
use static_cell::StaticCell;

static INSTANCE: StaticCell<ServiceContainer> = StaticCell::new();

impl ServiceContainer {
    pub fn get_instance() -> &'static mut Self {
        // Initialize once - panics if called again
        INSTANCE.init(ServiceContainer::new())
    }
}
```

### Expected Behavior After Fix

- StaticCell ensures single initialization
- No mutable aliasing possible
- Code compiles cleanly without unsafe

---

## Bug F: README vs PROTOCOL.md Mismatch

### Current Problem

The README.md documents the Artisan+ protocol but may not match the actual implementation. Need to verify:

1. **Command support**: README says OT1, OT2, IO3, UP, DOWN, START, STOP, READ
2. **Initialization**: README mentions CHAN→UNITS→FILT sequence
3. **Response format**: README shows `ET,BT,ET2,BT2,ambient,fan,heater`

### Root Cause

Documentation drift - the code may have evolved without updating docs.

### Correct Fix Approach

1. **Audit actual implementation** in:

   - `src/input/parser.rs` - command parsing
   - `src/output/artisan.rs` - response formatting
   - `src/control/roaster_refactored.rs` - command handling

2. **Update README.md** to match actual behavior:

   - List exactly supported commands
   - Document actual response format
   - Clarify initialization requirements (if any)

### Expected Behavior After Fix

- README accurately reflects implementation
- All documented commands work as described
- Response format matches documentation

---

## Bug G: Blocking MAX31856 Temperature Read

### Current Problem

At `src/hardware/max31856.rs:45-53`:

```rust
pub fn read_temperature(&mut self) -> Result<f32, Max31856Error> {
    self.write_register(0x80, 0x80)?; // Set one-shot bit

    const DELAY_MS: u64 = 160;

    // BLOCKING SPIN LOOP - wastes CPU!
    for _ in 0..(DELAY_MS * 10000) {
        core::hint::spin_loop();
    }
    // ...
}
```

**Issue:** The 160ms delay uses a busy-wait spin loop that blocks the CPU completely. In an async Embassy system, this prevents other tasks from running.

### Root Cause

Using synchronous blocking delay instead of async delay in an async runtime environment.

### Correct Fix Approach

**Option 1: Use embassy_time::Timer (Recommended)**

```rust
use embassy_time::Timer;

pub async fn read_temperature_async(&mut self) -> Result<f32, Max31856Error> {
    self.write_register(0x80, 0x80)?; // Set one-shot bit

    // Non-blocking async delay - yields to other tasks
    Timer::after_millis(160).await;
    
    // ... rest of reading logic
}
```

**Option 2: Use Proper Blocking Delay**

If blocking is required in sync context:

```rust
use esp_hal::delay::Delay;

let mut delay = Delay::new();
delay.delay_ms(160);
```

**Option 3: Poll-Based Wait**

For true async SPI, poll status register instead of fixed delay:

```rust
use embassy_time::Timer;

pub async fn read_temperature_async(&mut self) -> Result<f32, Max31856Error> {
    self.write_register(0x80, 0x80)?; // Set one-shot bit
    
    // Wait for conversion complete (poll status register)
    for _ in 0..160 {
        let status = self.read_register(0x0F)?;
        if status & 0x01 == 0 {  // Not fault
            break;
        }
        Timer::after_millis(1).await;
    }
    
    // ... read temperature
}
```

### Expected Behavior After Fix

- CPU is not busy-waiting during temperature conversion
- Other async tasks can run while waiting
- Temperature readings still complete correctly

---

## Bug H: SSR and Fan Share Same LEDC Timer

### Current Problem

In `main.rs:114-142`, both fan and SSR channels share the same timer:

```rust
let timer_ref: &'static mut dyn timer::TimerIFace<LowSpeed> =
    unsafe { &mut *(&mut fan_timer as *mut _ as *mut _) };

// Fan channel uses timer_ref
fan_channel.configure(ChannelConfig {
    timer: timer_ref,
    // ...
}).unwrap();

// SSR channel ALSO uses same timer_ref
ssr_channel.configure(ChannelConfig {
    timer: timer_ref,  // SAME TIMER!
    // ...
}).unwrap();
```

**Issue:** This is undefined behavior - the same mutable timer reference is used for two channels. Additionally, ESP32 LEDC timers have specific constraints:

- Only 4 LEDC timers (0-3) on ESP32-C3
- Channels share timers unless explicitly separated
- Sharing a timer between channels requires same frequency

### Root Cause

Incorrect assumption that timer references can be safely shared. The code uses unsafe transmute to extend lifetime, then shares the mutable reference.

### Correct Fix Approach

**Option 1: Use Separate Timers (Recommended)**

```rust
// Fan timer on Timer0
let mut fan_timer = ledc.timer(timer::Number::Timer0);
fan_timer.configure(TimerConfig {
    duty: timer::config::Duty::Duty8Bit,
    clock_source: timer::LSClockSource::APBClk,
    frequency: esp_hal::time::Rate::from_hz(FAN_PWM_FREQUENCY_HZ),
})?;

// SSR timer on Timer1 
let mut ssr_timer = ledc.timer(timer::Number::Timer1);
ssr_timer.configure(TimerConfig {
    duty: timer::config::Duty::Duty8Bit,
    clock_source: timer::LSClockSource::APBClk,
    frequency: esp_hal::time::Rate::from_hz(SSR_PWM_FREQUENCY_HZ),  // Can be different!
})?;

let fan_timer_ref: &'static mut dyn timer::TimerIFace<LowSpeed> = /* ... */;
let ssr_timer_ref: &'static mut dyn timer::TimerIFace<LowSpeed> = /* ... */;

// Configure channels with their own timers
fan_channel.configure(ChannelConfig {
    timer: fan_timer_ref,
    // ...
})?;

ssr_channel.configure(ChannelConfig {
    timer: ssr_timer_ref,
    // ...
})?;
```

### Additional Consideration

The ESP32-C3 has limited LEDC timers:

- 4 low-speed timers (Timer0-3)
- 6 channels (Channel0-5)
- Timer conflict errors occur if misconfigured

### Expected Behavior After Fix

- Each PWM output has independent timer
- No timer conflict errors at runtime
- Both fan and SSR operate at correct frequencies

---

## Fix Prioritization Summary

| Bug | Severity | Fix Complexity | Priority |
|-----|----------|----------------|----------|
| A: make_static UAF | **Critical** | Low (StaticCell) | P0 |
| D: Mutable static | High | Medium | P0 |
| E: ServiceContainer | High | Low (StaticCell) | P0 |
| G: Blocking delay | Medium | Medium (async) | P1 |
| H: Shared timer | **Critical** | Medium | P0 |
| C: Test failure | Low | Low | P1 |
| F: Doc mismatch | Low | Low | P2 |

---

## Dependencies Between Fixes

```
Bug A (make_static)
    └── Requires: StaticCell crate (already in use elsewhere)
    
Bug D (mutable static)
    └── Can use: Same StaticCell approach as Bug A
    
Bug E (ServiceContainer)
    └── Can use: Same StaticCell approach as Bug A
    └── After fix: Remove unsafe from callers
    
Bug G (blocking delay)
    └── Requires: embassy-time (async)
    └── May need: embedded_hal_async traits
    
Bug H (shared timer)
    └── Requires: Understanding of ESP32 LEDC hardware
    └── Changes: main.rs timer initialization
```

---

## Recommended Fix Order

1. **Phase 1 (P0 - Safety Critical)**
   - Bug A: Fix make_static with StaticCell
   - Bug H: Separate LEDC timers
   - Bug D: Fix mutable static
   - Bug E: Fix ServiceContainer singleton

2. **Phase 2 (P1 - Functionality)**
   - Bug G: Convert blocking delay to async
   - Bug C: Fix OT2 parser test

3. **Phase 3 (P2 - Documentation)**
   - Bug F: Update README vs implementation

---

## References

- [static_cell crate](https://docs.rs/static_cell) - Safe static initialization
- [ESP32 LEDC Documentation](https://docs.espressif.com/projects/esp-idf/en/latest/esp32c3/api-reference/peripherals/ledc.html) - Timer/channel constraints
- [Embassy async delay](https://docs.embassy.dev/embassy-time/) - Non-blocking delays
- [Rust static_mut RFC](https://github.com/rust-lang/rfcs/pull/2404) - Deprecation rationale
