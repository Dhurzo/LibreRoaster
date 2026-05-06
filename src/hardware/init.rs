// Note: This module uses alloc::format! for error message construction.
// This is acceptable because init runs exactly once at startup and is NOT
// in the hot path. Do NOT use alloc in the control loop or telemetry path.

use crate::error::app_error::InitError;
use crate::hardware::fan::FanController;
use crate::hardware::ledc_bus::{LedcBus, LedcChannelHandle};
use crate::hardware::max31856::Max31856;
use crate::hardware::shared_spi::SpiDeviceWithCs;
use crate::hardware::ssr::SsrControlSimple;
use alloc::format;
use core::cell::RefCell;
use critical_section;
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull};
use esp_hal::gpio::DriveMode;
use esp_hal::ledc::channel::{self, ChannelIFace};
use esp_hal::ledc::timer::{self, TimerIFace};
use esp_hal::ledc::{LSGlobalClkSource, Ledc, LowSpeed};
use crate::config::constants::*;
use esp_hal::peripherals::{GPIO1, GPIO10, GPIO3, GPIO4, GPIO5, GPIO6, GPIO7, GPIO8, GPIO9, LEDC, SPI2};
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use esp_hal::spi::Mode;
use esp_hal::time::Rate;
use static_cell::StaticCell;

/// Holds the peripheral handles needed for initialization
pub struct InitPeripherals {
    pub ledc: LEDC<'static>,
    pub spi2: SPI2<'static>,
    pub gpio9: GPIO9<'static>,
    pub gpio10: GPIO10<'static>,
    pub gpio8: GPIO8<'static>,
    pub gpio7: GPIO7<'static>,
    pub gpio6: GPIO6<'static>,
    pub gpio5: GPIO5<'static>,
    pub gpio4: GPIO4<'static>,
    pub gpio3: GPIO3<'static>,
    pub gpio1: GPIO1<'static>,
}

pub struct HardwareHandles {
    pub bean_sensor: Max31856<
        SpiDeviceWithCs<
            'static,
            esp_hal::spi::master::Spi<'static, esp_hal::Blocking>,
            esp_hal::gpio::Output<'static>,
        >,
    >,
    pub env_sensor: Max31856<
        SpiDeviceWithCs<
            'static,
            esp_hal::spi::master::Spi<'static, esp_hal::Blocking>,
            esp_hal::gpio::Output<'static>,
        >,
    >,
    pub ssr: SsrControlSimple<'static, Input<'static>, LedcChannelHandle<'static>>,
    pub fan: FanController<'static>,
    pub ledc_bus: &'static LedcBus<'static>,
    pub status_led: Output<'static>,
}

pub fn init_hardware(peripherals: InitPeripherals) -> Result<HardwareHandles, InitError> {
    // Verify at init time that the constants match what this function expects.
    // This is a safety net: if someone changes FAN_PWM_PIN in constants.rs
    // but forgets to update init.rs, this will catch it at runtime.
    assert_eq!(FAN_PWM_PIN, 9, "FAN_PWM_PIN must be 9 (GPIO9 for fan PWM)");
    assert_eq!(SSR_CONTROL_PIN, 10, "SSR_CONTROL_PIN must be 10 (GPIO10 for SSR PWM)");
    assert_eq!(SPI_SCLK_PIN, 6, "SPI_SCLK_PIN must be 6 (GPIO6)");
    assert_eq!(SPI_MOSI_PIN, 7, "SPI_MOSI_PIN must be 7 (GPIO7)");
    assert_eq!(SPI_MISO_PIN, 5, "SPI_MISO_PIN must be 5 (GPIO5)");
    assert_eq!(THERMOCOUPLE_BT_CS_PIN, 4, "THERMOCOUPLE_BT_CS_PIN must be 4 (GPIO4)");
    assert_eq!(THERMOCOUPLE_ET_CS_PIN, 3, "THERMOCOUPLE_ET_CS_PIN must be 3 (GPIO3)");
    assert_eq!(HEAT_DETECTION_PIN, 1, "HEAT_DETECTION_PIN must be 1 (GPIO1)");
    assert_eq!(UART_TX_PIN, 21, "UART_TX_PIN must be 21 (GPIO21)");
    assert_eq!(UART_RX_PIN, 20, "UART_RX_PIN must be 20 (GPIO20)");
    assert_eq!(STATUS_LED_PIN, 8, "STATUS_LED_PIN must be 8 (GPIO8)");

    // Initialize LEDC
    let mut ledc = Ledc::new(peripherals.ledc);
    ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);

    // Make timers static so channels can reference them
    static TIMER0: StaticCell<esp_hal::ledc::timer::Timer<'static, LowSpeed>> = StaticCell::new();
    static TIMER1: StaticCell<esp_hal::ledc::timer::Timer<'static, LowSpeed>> = StaticCell::new();

    let mut timer0_ref = ledc.timer::<LowSpeed>(timer::Number::Timer0);
    timer0_ref
        .configure(timer::config::Config {
            duty: timer::config::Duty::Duty8Bit,
            clock_source: timer::LSClockSource::APBClk,
            frequency: Rate::from_hz(310),
        })
        .map_err(|e| InitError::HardwareInit {
            what: "Timer 0",
            reason: format!("{:?}", e),
        })?;

    let mut timer1_ref = ledc.timer::<LowSpeed>(timer::Number::Timer1);
    timer1_ref
        .configure(timer::config::Config {
            duty: timer::config::Duty::Duty8Bit,
            clock_source: timer::LSClockSource::APBClk,
            frequency: Rate::from_hz(25000),
        })
        .map_err(|e| InitError::HardwareInit {
            what: "Timer 1",
            reason: format!("{:?}", e),
        })?;

    let timer0 = TIMER0.init(timer0_ref);
    let timer1 = TIMER1.init(timer1_ref);

    // Configure channels and bind to timers
    let fan_pin = Output::new(peripherals.gpio9, Level::Low, OutputConfig::default());
    let mut fan_channel = ledc.channel(channel::Number::Channel0, fan_pin);
    fan_channel
        .configure(channel::config::Config {
            timer: timer1,
            duty_pct: 0,
            drive_mode: DriveMode::PushPull,
        })
        .map_err(|e| InitError::HardwareInit {
            what: "Fan channel",
            reason: format!("{:?}", e),
        })?;

    let ssr_pin = Output::new(peripherals.gpio10, Level::Low, OutputConfig::default());
    let mut ssr_channel = ledc.channel(channel::Number::Channel1, ssr_pin);
    ssr_channel
        .configure(channel::config::Config {
            timer: timer0,
            duty_pct: 0,
            drive_mode: DriveMode::PushPull,
        })
        .map_err(|e| InitError::HardwareInit {
            what: "SSR channel",
            reason: format!("{:?}", e),
        })?;

    let ledc_bus = LedcBus::new(
        fan_channel,
        channel::Number::Channel0,
        ssr_channel,
        channel::Number::Channel1,
    );

    static LEDC_BUS: StaticCell<LedcBus<'static>> = StaticCell::new();
    let ledc_bus = LEDC_BUS.init(ledc_bus);

    // Initialize SPI
    // ESP32-C3 SPI2 (FSPI) IO MUX pin mapping:
    //   SCK  = GPIO6 (FSPICLK)
    //   MOSI = GPIO7 (FSPID)
    //   MISO = GPIO5 (GPIO Matrix — GPIO2/FSPIQ conflicts with strapping)
    let spi = Spi::new(
        peripherals.spi2,
        SpiConfig::default().with_frequency(Rate::from_khz(1000)).with_mode(Mode::_1),
    )
    .map_err(|e| InitError::HardwareInit {
        what: "SPI",
        reason: format!("{:?}", e),
    })?
    .with_sck(peripherals.gpio6)
    .with_mosi(peripherals.gpio7)
    .with_miso(peripherals.gpio5);

    static SPI_BUS: StaticCell<critical_section::Mutex<RefCell<Spi<esp_hal::Blocking>>>> =
        StaticCell::new();
    let spi_mutex = SPI_BUS.init(critical_section::Mutex::new(RefCell::new(spi)));

    // Create GPIO pins for SPI chip selects
    let bt_cs = Output::new(peripherals.gpio4, Level::High, OutputConfig::default());
    let bt_spi = SpiDeviceWithCs::new(spi_mutex, bt_cs);

    let et_cs = Output::new(peripherals.gpio3, Level::High, OutputConfig::default());
    let et_spi = SpiDeviceWithCs::new(spi_mutex, et_cs);

    // Initialize temperature sensors
    let bean_sensor = Max31856::new(bt_spi).map_err(|e| InitError::HardwareInit {
        what: "BT sensor",
        reason: format!("{:?}", e),
    })?;
    let env_sensor = Max31856::new(et_spi).map_err(|e| InitError::HardwareInit {
        what: "ET sensor",
        reason: format!("{:?}", e),
    })?;

    // Initialize heat detection
    let heat_detection_pin = Input::new(
        peripherals.gpio1,
        InputConfig::default().with_pull(Pull::Up),
    );

    // Get handles from LedcBus
    let ssr_handle = ledc_bus.ssr_handle();
    let fan_handle = ledc_bus.fan_handle();

    // Initialize SSR control
    let ssr = SsrControlSimple::new(heat_detection_pin, ssr_handle).map_err(|e| {
        InitError::HardwareInit {
            what: "SSR",
            reason: format!("{:?}", e),
        }
    })?;

    // Initialize fan controller
    let fan = FanController::with_handle(fan_handle).map_err(|e| {
        InitError::HardwareInit {
            what: "Fan",
            reason: format!("{:?}", e),
        }
    })?;

    // Initialize status LED (GPIO8 - strapping pin, push-pull only)
    let status_led = Output::new(peripherals.gpio8, Level::High, OutputConfig::default());

    Ok(HardwareHandles {
        bean_sensor,
        env_sensor,
        ssr,
        fan,
        ledc_bus,
        status_led,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_error_context() {
        let err = InitError::HardwareInit {
            what: "SPI",
            reason: "timeout".to_string(),
        };
        // Test that error can be created and contains the expected data
        assert_eq!(
            matches!(err, InitError::HardwareInit { what: "SPI", .. }),
            true
        );
    }

    #[test]
    fn test_init_error_display() {
        let err = InitError::TaskSpawn {
            what: "control_task",
            reason: "out of memory".to_string(),
        };
        // Test that error can be displayed
        let display_string = format!("{:?}", err);
        assert!(display_string.contains("control_task"));
    }
}
