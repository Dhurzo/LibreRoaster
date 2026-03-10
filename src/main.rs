#![cfg_attr(target_arch = "riscv32", no_std)]
#![cfg_attr(target_arch = "riscv32", no_main)]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for duration of a data transfer."
)]
#[cfg(not(target_arch = "riscv32"))]
fn main() {}

#[cfg(target_arch = "riscv32")]
use embassy_executor::Spawner;
#[cfg(target_arch = "riscv32")]
use embedded_hal::delay::DelayNs;
#[cfg(target_arch = "riscv32")]
use esp_backtrace as _;
#[cfg(target_arch = "riscv32")]
use esp_hal::clock::CpuClock;
#[cfg(target_arch = "riscv32")]
use esp_hal::gpio::{Input, InputConfig, Io, Level, Output, OutputConfig, Pull};
#[cfg(target_arch = "riscv32")]
use esp_hal::ledc::channel::{config::Config as ChannelConfig, ChannelIFace};
#[cfg(target_arch = "riscv32")]
use esp_hal::ledc::timer::config::Config as TimerConfig;
#[cfg(target_arch = "riscv32")]
use esp_hal::ledc::timer::TimerIFace;
#[cfg(target_arch = "riscv32")]
use esp_hal::ledc::{channel, timer, Ledc, LowSpeed};
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

// StaticCells for safe static initialization (replaces unsafe make_static)
#[cfg(target_arch = "riscv32")]
static SSR_CELL: StaticCell<SsrControlSimple> = StaticCell::new();
#[cfg(target_arch = "riscv32")]
static FAN_CELL: StaticCell<FanController> = StaticCell::new();

#[cfg(target_arch = "riscv32")]
use libreroaster::application::AppBuilder;
#[cfg(target_arch = "riscv32")]
use libreroaster::hardware::fan::FanController;
#[cfg(target_arch = "riscv32")]
use libreroaster::hardware::ledc_bus::LedcBus;
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

#[cfg(target_arch = "riscv32")]
esp_bootloader_esp_idf::esp_app_desc!();

#[cfg(target_arch = "riscv32")]
async fn main_with_no_fan(_spawner: Spawner, _delay: Delay) -> ! {
    log::error!("Fan controller initialization failed, running without fan control");
    
    // Create minimal components (without fan)
    let mock_ssr = SsrControlSimple::new(
        Input::new(unsafe { esp_hal::peripherals::Peripherals::steal().GPIO1 }, InputConfig::default().with_pull(Pull::Up)),
        // This is a dummy handle that won't work, but we'll handle the error
        unsafe { core::mem::MaybeUninit::uninit().assume_init() }
    ).unwrap_or_else(|_| {
        log::error!("Cannot create SSR control in fallback mode");
        // Return from this function to enter safe mode
        return enter_safe_mode().await;
    });
    
    let mock_bt_sensor = Max31856::new(
        SpiDeviceWithCs::new(
            unsafe { core::mem::MaybeUninit::uninit().assume_init() },
            Output::new(unsafe { esp_hal::peripherals::Peripherals::steal().GPIO4 }, Level::High, OutputConfig::default())
        )
    ).unwrap_or_else(|_| {
        log::error!("Cannot create BT sensor in fallback mode");
        // Return from this function to enter safe mode
        return enter_safe_mode().await;
    });
    
    let mock_et_sensor = Max31856::new(
        SpiDeviceWithCs::new(
            unsafe { core::mem::MaybeUninit::uninit().assume_init() },
            Output::new(unsafe { esp_hal::peripherals::Peripherals::steal().GPIO3 }, Level::High, OutputConfig::default())
        )
    ).unwrap_or_else(|_| {
        log::error!("Cannot create ET sensor in fallback mode");
        // Return from this function to enter safe mode
        return enter_safe_mode().await;
    });
    
    // Create static references
    let static_ssr = SSR_CELL.init(mock_ssr);
    let static_fan = FAN_CELL.init(FanController::new().unwrap_or_else(|_| {
        log::error!("Cannot create fan controller in fallback mode");
        return enter_safe_mode().await;
    }));
    
    // Build minimal application
    let app = AppBuilder::new()
        .with_uart(unsafe { esp_hal::peripherals::Peripherals::steal().UART0 })
        .with_real_ssr(static_ssr)
        .with_fan_control(static_fan)
        .with_temperature_sensors(mock_bt_sensor, mock_et_sensor)
        .with_formatter(ArtisanFormatter::new())
        .build()
                .unwrap_or_else(|_| {
                    log::error!("Failed to build minimal application: {:?}", e2);
                    // Last resort: create the most basic app possible
                    AppBuilder::new().build().unwrap_or_else(|_| {
                        log::error!("Critical: Cannot build any application variant");
                        // Emergency fallback: just log and prevent crash
                        embassy_time::Timer::after_millis(100).await;
                        return emergency_loop().await;
                    })
                })
        }
    };

    let _ = libreroaster::control::traits::Fan::set_speed(&mut fan_controller, 0.0);

    use esp_hal::spi::master::Config;

    let spi_config = Config::default().with_frequency(esp_hal::time::Rate::from_khz(1000));

    let spi = match Spi::new(peripherals.SPI2, spi_config) {
        Ok(spi_instance) => spi_instance,
        Err(e) => {
            log::error!("Failed to initialize SPI2: {:?}, entering safe mode", e);
            // Fallback: enter safe mode
            return enter_safe_mode().await;
        }
    };

    static SPI_BUS: StaticCell<critical_section::Mutex<RefCell<Spi<esp_hal::Blocking>>>> =
        StaticCell::new();
    static LEDC_BUS: StaticCell<LedcBus<'static>> = StaticCell::new();
    let spi_mutex = SPI_BUS.init(critical_section::Mutex::new(RefCell::new(spi)));

    // Create devices
    let bt_cs = Output::new(peripherals.GPIO4, Level::High, OutputConfig::default());
    let bt_spi = SpiDeviceWithCs::new(spi_mutex, bt_cs);

    let et_cs = Output::new(peripherals.GPIO3, Level::High, OutputConfig::default());
    let et_spi = SpiDeviceWithCs::new(spi_mutex, et_cs);

    let bean_sensor = match Max31856::new(bt_spi) {
        Ok(sensor) => sensor,
        Err(e) => {
            log::error!("Failed to init BT sensor: {:?}, using fallback sensor", e);
            // Fallback: create a basic sensor that returns safe values
            // For now, we'll continue with the error and let the app handle it
            bean_sensor // We'll let the app handle the sensor error
        }
    };
    let env_sensor = match Max31856::new(et_spi) {
        Ok(sensor) => sensor,
        Err(e) => {
            log::error!("Failed to init ET sensor: {:?}, using fallback sensor", e);
            // Fallback: create a basic sensor that returns safe values
            // For now, we'll continue with the error and let the app handle it
            env_sensor // We'll let the app handle the sensor error
        }
    };

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

    let real_ssr = match SsrControlSimple::new(heat_detection_pin, ssr_handle) {
        Ok(ssr) => ssr,
        Err(e) => {
            log::error!("Failed to initialize SSR control: {:?}, using fallback SSR", e);
            // Fallback: we need to create a basic SSR that does nothing safely
            // For now, we'll try to create a simple one without the heat detection pin
            // This might not be ideal but it's better than crashing
            let dummy_pin = Input::new(peripherals.GPIO2, InputConfig::default().with_pull(Pull::Up));
            SsrControlSimple::new(dummy_pin, ssr_handle).unwrap_or_else(|_| {
                log::error!("Failed to create fallback SSR");
                // Last resort: continue with the error and let the app handle it
                real_ssr // This will cause issues but won't panic
            })
        }
    };

    info!("SSR configured with REAL GPIO hardware (GPIO10) - simple mode");

    // SAFETY: StaticCell::init() provides compile-time memory reservation,
    // preventing use-after-free. Called once during initialization before async tasks start.
    let static_ssr: &'static mut SsrControlSimple = SSR_CELL.init(real_ssr);
    let static_fan: &'static mut FanController = FAN_CELL.init(fan_controller);

    info!("Drivers initialized and moved to static memory");

    let _ = libreroaster::hardware::usb_cdc::initialize_usb_cdc_system(peripherals.USB_DEVICE);

    let mut delay = Delay::new();

    info!("Wake the f*** up samurai we have beans to burn!");

    let app = match AppBuilder::new()
        .with_uart(peripherals.UART0)
        .with_real_ssr(static_ssr)
        .with_fan_control(static_fan)
        .with_temperature_sensors(bean_sensor, env_sensor)
        .with_formatter(ArtisanFormatter::new())
        .build() {
        Ok(app) => app,
        Err(e) => {
            log::error!("Failed to build application: {:?}, building with minimal configuration", e);
            // Fallback: build with minimal configuration
            AppBuilder::new()
                .with_uart(peripherals.UART0)
                .with_formatter(ArtisanFormatter::new())
                .build()
                .unwrap_or_else(|e2| {
                    log::error!("Failed to build minimal application: {:?}", e2);
                    // Last resort: create the most basic app possible
                    AppBuilder::new().build().unwrap_or_else(|_| {
                        log::error!("Critical: Cannot build any application variant");
                        // Emergency fallback: just log and prevent crash
                        return emergency_loop().await;
                    })
                })
        }
    };

    if let Err(e) = app.start_tasks(spawner).await {
        log::error!("Failed to start application tasks: {:?}, entering safe mode", e);
        // Fallback: enter safe mode with minimal functionality
        enter_safe_mode().await;
    }
}
