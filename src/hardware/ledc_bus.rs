use crate::hardware::ledc_guard::{LedcGuard, LedcGuardError};
use crate::hardware::ssr::{DutyWriteError, LedcDutyReader};
use core::cell::{Cell, RefCell};
use esp32c3::LEDC;
use esp_hal::ledc::channel::{self, ChannelHW, ChannelIFace};
use esp_hal::ledc::LowSpeed;
use log::warn;

struct ChannelEntry<'a> {
    channel: RefCell<channel::Channel<'a, LowSpeed>>,
    number: channel::Number,
    name: &'static str,
    duty: Cell<u16>,
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
    // Note: Timer configuration is handled internally by Channel implementation
}

impl<'a> LedcBus<'a> {
    pub fn new(
        fan_channel: channel::Channel<'a, LowSpeed>,
        fan_number: channel::Number,
        ssr_channel: channel::Channel<'a, LowSpeed>,
        ssr_number: channel::Number,
    ) -> Self {
        Self {
            guard: LedcGuard::new(),
            fan: ChannelEntry::new(fan_channel, fan_number, "fan"),
            ssr: ChannelEntry::new(ssr_channel, ssr_number, "ssr"),
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

    /// # Panics
    ///
    /// The closure `f` must NOT call back into `LedcBus` (e.g., via
    /// `LedcChannelHandle::set_duty`). Doing so would trigger a `BorrowMutError`
    /// panic from the `RefCell`. On single-core Embassy this is safe as long
    /// as the closure does not `.await`.
    fn with_channel_mut<R, F>(&self, entry: &ChannelEntry<'a>, f: F) -> Result<R, LedcGuardError>
    where
        F: FnOnce(&mut channel::Channel<'a, LowSpeed>) -> R,
    {
        let guard = self.guard.try_acquire(entry.name)?;
        let mut channel_ref = entry.channel.borrow_mut();
        let result = f(&mut channel_ref);
        drop(channel_ref);
        drop(guard);
        Ok(result)
    }

    /// Read the LIVE duty from DUTY_R — the value the hardware is currently
    /// applying on the wire. Read-only register tracking the actual output
    /// duty (esp32c3 PAC: `ch(n).duty_r().read().duty_r().bits()`, 19 bits).
    ///
    /// Bug RHC-2 (2026-07-26): the fade consumer must use THIS register — the
    /// config DUTY register already holds the fade's END target mid-fade, so
    /// a fade restarted from the config register would jump to the old target
    /// (surge). DUTY_R reflects where the hardware actually is.
    ///
    /// Audit 2026-08-10 (C1): this is the WRONG register for verifying a
    /// freshly-written duty. A DUTY+DUTY_START+PARA_UP write does not take
    /// effect on DUTY_R until the next PWM period (200 ms at 5 Hz), so an
    /// immediate readback sees the previous duty and fails the tolerance
    /// check. Write verification must use `read_config_register` (DUTY),
    /// which `set_duty_hw` updates synchronously. Keep the two reads
    /// separate: `live_duty()` → DUTY_R, `read_duty_ticks()` → DUTY.
    fn read_live_register(&self, entry: &ChannelEntry<'a>) -> u16 {
        let regs = unsafe { &*LEDC::ptr() };
        let raw = regs
            .ch(entry.number as usize)
            .duty_r()
            .read()
            .duty_r()
            .bits();
        (raw >> 4) as u16
    }

    /// Read the CONFIG DUTY register — the last value written by
    /// `set_duty_hw` (synchronous, no PWM-period lag). This is the register
    /// a post-write verification must compare against: `set_duty_hw` writes
    /// DUTY before arming the update, so a mismatch here means the write
    /// itself failed, not that the new duty has not been applied to the wire
    /// yet (the DUTY_R lag case, bug C1 2026-08-10).
    fn read_config_register(&self, entry: &ChannelEntry<'a>) -> u16 {
        let regs = unsafe { &*LEDC::ptr() };
        let raw = regs.ch(entry.number as usize).duty().read().duty().bits();
        (raw >> 4) as u16
    }

    fn store_duty(&self, entry: &ChannelEntry<'a>, duty: u16) {
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
        match self
            .bus
            .with_channel_mut(entry, |channel| channel.set_duty(duty))
        {
            Ok(Ok(())) => {
                // Bug V2-14 (B10 latent): the cache MUST store *ticks* (the
                // unit used by `set_duty_raw` and `start_duty_fade`'s
                // end-state), not the percentage 0–100 esp-hal's
                // `Channel::set_duty` accepts. Storing `duty as u16` (a
                // percentage) here left the cache in mixed units — the very
                // class of bug B10 closed elsewhere. There are no production
                // callers today (the live SSR/fan paths go through the fade
                // and `set_duty_raw`), but the HIL examples (hil_fan /
                // hil_ssr / gpio_roast_test) exercise it directly, so the
                // latent trap reintroduces B10 the moment a future feature
                // adopts the per-channel direct path. Convert with the same
                // formula `start_duty_fade` uses so the cache stays unit-
                // consistent across all three write APIs.
                // Bug L7 (2026-08-10): scale over `duty_range()` = 2^bits —
                // the exact range esp-hal's `set_duty` uses
                // (`duty_range = 2u32.pow(duty_exp); duty_value =
                // (duty_range * duty_pct) / 100`, no rounding). The register
                // max is `2^bits - 1` (max_duty()), so cap at that to avoid
                // the 1/256 off-by-one on the 8-bit fan at 100%.
                let ticks = ((duty as u32 * self.duty_range()) / 100).min(self.max_duty()) as u16;
                self.bus.store_duty(entry, ticks);
                Ok(())
            }
            Ok(Err(err)) => Err(err),
            Err(err) => {
                warn!("SAFETY LEDC-GUARD timeout for {}", err.channel());
                Err(channel::Error::Channel)
            }
        }
    }

    /// Set duty as a raw value, bypassing [`ChannelIFace::set_duty`]'s
    /// percentage conversion. Use this when you have already computed a raw
    /// duty value via [`percentage_to_ledc_duty`].
    pub fn set_duty_raw(&self, duty: u16) -> Result<(), channel::Error> {
        let entry = self.entry();
        match self.bus.with_channel_mut(entry, |channel| {
            ChannelHW::set_duty_hw(channel, duty as u32);
        }) {
            Ok(_) => {
                self.bus.store_duty(entry, duty);
                Ok(())
            }
            Err(err) => {
                warn!("SAFETY LEDC-GUARD timeout for {}", err.channel());
                Err(channel::Error::Channel)
            }
        }
    }

    pub fn start_duty_fade(
        &self,
        start_duty: u8,
        end_duty: u8,
        duration_ms: u16,
    ) -> Result<(), channel::Error> {
        let entry = self.entry();
        match self.bus.with_channel_mut(entry, |channel| {
            channel.start_duty_fade(start_duty, end_duty, duration_ms)
        }) {
            Ok(Ok(())) => {
                // Bug B10: store the *ticks* matching the fade's end value
                // (not the percentage), so the duty cache keeps a single
                // unit. Previously `set_duty_raw` stored ticks but this path
                // stored `end_duty as u16` (a percentage 0–100), leaving
                // the cache with mixed units. Subsequent fade-vs-direct
                // decisions then compared ticks against percent (the 12-tick
                // threshold is 0.7 °C-equivalent of percent — random).
                // Bug L7 (2026-08-10): same 2^bits scale fix as `set_duty` —
                // the fade's end-state register holds `duty_range * pct / 100`.
                // Cap at max_duty() to avoid 1/256 off-by-one on 8-bit fan.
                let ticks =
                    ((end_duty as u32 * self.duty_range()) / 100).min(self.max_duty()) as u16;
                self.bus.store_duty(entry, ticks);
                Ok(())
            }
            Ok(Err(err)) => Err(err),
            Err(err) => {
                warn!("SAFETY LEDC-GUARD timeout for {}", err.channel());
                Err(channel::Error::Channel)
            }
        }
    }

    /// Maximum raw duty ticks for this channel's PWM resolution.
    ///
    /// Bug B10: the SSR channel runs at 14-bit resolution (16383 ticks) but
    /// the fan channel runs at 8-bit (255 ticks). `applied_percent()` and
    /// the fade-vs-direct decision both need this per-channel value; dividing
    /// a fan duty by the SSR resolution reported a 100% fan as ~1.6% and a
    /// later fade's percentage was treated as ticks, mis-computing `duty_delta`.
    pub fn max_duty(&self) -> u32 {
        match self.role {
            ChannelRole::Fan => (1u32 << crate::config::constants::FAN_PWM_RESOLUTION) - 1,
            ChannelRole::Ssr => (1u32 << crate::config::constants::SSR_PWM_RESOLUTION) - 1,
        }
    }

    /// The duty RANGE esp-hal's `ChannelIFace::set_duty` scales percentages
    /// over: `2^bits` (not `2^bits − 1`). `max_duty()` reports the largest
    /// representable tick for display; `duty_range()` is the divisor used by
    /// the hardware when converting a percentage — Bug L7 (2026-08-10).
    fn duty_range(&self) -> u32 {
        match self.role {
            ChannelRole::Fan => 1u32 << crate::config::constants::FAN_PWM_RESOLUTION,
            ChannelRole::Ssr => 1u32 << crate::config::constants::SSR_PWM_RESOLUTION,
        }
    }

    pub fn applied_duty(&self) -> u16 {
        self.entry().duty.get()
    }

    /// Bug DRH-1 (2026-07-26): read the LIVE wire duty (DUTY_R register)
    /// instead of the cached config duty. If a previous fade is still
    /// mid-flight, the cache holds that fade's END target — restarting a
    /// fade from the cache would jump the fan to the old target before
    /// ramping to the new one (surge). DUTY_R reflects the actual output,
    /// so a fade restarted mid-fade continues from where the hardware is.
    pub fn live_duty(&self) -> u16 {
        self.bus.read_live_register(self.entry())
    }

    pub fn applied_percent(&self) -> f32 {
        // Bug B10: divide by THIS channel's resolution, not always the SSR's.
        (self.applied_duty() as f32) * 100.0 / self.max_duty() as f32
    }
}

impl<'a> LedcDutyReader for LedcChannelHandle<'a> {
    /// Read the CONFIG DUTY register (synchronous with the last write).
    ///
    /// Bug C1 (2026-08-10): this used to read DUTY_R (the applied/live duty),
    /// which lags a fresh write by up to one PWM period (200 ms at 5 Hz SSR).
    /// `monitor_ledc_after_set` verifies a write microseconds after issuing
    /// it, so a correct write was misread as the PREVIOUS duty and escalated
    /// to `emergency_shutdown("Heater control failure")`. `set_duty_hw`
    /// updates DUTY synchronously, so the config register is the correct
    /// readback target for write verification. Consumers that need the wire
    /// value (fan fade restart) use `live_duty()` (DUTY_R) instead.
    fn read_duty_ticks(&self) -> u16 {
        self.bus.read_config_register(self.entry())
    }

    fn set_duty_raw(&self, duty: u16) -> Result<(), DutyWriteError> {
        LedcChannelHandle::set_duty_raw(self, duty).map_err(|_| DutyWriteError)
    }
}

impl<'a> ChannelIFace<'a, LowSpeed> for LedcChannelHandle<'a> {
    fn configure(
        &mut self,
        config: channel::config::Config<'a, LowSpeed>,
    ) -> Result<(), channel::Error> {
        let entry = self.entry();
        match self
            .bus
            .with_channel_mut(entry, |channel| channel.configure(config))
        {
            Ok(result) => result,
            Err(err) => {
                warn!("SAFETY LEDC-GUARD timeout for {}", err.channel());
                Err(channel::Error::Channel)
            }
        }
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
        match self
            .bus
            .with_channel_mut(entry, |channel| channel.is_duty_fade_running())
        {
            Ok(value) => value,
            Err(err) => {
                warn!("SAFETY LEDC-GUARD timeout for {}", err.channel());
                false
            }
        }
    }
}

unsafe impl<'a> Send for LedcBus<'a> {}
unsafe impl<'a> Send for LedcChannelHandle<'a> {}
