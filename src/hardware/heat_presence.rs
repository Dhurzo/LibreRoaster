//! Debounce logic for the SSR heat-source detection pin.
//!
//! Kept in its own un-gated module (compiled on BOTH host and embedded) so
//! the decision logic is covered by host unit tests — the hardware modules
//! (`hardware::ssr`) are stubbed on host, which is how the previous
//! single-sample flip survived CI.
//!
//! # Why the debounce is needed
//!
//! The SSR is a 5 Hz LEDC PWM (200 ms period). At duty ≥ 50 % the ON window
//! is ≥ 100 ms, so two samples whose PWM phases are separated by > 100 ms
//! cannot both land in the OFF window: at least one must read "heat".
//! Consecutive detects are one control-loop tick apart (~330 ms on embedded
//! → ~130 ms of phase separation, always ∈ (100, 200) ms while the tick
//! stays below 400 ms), so a functioning SSR can produce runs of AT MOST 2
//! consecutive "no heat" samples regardless of phase drift or jitter.
//!
//! Bug audit 2026-08-02: the previous one-sample flip latched `NotDetected`
//! mid-roast whenever a single sample landed in the PWM OFF window (phase
//! drift between the ~1 s detect cadence and the 200 ms PWM makes this
//! inevitable at duty 50–90 %). Because `NotDetected` forces the heater to
//! 0 % and duty 0 falls below the observability gate, the heater dead-locked
//! until power cycle.

/// Number of consecutive "no heat" samples (each with duty ≥ 50 %) required
/// before the caller transitions to `NotDetected`.
///
/// `HEAT_ABSENT_DEBOUNCE = 5` leaves a margin of 3 samples over the run
/// bound of 2 (≈ 1.7 s of sustained "no heat" at ≥ 50 % duty before the
/// heater is blocked), while a genuinely dead SSR — or a build without the
/// optional current-sense circuit — latches `NotDetected` within ~2 s.
pub const HEAT_ABSENT_DEBOUNCE: u8 = 5;

/// Outcome of feeding one detection-pin sample to `debounce_heat_absent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeatPresenceOutcome {
    /// Sample was informative and consistent — no status transition.
    NoChange,
    /// A LOW sample (current flowing) — trustworthy evidence of heat; the
    /// caller should transition to `Available` immediately.
    HeatDetected,
    /// The debounce threshold of consecutive "no heat" samples was reached;
    /// the caller should transition to `NotDetected`.
    HeatAbsent,
}

/// One-sample debounce state transition for the heat-source detection pin.
///
/// * `absent_count` — consecutive "no heat" samples seen so far.
/// * `pin_detects_heat` — the raw pin sample (true = LOW = current flowing).
/// * `duty_observable` — whether the commanded duty is ≥ 50 % (below it the
///   pin is uninformative: the PWM OFF window can exceed the sample interval
///   and a HIGH sample says nothing about the SSR).
///
/// Returns the new count plus the outcome. A "heat" sample is trusted
/// immediately (`HeatDetected`, counter reset): LOW means current is flowing
/// right now and cannot be produced by the PWM OFF window. A "no heat"
/// sample is ambiguous, so it only accumulates toward `HeatAbsent`. When the
/// duty is not observable the counter is reset — a low-power stretch must
/// never accumulate toward a false `NotDetected` across unrelated stretches
/// (e.g. PID output oscillating around 50 %).
pub fn debounce_heat_absent(
    absent_count: u8,
    pin_detects_heat: bool,
    duty_observable: bool,
) -> (u8, HeatPresenceOutcome) {
    if !duty_observable {
        return (0, HeatPresenceOutcome::NoChange);
    }
    if pin_detects_heat {
        return (0, HeatPresenceOutcome::HeatDetected);
    }
    let new_count = absent_count.saturating_add(1);
    if new_count >= HEAT_ABSENT_DEBOUNCE {
        (0, HeatPresenceOutcome::HeatAbsent)
    } else {
        (new_count, HeatPresenceOutcome::NoChange)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_off_sample_does_not_transition() {
        let (count, outcome) = debounce_heat_absent(0, false, true);
        assert_eq!(count, 1);
        assert_eq!(outcome, HeatPresenceOutcome::NoChange);
    }

    #[test]
    fn off_samples_accumulate_until_threshold() {
        let mut count = 0;
        for i in 0..HEAT_ABSENT_DEBOUNCE - 1 {
            let (new_count, outcome) = debounce_heat_absent(count, false, true);
            assert_eq!(
                outcome,
                HeatPresenceOutcome::NoChange,
                "sample {} must not transition yet",
                i + 1
            );
            count = new_count;
        }
        let (new_count, outcome) = debounce_heat_absent(count, false, true);
        assert_eq!(outcome, HeatPresenceOutcome::HeatAbsent);
        assert_eq!(new_count, 0, "counter resets on the transition");
    }

    #[test]
    fn heat_sample_resets_counter_and_signals_detection() {
        // Two accumulated absent samples…
        let (count, _) = debounce_heat_absent(2, false, true);
        // …then a LOW sample (current flowing) resets and signals heat.
        let (new_count, outcome) = debounce_heat_absent(count, true, true);
        assert_eq!(new_count, 0);
        assert_eq!(outcome, HeatPresenceOutcome::HeatDetected);
    }

    #[test]
    fn unobservable_duty_resets_accumulation() {
        let (count, _) = debounce_heat_absent(3, false, true);
        let (new_count, outcome) = debounce_heat_absent(count, false, false);
        assert_eq!(new_count, 0);
        assert_eq!(outcome, HeatPresenceOutcome::NoChange);
    }

    #[test]
    fn unobservable_duty_never_accumulates_from_zero() {
        let mut count = 0;
        for _ in 0..HEAT_ABSENT_DEBOUNCE * 2 {
            let (new_count, outcome) = debounce_heat_absent(count, false, false);
            assert_eq!(outcome, HeatPresenceOutcome::NoChange);
            assert_eq!(new_count, 0);
            count = new_count;
        }
    }

    #[test]
    fn counter_is_saturating() {
        // A corrupt caller state (count above threshold) still flips exactly
        // once and resets — never panics, never double-fires.
        let (new_count, outcome) = debounce_heat_absent(200, false, true);
        assert_eq!(outcome, HeatPresenceOutcome::HeatAbsent);
        assert_eq!(new_count, 0);
    }
}
