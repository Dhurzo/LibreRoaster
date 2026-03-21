use crate::error::app_error::InitError;
use crate::hardware::fan::FanController;
use crate::hardware::ledc_bus::{LedcBus, LedcChannelHandle};
use crate::hardware::max31856::Max31856;
use crate::hardware::shared_spi::SpiDeviceWithCs;
use crate::hardware::ssr::SsrControlSimple;
use alloc::{format, string::String};
use core::cell::RefCell;
use critical_section;
use esp_hal::gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull};
use esp_hal::ledc::channel;
use esp_hal::ledc::timer::{self, TimerIFace};
use esp_hal::ledc::{LSGlobalClkSource, Ledc, LowSpeed};
use esp_hal::peripherals::{GPIO1, GPIO10, GPIO3, GPIO4, GPIO9, LEDC, SPI2};
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use esp_hal::time::Rate;
use static_cell::StaticCell;

/// Holds the peripheral handles needed for initialization
pub struct InitPeripherals {
    pub ledc: LEDC<'static>,
    pub spi2: SPI2<'static>,
    pub gpio9: GPIO9<'static>,
    pub gpio10: GPIO10<'static>,
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
}

pub fn init_hardware(peripherals: InitPeripherals) -> Result<HardwareHandles, InitError> {
    // Initialize LEDC
    let mut ledc = Ledc::new(peripherals.ledc);
    ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);

    let mut timer0 = ledc.timer::<LowSpeed>(timer::Number::Timer0);
    timer0
        .configure(timer::config::Config {
            duty: timer::config::Duty::Duty8Bit,
            clock_source: timer::LSClockSource::APBClk,
            frequency: Rate::from_hz(1),
        })
        .map_err(|e| InitError::HardwareInit {
            what: "Timer 0",
            reason: format!("{:?}", e),
        })?;

    let mut timer1 = ledc.timer::<LowSpeed>(timer::Number::Timer1);
    timer1
        .configure(timer::config::Config {
            duty: timer::config::Duty::Duty8Bit,
            clock_source: timer::LSClockSource::APBClk,
            frequency: Rate::from_hz(25000),
        })
        .map_err(|e| InitError::HardwareInit {
            what: "Timer 1",
            reason: format!("{:?}", e),
        })?;

    // Configure channels
    let fan_pin = Output::new(peripherals.gpio9, Level::Low, OutputConfig::default());
    let fan_channel = ledc.channel(channel::Number::Channel0, fan_pin);

    let ssr_pin = Output::new(peripherals.gpio10, Level::Low, OutputConfig::default());
    let ssr_channel = ledc.channel(channel::Number::Channel1, ssr_pin);

    let ledc_bus = LedcBus::new(
        fan_channel,
        channel::Number::Channel0,
        ssr_channel,
        channel::Number::Channel1,
    );

    static LEDC_BUS: StaticCell<LedcBus<'static>> = StaticCell::new();
    let ledc_bus = LEDC_BUS.init(ledc_bus);

    // Initialize SPI
    let spi = Spi::new(
        peripherals.spi2,
        SpiConfig::default().with_frequency(Rate::from_khz(1000)),
    )
    .map_err(|e| InitError::HardwareInit {
        what: "SPI",
        reason: format!("{:?}", e),
    })?;

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
    let fan = FanController::with_handle(fan_handle).map_err(|e| InitError::HardwareInit {
        what: "Fan",
        reason: format!("{:?}", e),
    })?;

    Ok(HardwareHandles {
        bean_sensor,
        env_sensor,
        ssr,
        fan,
        ledc_bus,
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
