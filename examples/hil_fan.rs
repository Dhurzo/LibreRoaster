//! HIL Test: Fan PWM on ESP32-C3 (GPIO9, LEDC Channel 0, Timer 1, 25kHz)
//!
//! Sweeps duty cycle 0→25→50→75→100→0 and verifies each step via
//! hardware register readback. Reports results over serial (esp-println).

#![cfg_attr(target_arch = "riscv32", no_std)]
#![cfg_attr(target_arch = "riscv32", no_main)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[cfg(target_arch = "riscv32")]
use esp32c3;
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
use esp_hal::peripherals::Peripherals;
#[cfg(target_arch = "riscv32")]
use esp_hal::time::Rate;
#[cfg(target_arch = "riscv32")]
use static_cell::StaticCell;

#[cfg(not(target_arch = "riscv32"))]
fn main() {}

#[cfg(target_arch = "riscv32")]
/// Read current duty from LEDC Channel 0 hardware register.
/// Mirrors `LedcBus::read_register()` in `src/hardware/ledc_bus.rs`.
fn read_duty_ch0() -> u16 {
    let regs = unsafe { &*esp32c3::LEDC::ptr() };
    let raw = regs.ch(0).duty().read().duty().bits();
    (raw >> 4) as u16
}

#[cfg(target_arch = "riscv32")]
fn delay_ms(ms: u32) {
    esp_hal::rom::ets_delay_us(ms * 1000);
}

#[cfg(target_arch = "riscv32")]
/// Spin forever with duty = 0 — safe-shutdown sink so the fan is
/// never left driving after an unrecoverable failure.
fn safe_shutdown() -> ! {
    let regs = unsafe { &*esp32c3::LEDC::ptr() };
    regs.ch(0).duty().write(|w| unsafe { w.duty().bits(0) });
    loop {
        esp_hal::rom::ets_delay_us(1_000_000);
    }
}

#[cfg(target_arch = "riscv32")]
#[esp_hal::main]
fn main() -> ! {
    let config = esp_hal::Config::default();
    let peripherals = esp_hal::init(config);

    esp_println::println!("LibreRoaster HIL Test: hil_fan");

    let mut ledc = Ledc::new(peripherals.LEDC);
    ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);

    static TIMER1: StaticCell<esp_hal::ledc::timer::Timer<'static, LowSpeed>> = StaticCell::new();
    let mut timer1 = ledc.timer::<LowSpeed>(timer::Number::Timer1);

    if timer1
        .configure(timer::config::Config {
            duty: timer::config::Duty::Duty8Bit,
            clock_source: timer::LSClockSource::APBClk,
            frequency: Rate::from_hz(25_000),
        })
        .is_err()
    {
        esp_println::println!("TEST:fan_raw_01_init:FAIL:timer_config");
        esp_println::println!("TESTSUITE:COMPLETE:0/7:FAIL");
        safe_shutdown();
    }

    let timer1 = TIMER1.init(timer1);

    let fan_pin = Output::new(peripherals.GPIO9, Level::Low, OutputConfig::default());
    let mut fan_ch = ledc.channel(channel::Number::Channel0, fan_pin);

    if fan_ch
        .configure(channel::config::Config {
            timer: timer1,
            duty_pct: 0,
            drive_mode: DriveMode::PushPull,
        })
        .is_err()
    {
        esp_println::println!("TEST:fan_raw_01_init:FAIL:channel_config");
        esp_println::println!("TESTSUITE:COMPLETE:0/7:FAIL");
        safe_shutdown();
    }

    esp_println::println!("TEST:fan_raw_01_init:PASS:ledc_configured");
    let mut passed: u8 = 1;

    macro_rules! duty_test {
        ($name:literal, $pct:expr, $expected_duty:expr, $tolerance:expr) => {{
            // Pass percentage directly — Channel::set_duty expects 0-100
            let _ = fan_ch.set_duty($pct);
            delay_ms(2); // allow hardware to latch the new duty value

            let readback = read_duty_ch0();
            let delta = if readback >= $expected_duty as u16 {
                readback - $expected_duty as u16
            } else {
                $expected_duty as u16 - readback
            };

            if delta <= $tolerance as u16 {
                esp_println::println!(
                    "TEST:{}:PASS:readback={}:expected={}:delta={}",
                    $name,
                    readback,
                    $expected_duty,
                    delta,
                );
                passed += 1;
            } else {
                esp_println::println!(
                    "TEST:{}:FAIL:readback={}:expected={}:delta={}:tolerance={}",
                    $name,
                    readback,
                    $expected_duty,
                    delta,
                    $tolerance,
                );
            }
        }};
    }

    // FAN-RAW-02: duty = 0%
    duty_test!("fan_raw_02_duty_0", 0, 0, 0);

    // FAN-RAW-03: 25% → tick 64
    duty_test!("fan_raw_03_duty_25", 25, 64, 2);

    // FAN-RAW-04: 50% → tick 128
    duty_test!("fan_raw_04_duty_50", 50, 128, 2);

    // FAN-RAW-05: 75% → tick 191
    duty_test!("fan_raw_05_duty_75", 75, 191, 2);

    // FAN-RAW-06: 100% → tick 255
    duty_test!("fan_raw_06_duty_100", 100, 255, 2);

    // FAN-RAW-07: safe shutdown
    {
        let _ = fan_ch.set_duty(0);
        delay_ms(2);
        let readback = read_duty_ch0();
        if readback == 0 {
            esp_println::println!("TEST:fan_raw_07_safe_shutdown:PASS:duty=0");
            passed += 1;
        } else {
            esp_println::println!(
                "TEST:fan_raw_07_safe_shutdown:FAIL:duty={}:expected=0",
                readback,
            );
        }
    }

    let total: u8 = 7;
    if passed == total {
        esp_println::println!("TESTSUITE:COMPLETE:{}/{}:PASS", passed, total);
    } else {
        esp_println::println!("TESTSUITE:COMPLETE:{}/{}:FAIL", passed, total);
    }

    loop {
        esp_hal::rom::ets_delay_us(1_000_000);
    }
}
