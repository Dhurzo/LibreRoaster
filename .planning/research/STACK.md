# Stack Research

**Domain:** LibreRoaster hardware reliability (SSR duty clamps, LEDC fan control, responsive UART/USB)
**Researched:** 2026-02-17
**Confidence:** MEDIUM

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| `esp-hal` (with the `unstable` feature) | 1.0.0 | Direct LEDC timers and async UART for ESP32-C3 | Docs show `ledc` lives behind `unstable` while `uart` implements `embedded-io-async`/`embedded-hal-async`, so enabling it gives FanController direct access to hardware PWM channels plus non-blocking serial primitives needed for the command multiplexer. |
| `embedded-io-async` | 0.6.1 | Byte-stream traits for async UART & USB CDC | Both `esp-hal::uart` and `embassy-usb` surface these traits, so reusing the same version keeps futures-based reads/writes compatible and lets the executor poll I/O without blocking SSR math updates. |
| `embassy-usb` | 0.5.1 | Asynchronous CDC ACM stack | Native async, lock-free endpoints, and built-in CDC class keep USB traffic off the critical SSR/Fan tasks; it integrates with the existing `embassy-executor` and reuses `embedded-io-async` so the executor stays responsive while USB transfers happen. |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `fixed` | 1.30.0 | Deterministic duty math with saturating arithmetic | FanController/SSR duty clamp routines use `fixed::Saturating` + static scaling to map 0..1 duty requests into LEDC resolution without overflow or floating round-off. |
| `fugit` | 0.3.9 | Frequency/duty conversions (`Rate`, `Duration`) | LEDC configuration examples already use `fugit::Rate::from_khz`, so reuse the same crate when computing timer ticks per SSR increment to keep hardware math aligned with controller units. |
| `embassy-usb-synopsys-otg` | 0.3.1 | Synopsys OTG driver for `embassy-usb` | Required glue for ESP32-C3 USB controller; use it when instantiating `embassy_usb::Builder` so the async stack talks to the on-chip hardware. |
| `heapless` | 0.8.0 | Allocate-free command / event buffers | Ring buffers such as `heapless::spsc::Queue` decouple UART/USB producers from the executor-driven consumers, so the non-blocking I/O paths never allocate and stay deterministic. |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| `embassy-executor` | Async runtime | Already present (`0.9.1`); keep in sync so LEDC/USB/UART tasks continue to run cooperatively without blocking the executor. |
| `cargo test --features embedded` | Validate async paths | Use hardware regression tests to exercise the non-blocking UART/USB stack and duty clamp logic before deploying. |

## Installation

```bash
# Add the USB + async helpers needed for the new milestone
cargo add embassy-usb@0.5.1 embassy-usb-synopsys-otg@0.3.1 embedded-io-async@0.6.1 fixed@1.30.0
```

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| `embassy-usb` + `embedded-io-async` | `usb-device` + manual `poll` loops | Only when rewriting the whole USB stack to a simpler blocking driver (e.g., for a throwaway prototype) and you can tolerate executor starvation. |
| `fixed` (saturating) + `fugit::Rate` | `f32` + `u32::saturating_*` | If FIR precision is not critical and you must drop the extra crate weight, but expect rounding errors and inefficient float math to complicate SSR clamp tests. |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| Blocking loops around `nb::block!`/`embedded-io::Read::read` | They starve the executor and delay LEDC updates, which is exactly what the milestone forbids | Use the async `embedded-io-async` traits and await readiness inside executor tasks so SSR math can continue during I/O. |
| `usb-device` synchronous `poll` strategy | Polling USB every cycle keeps the main loop busy and negates the new non-blocking guarantee | Let `embassy-usb` handle USB interrupts and futures; it already provides CDC ACM and cooperates with other async work. |

## Stack Patterns by Variant

**If clamping SSR duty for hardware safety:**
- Use `fixed::Saturating<FixedU16<_>>` together with `fugit::Rate` to derive the PWM steps that match LEDC resolution.
- Because deterministic, saturating math avoids wrapping duty and keeps the SSR within safe duty/time windows even under rapid setpoint changes.

**If USB/serial traffic must stay responsive:**
- Use `esp-hal::uart` in `Async` mode plus `embassy-usb` CDC under the `embassy-executor` runtime, all wired through `embedded-io-async` traits.
- Because the executor can now poll each stream independently and never waits on a blocking UART/USB transfer, keeping instrumentation and command handling alive while SSR math runs.

## Version Compatibility

| Package A | Compatible With | Notes |
|-----------|-----------------|-------|
| `esp-hal@1.0.0` | `embedded-io-async@0.6.1`, `embassy-sync@0.7.2` | Async UART/LEDC drivers were built with these versions; enabling `unstable` unlocks `ledc` while the async traits stay on 0.6.1 to match `embassy-usb`. |
| `embassy-usb@0.5.1` | `embedded-io-async@0.6.1`, `embassy-sync@0.7.2` | The docs list these exact dependencies, so keep the dependency graph aligned to avoid duplicate versions. |
| `embassy-usb-synopsys-otg@0.3.1` | `esp-hal@1.0.0` | esp-hal already exposes this driver, so add it once and re-use the HAL’s initialization. |
| `fixed@1.30.0` | Rust ≥ 1.85 (project uses 1.88) | The crate requires at least Rust 1.85; our toolchain already meets that, so no conflicts. |
| `fugit@0.3.9` | `esp-hal` timers | esp-hal examples use `fugit::Rate`, so staying on this release keeps conversions matching the HAL. |

## Sources

- https://docs.rs/esp-hal/latest/esp_hal/index.html — esp-hal peripheral overview, async/unstable features, LEDC + UART documentation (HIGH)
- https://docs.rs/esp-hal/latest/esp_hal/ledc/index.html — LEDC driver behind the `unstable` feature (HIGH)
- https://docs.rs/esp-hal/latest/esp_hal/uart/index.html — UART driver implementing `embedded-io-async`/`embedded-hal-async` traits (HIGH)
- https://docs.rs/embedded-io-async/latest/embedded_io_async/ — Async byte-stream traits that `esp-hal` and `embassy-usb` share (HIGH)
- https://docs.rs/embassy-usb/latest/embassy_usb/ — Async USB device stack, native CDC ACM, lock-free endpoints (HIGH)
- https://docs.rs/embassy-usb-synopsys-otg/latest/embassy_usb_synopsys_otg/ — Synopsys driver needed for ESP32-C3 (HIGH)
- https://docs.rs/fixed/latest/fixed/ — Fixed-point numbers with `Saturating` arithmetic (MEDIUM)
- https://docs.rs/fugit/latest/fugit/ — `Rate`/`Duration` helpers matching esp-hal examples (MEDIUM)
- https://docs.rs/heapless/latest/heapless/ — Static data structures for `spsc::Queue` command buffering (MEDIUM)

---

# Stack Research: v3.0 Safety Fixes

**Focus:** StaticCell patterns, async I/O safety, LEDC configuration for Use-After-Free, unsafe statics, blocking I/O fixes

**Researched:** 2026-02-18  
**Confidence:** HIGH

---

## Existing Validated Stack (DO NOT Change)

| Technology | Version | Purpose |
|------------|---------|---------|
| esp-hal | ~1.0 | LEDC, UART, USB CDC peripherals |
| embassy-rs | 0.9.1 | Async executor |
| embedded-io-async | 0.6.1 | Async I/O traits |
| StaticCell | 2.1.1 | Static initialization (partially used) |

---

## Recommended Changes for Safety Fixes

### 1. StaticCell Pattern (REPLACE unsafe code)

The codebase already has `static_cell = "2.1.1"` in Cargo.toml. Use it consistently instead of unsafe patterns.

#### Current Problem (main.rs:46-49)

```rust
// UNSAFE - causes Use-After-Free
unsafe fn make_static<T>(mut value: T) -> &'static mut T {
    let ptr = &mut value as *mut T;
    &mut *ptr
}
```

**Issue:** This creates a dangling pointer. The local `value` goes out of scope, but the returned reference still points to that memory.

#### Fix: Use StaticCell consistently

```rust
// Already used correctly elsewhere in codebase:
static LEDC_BUS: StaticCell<LedcBus<'static>> = StaticCell::new();

// Use same pattern for SSR and Fan:
static SSR_CONTROLLER: StaticCell<SsrControlSimple> = StaticCell::new();
static FAN_CONTROLLER: StaticCell<FanController> = StaticCell::new();

// In initialization code:
let static_ssr = SSR_CONTROLLER.init(real_ssr);
let static_fan = FAN_CONTROLLER.init(fan_controller);
```

**Sources:**
- [static_cell crate documentation](https://docs.rs/static_cell/2.1.1) - HIGH confidence
- [embassy-rs/static-cell GitHub](https://github.com/embassy-rs/static-cell) - HIGH confidence

---

### 2. ServiceContainer Singleton Pattern (REPLACE unsafe static mut)

#### Current Problem (service_container.rs:41-44)

```rust
// UNSAFE - violates Rust's aliasing rules
pub fn get_instance() -> &'static mut Self {
    static mut INSTANCE: ServiceContainer = ServiceContainer::new();
    unsafe { &mut *core::ptr::addr_of_mut!(INSTANCE) }
}
```

**Issue:** Multiple mutable references can exist simultaneously, causing data races.

#### Fix: Use StaticCell

```rust
// In service_container.rs
static SERVICE_CONTAINER: StaticCell<ServiceContainer> = StaticCell::new();

pub fn get_instance() -> &'static ServiceContainer {
    SERVICE_CONTAINER.init(ServiceContainer::new())
}
```

**Key insight:** The existing code already uses `Mutex<RefCell<Option<T>>>` for interior mutability. The fix is to make the container itself statically initialized safely.

---

### 3. UART Driver Transmute (REPLACE unsafe transmute)

#### Current Problem (uart/driver.rs:70-76)

```rust
// UNSAFE lifetime extension via transmute
let tx_static = unsafe {
    core::mem::transmute::<UartTx<esp_hal::Async>, UartTx<'static, esp_hal::Async>>(tx)
};
```

**Issue:** Transmuting lifetime without proper ownership transfer can cause use-after-free.

#### Fix: Use StaticCell for UART driver

```rust
// In driver.rs:
static UART_DRIVER: StaticCell<UartDriver> = StaticCell::new();

pub fn init_uart(uart0: esp_hal::peripherals::UART0) -> Result<(), UartError> {
    let config = Config::default().with_baudrate(115200);
    let uart = Uart::new(uart0, config).map_err(|_| UartError::TransmissionError)?;
    let uart = uart.into_async();
    let (rx, tx) = uart.split();
    
    // Safe: StaticCell owns the data, we get 'static reference
    let driver = UartDriver::new(
        UartTx::<'static, esp_hal::Async>::new(tx),
        UartRx::<'static, esp_hal::Async>::new(rx),
    );
    
    UART_DRIVER.init(driver);
    Ok(())
}
```

**Note:** The underlying issue is that esp-hal's `UartTx::new()` requires a lifetime parameter. The proper fix may require checking esp-hal's latest API for `'static` support or using the `into_async` method differently.

---

### 4. LEDC Timer Configuration (VALIDATED - No Changes Needed)

The current LEDC configuration in main.rs is appropriate:

```rust
let mut fan_timer = ledc.timer(timer::Number::Timer0);
fan_timer.configure(TimerConfig {
    duty: timer::config::Duty::Duty8Bit,        // 8-bit = 0-255 range
    clock_source: timer::LSClockSource::APBClk,  // 80MHz APB clock
    frequency: Rate::from_hz(FAN_PWM_FREQUENCY_HZ), // 25kHz
})?;
```

**Validation:**
- **Frequency:** 25kHz is appropriate for PC fans (avoids audible noise)
- **Resolution:** 8-bit provides 256 duty cycle steps (sufficient for fan control)
- **Clock source:** APBClk (80MHz) is correct for ESP32-C3
- **LowSpeed:** Correct for ESP32-C3 (no HighSpeed available)

**Sources:**
- [ESP32 LEDC maximum frequencies](https://gist.github.com/benpeoples/3aa57bffc0f26ede6623ca520f26628c) - HIGH confidence
- [esp-hal LEDC timer docs](https://docs.rs/esp-hal/latest/src/esp_hal/ledc/timer.rs.html) - HIGH confidence

---

## What NOT to Change

| Area | Recommendation | Reason |
|------|---------------|--------|
| esp-hal version | Keep ~1.0 | Already validated, breaking changes unlikely worth it |
| embassy-rs executor | Keep 0.9.1 | Works correctly with current async patterns |
| embedded-io-async | Keep 0.6.1 | Provides correct async traits |
| LEDC timer config | Keep current | Already optimal for fan control |
| LedcBus abstraction | Keep | Provides safe channel access |

---

## Integration Points

### Phase 1: Replace make_static in main.rs

```rust
// BEFORE (unsafe):
let static_ssr = unsafe { make_static(real_ssr) };
let static_fan = unsafe { make_static(fan_controller) };

// AFTER (safe):
static SSR: StaticCell<SsrControlSimple> = StaticCell::new();
static FAN: StaticCell<FanController> = StaticCell::new();

let static_ssr = SSR.init(real_ssr);
let static_fan = FAN.init(fan_controller);
```

### Phase 2: Fix ServiceContainer singleton

```rust
// BEFORE (unsafe):
pub fn get_instance() -> &'static mut Self {
    static mut INSTANCE: ServiceContainer = ServiceContainer::new();
    unsafe { &mut *core::ptr::addr_of_mut!(INSTANCE) }
}

// AFTER (safe):
static SERVICE_CONTAINER: StaticCell<ServiceContainer> = StaticCell::new();

pub fn get_instance() -> &'static ServiceContainer {
    SERVICE_CONTAINER.init(ServiceContainer::new())
}
```

### Phase 3: Fix UART driver transmute

Replace the unsafe lifetime transmute with proper static cell initialization or updated esp-hal API.

---

## Dependencies Summary

**No new dependencies needed.** The existing stack already includes:

- `static_cell = "2.1.1"` - For safe static initialization
- `critical-section = "1.2.0"` - For interrupt-safe interior mutability
- `embassy-sync = "0.6.1"` - For channels and mutexes
- `embedded-io-async = "0.6.1"` - For async I/O traits

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| StaticCell pattern | HIGH | Well-documented, already partially used in codebase |
| ServiceContainer fix | HIGH | Pattern is clear, matches existing interior mutability |
| UART transmute fix | MEDIUM | May need esp-hal API adjustment |
| LEDC config | HIGH | Current config is optimal for application |

---

## Sources

- [static_cell crate docs](https://docs.rs/static_cell/2.1.1/static_cell/) - HIGH
- [embassy-rs static-cell](https://github.com/embassy-rs/static-cell) - HIGH
- [esp-hal LEDC timer](https://docs.rs/esp-hal/latest/src/esp_hal/ledc/timer.rs.html) - HIGH
- [ESP32 LEDC frequency table](https://gist.github.com/benpeoples/3aa57bffc0f26ede6623ca520f26628c) - HIGH

---

*Stack research for: LibreRoaster v3.0 Safety Fixes*
*Researched: 2026-02-18*
