#![cfg(target_arch = "riscv32")]

use crate::hardware::ssr::LedcDutyReader;
use core::cell::{Cell, RefCell};
use core::hint::spin_loop;
use esp32c3::LEDC;
use esp_hal::ledc::channel::{self, ChannelIFace};
use esp_hal::ledc::LowSpeed;
use log::info;
use portable_atomic::{AtomicBool, Ordering};

struct LedcGuard {
    locked: AtomicBool,
}

impl LedcGuard {
    const fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
        }
    }

    fn lock(&self, channel_name: &'static str) -> LedcGuardToken<'_> {
        if self.locked.swap(true, Ordering::Acquire) {
            info!("LEDC guard busy - waiting for {}", channel_name);
            while self.locked.swap(true, Ordering::Acquire) {
                spin_loop();
            }
        }

        LedcGuardToken { guard: self }
    }
}

struct LedcGuardToken<'a> {
    guard: &'a LedcGuard,
}

impl Drop for LedcGuardToken<'_> {
    fn drop(&mut self) {
        self.guard.locked.store(false, Ordering::Release);
    }
}

struct ChannelEntry<'a> {
    channel: RefCell<channel::Channel<'a, LowSpeed>>,
    number: channel::Number,
    name: &'static str,
    duty: Cell<u8>,
}

impl<'a> ChannelEntry<'a> {
    fn new(
        channel: channel::Channel<'a, LowSpeed>,
        number: channel::Number,
        name: &'static str,
    ) -> Self {
        Self {
            channel: RefCell::new(channel),
            number,
            name,
            duty: Cell::new(0),
        }
    }
}

pub struct LedcBus<'a> {
    guard: LedcGuard,
    fan: ChannelEntry<'a>,
    ssr: ChannelEntry<'a>,
    /// Timer number used by Fan channel (Timer1 = 25kHz)
    fan_timer: u8,
    /// Timer number used by SSR channel (Timer0 = 1Hz)
    ssr_timer: u8,
}

impl<'a> LedcBus<'a> {
    pub fn new(
        fan_channel: channel::Channel<'a, LowSpeed>,
        fan_number: channel::Number,
        fan_timer: u8,
        ssr_channel: channel::Channel<'a, LowSpeed>,
        ssr_number: channel::Number,
        ssr_timer: u8,
    ) -> Self {
        Self {
            guard: LedcGuard::new(),
            fan: ChannelEntry::new(fan_channel, fan_number, "fan"),
            ssr: ChannelEntry::new(ssr_channel, ssr_number, "ssr"),
            fan_timer,
            ssr_timer,
        }
    }

    pub fn fan_handle(&'a self) -> LedcChannelHandle<'a> {
        LedcChannelHandle {
            bus: self,
            role: ChannelRole::Fan,
        }
    }

    pub fn ssr_handle(&'a self) -> LedcChannelHandle<'a> {
        LedcChannelHandle {
            bus: self,
            role: ChannelRole::Ssr,
        }
    }

    fn with_channel_mut<R, F>(&self, entry: &ChannelEntry<'a>, f: F) -> R
    where
        F: FnOnce(&mut channel::Channel<'a, LowSpeed>) -> R,
    {
        let _guard = self.guard.lock(entry.name);
        let mut channel_ref = entry.channel.borrow_mut();
        let result = f(&mut channel_ref);
        drop(channel_ref);
        result
    }

    fn read_register(&self, entry: &ChannelEntry<'a>) -> u16 {
        let regs = unsafe { &*LEDC::ptr() };
        let raw = regs.ch(entry.number as usize).duty().read().duty().bits();
        (raw >> 4) as u16
    }

    fn store_duty(&self, entry: &ChannelEntry<'a>, duty: u8) {
        entry.duty.set(duty);
    }
}

#[derive(Clone, Copy)]
enum ChannelRole {
    Fan,
    Ssr,
}

impl ChannelRole {
    fn entry<'a>(&self, bus: &'a LedcBus<'a>) -> &'a ChannelEntry<'a> {
        match self {
            ChannelRole::Fan => &bus.fan,
            ChannelRole::Ssr => &bus.ssr,
        }
    }
}

#[derive(Clone, Copy)]
pub struct LedcChannelHandle<'a> {
    bus: &'a LedcBus<'a>,
    role: ChannelRole,
}

impl<'a> LedcChannelHandle<'a> {
    fn entry(&self) -> &'a ChannelEntry<'a> {
        self.role.entry(self.bus)
    }

    pub fn set_duty(&self, duty: u8) -> Result<(), channel::Error> {
        let entry = self.entry();
        self.bus
            .with_channel_mut(entry, |channel| channel.set_duty(duty))?;
        self.bus.store_duty(entry, duty);
        Ok(())
    }

    pub fn start_duty_fade(
        &self,
        start_duty: u8,
        end_duty: u8,
        duration_ms: u16,
    ) -> Result<(), channel::Error> {
        let entry = self.entry();
        self.bus.with_channel_mut(entry, |channel| {
            channel.start_duty_fade(start_duty, end_duty, duration_ms)
        })?;
        self.bus.store_duty(entry, end_duty);
        Ok(())
    }

    pub fn applied_duty(&self) -> u8 {
        self.entry().duty.get()
    }

    pub fn applied_percent(&self) -> f32 {
        (self.applied_duty() as f32) * 100.0 / 255.0
    }
}

impl<'a> LedcDutyReader for LedcChannelHandle<'a> {
    fn read_duty_ticks(&self) -> u16 {
        self.bus.read_register(self.entry())
    }
}

impl<'a> ChannelIFace<'a, LowSpeed> for LedcChannelHandle<'a> {
    fn configure(
        &mut self,
        config: channel::config::Config<'a, LowSpeed>,
    ) -> Result<(), channel::Error> {
        let entry = self.entry();
        self.bus
            .with_channel_mut(entry, |channel| channel.configure(config))
    }

    fn set_duty(&self, duty_pct: u8) -> Result<(), channel::Error> {
        self.set_duty(duty_pct)
    }

    fn start_duty_fade(
        &self,
        start_duty_pct: u8,
        end_duty_pct: u8,
        duration_ms: u16,
    ) -> Result<(), channel::Error> {
        self.start_duty_fade(start_duty_pct, end_duty_pct, duration_ms)
    }

    fn is_duty_fade_running(&self) -> bool {
        let entry = self.entry();
        self.bus
            .with_channel_mut(entry, |channel| channel.is_duty_fade_running())
    }
}

unsafe impl<'a> Send for LedcBus<'a> {}
unsafe impl<'a> Send for LedcChannelHandle<'a> {}
