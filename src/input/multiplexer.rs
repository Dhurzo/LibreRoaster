// Bug L9 (2026-07-25): the `Instant` mock used to be gated to
// `#[cfg(not(target_arch = "riscv32"))]`, which kept it active on EVERY host
// build — including non-test host builds where `CommandMultiplexer`'s
// 60 s idle-failover is supposed to be a real safety feature (the host-side
// simulation runs the multiplexer with the same Embassy executor that feeds
// the device build, backed by `HostTimeDriver`). The mock returned
// `Instant(u64::MAX - 1)` for every `now()` and `Duration::from_secs(0)` for
// every `duration_since`, so on a host build the failover fired instantly
// (or never, depending on the branch) regardless of wall-clock reality.
//
// Gate the mock to `#[cfg(all(not(target_arch = "riscv32"), test))]` so it
// only applies to the unit tests; everywhere else (device build AND host
// non-test build) we use the real `embassy_time::Instant`.
#[cfg(any(target_arch = "riscv32", not(feature = "test")))]
use embassy_time::Instant;

#[cfg(all(not(target_arch = "riscv32"), feature = "test"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Instant(u64);

#[cfg(all(not(target_arch = "riscv32"), feature = "test"))]
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

// NOTE: Handshake (CHAN → UNITS → FILT) is DISABLED for Artisan Scope compatibility.
// Artisan Scope does not perform handshake — it sends commands immediately.
// The `# ` acknowledgment documented in README is therefore not sent.
// If re-enabling, restore init_state.rs and uncomment init flow.

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

    pub fn on_command_received(&mut self, channel: CommChannel) -> bool {
        let now = Instant::now();

        if let Some(last_time) = self.last_command_time {
            let elapsed = now.duration_since(last_time);
            if elapsed.as_secs() >= IDLE_TIMEOUT_SECS {
                self.active_channel = CommChannel::None;
                self.last_command_time = None;
                log::info!(
                    "No artisan commands for {}s, switching active channel to None",
                    IDLE_TIMEOUT_SECS
                );
            }
        }

        match self.active_channel {
            CommChannel::None => {
                // Audit MP-2 (2026-08-11): boot-time hijack risk, ACCEPTED
                // and documented. The first syntactically-valid command on
                // EITHER wire claims the session (e.g. UART line noise that
                // assembles `READ\r` before Artisan connects over USB takes
                // the session until the 60 s idle reset). The audit's
                // suggested fixes were evaluated and REJECTED as regressions:
                //   - "N consecutive valid commands to activate": drops
                //     Artisan's very first `READ` (it sends one command, not
                //     a burst), breaking every session start.
                //   - "USB always starts active": breaks UART-only
                //     deployments, where Artisan runs on UART0.
                //   - boot grace timers: same UART-only breakage inside the
                //     grace window.
                // Existing mitigations: P8 (only VALID commands reach this
                // match — parse garbage is dropped before the gate; the
                // MP-1 pre-parse gate now also prevents parser-side FIFO
                // side effects on refused lines), and the 60 s idle reset
                // restoring `None`. A hijacked session recovers on the next
                // idle window; no session is ever processed on the wrong
                // transport (the gate is authoritative).
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

    /// Audit MP-1 (2026-08-11): pure predicate — would a command on `channel`
    /// be accepted, WITHOUT activating the session? Unlike
    /// `should_process_command` / `on_command_received` (which activate from
    /// `None`), this NEVER mutates state. Transport layers use it to skip
    /// PARSING — and with it the parser-side PROFILE/FANPROFILE FIFO side
    /// effects (F3-MP1: a dropped/refused command used to leak its profile
    /// into the FIFO where a later session's `SetProfile` consumed it) — for
    /// lines the multiplexer gate would refuse anyway. Activation stays
    /// reserved for successfully parsed commands (P8), so a garbage line can
    /// never claim the session through this path.
    pub fn would_process_command(&self, channel: CommChannel) -> bool {
        if self.is_idle() {
            // An idle mux behaves as if `active_channel` were `None`: the
            // next `on_command_received` call resets it (IDLE_TIMEOUT_SECS
            // branch) and accepts the first command on any channel.
            true
        } else {
            matches!(self.active_channel, CommChannel::None) || self.active_channel == channel
        }
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
    fn test_commands_work_without_handshake() {
        let mut mux = CommandMultiplexer::new();

        let activated = mux.should_process_command(CommChannel::Usb);
        assert!(activated, "First command should activate USB channel");

        assert!(mux.should_process_command(CommChannel::Usb));
    }
}
