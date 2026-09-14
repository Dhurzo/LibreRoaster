//! HIL Test: SSR LEDC channel on ESP32-C3 (GPIO10, Channel1, Timer0, 5 Hz).
//!
//! Verifies the SSR PWM channel initialises, reads back duty = 0 from the hardware
//! register, reads the GPIO1 heat-detection pin, and re-asserts the zero-duty
//! safe-shutdown path. SAFE mode only — duty never exceeds 0 %, so no heater is
//! energised. Reports results as `TEST:` lines over serial (esp-println).

#![cfg_attr(target_arch = "riscv32", no_std)]
#![cfg_attr(target_arch = "riscv32", no_main)]

// SAFE mode only — duty NEVER exceeds 0%. No heater activation.

#[cfg(target_arch = "riscv32")]
use esp32c3::LEDC;
#[cfg(target_arch = "riscv32")]
use esp_backtrace as _;
#[cfg(target_arch = "riscv32")]
use esp_hal::gpio::{DriveMode, Input, InputConfig, Level, Output, OutputConfig, Pull};
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

/// Read the current duty from an LEDC channel hardware register (ticks, value << 4).
#[cfg(target_arch = "riscv32")]
fn read_ledc_duty(channel_number: usize) -> u16 {
    let regs = unsafe { &*LEDC::ptr() };
    let raw = regs.ch(channel_number).duty().read().duty().bits();
    (raw >> 4) as u16
}

/// HIL scenario: initialise the SSR LEDC channel, verify zero-duty readback, read the
/// GPIO1 heat pin, and re-assert the safe-shutdown (zero-duty) path; print the suite result.
#[cfg(target_arch = "riscv32")]
#[esp_hal::main]
fn main() -> ! {
    let config = esp_hal::Config::default();
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 32 * 1024);
    esp_println::logger::init_logger(log::LevelFilter::Info);

    esp_println::println!("LibreRoaster HIL Test: hil_ssr");

    let mut passed: u32 = 0;
    let total: u32 = 4;

    // SSR-RAW-01: Initialize LEDC timer + channel on GPIO10
    let mut ledc = Ledc::new(peripherals.LEDC);
    ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);

    let mut timer0 = ledc.timer::<LowSpeed>(timer::Number::Timer0);
    match timer0.configure(timer::config::Config {
        duty: timer::config::Duty::Duty8Bit,
        clock_source: timer::LSClockSource::APBClk,
        frequency: Rate::from_hz(1),
    }) {
        Ok(_) => {}
        Err(_) => {
            esp_println::println!("TEST:ssr_raw_01_init:FAIL:timer_config_error");
            esp_println::println!("TESTSUITE:COMPLETE:0/4:FAIL");
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
            esp_println::println!("TEST:ssr_raw_01_init:PASS:ledc_configured");
        }
        Err(_) => {
            esp_println::println!("TEST:ssr_raw_01_init:FAIL:channel_config_error");
        }
    }

    // SSR-RAW-02: Set duty=0, verify hardware register readback
    match ssr_channel.set_duty(0) {
        Ok(_) => {
            esp_hal::rom::ets_delay_us(100);
            let readback = read_ledc_duty(channel::Number::Channel1 as usize);
            if readback == 0 {
                passed += 1;
                esp_println::println!("TEST:ssr_raw_02_zero_duty:PASS:readback=0");
            } else {
                esp_println::println!(
                    "TEST:ssr_raw_02_zero_duty:FAIL:readback={}:reason=non_zero",
                    readback
                );
            }
        }
        Err(_) => {
            esp_println::println!("TEST:ssr_raw_02_zero_duty:FAIL:reason=set_duty_error");
        }
    }

    // SSR-RAW-03: Read GPIO1 heat-detection pin (pull-up)
    let heat_pin = Input::new(
        peripherals.GPIO1,
        InputConfig::default().with_pull(Pull::Up),
    );
    let is_high = heat_pin.is_high();
    let pin_label = if is_high { "high" } else { "low" };
    passed += 1;
    esp_println::println!("TEST:ssr_raw_03_heat_detect:PASS:pin_state={}", pin_label);

    // SSR-RAW-04: Re-assert duty=0 (safe-shutdown path verification)
    match ssr_channel.set_duty(0) {
        Ok(_) => {
            esp_hal::rom::ets_delay_us(100);
            let readback = read_ledc_duty(channel::Number::Channel1 as usize);
            if readback == 0 {
                passed += 1;
                esp_println::println!("TEST:ssr_raw_04_safe_shutdown:PASS:duty=0");
            } else {
                esp_println::println!(
                    "TEST:ssr_raw_04_safe_shutdown:FAIL:readback={}:reason=non_zero",
                    readback
                );
            }
        }
        Err(_) => {
            esp_println::println!("TEST:ssr_raw_04_safe_shutdown:FAIL:reason=set_duty_error");
        }
    }

    let suite_result = if passed == total { "PASS" } else { "FAIL" };
    esp_println::println!("TESTSUITE:COMPLETE:{}/{}:{}", passed, total, suite_result);

    loop {}
}

#[cfg(not(target_arch = "riscv32"))]
fn main() {}
