# Plan: GPIO Pinout Testing Without Hardware

**Goal:** Validate every GPIO pin assignment, direction, peripheral mapping, and strapping-pin constraint at compile/test time on the host (x86_64), so that when real hardware arrives the wiring is guaranteed correct.

**Problem it solves:** Last time, a pinout mismatch fried the ESP. This plan creates automated guardrails that catch pin errors before flashing.

---

## Current State (What We Have)

| Artifact | Location | Notes |
|----------|----------|-------|
| Pin constants | `src/config/constants.rs` | 11 GPIO pins as `u8` consts |
| Pinout doc | `pinout.md` | Authoritative reference with strapping warnings |
| Board init | `src/hardware/board.rs` | Concrete types for ESP32-C3 only (`#[cfg(target_arch = "riscv32")]`) |
| Hardware init | `src/hardware/init.rs` | ESP-only, sets up GPIO, SPI, LEDC, UART |
| Control traits | `src/control/traits.rs` | `Thermometer`, `Heater`, `Fan` — already abstracted! |
| Test mocks | `src/hardware/test_mocks.rs` | `MockThermometer`, `MockSsr`, `MockFan` |
| SSR stub | `src/hardware/ssr_stub.rs` | Host stub with just the error type |
| Fan host | `src/hardware/fan_host.rs` | Host stub for fan |
| Existing tests | `tests/` | 30+ test files, already using mocks + `embedded-hal-mock` |
| `embedded-hal-mock` | `Cargo.toml` (optional dep, `regression` feature) | Already available |

**Key insight:** The project already has `Thermometer`/`Heater`/`Fan` traits and mocks. The gap is: **nobody validates that the pin constants themselves are correct and consistent.**

---

## Plan: 5 Steps

### Step 1: Create a Single Source-of-Truth Pin Table (`src/config/pinout.rs`)

**Problem:** Pin numbers are loose `u8` constants scattered in `constants.rs`. No structured metadata about direction, peripheral, strapping risk, or constraints.

**Action:** Create `src/config/pinout.rs` with a declarative pin table:

```rust
/// Single source of truth for every GPIO pin on the ESP32-C3.
/// All hardware init code MUST reference this — never hardcode pin numbers.
pub struct PinSpec {
    pub gpio: u8,
    pub function: &'static str,
    pub direction: PinDirection,
    pub peripheral: PinPeripheral,
    pub strapping: Option<StrappingWarning>,
    pub constraints: &'static [PinConstraint],
    pub voltage_max: f32, // 3.3 for all GPIOs on ESP32-C3
}

pub enum PinDirection { Input, Output, Bidirectional }
pub enum PinPeripheral { Spi, LedcPwm, Uart, GpioDirect, Usb, Unused }
pub enum StrappingWarning { BootMode, JtagEnable, VddSpi }
pub enum PinConstraint {
    ExternalPullUpRequired,      // GPIO9
    PushPullOnly,                // GPIO8
    DoNotConnect,                // GPIO2
    MaxCurrent20mA,
    SeriesResistorRequired,      // UART RX from CH341
    FlybackDiodeRequired,        // GPIO9 fan MOSFET
}

/// Complete pin table — one entry per GPIO used.
/// Tests validate this against pinout.md.
pub const PIN_TABLE: &[PinSpec] = &[
    PinSpec { gpio: 1, function: "Heat Detection", direction: PinDirection::Input, ... },
    PinSpec { gpio: 3, function: "MAX31856 ET CS", direction: PinDirection::Output, ... },
    // ... all 11 used pins ...
];

/// Pins that are forbidden to use.
pub const FORBIDDEN_PINS: &[u8] = &[2]; // GPIO2 = VDD_SPI strapping
```

**Files changed:**
- NEW: `src/config/pinout.rs`
- MODIFY: `src/config/mod.rs` (add `pub mod pinout;`)
- MODIFY: `src/config/constants.rs` (import from pinout or add doc references)

**Validation:** Compiles on both host and ESP target.

---

### Step 2: Static Pin Validation Tests (`tests/pinout_validation.rs`)

**Problem:** Nobody checks if two functions claim the same GPIO, or if a strapping pin is used without its required constraint.

**Action:** Create a test file that runs on host (`cargo test --target x86_64-unknown-linux-gnu`) and validates:

```rust
#[cfg(test)]
mod tests {
    use libreroaster::config::pinout::*;

    #[test]
    fn no_duplicate_gpio_assignments() {
        // Every GPIO appears at most once in PIN_TABLE
    }

    #[test]
    fn no_forbidden_pins_used() {
        // No entry in PIN_TABLE has a GPIO in FORBIDDEN_PINS
    }

    #[test]
    fn all_strapping_pins_have_constraints() {
        // Any pin with StrappingWarning must have matching PinConstraint
        // GPIO9 (ULW) → ExternalPullUpRequired + FlybackDiodeRequired
        // GPIO8 (JTAG) → PushPullOnly
    }

    #[test]
    fn spi_cs_pins_are_output_only() {
        // GPIO3, GPIO4 must be Output
    }

    #[test]
    fn uart_rx_is_input_tx_is_output() {
        // GPIO20 = Input, GPIO21 = Output
    }

    #[test]
    fn pwm_pins_are_output() {
        // GPIO9, GPIO10 must be Output
    }

    #[test]
    fn heat_detection_is_input_with_pullup() {
        // GPIO1 must be Input with pull-up constraint
    }

    #[test]
    fn pin_table_matches_constants_module() {
        // Cross-check: every const in constants.rs maps to the same GPIO in PIN_TABLE
        // e.g., SPI_SCLK_PIN == find_pin("SPI SCLK").gpio
    }

    #[test]
    fn all_used_pins_documented() {
        // Every pin in PIN_TABLE has a non-empty function string
    }

    #[test]
    fn voltage_never_exceeds_3v3() {
        // All entries have voltage_max <= 3.6 (absolute max)
    }
}
```

**Files changed:**
- NEW: `tests/pinout_validation.rs`

**Validation:** `cargo test --target x86_64-unknown-linux-gnu --test pinout_validation` passes.

---

### Step 3: Wiring Diagram Verification (doc + data consistency)

**Problem:** `pinout.md` is a manual document that can drift from code. If someone changes a pin in code but not the doc (or vice versa), the mismatch is invisible.

**Action:**

1. **Add a `PinoutManifest` struct** that can be serialized/deserialized, representing the expected wiring:
   ```rust
   // src/config/pinout.rs (or a separate manifest file)
   /// Expected connections — each entry maps GPIO → external component.
   pub const WIRING_MANIFEST: &[(&str, u8, &str)] = &[
       ("MAX31856 ET", 3, "CS pin — active low"),
       ("MAX31856 BT", 4, "CS pin — active low"),
       ("MAX31856 SCLK", 6, "Shared SPI clock"),
       ("MAX31856 MOSI", 7, "Shared SPI data out"),
       ("MAX31856 MISO", 5, "Shared SPI data in"),
       ("SSR", 10, "PWM control via LEDC"),
       ("Fan MOSFET", 9, "25kHz PWM + external pull-up"),
       ("SSR Feedback", 1, "Optocoupler collector to GPIO"),
       ("CH341 TX→RX", 20, "USB-UART adapter TX to ESP RX"),
       ("CH341 RX←TX", 21, "ESP TX to USB-UART adapter RX"),
       ("Status LED", 8, "Push-pull, 330Ω series resistor"),
   ];
   ```

2. **Test that pinout.md is consistent with code:**
   ```rust
   #[test]
   fn wiring_manifest_matches_pin_table() {
       // Every entry in WIRING_MANIFEST maps to a PIN_TABLE entry with same GPIO
   }
   ```

**Files changed:**
- MODIFY: `src/config/pinout.rs` (add `WIRING_MANIFEST`)
- NEW: `tests/pinout_validation.rs` (add cross-check tests)

**Validation:** Tests prove code ↔ manifest ↔ pinout.md are in sync.

---

### Step 4: Mock-Based Digital Twin for Pin Behavior (`tests/pin_digital_twin.rs`)

**Problem:** Even with correct pin numbers, the logic of "what happens when GPIO X goes HIGH" isn't tested. E.g., does setting fan to 50% actually drive GPIO9 at the right duty cycle? Does heat detection correctly read GPIO1 as LOW = SSR conducting?

**Action:** Create a lightweight digital twin that simulates pin state:

```rust
/// Virtual GPIO state for host-side testing.
struct VirtualBoard {
    pin_states: [PinState; 22], // GPIO0-21
}

struct PinState {
    level: Option<bool>,        // None = floating
    direction: Option<PinDirection>,
    pull_up: bool,
    pull_down: bool,
    peripheral_attached: Option<PinPeripheral>,
}

impl VirtualBoard {
    /// Simulate: "set GPIO9 HIGH" → does the fan virtual motor spin?
    fn set_pin(&mut self, gpio: u8, high: bool) { ... }
    fn read_pin(&self, gpio: u8) -> bool { ... }

    /// Simulate SSR feedback: when SSR PWM is active, GPIO1 should read LOW
    fn simulate_ssr_feedback(&mut self, ssr_active: bool) {
        self.pin_states[1].level = Some(!ssr_active); // LOW = conducting
    }
}

// Tests:
#[test]
fn ssr_on_means_heat_detection_reads_low() {
    let mut board = VirtualBoard::new();
    board.simulate_ssr_feedback(true); // SSR conducting
    assert_eq!(board.read_pin(1), false); // GPIO1 = LOW
}

#[test]
fn fan_at_100_percent_gpio9_is_high() { ... }
#[test]
fn both_spi_cs_never_low_simultaneously() { ... }
#[test]
fn boot_state_gpio9_is_high_with_pullup() { ... }
#[test]
fn boot_state_gpio8_is_not_forced_low() { ... }
#[test]
fn emergency_stop_drives_gpio10_low_and_gpio9_high() { ... }
```

**Files changed:**
- NEW: `tests/pin_digital_twin.rs`
- MAYBE: `tests/common/virtual_board.rs` (shared helper)

**Validation:** `cargo test --target x86_64-unknown-linux-gnu --test pin_digital_twin` passes.

---

### Step 5: Pre-Flash Safety Checklist Script (`scripts/preflight-check.sh`)

**Problem:** Before flashing, you need a quick sanity check that the firmware won't immediately misconfigure pins dangerously.

**Action:** Create a script that:
1. Runs `cargo test --target x86_64-unknown-linux-gnu --test pinout_validation`
2. Runs `cargo test --target x86_64-unknown-linux-gnu --test pin_digital_twin`
3. Checks that embedded build compiles (`cargo check --target riscv32imc-unknown-none-elf --features embedded`)
4. Prints a summary:
   ```
   ✅ No duplicate GPIO assignments
   ✅ No forbidden pins used (GPIO2 avoided)
   ✅ GPIO9 has external pull-up constraint documented
   ✅ GPIO8 has push-pull-only constraint documented
   ✅ SPI CS pins never both active
   ✅ UART RX/TX directions correct
   ✅ Emergency stop logic verified in digital twin
   ✅ Embedded target compiles

   🟢 PREFLIGHT PASSED — safe to flash to ESP32-C3
   ```

**Files changed:**
- NEW: `scripts/preflight-check.sh`

**Validation:** `./scripts/preflight-check.sh` exits 0.

---

## Dependency Graph

```
Step 1 (pinout.rs) ──► Step 2 (validation tests)
       │                      │
       ▼                      ▼
Step 3 (wiring manifest) ──► Step 5 (preflight script)
                              ▲
Step 4 (digital twin) ───────┘
```

Steps 1-4 can be implemented in parallel. Step 5 depends on all of them.

---

## Risk Assessment

| Risk | Mitigation |
|------|-----------|
| Pin table drifts from constants.rs | Step 2 test cross-checks both |
| pinout.md drifts from code | Step 3 manifest is the bridge |
| Strapping pin misconfig burns ESP | Step 2 validates constraints exist |
| Logic errors (SSR on = GPIO1 low) | Step 4 digital twin tests behavior |
| Overconfident in simulation | pinout.md safety section stays as manual checklist |

---

## What This Does NOT Cover (Still Need Manual Checks)

1. **Actual voltage levels** — simulation can't verify 3.3V rail
2. **Current limits** — simulation can't verify 40mA per GPIO
3. **Inductive kickback** — needs real flyback diode
4. **ESD** — needs real TVS diodes
5. **Boot behavior timing** — strapping pins must be sampled correctly at power-on

For those, the existing `pinout.md` safety section remains the reference.

---

## Estimated Effort

| Step | Effort | LOC (approx) |
|------|--------|--------------|
| Step 1: Pin table | 1-2h | ~120 lines |
| Step 2: Validation tests | 1-2h | ~100 lines |
| Step 3: Wiring manifest + cross-checks | 1h | ~60 lines |
| Step 4: Digital twin | 2-3h | ~200 lines |
| Step 5: Preflight script | 30min | ~40 lines |
| **Total** | **~6-8h** | **~520 lines** |
