//! Status LED pattern logic (pure, host-testable).
//!
//! BUG-06 (2026-08-21): the status LED handle was created in
//! `init_hardware` and never consumed, while `enter_safe_shutdown` created a
//! second `Output` on the same GPIO8 via `Peripherals::steal()` — two live,
//! aliased handles in violation of the esp-hal ownership model, and the only
//! local indicator of a screen-less device did nothing during normal
//! operation.
//!
//! The GPIO write lives in the control-loop task (embedded-only); the
//! decision logic lives here, un-gated, so the pattern table is pinned by
//! host unit tests.

use crate::config::constants::RoasterState;

/// What the LED should be doing right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedPattern {
    /// LED off.
    Off,
    /// LED continuously on.
    Solid,
    /// 1 Hz blink (500 ms on, 500 ms off).
    Blink1Hz,
    /// 4 Hz blink (125 ms on, 125 ms off) — attention/error.
    Blink4Hz,
}

/// Map the roast state + fault flag to an LED pattern.
///
/// Fault/emergency overrides everything (the operator must see that the
/// device is in a safety latch even if the state register were stale).
pub fn pattern_for(state: RoasterState, fault: bool) -> LedPattern {
    if fault {
        return LedPattern::Blink4Hz;
    }
    match state {
        RoasterState::Idle => LedPattern::Off,
        RoasterState::Preheating => LedPattern::Blink1Hz,
        RoasterState::Heating | RoasterState::Stable => LedPattern::Solid,
        RoasterState::Error => LedPattern::Blink4Hz,
    }
}

/// Compute whether the LED is ON at `elapsed_ms` since an epoch, given the
/// pattern. Blink patterns are phase-locked to the epoch (no per-task toggle
/// state needed): ON during the first half of each period.
pub fn led_on(pattern: LedPattern, elapsed_ms: u64) -> bool {
    match pattern {
        LedPattern::Off => false,
        LedPattern::Solid => true,
        LedPattern::Blink1Hz => (elapsed_ms % 1000) < 500,
        LedPattern::Blink4Hz => (elapsed_ms % 250) < 125,
    }
}

#[cfg(all(test, not(target_arch = "riscv32")))]
mod tests {
    use super::*;

    #[test]
    fn idle_is_off() {
        assert_eq!(pattern_for(RoasterState::Idle, false), LedPattern::Off);
        assert!(!led_on(LedPattern::Off, 0));
        assert!(!led_on(LedPattern::Off, 999_999));
    }

    #[test]
    fn preheating_blinks_at_1hz() {
        assert_eq!(
            pattern_for(RoasterState::Preheating, false),
            LedPattern::Blink1Hz
        );
        assert!(led_on(LedPattern::Blink1Hz, 0));
        assert!(led_on(LedPattern::Blink1Hz, 499));
        assert!(!led_on(LedPattern::Blink1Hz, 500));
        assert!(!led_on(LedPattern::Blink1Hz, 999));
        assert!(led_on(LedPattern::Blink1Hz, 1000));
    }

    #[test]
    fn heating_and_stable_are_solid() {
        assert_eq!(pattern_for(RoasterState::Heating, false), LedPattern::Solid);
        assert_eq!(pattern_for(RoasterState::Stable, false), LedPattern::Solid);
        assert!(led_on(LedPattern::Solid, 0));
        assert!(led_on(LedPattern::Solid, 12_345_678));
    }

    #[test]
    fn error_blinks_at_4hz() {
        assert_eq!(
            pattern_for(RoasterState::Error, false),
            LedPattern::Blink4Hz
        );
        assert!(led_on(LedPattern::Blink4Hz, 0));
        assert!(led_on(LedPattern::Blink4Hz, 124));
        assert!(!led_on(LedPattern::Blink4Hz, 125));
        assert!(led_on(LedPattern::Blink4Hz, 250));
    }

    #[test]
    fn fault_overrides_every_state() {
        for state in [
            RoasterState::Idle,
            RoasterState::Preheating,
            RoasterState::Heating,
            RoasterState::Stable,
            RoasterState::Error,
        ] {
            assert_eq!(
                pattern_for(state, true),
                LedPattern::Blink4Hz,
                "fault must override {:?}",
                state
            );
        }
    }
}
