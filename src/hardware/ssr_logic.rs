//! Pure decision logic for SSR heat-source detection and availability.
//!
//! The physical pin interpretation is OPT-IN via the `heat-sense` cargo
//! feature (H4): without it (default build, or under `simulated-sensors`)
//! both methods are no-ops that keep `Available`, so boards without the
//! GPIO1 current-sense circuit heat normally. Enable `heat-sense` only with
//! a validated stretched-pulse circuit (see `docs/HARDWARE.md` §8).
//!
//! Kept in its own un-gated module (compiled on BOTH host and embedded) so
//! the decision logic is covered by host unit tests (run with
//! `--features heat-sense`).

use crate::config::constants::{SSR_MIN_DUTY_TICKS, SSR_PWM_RESOLUTION};
#[cfg(all(feature = "heat-sense", not(feature = "simulated-sensors")))]
use crate::hardware::heat_presence::{debounce_heat_absent, HeatPresenceOutcome};
use log::info;
#[cfg(all(feature = "heat-sense", not(feature = "simulated-sensors")))]
use log::{error, warn};

/// Error returned by SSR control operations.
///
/// Re-exported from `hardware::ssr` so existing callers (including the host
/// `ssr_stub`, which keeps its own copy) are unaffected.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SsrError {
    /// GPIO write (SSR pin) failed.
    OutputError { source: &'static str },
    /// GPIO read (detection pin) failed.
    InputError { source: &'static str },
    /// Heat source not detected despite commanded duty.
    HeatSourceNotDetected { source: &'static str },
    /// LEDC PWM write or duty verification failed.
    PwmError { source: &'static str },
}

impl embedded_hal::digital::Error for SsrError {
    fn kind(&self) -> embedded_hal::digital::ErrorKind {
        embedded_hal::digital::ErrorKind::Other
    }
}

/// Heat-source hardware availability state for an SSR channel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SsrHardwareStatus {
    /// Heat source present and responding to commanded duty.
    Available,
    /// Heater commanded but no heat detected (debounced).
    NotDetected,
    /// Detection pin error or stuck-on cross-check tripped.
    Error,
}

/// Number of consecutive "heater ON but no heat detected" samples (at
/// duty ≥ 50 %) before `hardware_status` latches to `Error`.
///
/// Legacy count-based trip, kept for the per-sample warn counter. The actual
/// latch decision is time-based (`HEAT_MISMATCH_WINDOW_MS`): with one sample
/// per control tick, counting consecutive samples of the SAME instant (the
/// old write-path + periodic double-sample, H4) tripped an ideal square-wave
/// signal mid-roast. Time debouncing is alias-proof.
#[allow(dead_code)]
const HEAT_MISMATCH_MAX: u8 = 5;
/// `heat_mismatch_count`/`heat_present_count` thresholds are sampled every
/// control-loop tick (~330 ms on embedded: 210 ms MAX31856 wait + 100 ms
/// timer + overhead). `HEAT_MISMATCH_MAX = 5` therefore represents ≈ 1.7 s
/// ≈ 8.5 full PWM cycles at 5 Hz, which filters out the ~70 % of the cycle
/// that legitimately reads "no heat" at duty = 30 %. `HEAT_PRESENT_MISMATCH_MAX
/// = 10` (≈ 3.3 s) tolerates the residual heat of the metal mass and only fires
/// when the SSR is genuinely stuck on.
#[allow(dead_code)]
const HEAT_PRESENT_MISMATCH_MAX: u8 = 10;

/// Time-debounce window (H4): with duty observable (≥ 50 %) the cross-check
/// only latches `Error` when NO heat sample (LOW) has been seen for longer
/// than this window. ≈ 7 PWM periods at 5 Hz — a stretched-pulse circuit
/// holds LOW continuously while conducting, while an unstretched AC
/// optocoupler still shows long HIGH stretches and trips (correctly: that
/// hardware cannot distinguish conduction, see `docs/HARDWARE.md` §8).
pub const HEAT_MISMATCH_WINDOW_MS: u32 = 1_500;

/// Percentage the SSR really delivers after the min-duty snap-to-zero (H7).
///
/// Mirrors `ssr::percentage_to_ledc_duty` (which is riscv-only): any
/// positive request below one AC half-cycle (`SSR_MIN_DUTY_TICKS`) lands
/// zero ticks on the LEDC. Telemetry and safety gates must use this
/// effective value, not the requested one — otherwise `OT1 1..5` arms the
/// fan floor, comms-idle, roast-time and probe-stuck supervision with the
/// heater physically off.
pub fn effective_percentage(requested: f32) -> f32 {
    let max = ((1u32 << SSR_PWM_RESOLUTION) - 1) as f32;
    let ticks = ((requested.clamp(0.0, 100.0) / 100.0) * max + 0.5) as u32;
    if ticks > 0 && ticks < SSR_MIN_DUTY_TICKS as u32 {
        0.0
    } else {
        requested.clamp(0.0, 100.0)
    }
}

/// Common status queries implemented by SSR control types.
pub trait StatusGetters {
    /// Return the current `SsrHardwareStatus`.
    fn get_hardware_status(&self) -> SsrHardwareStatus;
    /// True when the heat source is `Available`.
    fn is_heating_available(&self) -> bool;
    /// Return the last commanded raw duty in LEDC ticks.
    fn get_current_duty(&self) -> u16;
    /// True when the PWM output is enabled.
    fn is_pwm_enabled(&self) -> bool;
    /// Last measured duty delta (commanded vs readback) in ticks.
    fn last_lead_delta_ticks(&self) -> i16;
    /// Number of duty-retry attempts on the last set.
    fn last_retry_count(&self) -> u8;
}

/// Common state for SSR control implementations.
/// Embedded by both SsrControl and SsrControlSimple to eliminate code duplication.
pub struct SsrControlBase {
    pub(crate) hardware_status: SsrHardwareStatus,
    pub(crate) current_duty: u16,
    pub(crate) last_duty_delta_ticks: i16,
    pub(crate) retry_count: u8,
    pub(crate) is_pwm_enabled: bool,
    /// Consecutive `detect_heat_source` samples that read "no heat" while the
    /// duty is ≥ 50 %. Managed by `heat_presence::debounce_heat_absent`;
    /// only when it reaches `HEAT_ABSENT_DEBOUNCE` does the status flip to
    /// `NotDetected`.
    ///
    /// A single OFF sample is ambiguous (the PWM OFF window at duty ≥ 50 %
    /// reads HIGH even when the SSR is conducting) — a single-sample flip
    /// would latch `NotDetected` mid-roast, which forces the heater to 0 %
    /// and (because duty 0 falls below the observability gate) dead-locks
    /// the heater until power cycle.
    ///
    /// Only interpreted with the `heat-sense` feature; otherwise written by
    /// `new`/`rearm` but never read.
    #[allow(dead_code)]
    heat_absent_count: u8,
    #[allow(dead_code)]
    heat_mismatch_count: u8,
    /// Timestamp (ms) of the last heat-detected (LOW) cross-check sample.
    /// `None` until the first observable sample; the time-debounce window
    /// (`HEAT_MISMATCH_WINDOW_MS`) is measured from here. Reset by `rearm()`.
    ///
    /// Only interpreted with the `heat-sense` feature; otherwise written by
    /// `new`/`rearm` but never read.
    #[allow(dead_code)]
    last_heat_seen_ms: Option<u32>,
    /// Debounce counter for the "heat present while heater off" branch.
    /// The SSR is PWM at 5 Hz; with a metal heat mass, residual heat can keep
    /// the sensor reading hot long after the duty drops to zero. We require
    /// `HEAT_PRESENT_MISMATCH_MAX` consecutive mismatched samples before
    /// declaring the SSR stuck on, so a single transient does not trip the
    /// safety interlock mid-roast.
    #[allow(dead_code)]
    heat_present_count: u8,
}

impl SsrControlBase {
    pub fn new() -> Self {
        // Boot-time status is `Available`: at duty 0 the SSR does not conduct
        // and the documentation-defined wiring (pin pulled HIGH when SSR off,
        // LOW when SSR conducts) makes a single sample at boot uninformative.
        // Treating the heater as available at boot is necessary so manual and
        // PID commands are not silently masked out by a false NotDetected latch
        // (0% duty → pin HIGH → NotDetected → output forced to 0 → 0% duty forever).
        SsrControlBase {
            hardware_status: SsrHardwareStatus::Available,
            current_duty: 0,
            last_duty_delta_ticks: 0,
            retry_count: 0,
            is_pwm_enabled: true,
            heat_absent_count: 0,
            heat_mismatch_count: 0,
            heat_present_count: 0,
            last_heat_seen_ms: None,
        }
    }

    /// Re-arms the SSR availability state machine.
    pub fn rearm(&mut self) {
        if self.hardware_status != SsrHardwareStatus::Available {
            info!(
                "SSR hardware status re-armed by operator recovery (was {:?})",
                self.hardware_status
            );
        }
        self.hardware_status = SsrHardwareStatus::Available;
        self.heat_absent_count = 0;
        self.heat_mismatch_count = 0;
        self.heat_present_count = 0;
        self.last_heat_seen_ms = None;
    }

    /// Detect heat source using a closure to read the detection pin.
    /// This eliminates duplicate code in SsrControl and SsrControlSimple.
    ///
    /// Observability gate: when `current_duty` is too low (belly of the 5 Hz
    /// PWM cycle shorter than the sample interval, including duty 0 at boot),
    /// the detection pin is uninformative — a HIGH sample does NOT mean "SSR
    /// stuck off", it means "we sampled during the OFF window". Skipping the
    /// state update for low duty windows prevents the boot-time dead-lock
    /// where the SSR could never become `Available` (0% duty → HIGH → NOTDET
    /// → output forced to 0 → never 50%+ duty → never Available).
    ///
    /// Debounce: the OFF → `NotDetected` transition requires
    /// `HEAT_ABSENT_DEBOUNCE` consecutive samples. At duty ≥ 50 % a HIGH sample
    /// is still ambiguous (it may be the PWM OFF window), so it only
    /// accumulates via `heat_presence::debounce_heat_absent` (see the module
    /// docs for the run-bound argument that makes this aliasing-proof at the
    /// real tick cadence). A LOW sample, by contrast, is trustworthy evidence
    /// of current flow and restores `Available` immediately.
    pub fn detect_heat_source<F, E>(
        &mut self,
        _current_time: u32,
        read_pin: F,
    ) -> Result<(), SsrError>
    where
        F: FnMut() -> Result<bool, E>,
    {
        #[cfg(any(not(feature = "heat-sense"), feature = "simulated-sensors"))]
        {
            let _ = (_current_time, read_pin);
            Ok(())
        }

        #[cfg(all(feature = "heat-sense", not(feature = "simulated-sensors")))]
        {
            let mut read_pin = read_pin;
            // ≥50% duty ≈ one full sample interval of conduction per PWM period at
            // 5 Hz vs. ~330 ms sampling; below that the pin read may legitimately
            // land in the OFF window even when the SSR is wired and functional.
            let min_observable_ticks = (1u32 << SSR_PWM_RESOLUTION) / 2;
            if (self.current_duty as u32) < min_observable_ticks {
                // Duty too low for the pin to be informative — and a low-power
                // stretch must not accumulate toward NotDetected.
                self.heat_absent_count = 0;
                // A single LOW sample is trustworthy evidence of current flow
                // at ANY duty (the PWM OFF window can only produce HIGH), so
                // honour it here as a re-detection.
                if self.hardware_status != SsrHardwareStatus::Available
                    && matches!(read_pin(), Ok(true))
                {
                    info!("Heat source re-detected at low duty - clearing latch");
                    self.hardware_status = SsrHardwareStatus::Available;
                }
                return Ok(());
            }

            match read_pin() {
                Ok(is_detected) => {
                    let (new_count, outcome) =
                        debounce_heat_absent(self.heat_absent_count, is_detected, true);
                    self.heat_absent_count = new_count;
                    match outcome {
                        HeatPresenceOutcome::HeatDetected => {
                            if self.hardware_status != SsrHardwareStatus::Available {
                                info!("Heat source detected - SSR heating operational");
                                self.hardware_status = SsrHardwareStatus::Available;
                            }
                        }
                        HeatPresenceOutcome::HeatAbsent => {
                            if self.hardware_status == SsrHardwareStatus::Available {
                                warn!(
                                    "Heat source not detected - SSR commands work but no heat generated"
                                );
                                self.hardware_status = SsrHardwareStatus::NotDetected;
                            }
                        }
                        HeatPresenceOutcome::NoChange => {}
                    }
                    Ok(())
                }
                Err(_) => {
                    if self.hardware_status != SsrHardwareStatus::Error {
                        error!("SSR detection pin error - switching to error state");
                        self.hardware_status = SsrHardwareStatus::Error;
                    }
                    Err(SsrError::InputError {
                        source: "detection_pin_read_failed",
                    })
                }
            }
        }
    }

    /// Cross-check commanded duty against the detection pin to catch a stuck-on
    /// or never-heating SSR.
    ///
    /// Active only with the `heat-sense` feature on real hardware; otherwise
    /// (default build, `simulated-sensors`) a no-op. Call EXACTLY once per
    /// control tick from `periodic_check` (H4): sampling the same instant
    /// twice per tick makes consecutive-sample counting alias with the PWM
    /// phase and false-trips on an ideal signal.
    ///
    /// Latch rule is time-based: `Error` only when no heat (LOW) has been
    /// seen for `HEAT_MISMATCH_WINDOW_MS` with duty observable (≥ 50 %).
    /// A single LOW is trustworthy evidence of current flow and resets the
    /// window immediately.
    pub fn cross_check_heat_detection<F, E>(
        &mut self,
        current_duty: u16,
        now_ms: u32,
        read_pin: F,
    ) -> Result<(), SsrError>
    where
        F: FnMut() -> Result<bool, E>,
    {
        #[cfg(any(feature = "simulated-sensors", not(feature = "heat-sense")))]
        {
            let _ = (current_duty, now_ms, read_pin);
            Ok(())
        }

        #[cfg(all(feature = "heat-sense", not(feature = "simulated-sensors")))]
        {
            let mut read_pin = read_pin;
            match read_pin() {
                Ok(is_detected) => {
                    let heat_detected = is_detected;

                    // With the SSR driven by a 5 Hz LEDC PWM (200 ms
                    // period), for duty < 50 % the ON window is shorter than
                    // the sampling interval, so phase alignments exist where
                    // samples land in the OFF window even though the SSR is
                    // functioning correctly. Only declare a mismatch when
                    // the ON window is observably wide at this cadence
                    // (≥50 % duty = one full sample interval of ON per
                    // period), so the cross-check cannot alias with the PWM.
                    let min_observable_ticks = (1u32 << SSR_PWM_RESOLUTION) / 2;
                    let duty_observable = (current_duty as u32) >= min_observable_ticks;

                    if duty_observable && !heat_detected {
                        self.heat_mismatch_count = self.heat_mismatch_count.saturating_add(1);
                        // Time debounce (H4): the window is measured from the
                        // last trustworthy LOW. The first observable-absent
                        // sample anchors the baseline instead of tripping.
                        let last_seen = *self.last_heat_seen_ms.get_or_insert(now_ms);
                        let since_ms = now_ms.saturating_sub(last_seen);
                        warn!(
                            "Heat detection mismatch: heater ON (duty {}) but no heat detected (mismatch count: {}, {} ms since heat seen)",
                            current_duty, self.heat_mismatch_count, since_ms
                        );

                        if since_ms >= HEAT_MISMATCH_WINDOW_MS {
                            error!("Heat detection mismatch window elapsed - SSR error");
                            self.hardware_status = SsrHardwareStatus::Error;
                            return Err(SsrError::HeatSourceNotDetected {
                                source: "heat_mismatch_window",
                            });
                        }
                    } else if current_duty == 0 && heat_detected {
                        // Residual heat after cut-off — the metal mass stays
                        // hot. Require HEAT_PRESENT_MISMATCH_MAX consecutive
                        // samples before declaring the SSR physically stuck on.
                        self.heat_present_count = self.heat_present_count.saturating_add(1);
                        warn!(
                            "Heat present with heater off (count: {}/{}) — possible SSR stuck-on",
                            self.heat_present_count, HEAT_PRESENT_MISMATCH_MAX
                        );
                        if self.heat_present_count >= HEAT_PRESENT_MISMATCH_MAX {
                            error!(
                                "SSR stuck-on detected: heat {} samples after heater off",
                                self.heat_present_count
                            );
                            self.hardware_status = SsrHardwareStatus::Error;
                            return Err(SsrError::HeatSourceNotDetected {
                                source: "ssr_stuck_on_detected",
                            });
                        }
                    } else {
                        // Heat seen (or duty unobservable): a LOW sample is
                        // trustworthy evidence of current flow — refresh the
                        // time-debounce baseline so the window measures from
                        // the last proof of conduction.
                        if heat_detected {
                            self.last_heat_seen_ms = Some(now_ms);
                        }
                        self.heat_mismatch_count = 0;
                        self.heat_present_count = 0;
                    }

                    Ok(())
                }
                Err(_) => {
                    if self.hardware_status != SsrHardwareStatus::Error {
                        error!(
                            "SSR detection pin error during cross-check - switching to error state"
                        );
                        self.hardware_status = SsrHardwareStatus::Error;
                    }
                    Err(SsrError::InputError {
                        source: "detection_pin_read_failed_during_cross_check",
                    })
                }
            }
        }
    }
}

impl Default for SsrControlBase {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusGetters for SsrControlBase {
    fn get_hardware_status(&self) -> SsrHardwareStatus {
        self.hardware_status
    }

    fn is_heating_available(&self) -> bool {
        self.hardware_status == SsrHardwareStatus::Available
    }

    fn get_current_duty(&self) -> u16 {
        self.current_duty
    }

    fn is_pwm_enabled(&self) -> bool {
        self.is_pwm_enabled
    }

    fn last_lead_delta_ticks(&self) -> i16 {
        self.last_duty_delta_ticks
    }

    fn last_retry_count(&self) -> u8 {
        self.retry_count
    }
}

/// State-machine tests. They exercise the real pin-interpretation path, so
/// they only compile with the `heat-sense` feature on a non-simulated host
/// (`--features heat-sense`); without it the methods are no-ops. CI runs
/// this cell explicitly (see `.github/workflows/ci.yml`).
#[cfg(all(
    test,
    not(target_arch = "riscv32"),
    feature = "heat-sense",
    not(feature = "simulated-sensors")
))]
mod tests {
    use super::*;
    use crate::hardware::heat_presence::HEAT_ABSENT_DEBOUNCE;

    fn base_with_duty(duty: u16) -> SsrControlBase {
        let mut base = SsrControlBase::new();
        base.current_duty = duty;
        base
    }

    const DUTY_OBSERVABLE: u16 = (1u16 << (SSR_PWM_RESOLUTION - 1)) + 1;
    /// Real control-loop cadence: one cross-check sample per tick.
    const TICK_MS: u32 = 320;

    #[test]
    fn rearm_restores_available_from_not_detected() {
        let mut base = SsrControlBase::new();
        base.hardware_status = SsrHardwareStatus::NotDetected;
        base.heat_absent_count = 7;
        base.rearm();
        assert_eq!(base.hardware_status, SsrHardwareStatus::Available);
        assert_eq!(base.heat_absent_count, 0);
    }

    #[test]
    fn rearm_restores_available_from_error_and_zeroes_all_counters() {
        let mut base = SsrControlBase::new();
        base.hardware_status = SsrHardwareStatus::Error;
        base.heat_absent_count = 3;
        base.heat_mismatch_count = 4;
        base.heat_present_count = 9;
        base.last_heat_seen_ms = Some(1234);
        base.rearm();
        assert_eq!(base.hardware_status, SsrHardwareStatus::Available);
        assert_eq!(base.heat_absent_count, 0);
        assert_eq!(base.heat_mismatch_count, 0);
        assert_eq!(base.heat_present_count, 0);
        assert_eq!(base.last_heat_seen_ms, None);
    }

    #[test]
    fn rearm_is_idempotent_when_available() {
        let mut base = SsrControlBase::new();
        base.rearm();
        base.rearm();
        assert_eq!(base.hardware_status, SsrHardwareStatus::Available);
    }

    #[test]
    fn low_duty_gate_never_changes_status_without_low_sample() {
        let mut base = base_with_duty(100);
        base.hardware_status = SsrHardwareStatus::Available;
        for _ in 0..(HEAT_ABSENT_DEBOUNCE * 4) {
            let result = base.detect_heat_source(0, || Ok::<bool, ()>(false));
            assert!(result.is_ok());
            assert_eq!(base.hardware_status, SsrHardwareStatus::Available);
            assert_eq!(base.heat_absent_count, 0);
        }
    }

    #[test]
    fn low_duty_low_sample_clears_latch() {
        let mut base = base_with_duty(100);
        base.hardware_status = SsrHardwareStatus::NotDetected;
        base.detect_heat_source(0, || Ok::<bool, ()>(true))
            .expect("detect must succeed");
        assert_eq!(base.hardware_status, SsrHardwareStatus::Available);
    }

    #[test]
    fn not_detected_requires_debounce_consecutive_absent_samples() {
        let mut base = base_with_duty(DUTY_OBSERVABLE);
        for _ in 0..HEAT_ABSENT_DEBOUNCE - 1 {
            base.detect_heat_source(0, || Ok::<bool, ()>(false))
                .expect("detect must succeed");
            assert_eq!(base.hardware_status, SsrHardwareStatus::Available);
        }
        base.detect_heat_source(0, || Ok::<bool, ()>(false))
            .expect("detect must succeed");
        assert_eq!(base.hardware_status, SsrHardwareStatus::NotDetected);
    }

    #[test]
    fn low_sample_restores_available_immediately() {
        let mut base = base_with_duty(DUTY_OBSERVABLE);
        base.hardware_status = SsrHardwareStatus::NotDetected;
        base.heat_absent_count = 4;
        base.detect_heat_source(0, || Ok::<bool, ()>(true))
            .expect("detect must succeed");
        assert_eq!(base.hardware_status, SsrHardwareStatus::Available);
        assert_eq!(base.heat_absent_count, 0);
    }

    #[test]
    fn pin_read_error_latches_error_and_returns_input_error() {
        let mut base = base_with_duty(DUTY_OBSERVABLE);
        let result = base.detect_heat_source(0, || Err::<bool, ()>(()));
        assert!(matches!(
            result,
            Err(SsrError::InputError {
                source: "detection_pin_read_failed"
            })
        ));
        assert_eq!(base.hardware_status, SsrHardwareStatus::Error);
    }

    // The cross-check below only exercises the real-pin path (`heat-sense`
    // on real hardware); without the feature the method is a no-op.
    #[test]
    fn stuck_on_requires_ten_consecutive_heat_samples_at_zero_duty() {
        let mut base = base_with_duty(0);
        for i in 0..9 {
            base.cross_check_heat_detection(0, i * TICK_MS, || Ok::<bool, ()>(true))
                .expect("cross-check must not fail yet");
            assert_eq!(base.hardware_status, SsrHardwareStatus::Available);
        }
        let result = base.cross_check_heat_detection(0, 9 * TICK_MS, || Ok::<bool, ()>(true));
        assert!(matches!(
            result,
            Err(SsrError::HeatSourceNotDetected {
                source: "ssr_stuck_on_detected"
            })
        ));
        assert_eq!(base.hardware_status, SsrHardwareStatus::Error);
    }

    #[test]
    fn mismatch_window_trips_only_after_window_without_heat() {
        // H4: no heat (stuck HIGH) at observable duty, one sample per tick.
        // The first sample anchors the baseline; the latch fires once the
        // window elapses with no LOW in between.
        let mut base = base_with_duty(DUTY_OBSERVABLE);
        for i in 0..5 {
            let t = i * TICK_MS;
            base.cross_check_heat_detection(DUTY_OBSERVABLE, t, || Ok::<bool, ()>(false))
                .expect("cross-check must not fail inside the window");
            assert_eq!(base.hardware_status, SsrHardwareStatus::Available);
        }
        // t = 5 × 320 = 1600 ms ≥ HEAT_MISMATCH_WINDOW_MS (1500).
        let result =
            base.cross_check_heat_detection(DUTY_OBSERVABLE, 5 * TICK_MS, || Ok::<bool, ()>(false));
        assert!(matches!(
            result,
            Err(SsrError::HeatSourceNotDetected {
                source: "heat_mismatch_window"
            })
        ));
        assert_eq!(base.hardware_status, SsrHardwareStatus::Error);
    }

    #[test]
    fn rapid_same_instant_samples_do_not_trip() {
        // H4 regression: repeating the SAME physical instant (the old
        // write-path + periodic double-sample) must not accumulate toward
        // the latch — the decision is time-based, not count-based.
        let mut base = base_with_duty(DUTY_OBSERVABLE);
        for _ in 0..20 {
            base.cross_check_heat_detection(DUTY_OBSERVABLE, 1000, || Ok::<bool, ()>(false))
                .expect("same-instant repeats must never trip");
            assert_eq!(base.hardware_status, SsrHardwareStatus::Available);
        }
    }

    #[test]
    fn heat_seen_resets_mismatch_window() {
        // A LOW sample refreshes the baseline: HIGH runs on both sides of
        // it stay inside the window.
        let mut base = base_with_duty(DUTY_OBSERVABLE);
        for i in 0..4 {
            base.cross_check_heat_detection(DUTY_OBSERVABLE, i * TICK_MS, || Ok::<bool, ()>(false))
                .expect("pre-heat HIGHs inside window");
        }
        base.cross_check_heat_detection(DUTY_OBSERVABLE, 4 * TICK_MS, || Ok::<bool, ()>(true))
            .expect("LOW resets window");
        for i in 5..9 {
            base.cross_check_heat_detection(DUTY_OBSERVABLE, i * TICK_MS, || Ok::<bool, ()>(false))
                .expect("post-heat HIGHs inside renewed window");
            assert_eq!(base.hardware_status, SsrHardwareStatus::Available);
        }
        // 4 × 320 + 1500 = 2780 → t = 9 × 320 = 2880 trips.
        let result =
            base.cross_check_heat_detection(DUTY_OBSERVABLE, 9 * TICK_MS, || Ok::<bool, ()>(false));
        assert!(result.is_err());
        assert_eq!(base.hardware_status, SsrHardwareStatus::Error);
    }

    #[test]
    fn ideal_square_single_sample_per_tick_never_trips() {
        // H4 correction #2 validation: with ONE sample per 320 ms tick, an
        // ideal 5 Hz square wave (LOW while the LEDC is ON) at 55 % duty
        // always shows a LOW within any 1500 ms window, whatever the phase.
        let mut base = base_with_duty(DUTY_OBSERVABLE);
        for n in 0..300u32 {
            // Phase drifts ~120 ms per tick (320 − 200), covering all alignments.
            let t_ms = n * TICK_MS;
            let phase = (t_ms % 200) as f32;
            let on = phase < 0.55 * 200.0;
            let t = t_ms;
            base.cross_check_heat_detection(DUTY_OBSERVABLE, t, || Ok::<bool, ()>(on))
                .expect("ideal square must never trip with one sample per tick");
            assert_eq!(base.hardware_status, SsrHardwareStatus::Available);
        }
    }

    #[test]
    fn no_circuit_trips_after_window() {
        // Without the circuit the pin is always HIGH: single-sample ticks
        // trip shortly after the window elapses (~1.6 s at 320 ms/tick).
        let mut base = base_with_duty(DUTY_OBSERVABLE);
        let mut tripped_at = None;
        for n in 0..20u32 {
            let r = base
                .cross_check_heat_detection(DUTY_OBSERVABLE, n * TICK_MS, || Ok::<bool, ()>(false));
            if r.is_err() {
                tripped_at = Some(n * TICK_MS);
                break;
            }
        }
        let t = tripped_at.expect("no-circuit must trip");
        assert!(
            (HEAT_MISMATCH_WINDOW_MS..=HEAT_MISMATCH_WINDOW_MS + TICK_MS).contains(&t),
            "trip at {} ms, expected just after the {} ms window",
            t,
            HEAT_MISMATCH_WINDOW_MS
        );
        assert_eq!(base.hardware_status, SsrHardwareStatus::Error);
    }

    #[test]
    fn low_duty_cross_check_never_accumulates_mismatch() {
        let mut base = base_with_duty(100);
        for i in 0..(HEAT_MISMATCH_MAX * 2) {
            base.cross_check_heat_detection(100, i as u32 * TICK_MS, || Ok::<bool, ()>(false))
                .expect("cross-check must succeed");
            assert_eq!(base.hardware_status, SsrHardwareStatus::Available);
        }
    }

    #[test]
    fn property_every_latched_state_is_recoverable() {
        for latched in [SsrHardwareStatus::NotDetected, SsrHardwareStatus::Error] {
            let mut via_rearm = SsrControlBase::new();
            via_rearm.hardware_status = latched;
            via_rearm.rearm();
            assert_eq!(via_rearm.hardware_status, SsrHardwareStatus::Available);

            let mut via_low_sample = base_with_duty(100);
            via_low_sample.hardware_status = latched;
            via_low_sample
                .detect_heat_source(0, || Ok::<bool, ()>(true))
                .expect("detect must succeed");
            assert_eq!(via_low_sample.hardware_status, SsrHardwareStatus::Available);
        }
    }
}

/// `effective_percentage` is feature-independent (telemetry honesty, H7),
/// so its tests run on every host configuration.
#[cfg(all(test, not(target_arch = "riscv32")))]
mod effective_percentage_tests {
    use super::effective_percentage;
    use crate::config::constants::{SSR_MIN_DUTY_TICKS, SSR_PWM_RESOLUTION};

    fn ticks(p: f32) -> u32 {
        let max = ((1u32 << SSR_PWM_RESOLUTION) - 1) as f32;
        ((p.clamp(0.0, 100.0) / 100.0) * max + 0.5) as u32
    }

    #[test]
    fn sub_half_cycle_requests_report_zero() {
        // R1: 1–4 % land below SSR_MIN_DUTY_TICKS (819) → zero ticks.
        assert!(ticks(4.0) < SSR_MIN_DUTY_TICKS as u32);
        assert_eq!(effective_percentage(1.0), 0.0);
        assert_eq!(effective_percentage(4.0), 0.0);
    }

    #[test]
    fn at_and_above_floor_reports_requested() {
        assert_eq!(SSR_MIN_DUTY_TICKS, 819);
        assert_eq!(ticks(5.0), 819);
        assert!(ticks(5.0) >= SSR_MIN_DUTY_TICKS as u32);
        assert!(ticks(6.0) >= SSR_MIN_DUTY_TICKS as u32);
        assert_eq!(effective_percentage(0.0), 0.0);
        assert_eq!(effective_percentage(5.0), 5.0);
        assert_eq!(effective_percentage(6.0), 6.0);
        assert_eq!(effective_percentage(50.0), 50.0);
        assert_eq!(effective_percentage(100.0), 100.0);
    }
}
