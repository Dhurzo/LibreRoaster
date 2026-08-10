#![cfg_attr(target_arch = "riscv32", no_std)]
#![cfg_attr(target_arch = "riscv32", no_main)]

//! hil_c1 — DUTY_R latency / write-verification measurement (audit C1, 2026-08-10).
//!
//! Reproduces the EXACT production SSR configuration (Timer0, 14-bit, 5 Hz,
//! Channel1, GPIO10) and measures the register semantics the C1 fix relies on:
//!
//!   1. **Config DUTY is synchronous**: `set_duty_hw` writes `duty << 4` into
//!      the `ch(n).duty()` register before arming the update, so a readback at
//!      +1 ms after the write must already show the commanded value. This is
//!      what the fixed write-verification (`monitor_ledc_after_set`) compares.
//!   2. **DUTY_R lags**: the live-duty register only updates at the start of
//!      the next PWM period (up to 200 ms at 5 Hz). Reading it microseconds
//!      after a write returns the PREVIOUS duty — the exact mechanism by which
//!      the pre-fix code failed every heater ramp above ~0.8 %.
//!   3. **Ramp verification**: a 1 % → 60 % sequence verifies OK via config-
//!      DUTY readback (the pre-fix path returned `Err` → `emergency_shutdown`).
//!
//! ⚠️⚠️ SAFETY: this example drives GPIO10 with **NON-ZERO duty**. With an
//! SSR + load attached, the load WILL be powered. Disconnect heater/SSR
//! power before running. The channel is left at 0 % when the suite ends.
//!
//! Manual C1 bench procedure (from the 2026-08-10 audit):
//!   write a duty at 5 Hz → read `duty_r()` at +1 ms (expect previous duty)
//!   → read `duty_r()` at +250 ms (expect new duty); read `duty()` at +1 ms
//!   (expect new duty, synchronous).

#[cfg(target_arch = "riscv32")]
use esp32c3::LEDC;
#[cfg(target_arch = "riscv32")]
use esp_backtrace as _;
#[cfg(target_arch = "riscv32")]
use esp_hal::gpio::{DriveMode, Level, Output, OutputConfig};
#[cfg(target_arch = "riscv32")]
use esp_hal::ledc::channel::{self, ChannelIFace};
#[cfg(target_arch = "riscv32")]
use esp_hal::ledc::timer::{self, TimerIFace};
#[cfg(target_arch = "riscv32")]
use esp_hal::ledc::{LSGlobalClkSource, Ledc, LowSpeed};
#[cfg(target_arch = "riscv32")]
use esp_hal::time::Rate;
#[cfg(target_arch = "riscv32")]
use static_cell::StaticCell;

#[cfg(target_arch = "riscv32")]
esp_bootloader_esp_idf::esp_app_desc!();

/// Production constants mirrored from `src/config/constants.rs` +
/// `src/hardware/init.rs` (SSR channel, 14-bit, 5 Hz).
#[cfg(target_arch = "riscv32")]
const SSR_FREQ_HZ: u32 = 5;
#[cfg(target_arch = "riscv32")]
const SSR_DUTY_RANGE: u32 = 1u32 << 14; // 2^14 = 16384 (esp-hal scales pct over 2^bits)
#[cfg(target_arch = "riscv32")]
const VERIFY_TOLERANCE_TICKS: u32 = 128; // same tolerance the old monitor used (0.78 %)

/// Read the CONFIG DUTY register (synchronous mirror of the last write).
#[cfg(target_arch = "riscv32")]
fn read_config_duty(channel_number: usize) -> u32 {
    let regs = unsafe { &*LEDC::ptr() };
    let raw = regs.ch(channel_number).duty().read().duty().bits();
    (raw >> 4) as u32 // esp-hal stores `value << 4` in the 19-bit field
}

/// Read the LIVE DUTY_R register (what the hardware is applying on the wire).
#[cfg(target_arch = "riscv32")]
fn read_live_duty(channel_number: usize) -> u32 {
    let regs = unsafe { &*LEDC::ptr() };
    let raw = regs.ch(channel_number).duty_r().read().duty_r().bits();
    (raw >> 4) as u32
}

/// esp-hal's own percentage → ticks formula: `(2^bits * pct) / 100`.
#[cfg(target_arch = "riscv32")]
fn pct_to_ticks(pct: u8) -> u32 {
    (SSR_DUTY_RANGE * pct as u32) / 100
}

#[cfg(target_arch = "riscv32")]
#[esp_hal::main]
fn main() -> ! {
    let config = esp_hal::Config::default();
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 32 * 1024);
    esp_println::logger::init_logger(log::LevelFilter::Info);

    esp_println::println!("LibreRoaster HIL Test: hil_c1 (DUTY_R latency / C1)");
    esp_println::println!("WARNING: GPIO10 will carry NON-ZERO duty — disconnect SSR/load power!");

    let mut passed: u32 = 0;
    let total: u32 = 5;

    // C1-RAW-01: Init LEDC exactly like production (Timer0, 14-bit, 5 Hz, Channel1).
    let mut ledc = Ledc::new(peripherals.LEDC);
    ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);

    let mut timer0 = ledc.timer::<LowSpeed>(timer::Number::Timer0);
    match timer0.configure(timer::config::Config {
        duty: timer::config::Duty::Duty14Bit,
        clock_source: timer::LSClockSource::APBClk,
        frequency: Rate::from_hz(SSR_FREQ_HZ),
    }) {
        Ok(_) => {}
        Err(_) => {
            esp_println::println!("TEST:c1_raw_01_init:FAIL:timer_config_error");
            esp_println::println!("TESTSUITE:COMPLETE:0/5:FAIL");
            loop {}
        }
    }

    static TIMER0: StaticCell<esp_hal::ledc::timer::Timer<'static, LowSpeed>> = StaticCell::new();
    let timer0 = TIMER0.init(timer0);

    let ssr_pin = Output::new(peripherals.GPIO10, Level::Low, OutputConfig::default());

    let mut ssr_channel = ledc.channel(channel::Number::Channel1, ssr_pin);
    match ssr_channel.configure(channel::config::Config {
        timer: timer0,
        duty_pct: 0,
        drive_mode: DriveMode::PushPull,
    }) {
        Ok(_) => {
            passed += 1;
            esp_println::println!("TEST:c1_raw_01_init:PASS:ledc_5hz_14bit");
        }
        Err(_) => {
            esp_println::println!("TEST:c1_raw_01_init:FAIL:channel_config_error");
        }
    }

    // C1-RAW-02: Config DUTY readback is SYNCHRONOUS (the fixed monitor path).
    // Write 50 % and read `duty()` at +1 ms — must already equal 8192 ticks.
    let expected_50 = pct_to_ticks(50);
    match ssr_channel.set_duty(50) {
        Ok(_) => {
            esp_hal::rom::ets_delay_us(1000); // +1 ms
            let cfg = read_config_duty(channel::Number::Channel1 as usize);
            if cfg.abs_diff(expected_50) <= VERIFY_TOLERANCE_TICKS {
                passed += 1;
                esp_println::println!(
                    "TEST:c1_raw_02_config_sync:PASS:config_duty={} expected={}",
                    cfg,
                    expected_50
                );
            } else {
                esp_println::println!(
                    "TEST:c1_raw_02_config_sync:FAIL:config_duty={} expected={}",
                    cfg,
                    expected_50
                );
            }
        }
        Err(_) => {
            esp_println::println!("TEST:c1_raw_02_config_sync:FAIL:set_duty_error");
        }
    }

    // C1-RAW-03: DUTY_R lag semantics. Write 30 % and sample the live register
    // at +1 ms and +250 ms. The +1 ms sample may be old OR new (period-boundary
    // dependent) — report it; the hard assertion is convergence at +250 ms.
    esp_hal::rom::ets_delay_us(250_000); // let DUTY_R settle at 50 % (old value)
    let expected_30 = pct_to_ticks(30);
    let mut lag_observed = false;
    match ssr_channel.set_duty(30) {
        Ok(_) => {
            esp_hal::rom::ets_delay_us(1000); // +1 ms
            let at_1ms = read_live_duty(channel::Number::Channel1 as usize);
            if at_1ms != expected_30 {
                lag_observed = true;
            }
            esp_hal::rom::ets_delay_us(249_000); // +250 ms total
            let at_250ms = read_live_duty(channel::Number::Channel1 as usize);
            let converged = at_250ms.abs_diff(expected_30) <= VERIFY_TOLERANCE_TICKS;
            if converged {
                passed += 1;
                esp_println::println!(
                    "TEST:c1_raw_03_live_lag:PASS:duty_r@1ms={} duty_r@250ms={} lag_observed={}",
                    at_1ms,
                    at_250ms,
                    lag_observed
                );
            } else {
                esp_println::println!(
                    "TEST:c1_raw_03_live_lag:FAIL:duty_r@250ms={} expected={}",
                    at_250ms,
                    expected_30
                );
            }
        }
        Err(_) => {
            esp_println::println!("TEST:c1_raw_03_live_lag:FAIL:set_duty_error");
        }
    }

    // C1-RAW-04: The pre-fix failure case — a heater RAMP (1 % → 60 %) verified
    // via config-DUTY readback, exactly as `monitor_ledc_after_set` now does.
    // The pre-fix code read DUTY_R here and failed every step > 0.8 %.
    let expected_1 = pct_to_ticks(1);
    let expected_60 = pct_to_ticks(60);
    let mut ramp_ok = true;
    for (pct, expected) in [(1u8, expected_1), (60u8, expected_60)] {
        match ssr_channel.set_duty(pct) {
            Ok(_) => {
                esp_hal::rom::ets_delay_us(1000); // +1 ms
                let cfg = read_config_duty(channel::Number::Channel1 as usize);
                if cfg.abs_diff(expected) > VERIFY_TOLERANCE_TICKS {
                    esp_println::println!(
                        "TEST:c1_raw_04_ramp_verify:FAIL:step={}% config_duty={} expected={}",
                        pct,
                        cfg,
                        expected
                    );
                    ramp_ok = false;
                }
            }
            Err(_) => {
                esp_println::println!(
                    "TEST:c1_raw_04_ramp_verify:FAIL:step={}% set_duty_error",
                    pct
                );
                ramp_ok = false;
            }
        }
    }
    if ramp_ok {
        passed += 1;
        esp_println::println!(
            "TEST:c1_raw_04_ramp_verify:PASS:1pct={} 60pct={} via config-dut",
            expected_1,
            expected_60
        );
    }

    // C1-RAW-05: Safe shutdown — 0 % write, config readback at +1 ms, DUTY_R
    // converged at +250 ms. Channel left at 0 %.
    let mut safe_ok = true;
    match ssr_channel.set_duty(0) {
        Ok(_) => {
            esp_hal::rom::ets_delay_us(1000);
            let cfg = read_config_duty(channel::Number::Channel1 as usize);
            if cfg != 0 {
                esp_println::println!("TEST:c1_raw_05_safe_zero:FAIL:config_duty={}", cfg);
                safe_ok = false;
            }
            esp_hal::rom::ets_delay_us(249_000);
            let live = read_live_duty(channel::Number::Channel1 as usize);
            if live != 0 {
                esp_println::println!("TEST:c1_raw_05_safe_zero:FAIL:duty_r={}", live);
                safe_ok = false;
            }
        }
        Err(_) => {
            esp_println::println!("TEST:c1_raw_05_safe_zero:FAIL:set_duty_error");
            safe_ok = false;
        }
    }
    if safe_ok {
        passed += 1;
        esp_println::println!("TEST:c1_raw_05_safe_zero:PASS:channel_left_at_0pct");
    }

    let suite_result = if passed == total { "PASS" } else { "FAIL" };
    esp_println::println!("TESTSUITE:COMPLETE:{}/{}:{}", passed, total, suite_result);

    loop {}
}

#[cfg(not(target_arch = "riscv32"))]
fn main() {}
