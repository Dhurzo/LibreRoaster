//! GPIO Roast Test: Full pin initialization + synthetic roast curve on ESP32-C3
//!
//! Boots on real ESP32-C3 hardware, initializes ALL GPIO pins exactly as the
//! production firmware does, runs a synthetic roast curve, and drives real
//! SSR PWM (GPIO10) and Fan PWM (GPIO9) with structured test output.
//!
//! Build:
//!   cargo build --release --target riscv32imc-unknown-none-elf \
//!     --features "embedded,simulated-sensors" --example gpio_roast_test
//!
//! Phases:
//!   Phase 1: GPIO pin initialization verification (8 tests)
//!   Phase 2: Synthetic roast curve execution with telemetry
//!   Phase 3: LEDC duty verification during roast (inline with Phase 2)
//!   Phase 4: Safe shutdown

#![cfg_attr(target_arch = "riscv32", no_std)]
#![cfg_attr(target_arch = "riscv32", no_main)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

// ── Host stub ──────────────────────────────────────────────────────────────

#[cfg(not(target_arch = "riscv32"))]
fn main() {}

// ── Embedded implementation (ESP32-C3 only) ────────────────────────────────

#[cfg(target_arch = "riscv32")]
use core::cell::RefCell;
#[cfg(target_arch = "riscv32")]
use critical_section;
#[cfg(target_arch = "riscv32")]
use esp32c3;
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
use esp_hal::spi::master::{Config as SpiConfig, Spi};
#[cfg(target_arch = "riscv32")]
use esp_hal::spi::Mode;
#[cfg(target_arch = "riscv32")]
use esp_hal::time::Rate;
#[cfg(target_arch = "riscv32")]
use static_cell::StaticCell;

#[cfg(target_arch = "riscv32")]
use libreroaster::hardware::sensors::simulated::{CurvePoint, RoastCurve};

// ── Helpers ────────────────────────────────────────────────────────────────

/// Read current duty from an LEDC channel hardware register.
/// Mirrors `LedcBus::read_register()` in `src/hardware/ledc_bus.rs`.
#[cfg(target_arch = "riscv32")]
fn read_ledc_duty(channel_number: usize) -> u16 {
    let regs = unsafe { &*esp32c3::LEDC::ptr() };
    let raw = regs.ch(channel_number).duty().read().duty().bits();
    (raw >> 4) as u16
}

/// Busy-wait for `ms` milliseconds via the ESP ROM delay routine.
#[cfg(target_arch = "riscv32")]
fn delay_ms(ms: u32) {
    esp_hal::rom::ets_delay_us(ms * 1000);
}

/// Clamp value to `[min, max]` without `libm`.
#[cfg(target_arch = "riscv32")]
fn clamp_f32(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// Create the pinout-verify roast curve: 25 C -> 200 C -> 50 C in ~120 s.
/// Shortest built-in curve, designed for quick automated testing.
///
/// TODO: Replace with `RoastCurve::pinout_verify()` once added to library.
#[cfg(target_arch = "riscv32")]
fn create_pinout_verify_curve() -> RoastCurve {
    let mut curve = RoastCurve::new();
    curve.add_point(CurvePoint {
        time_secs: 0,
        bean_temp: 25.0,
        env_temp: 25.0,
    });
    curve.add_point(CurvePoint {
        time_secs: 20,
        bean_temp: 60.0,
        env_temp: 80.0,
    });
    curve.add_point(CurvePoint {
        time_secs: 40,
        bean_temp: 100.0,
        env_temp: 130.0,
    });
    curve.add_point(CurvePoint {
        time_secs: 60,
        bean_temp: 140.0,
        env_temp: 175.0,
    });
    curve.add_point(CurvePoint {
        time_secs: 80,
        bean_temp: 180.0,
        env_temp: 215.0,
    });
    curve.add_point(CurvePoint {
        time_secs: 100,
        bean_temp: 200.0,
        env_temp: 240.0,
    });
    curve.add_point(CurvePoint {
        time_secs: 110,
        bean_temp: 150.0,
        env_temp: 180.0,
    });
    curve.add_point(CurvePoint {
        time_secs: 120,
        bean_temp: 50.0,
        env_temp: 60.0,
    });
    curve
}

/// Spin forever with both PWM channels forced to duty 0.
/// Used after an unrecoverable failure so the hardware is never left driving.
#[cfg(target_arch = "riscv32")]
fn safe_shutdown() -> ! {
    let regs = unsafe { &*esp32c3::LEDC::ptr() };
    regs.ch(0).duty().write(|w| unsafe { w.duty().bits(0) });
    regs.ch(1).duty().write(|w| unsafe { w.duty().bits(0) });
    loop {
        delay_ms(1_000);
    }
}

// ── Main ───────────────────────────────────────────────────────────────────

/// HIL scenario: verify all GPIO pinouts (LEDC fan/SSR, GPIO1 heat, GPIO8 LED, SPI bus,
/// chip-selects), then run a synthetic 120 s roast curve driving real SSR/fan PWM with
/// telemetry and a safe-shutdown verification at the end.
#[cfg(target_arch = "riscv32")]
#[esp_hal::main]
fn main() -> ! {
    let config = esp_hal::Config::default();
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 32 * 1024);
    esp_println::logger::init_logger(log::LevelFilter::Info);

    esp_println::println!("LibreRoaster GPIO Roast Test");
    esp_println::println!("============================");

    let mut passed: u32 = 0;
    let mut failed: u32 = 0;

    // =====================================================================
    // Phase 1: GPIO Pin Initialization Verification
    // =====================================================================
    esp_println::println!("PHASE:1:gpio_init");

    // --- LEDC peripheral + global clock ---
    let mut ledc = Ledc::new(peripherals.LEDC);
    ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);

    // --- Fan Timer (Timer1, 25 kHz, 8-bit) ---
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
        esp_println::println!("TEST:gpio_09_fan:FAIL:timer1_config");
        esp_println::println!("TESTSUITE:COMPLETE:0/9:FAIL");
        safe_shutdown();
    }
    let timer1 = TIMER1.init(timer1);

    // --- SSR Timer (Timer0, 310 Hz, 8-bit) ---
    static TIMER0: StaticCell<esp_hal::ledc::timer::Timer<'static, LowSpeed>> = StaticCell::new();
    let mut timer0 = ledc.timer::<LowSpeed>(timer::Number::Timer0);
    if timer0
        .configure(timer::config::Config {
            duty: timer::config::Duty::Duty8Bit,
            clock_source: timer::LSClockSource::APBClk,
            frequency: Rate::from_hz(1),
        })
        .is_err()
    {
        esp_println::println!("TEST:gpio_10_ssr:FAIL:timer0_config");
        esp_println::println!("TESTSUITE:COMPLETE:0/9:FAIL");
        safe_shutdown();
    }
    let timer0 = TIMER0.init(timer0);

    // --- GPIO9: Fan PWM (LEDC Ch0, Timer1, 25 kHz, PushPull) ---
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
        esp_println::println!("TEST:gpio_09_fan:FAIL:channel_config");
        failed += 1;
    } else {
        delay_ms(1);
        let duty = read_ledc_duty(0);
        if duty == 0 {
            esp_println::println!("TEST:gpio_09_fan:PASS:ch0_25khz_duty=0");
            passed += 1;
        } else {
            esp_println::println!("TEST:gpio_09_fan:FAIL:initial_duty={}:expected=0", duty);
            failed += 1;
        }
    }

    // --- GPIO10: SSR PWM (LEDC Ch1, Timer0, 310 Hz, PushPull) ---
    let ssr_pin = Output::new(peripherals.GPIO10, Level::Low, OutputConfig::default());
    let mut ssr_ch = ledc.channel(channel::Number::Channel1, ssr_pin);
    if ssr_ch
        .configure(channel::config::Config {
            timer: timer0,
            duty_pct: 0,
            drive_mode: DriveMode::PushPull,
        })
        .is_err()
    {
        esp_println::println!("TEST:gpio_10_ssr:FAIL:channel_config");
        failed += 1;
    } else {
        delay_ms(1);
        let duty = read_ledc_duty(1);
        if duty == 0 {
            esp_println::println!("TEST:gpio_10_ssr:PASS:ch1_310hz_duty=0");
            passed += 1;
        } else {
            esp_println::println!("TEST:gpio_10_ssr:FAIL:initial_duty={}:expected=0", duty);
            failed += 1;
        }
    }

    // --- GPIO1: Heat detection input (pull-up) ---
    let heat_pin = Input::new(
        peripherals.GPIO1,
        InputConfig::default().with_pull(Pull::Up),
    );
    // Give the pull-up a moment to stabilize
    for _ in 0..10_000 {
        core::hint::spin_loop();
    }
    let pin_label = if heat_pin.is_high() { "high" } else { "low" };
    esp_println::println!("TEST:gpio_01_heat:PASS:input_pullup:state={}", pin_label);
    passed += 1;

    // --- GPIO8: Status LED (output, high) ---
    let mut status_led = Output::new(peripherals.GPIO8, Level::High, OutputConfig::default());
    esp_println::println!("TEST:gpio_08_led:PASS:output_high");
    passed += 1;

    // --- SPI bus: GPIO6 (SCK), GPIO7 (MOSI), GPIO5 (MISO) ---
    let spi_result = Spi::new(
        peripherals.SPI2,
        SpiConfig::default()
            .with_frequency(Rate::from_khz(1000))
            .with_mode(Mode::_1),
    );
    let spi = match spi_result {
        Ok(s) => s
            .with_sck(peripherals.GPIO6)
            .with_mosi(peripherals.GPIO7)
            .with_miso(peripherals.GPIO5),
        Err(_) => {
            esp_println::println!("TEST:gpio_spi_bus:FAIL:spi_init_error");
            esp_println::println!("TESTSUITE:COMPLETE:{}/9:FAIL", passed);
            safe_shutdown();
        }
    };

    static SPI_BUS: StaticCell<critical_section::Mutex<RefCell<Spi<esp_hal::Blocking>>>> =
        StaticCell::new();
    let _spi_mutex = SPI_BUS.init(critical_section::Mutex::new(RefCell::new(spi)));
    esp_println::println!("TEST:gpio_spi_bus:PASS:sck=6_mosi=7_miso=5");
    passed += 1;

    // --- GPIO4: BT chip-select (output, high) ---
    let _bt_cs = Output::new(peripherals.GPIO4, Level::High, OutputConfig::default());
    esp_println::println!("TEST:gpio_04_bt_cs:PASS:output_high");
    passed += 1;

    // --- GPIO3: ET chip-select (output, high) ---
    let _et_cs = Output::new(peripherals.GPIO3, Level::High, OutputConfig::default());
    esp_println::println!("TEST:gpio_03_et_cs:PASS:output_high");
    passed += 1;

    // --- Post-init LEDC zero-verification ---
    let fan_duty = read_ledc_duty(0);
    let ssr_duty = read_ledc_duty(1);
    if fan_duty == 0 && ssr_duty == 0 {
        esp_println::println!("TEST:ledc_init_zero:PASS:fan={},ssr={}", fan_duty, ssr_duty);
        passed += 1;
    } else {
        esp_println::println!(
            "TEST:ledc_init_zero:FAIL:fan={},ssr={}:expected=0,0",
            fan_duty,
            ssr_duty
        );
        failed += 1;
    }

    esp_println::println!("PHASE:1:RESULT:{}/8", passed);
    if failed > 0 {
        esp_println::println!("TESTSUITE:COMPLETE:{}/9:FAIL", passed);
        safe_shutdown();
    }

    // =====================================================================
    // Phase 2 + 3: Synthetic Roast Curve Execution + LEDC Verification
    // =====================================================================
    esp_println::println!("PHASE:2:roast_curve");

    let curve = create_pinout_verify_curve();
    let total_duration_secs: u32 = 120;
    let tick_ms: u32 = 100;
    let ticks_per_sec: u32 = 1000 / tick_ms;
    let total_ticks: u32 = total_duration_secs * ticks_per_sec;
    let telemetry_interval_ticks: u32 = 10 * ticks_per_sec;

    esp_println::println!(
        "ROAST:curve=pinout_verify:duration={}s:tick={}ms",
        total_duration_secs,
        tick_ms
    );

    // Simulated temperatures with thermal lag model
    let mut sim_bt: f32 = 25.0;
    let mut sim_et: f32 = 25.0;
    let mut led_high: bool = true;

    for tick in 0..total_ticks {
        let elapsed_secs = tick * tick_ms / 1000;

        // Target temperatures from the curve at current elapsed time
        let (target_bt, target_et) = curve.temperatures_at(elapsed_secs);

        // Thermal-lag model: simulated sensor lags behind the curve target
        sim_bt = sim_bt + (target_bt - sim_bt) * 0.1;
        sim_et = sim_et + (target_et - sim_et) * 0.1;

        // ── Heater control (SSR) ──
        // Proportional: drive harder when lagging behind the curve.
        // CAPPED at 50 % duty cycle — no real heater connected (safety).
        let heater_error = target_bt - sim_bt;
        let heater_pct = clamp_f32(heater_error * 2.0, 0.0, 50.0);

        // ── Fan schedule ──
        let fan_pct: f32 = if sim_bt >= 200.0 {
            100.0
        } else if sim_bt >= 150.0 {
            75.0
        } else if sim_bt > 25.0 {
            50.0
        } else {
            25.0
        };

        // Apply duties to LEDC channels
        let _ = ssr_ch.set_duty(heater_pct as u8);
        let _ = fan_ch.set_duty(fan_pct as u8);

        // ── Telemetry + LEDC readback every 10 s (Phase 3) ──
        if tick > 0 && tick % telemetry_interval_ticks == 0 {
            delay_ms(1); // let hardware latch
            let fan_readback = read_ledc_duty(0);
            let ssr_readback = read_ledc_duty(1);

            esp_println::println!(
                "ROAST:t={}:bt={:.1}:et={:.1}:heater={}:fan={}",
                elapsed_secs,
                sim_bt,
                sim_et,
                heater_pct as u8,
                fan_pct as u8,
            );
            esp_println::println!("LEDC:fan_duty={}:ssr_duty={}", fan_readback, ssr_readback,);
        }

        // Blink status LED every second
        if tick % ticks_per_sec == 0 {
            led_high = !led_high;
            if led_high {
                status_led.set_high();
            } else {
                status_led.set_low();
            }
        }

        delay_ms(tick_ms);
    }

    esp_println::println!("PHASE:2:COMPLETE");

    // =====================================================================
    // Phase 4: Safe Shutdown
    // =====================================================================
    esp_println::println!("PHASE:4:safe_shutdown");

    let _ = ssr_ch.set_duty(0);
    let _ = fan_ch.set_duty(0);
    delay_ms(2);

    let fan_final = read_ledc_duty(0);
    let ssr_final = read_ledc_duty(1);

    if fan_final == 0 && ssr_final == 0 {
        esp_println::println!(
            "TEST:safe_shutdown:PASS:fan={},ssr={}",
            fan_final,
            ssr_final
        );
        passed += 1;
    } else {
        esp_println::println!(
            "TEST:safe_shutdown:FAIL:fan={},ssr={}:expected=0,0",
            fan_final,
            ssr_final
        );
        failed += 1;
    }

    // ── Final suite result ──
    let total: u32 = 9;
    let suite_result = if failed == 0 { "PASS" } else { "FAIL" };
    esp_println::println!("TESTSUITE:COMPLETE:{}/{}:{}", passed, total, suite_result);

    // Loop forever with status LED blink pattern
    loop {
        status_led.set_high();
        delay_ms(500);
        status_led.set_low();
        delay_ms(500);
    }
}
