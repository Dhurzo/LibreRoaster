#[allow(unused_imports)]
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

// NOTE: Handshake (CHAN → UNITS → FILT) is DISABLED for Artisan Scope compatibility
// Artisan Scope does not perform handshake - it simply sends and receives data
// Placeholder types kept for potential future re-enabling
#[allow(dead_code)]
pub struct InitState;

#[allow(dead_code)]
pub struct InitEvent;

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
}

impl CommandMultiplexer {
    pub fn new() -> Self {
        Self {
            active_channel: CommChannel::None,
            last_command_time: None,
        }
    }

    /// NOTE: Handshake is disabled - Artisan Scope does not require initialization
    /// Placeholder kept for backwards compatibility
    #[allow(dead_code)]
    pub fn init_state(&self) -> InitState {
        InitState
    }

    /// NOTE: Always returns true since handshake is disabled
    #[allow(dead_code)]
    pub fn is_init_complete(&self) -> bool {
        true
    }

    /// NOTE: Handshake values not used (returns None)
    #[allow(dead_code)]
    pub fn chan_value(&self) -> Option<u16> {
        None
    }

    /// NOTE: Handshake values not used (returns None)
    #[allow(dead_code)]
    pub fn units_value(&self) -> Option<bool> {
        None
    }

    /// NOTE: Handshake values not used (returns None)
    #[allow(dead_code)]
    pub fn filt_value(&self) -> Option<u8> {
        None
    }

    /// NOTE: Handshake is disabled - commands are processed immediately
    #[allow(dead_code)]
    pub fn on_init_command(
        &mut self,
        _command: crate::config::ArtisanCommand,
    ) -> Result<InitEvent, crate::input::parser::ParseError> {
        Ok(InitEvent)
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
    }
}

impl Default for CommandMultiplexer {
    fn default() -> Self {
        Self::new()
    }
}

// NOTE: Handshake tests are disabled because Artisan Scope does not use handshake
// The following tests verify that Artisan Scope compatible mode works correctly
// (commands are processed immediately without initialization sequence)
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

    #[test]
    fn test_handshake_always_complete() {
        // Verify that handshake is disabled and system is always "ready"
        let mux = CommandMultiplexer::new();
        assert!(mux.is_init_complete(), "Handshake disabled - always ready");
        assert!(mux.chan_value().is_none(), "No channel value stored");
        assert!(mux.units_value().is_none(), "No units value stored");
        assert!(mux.filt_value().is_none(), "No filter value stored");
    }

    #[test]
    fn test_commands_work_without_handshake() {
        // Verify all Artisan commands work without handshake
        let mut mux = CommandMultiplexer::new();

        // First command should activate channel
        let activated = mux.should_process_command(CommChannel::Usb);
        assert!(activated, "First command should activate USB channel");

        // Commands on same channel should work
        assert!(mux.should_process_command(CommChannel::Usb));
    }
}
