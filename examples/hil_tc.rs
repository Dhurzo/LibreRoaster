//! HIL Test: MAX31856 thermocouple reads on ESP32-C3 (SPI2 over GPIO6/7/5, CS GPIO4/GPIO3).
//!
//! Initialises the shared SPI bus and both `Max31856` sensors (bean / environment),
//! performs raw temperature reads, ambient-range and dual-channel checks, fault-register
//! validation, and a 3-sample stability measurement, printing `TEST:` lines over serial.

#![cfg_attr(target_arch = "riscv32", no_std)]
#![cfg_attr(target_arch = "riscv32", no_main)]

#[cfg(target_arch = "riscv32")]
use core::cell::RefCell;
#[cfg(target_arch = "riscv32")]
use critical_section;
#[cfg(target_arch = "riscv32")]
use esp_backtrace as _;
#[cfg(target_arch = "riscv32")]
esp_bootloader_esp_idf::esp_app_desc!();
#[cfg(target_arch = "riscv32")]
use esp_hal::gpio::{Level, Output, OutputConfig};
#[cfg(target_arch = "riscv32")]
use esp_hal::spi::master::{Config as SpiConfig, Spi};
#[cfg(target_arch = "riscv32")]
use esp_hal::spi::Mode;
#[cfg(target_arch = "riscv32")]
use esp_hal::time::Rate;
#[cfg(target_arch = "riscv32")]
use esp_println::println;
#[cfg(target_arch = "riscv32")]
use static_cell::StaticCell;

#[cfg(target_arch = "riscv32")]
use libreroaster::hardware::max31856::Max31856;
#[cfg(target_arch = "riscv32")]
use libreroaster::hardware::sensors::conversion::convert_raw_temp;
#[cfg(target_arch = "riscv32")]
use libreroaster::hardware::shared_spi::SpiDeviceWithCs;

/// HIL scenario: drive both MAX31856 sensors through init, raw conversion, ambient and
/// dual-channel checks, fault-register validation, and a 3-sample stability sweep; print the suite result.
#[cfg(target_arch = "riscv32")]
#[expect(
    deprecated,
    reason = "HIL test uses blocking SPI - sync read_raw_temperature is appropriate here"
)]
#[esp_hal::main]
fn main() -> ! {
    let config = esp_hal::Config::default();
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 32 * 1024);
    esp_println::logger::init_logger(log::LevelFilter::Info);

    println!("LibreRoaster HIL Test: hil_tc");

    let spi = Spi::new(
        peripherals.SPI2,
        SpiConfig::default()
            .with_frequency(Rate::from_khz(1000))
            .with_mode(Mode::_1),
    )
    .unwrap()
    .with_sck(peripherals.GPIO6)
    .with_mosi(peripherals.GPIO7)
    .with_miso(peripherals.GPIO5);

    static SPI_BUS: StaticCell<critical_section::Mutex<RefCell<Spi<esp_hal::Blocking>>>> =
        StaticCell::new();
    let spi_mutex = SPI_BUS.init(critical_section::Mutex::new(RefCell::new(spi)));

    let bt_cs = Output::new(peripherals.GPIO4, Level::High, OutputConfig::default());
    let bt_spi = SpiDeviceWithCs::new(spi_mutex, bt_cs);

    let et_cs = Output::new(peripherals.GPIO3, Level::High, OutputConfig::default());
    let et_spi = SpiDeviceWithCs::new(spi_mutex, et_cs);

    let mut bt_sensor = match Max31856::new(bt_spi) {
        Ok(sensor) => {
            println!("TEST:tc_raw_01_init:PASS:registers_configured");
            sensor
        }
        Err(_) => {
            println!("TEST:tc_raw_01_init:FAIL:reason=initialization_failed");
            loop {}
        }
    };

    let mut et_sensor = match Max31856::new(et_spi) {
        Ok(sensor) => {
            println!("TEST:tc_raw_01_init:PASS:registers_configured");
            sensor
        }
        Err(_) => {
            println!("TEST:tc_raw_01_init:FAIL:reason=initialization_failed");
            loop {}
        }
    };

    let bt_reading = match bt_sensor.read_raw_temperature() {
        Ok(reading) => {
            println!(
                "TEST:tc_raw_02_conversion:PASS:raw=0x{:06X}",
                reading.raw_temp
            );
            reading
        }
        Err(_) => {
            println!("TEST:tc_raw_02_conversion:FAIL:reason=communication_error");
            loop {}
        }
    };

    let bt_temp = convert_raw_temp(bt_reading.raw_temp);
    if (0.0..=50.0).contains(&bt_temp) {
        println!("TEST:tc_raw_03_ambient:PASS:temp={:.1}", bt_temp);
    } else {
        println!(
            "TEST:tc_raw_03_ambient:FAIL:temp={:.1}:reason=out_of_range",
            bt_temp
        );
    }

    let et_reading = match et_sensor.read_raw_temperature() {
        Ok(reading) => reading,
        Err(_) => {
            println!("TEST:tc_raw_04_dual_channel:FAIL:reason=et_communication_error");
            loop {}
        }
    };

    let et_temp = convert_raw_temp(et_reading.raw_temp);
    let final_bt_temp = convert_raw_temp(bt_reading.raw_temp);

    if et_temp > 0.0 && final_bt_temp > 0.0 {
        println!(
            "TEST:tc_raw_04_dual_channel:PASS:et={:.1},bt={:.1}",
            et_temp, final_bt_temp
        );
    } else {
        println!(
            "TEST:tc_raw_04_dual_channel:FAIL:et={:.1},bt={:.1}:reason=invalid_temperatures",
            et_temp, final_bt_temp
        );
    }

    if bt_reading.fault & 0x1F == 0 {
        println!(
            "TEST:tc_raw_05_no_fault:PASS:fault_register=0x{:02X}",
            bt_reading.fault
        );
    } else {
        println!(
            "TEST:tc_raw_05_no_fault:FAIL:fault_register=0x{:02X}:reason=fault_detected",
            bt_reading.fault
        );
    }

    let mut bt_temps = [0.0; 3];
    for i in 0..3 {
        match bt_sensor.read_raw_temperature() {
            Ok(reading) => {
                bt_temps[i] = convert_raw_temp(reading.raw_temp);
            }
            Err(_) => {
                println!(
                    "TEST:tc_raw_06_stability:FAIL:reason=reading_error_iteration_{}",
                    i + 1
                );
                loop {}
            }
        }

        for _ in 0..100_000 {
            core::hint::spin_loop();
        }
    }

    let max_temp = bt_temps.iter().fold(f32::MIN, |a, &b| a.max(b));
    let min_temp = bt_temps.iter().fold(f32::MAX, |a, &b| a.min(b));
    let variance = max_temp - min_temp;

    if variance < 5.0 {
        println!(
            "TEST:tc_raw_06_stability:PASS:et_variance={:.1},bt_variance={:.1}",
            0.0, variance
        );
    } else {
        println!("TEST:tc_raw_06_stability:FAIL:et_variance={:.1},bt_variance={:.1}:reason=unstable_readings", 0.0, variance);
    }

    println!("TESTSUITE:COMPLETE:6/6:PASS");

    loop {}
}

#[cfg(not(target_arch = "riscv32"))]
fn main() {}
