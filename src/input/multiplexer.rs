use log::info;

#[cfg(target_arch = "riscv32")]
use embassy_time::Instant;

#[cfg(not(target_arch = "riscv32"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Instant(u64);

#[cfg(not(target_arch = "riscv32"))]
impl Instant {
    pub fn now() -> Self {
        Self(u64::MAX - 1) // Mock time for testing
    }

    pub fn duration_since(self, _other: Instant) -> core::time::Duration {
        core::time::Duration::from_secs(0)
    }

    pub fn as_secs(&self) -> u64 {
        self.0
    }
}

use crate::input::init_state::{ArtisanInitState, InitEvent, InitState};

pub const IDLE_TIMEOUT_SECS: u64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommChannel {
    None,
    Usb,
    Uart,
}

pub struct CommandMultiplexer {
    active_channel: CommChannel,
    last_command_time: Option<Instant>,
    init_state: ArtisanInitState,
}

impl CommandMultiplexer {
    pub fn new() -> Self {
        Self {
            active_channel: CommChannel::None,
            last_command_time: None,
            init_state: ArtisanInitState::new(),
        }
    }

    /// Get the current initialization state
    pub fn init_state(&self) -> InitState {
        self.init_state.state()
    }

    /// Check if initialization handshake is complete
    pub fn is_init_complete(&self) -> bool {
        self.init_state.is_ready()
    }

    /// Get the configured channel value
    pub fn chan_value(&self) -> Option<u16> {
        self.init_state.chan_value()
    }

    /// Get the configured units value
    pub fn units_value(&self) -> Option<bool> {
        self.init_state.units_value()
    }

    /// Get the configured filter value
    pub fn filt_value(&self) -> Option<u8> {
        self.init_state.filt_value()
    }

    /// Process an initialization command through the state machine
    pub fn on_init_command(
        &mut self,
        command: crate::config::ArtisanCommand,
    ) -> Result<InitEvent, crate::input::parser::ParseError> {
        self.init_state.on_command(command)
    }

    pub fn on_command_received(&mut self, channel: CommChannel) -> bool {
        let now = Instant::now();

        match self.active_channel {
            CommChannel::None => {
                self.active_channel = channel;
                self.last_command_time = Some(now);
                log::info!(
                    "Artisan command received on {:?}, switching active channel to {:?}",
                    channel,
                    channel
                );
                true
            }
            current if current == channel => {
                if let Some(last_time) = self.last_command_time {
                    let elapsed = now.duration_since(last_time);
                    if elapsed.as_secs() >= IDLE_TIMEOUT_SECS {
                        self.active_channel = channel;
                        self.last_command_time = Some(now);
                        self.init_state.reset();
                        log::info!(
                            "Idle timeout expired, artisan command received on {:?}, switching active channel to {:?}",
                            channel,
                            channel
                        );
                        return true;
                    }
                }
                self.last_command_time = Some(now);
                true
            }
            _ => {
                log::info!(
                    "Ignoring artisan command on {:?}, active channel is {:?}",
                    channel,
                    self.active_channel
                );
                false
            }
        }
    }

    pub fn should_process_command(&mut self, channel: CommChannel) -> bool {
        self.on_command_received(channel)
    }

    pub fn should_write_to(&self, channel: CommChannel) -> bool {
        self.active_channel == channel
    }

    pub fn get_active_channel(&self) -> CommChannel {
        self.active_channel
    }

    pub fn is_idle(&self) -> bool {
        if let Some(last_time) = self.last_command_time {
            let elapsed = Instant::now().duration_since(last_time);
            elapsed.as_secs() >= IDLE_TIMEOUT_SECS
        } else {
            true
        }
    }

    pub fn reset(&mut self) {
        if self.active_channel != CommChannel::None {
            log::info!(
                "No artisan commands for {}s, switching active channel to None",
                IDLE_TIMEOUT_SECS
            );
        }
        self.active_channel = CommChannel::None;
        self.last_command_time = None;
        self.init_state.reset();
    }
}

impl Default for CommandMultiplexer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_multiplexer_starts_in_none() {
        let mux = CommandMultiplexer::new();
        assert_eq!(mux.get_active_channel(), CommChannel::None);
        assert!(mux.is_idle(), "New multiplexer should be idle");
    }

    #[test]
    fn test_channel_activation_usb() {
        let mut mux = CommandMultiplexer::new();
        assert_eq!(mux.get_active_channel(), CommChannel::None);

        let activated = mux.on_command_received(CommChannel::Usb);
        assert!(activated, "First command should activate channel");
        assert_eq!(mux.get_active_channel(), CommChannel::Usb);
        assert!(!mux.is_idle(), "Channel should not be idle after command");
    }

    #[test]
    fn test_channel_activation_uart() {
        let mut mux = CommandMultiplexer::new();
        let activated = mux.on_command_received(CommChannel::Uart);
        assert!(activated);
        assert_eq!(mux.get_active_channel(), CommChannel::Uart);
    }

    #[test]
    fn test_ignore_inactive_channel() {
        let mut mux = CommandMultiplexer::new();
        mux.on_command_received(CommChannel::Usb);

        let ignored = mux.on_command_received(CommChannel::Uart);
        assert!(!ignored, "Command on inactive channel should be ignored");
        assert_eq!(mux.get_active_channel(), CommChannel::Usb);
    }

    #[test]
    fn test_same_channel_allowed() {
        let mut mux = CommandMultiplexer::new();
        mux.on_command_received(CommChannel::Usb);

        let allowed = mux.on_command_received(CommChannel::Usb);
        assert!(allowed, "Same channel commands should be allowed");
    }
}
