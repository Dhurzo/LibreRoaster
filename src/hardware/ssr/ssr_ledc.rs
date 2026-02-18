#![cfg(target_arch = "riscv32")]

use super::LedcDutyReader;
use esp32c3::LEDC;
use esp_hal::ledc::{channel, LowSpeed};

pub struct LedcChannelMonitor<'a> {
    channel: channel::Channel<'a, LowSpeed>,
    channel_number: channel::Number,
}

impl<'a> LedcChannelMonitor<'a> {
    pub fn new(channel: channel::Channel<'a, LowSpeed>, channel_number: channel::Number) -> Self {
        Self {
            channel,
            channel_number,
        }
    }

    fn channel_index(&self) -> usize {
        self.channel_number as usize
    }
}

impl<'a> LedcDutyReader for LedcChannelMonitor<'a> {
    fn read_duty_ticks(&self) -> u16 {
        let regs = unsafe { &*LEDC::ptr() };
        let raw = regs.ch(self.channel_index()).duty().read().duty().bits();
        (raw >> 4) as u16
    }
}

impl<'a> channel::ChannelIFace<'a, LowSpeed> for LedcChannelMonitor<'a> {
    fn configure(
        &mut self,
        config: channel::config::Config<'a, LowSpeed>,
    ) -> Result<(), channel::Error> {
        self.channel.configure(config)
    }

    fn set_duty(&self, duty_pct: u8) -> Result<(), channel::Error> {
        self.channel.set_duty(duty_pct)
    }

    fn start_duty_fade(
        &self,
        start_duty_pct: u8,
        end_duty_pct: u8,
        duration_ms: u16,
    ) -> Result<(), channel::Error> {
        self.channel
            .start_duty_fade(start_duty_pct, end_duty_pct, duration_ms)
    }

    fn is_duty_fade_running(&self) -> bool {
        self.channel.is_duty_fade_running()
    }
}
