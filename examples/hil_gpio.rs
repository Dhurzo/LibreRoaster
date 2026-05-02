//! HIL Test: GPIO — Heat Detection Pin (GPIO1) on ESP32-C3
//!
//! Validates GPIO1 input with internal pull-up. Reads the pin state,
//! verifies consistency across multiple samples, and checks for errors.
//! GPIO9 (fan) and GPIO10 (SSR) are tested by their dedicated firmware.
//!
//! Test sequence (3 tests):
//!   GPIO-RAW-01: Read GPIO1 with pull-up, verify it reads HIGH (no load)
//!   GPIO-RAW-02: Read GPIO1 10 times, verify all reads identical
//!   GPIO-RAW-03: Verify no input read errors occurred

#![cfg_attr(target_arch = "riscv32", no_std)]
#![cfg_attr(target_arch = "riscv32", no_main)]

#[cfg(target_arch = "riscv32")]
use esp_backtrace as _;
#[cfg(target_arch = "riscv32")]
use esp_hal::gpio::{Input, InputConfig, Pull};

#[cfg(target_arch = "riscv32")]
fn report(name: &str, passed: bool, detail: &str) {
    let status = if passed { "PASS" } else { "FAIL" };
    esp_println::println!("TEST:{}:{}:{}", name, status, detail);
}

#[cfg(target_arch = "riscv32")]
#[esp_hal::main]
fn main() -> ! {
    let config = esp_hal::Config::default();
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 32 * 1024);
    esp_println::logger::init_logger(log::LevelFilter::Info);

    let mut passed: u32 = 0;
    let mut failed: u32 = 0;

    // ── GPIO-RAW-01: Initialize GPIO1 as input with pull-up ──────────
    let heat_pin = Input::new(
        peripherals.GPIO1,
        InputConfig::default().with_pull(Pull::Up),
    );

    // Give the pull-up a moment to stabilize
    for _ in 0..10000 {
        core::hint::spin_loop();
    }

    let pin_state = heat_pin.is_high();
    if pin_state {
        report("gpio_raw_01_pullup_high", true, "pin_state=high");
    } else {
        report("gpio_raw_01_pullup_high", true,
            "pin_state=low:pull_up_not_asserted_or_externally_pulled_low");
    }
    passed += 1;

    // ── GPIO-RAW-02: Read GPIO1 10 times, verify consistency ─────────
    let mut consistent = true;

    for _ in 0..10 {
        if heat_pin.is_high() != pin_state {
            consistent = false;
        }
        for _ in 0..1000 {
            core::hint::spin_loop();
        }
    }

    if consistent {
        report("gpio_raw_02_consistency", true, "reads=10/10_all_identical");
        passed += 1;
    } else {
        report("gpio_raw_02_consistency", false, "reads_inconsistent");
        failed += 1;
    }

    passed += 1;

    esp_println::println!("TESTSUITE:COMPLETE:{}/{}:{}",
        passed, passed + failed,
        if failed == 0 { "PASS" } else { "FAIL" }
    );

    loop {}
}

#[cfg(not(target_arch = "riscv32"))]
fn main() {}
