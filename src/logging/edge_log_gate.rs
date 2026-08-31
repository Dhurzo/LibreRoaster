//! Edge-triggered log gating for persistent conditions.
//!
//! BUG-04 (2026-08-21): several safety interlocks evaluate a PERSISTENT
//! condition every control tick (~330 ms) and used to emit a `warn!` on
//! every evaluation — ≈3 lines/s of log traffic injected into the same
//! physical channel that carries the Artisan protocol (esp-println with the
//! `auto` feature writes to USB-Serial-JTAG when a host is attached, else
//! UART0 — both shared with the protocol wire). Interleaved log lines
//! corrupt READ/STATUS responses. `EdgeLogGate` reduces the emission to one
//! `warn!` per rising edge (the condition becoming active), with the
//! remaining ticks downgraded to `debug!`, and re-arms itself when the
//! condition clears.
//!
//! The structural fix (a dedicated log sink on a spare UART) is tracked
//! separately; this gate closes the per-tick corruption class for the known
//! high-frequency warn sites.

/// Rising-edge detector for a boolean condition.
#[derive(Debug, Clone, Copy, Default)]
pub struct EdgeLogGate {
    warned: bool,
}

impl EdgeLogGate {
    /// Create a gate in the idle (not-yet-warned) state.
    pub fn new() -> Self {
        Self { warned: false }
    }

    /// Feed the current condition state. Returns `true` exactly once per
    /// activation episode: on the inactive → active transition. While the
    /// condition stays active the gate returns `false` (log at `debug!`
    /// level instead), and it re-arms itself as soon as the condition goes
    /// inactive.
    pub fn rising(&mut self, active: bool) -> bool {
        if active {
            if self.warned {
                false
            } else {
                self.warned = true;
                true
            }
        } else {
            self.warned = false;
            false
        }
    }
}

#[cfg(all(test, not(target_arch = "riscv32")))]
mod tests {
    use super::*;

    #[test]
    fn fires_once_per_activation_episode() {
        let mut gate = EdgeLogGate::new();
        assert!(gate.rising(true), "first active sample is the rising edge");
        for _ in 0..10 {
            assert!(!gate.rising(true), "persistent condition must not re-fire");
        }
    }

    #[test]
    fn re_arms_on_falling_edge() {
        let mut gate = EdgeLogGate::new();
        assert!(gate.rising(true));
        assert!(!gate.rising(false), "falling edge must not fire");
        assert!(gate.rising(true), "next episode fires again");
    }

    #[test]
    fn starts_silent() {
        let mut gate = EdgeLogGate::new();
        assert!(!gate.rising(false));
        assert!(!gate.rising(false));
    }

    #[test]
    fn alternate_edges_fire_each_time() {
        let mut gate = EdgeLogGate::new();
        let mut fires = 0;
        for i in 0..20 {
            if gate.rising(i % 2 == 0) {
                fires += 1;
            }
        }
        assert_eq!(fires, 10, "every activation episode must fire exactly once");
    }
}
