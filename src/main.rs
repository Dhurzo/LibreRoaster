#![no_std]
#![cfg_attr(target_arch = "riscv32", no_main)]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for duration of a data transfer."
)]
#[cfg(target_arch = "riscv32")]

#[cfg(not(target_arch = "riscv32"))]
fn main() {}

#[cfg(target_arch = "riscv32")]
use embassy_executor::Spawner;
#[cfg(target_arch = "riscv32")]
use esp_backtrace as _;
#[cfg(target_arch = "riscv32")]
use esp_hal::clock::CpuClock;
#[cfg(target_arch = "riscv32")]
use esp_hal::gpio::{Input, InputConfig, Io, Level, Output, OutputConfig, Pull};
#[cfg(target_arch = "riscv32")]
use esp_hal::ledc::timer::config::Config as TimerConfig;
#[cfg(target_arch = "riscv32")]
use esp_hal::ledc::timer::TimerIFace;
#[cfg(target_arch = "riscv32")]
use esp_hal::ledc::{channel, timer, Ledc, LowSpeed};
#[cfg(target_arch = "riscv32")]
use embedded_hal::delay::DelayNs;
use esp_hal::ledc::channel::{ChannelIFace, config::Config as ChannelConfig};
#[cfg(target_arch = "riscv32")]
use esp_hal::spi::master::Spi;

#[cfg(target_arch = "riscv32")]
use esp_hal::delay::Delay;

#[cfg(target_arch = "riscv32")]
use log::info;
#[cfg(target_arch = "riscv32")]
use static_cell::StaticCell;

#[cfg(target_arch = "riscv32")]
extern crate alloc;

/// SAFETY: The caller must ensure that the returned reference is only used
/// for the lifetime of the program, and that `value` is not dropped while the reference is in use.
#[cfg(target_arch = "riscv32")]
unsafe fn make_static<T>(mut value: T) -> &'static mut T {
    let ptr = &mut value as *mut T;
    &mut *ptr
}

#[cfg(target_arch = "riscv32")]
use libreroaster::application::AppBuilder;
#[cfg(target_arch = "riscv32")]
use libreroaster::hardware::fan::SimpleLedcFan;
#[cfg(target_arch = "riscv32")]
use libreroaster::hardware::max31856::Max31856;
#[cfg(target_arch = "riscv32")]
use libreroaster::hardware::shared_spi::SpiDeviceWithCs;
#[cfg(target_arch = "riscv32")]
use libreroaster::hardware::ssr::SsrControlSimple;
#[cfg(target_arch = "riscv32")]
use libreroaster::output::artisan::ArtisanFormatter;

#[cfg(target_arch = "riscv32")]
use core::cell::RefCell;
#[cfg(target_arch = "riscv32")]
use esp_bootloader_esp_idf;

#[cfg(target_arch = "riscv32")]
use critical_section;

esp_bootloader_esp_idf::esp_app_desc!();

#[cfg(target_arch = "riscv32")]
#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let mut delay = Delay::new();

    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 66320);

    let _io = Io::new(peripherals.IO_MUX);

    let heat_detection_pin = Input::new(
        peripherals.GPIO1,
        InputConfig::default().with_pull(Pull::Up),
    );

    let ledc = Ledc::new(peripherals.LEDC);

    let mut fan_timer = ledc.timer(timer::Number::Timer0);
    fan_timer
        .configure(TimerConfig {
            duty: timer::config::Duty::Duty8Bit,
            clock_source: timer::LSClockSource::APBClk,
            frequency: esp_hal::time::Rate::from_hz(libreroaster::config::FAN_PWM_FREQUENCY_HZ),
        })
        .map_err(|e| {
            log::error!("Failed to configure fan timer: {:?}", e);
            panic!("Fan timer configuration failed");
        })
        .unwrap();

    let gpio9 = peripherals.GPIO9;
    let mut fan_channel = ledc.channel::<LowSpeed>(channel::Number::Channel0, gpio9);

    // SAFETY: Extending the timer lifetime to static to satisfying the borrow checker for static initialization.
    let timer_ref: &'static mut dyn timer::TimerIFace<LowSpeed> = unsafe {
        &mut *(&mut fan_timer as *mut _ as *mut _)
    };

    fan_channel
        .configure(ChannelConfig {
            timer: timer_ref,
            duty_pct: 0,
            drive_mode: esp_hal::gpio::DriveMode::PushPull,
        })
        .map_err(|e| {
            log::error!("Failed to configure fan channel: {:?}", e);
            panic!("Fan channel configuration failed");
        })
        .unwrap();
    let mut fan_impl = SimpleLedcFan::new(fan_channel);

    let _ = libreroaster::control::traits::Fan::set_speed(&mut fan_impl, 0.0);

    use esp_hal::spi::master::Config;

    let spi_config = Config::default().with_frequency(esp_hal::time::Rate::from_khz(1000));

    let spi = match Spi::new(peripherals.SPI2, spi_config) {
        Ok(spi_instance) => spi_instance,
        Err(e) => {
            log::error!("Failed to initialize SPI2: {:?}", e);
            panic!("SPI2 initialization failed");
        }
    };

    static SPI_BUS: StaticCell<critical_section::Mutex<RefCell<Spi<esp_hal::Blocking>>>> =
        StaticCell::new();
    let spi_mutex = SPI_BUS.init(critical_section::Mutex::new(RefCell::new(spi)));

    // Create devices
    let bt_cs = Output::new(peripherals.GPIO4, Level::High, OutputConfig::default());
    let bt_spi = SpiDeviceWithCs::new(spi_mutex, bt_cs);

    let et_cs = Output::new(peripherals.GPIO3, Level::High, OutputConfig::default());
    let et_spi = SpiDeviceWithCs::new(spi_mutex, et_cs);

    let bean_sensor = Max31856::new(bt_spi)
        .map_err(|e| {
            log::error!("Failed to init BT sensor: {:?}", e);
            panic!("BT sensor initialization failed");
        })
        .unwrap();
    let env_sensor = Max31856::new(et_spi)
        .map_err(|e| {
            log::error!("Failed to init ET sensor: {:?}", e);
            panic!("ET sensor initialization failed");
        })
        .unwrap();

    info!("Temperature sensors initialized - BT: GPIO4, ET: GPIO3");

    let heat_detected = heat_detection_pin.is_low();
    info!(
        "Heat source detection (GPIO1): {}",
        if heat_detected {
            "DETECTED"
        } else {
            "NOT DETECTED"
        }
    );

    let ssr_pin_for_pwm = Output::new(peripherals.GPIO10, Level::Low, OutputConfig::default());

    let mut ssr_channel = ledc.channel::<LowSpeed>(channel::Number::Channel1, ssr_pin_for_pwm);
    ssr_channel
        .configure(ChannelConfig {
            timer: timer_ref,
            duty_pct: 0,
            drive_mode: esp_hal::gpio::DriveMode::PushPull,
        })
        .map_err(|e| {
            log::error!("Failed to configure SSR channel: {:?}", e);
            panic!("SSR channel configuration failed");
        })
        .unwrap();

    let real_ssr = SsrControlSimple::new(heat_detection_pin, ssr_channel)
        .map_err(|e| {
            log::error!("Failed to initialize SSR control: {:?}", e);
            panic!("SSR control initialization failed");
        })
        .unwrap();

    info!("SSR configured with REAL GPIO hardware (GPIO10) - simple mode");

    let static_ssr = unsafe { make_static(real_ssr) };
    let static_fan = unsafe { make_static(fan_impl) };

    info!("Drivers initialized and moved to static memory");

    let _ = libreroaster::hardware::usb_cdc::initialize_usb_cdc_system(peripherals.USB_DEVICE);

    let mut delay = Delay::new();

    info!("Wake the f*** up samurai we have beans to burn!");

    let app = AppBuilder::new()
        .with_uart(peripherals.UART0)
        .with_real_ssr(static_ssr)
        .with_fan_control(static_fan)
        .with_temperature_sensors(bean_sensor, env_sensor)
        .with_formatter(ArtisanFormatter::new())
        .build()
        .map_err(|e| {
            log::error!("Failed to build application: {:?}", e);
            panic!("Application build failed");
        })
        .unwrap();

    let _ = app
        .start_tasks(spawner)
        .await
        .map_err(|e| {
            log::error!("Failed to start application tasks: {:?}", e);
            panic!("Application tasks start failed");
        })
        .unwrap();

}
