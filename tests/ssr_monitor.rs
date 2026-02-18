// This test requires ESP-specific hardware (esp_hal) and can only run on riscv32 target
#![cfg(target_arch = "riscv32")]

use core::cell::{Cell, RefCell};
use core::convert::Infallible;
use std::collections::VecDeque;

use embedded_hal::digital::InputPin;
use esp_hal::ledc::channel::{self, ChannelIFace};
use esp_hal::ledc::LowSpeed;

use libreroaster::hardware::ssr::{percentage_to_ledc_duty, LedcDutyReader, SsrControlSimple};

struct FakeDetectPin;

impl InputPin for FakeDetectPin {
    type Error = Infallible;

    fn is_high(&self) -> Result<bool, Self::Error> {
        Ok(false)
    }

    fn is_low(&self) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

struct FakeLedcChannel {
    responses: RefCell<VecDeque<u16>>,
    last_commanded: Cell<u8>,
}

impl FakeLedcChannel {
    fn new(responses: Vec<u16>) -> Self {
        Self {
            responses: RefCell::new(responses.into()),
            last_commanded: Cell::new(0),
        }
    }

    fn next_reading(&self) -> u16 {
        self.responses
            .borrow_mut()
            .pop_front()
            .unwrap_or(self.last_commanded.get() as u16)
    }
}

impl LedcDutyReader for FakeLedcChannel {
    fn read_duty_ticks(&self) -> u16 {
        self.next_reading()
    }
}

impl<'a> ChannelIFace<'a, LowSpeed> for FakeLedcChannel {
    fn configure(
        &mut self,
        _: channel::config::Config<'a, LowSpeed>,
    ) -> Result<(), channel::Error> {
        Ok(())
    }

    fn set_duty(&self, duty_pct: u8) -> Result<(), channel::Error> {
        self.last_commanded.set(duty_pct);
        Ok(())
    }

    fn start_duty_fade(&self, _: u8, _: u8, _: u16) -> Result<(), channel::Error> {
        Ok(())
    }

    fn is_duty_fade_running(&self) -> bool {
        false
    }
}

#[test]
fn ssr_monitor_detects_drift() {
    let detection_pin = FakeDetectPin;
    let commanded = percentage_to_ledc_duty(50.0);
    let responses = vec![(commanded as u16 + 3), (commanded as u16 + 3)];
    let fake_channel = FakeLedcChannel::new(responses);

    let mut ssr = SsrControlSimple::new(detection_pin, fake_channel)
        .expect("SSR control should initialize in tests");

    let result = ssr.set_power(50.0);
    assert!(result.is_err(), "Persistent drift should return an error");
    assert_eq!(ssr.last_lead_delta_ticks(), 3);
    assert_eq!(ssr.last_retry_count(), 1);
}
