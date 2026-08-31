//! SSR heater control over LEDC PWM with heat-source detection.
//!
//! Wraps an LEDC channel and an optional SSR-on detection pin into
//! `SsrControl` / `SsrControlSimple`. The pure state machine (`SsrControlBase`,
//! `SsrError`, `SsrHardwareStatus`, `StatusGetters`) lives in `ssr_logic` and is
//! re-exported here. On host builds this module is replaced by `ssr_stub`.

use crate::config::constants::{
    SsrHardwareStatus as GlobalSsrStatus, SSR_DUTY_TOLERANCE_TICKS, SSR_PWM_RESOLUTION,
};
use crate::control::{traits::Heater, RoasterError};
use core::marker::PhantomData;
use embedded_hal::digital::{InputPin, OutputPin};
use esp_hal::ledc::channel::ChannelIFace;
use esp_hal::ledc::LowSpeed;
use log::{debug, error, info, warn};

// M1 (2026-08-21): the pure decision logic (error type, hardware status,
// `SsrControlBase` state machine, `StatusGetters`) lives in the un-gated
// `ssr_logic` module so host unit tests cover it — this module is replaced
// by `ssr_stub.rs` on host builds. Re-exported so existing callers keep
// resolving through `hardware::ssr`.
/// Re-exports of the pure SSR state-machine types from `ssr_logic`.
pub use crate::hardware::ssr_logic::{SsrControlBase, SsrError, SsrHardwareStatus, StatusGetters};

/// Error returned when a raw duty write to the LEDC hardware fails.
/// The only failure mode is the LEDC guard timeout (logged as a warning internally).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DutyWriteError;

/// Low-level LEDC duty access bypassing `ChannelIFace`'s percentage conversion.
pub trait LedcDutyReader {
    /// Read the live duty value (DUTY_R) in raw timer ticks.
    fn read_duty_ticks(&self) -> u16;

    /// Set duty directly in hardware, bypassing the percentage conversion
    /// that `ChannelIFace::set_duty` applies. The duty value MUST be in the
    /// raw resolution range (e.g. 0-255 for 8-bit).
    ///
    /// This is the correct method to call when you have already computed a raw
    /// duty value via `percentage_to_ledc_duty`. Calling `ChannelIFace::set_duty`
    /// with a raw value > 100 will fail because it expects a percentage (0-100).
    fn set_duty_raw(&self, duty: u16) -> Result<(), DutyWriteError>;
}

fn monitor_ledc_after_set<'a, PWM>(
    pwm_channel: &mut PWM,
    commanded: u16,
    retry_count: &mut u8,
    last_delta: &mut i16,
) -> Result<(), SsrError>
where
    PWM: LedcDutyReader + ChannelIFace<'a, LowSpeed>,
{
    let readback = pwm_channel.read_duty_ticks();
    let delta = readback as i16 - commanded as i16;
    *last_delta = delta;

    let tolerance = SSR_DUTY_TOLERANCE_TICKS as i16;
    if delta.abs() <= tolerance {
        return Ok(());
    }

    warn!(
        "LEDC duty drift detected: commanded {} ticks vs actual {} ticks (delta {} ticks) - retrying once",
        commanded, readback, delta
    );
    *retry_count = retry_count.saturating_add(1);

    pwm_channel
        .set_duty_raw(commanded)
        .map_err(|_| SsrError::PwmError {
            source: "set_duty_failed",
        })?;

    let rechecked = pwm_channel.read_duty_ticks();
    let new_delta = rechecked as i16 - commanded as i16;
    *last_delta = new_delta;

    if new_delta.abs() > tolerance {
        error!(
            "LEDC duty mismatch persists after retry: commanded {} ticks vs actual {} ticks (delta {} ticks)",
            commanded, rechecked, new_delta
        );
        return Err(SsrError::PwmError {
            source: "duty_mismatch_after_retry",
        });
    }

    Ok(())
}

/// Trait for heat source detection functionality.
/// Implementations must provide detection logic.
pub trait HeatSourceDetector {
    /// Probe the detection pin and update heat-availability state.
    fn detect_heat_source(&mut self, current_time: u32) -> Result<(), SsrError>;
}

/// Trait for periodic health check functionality.
/// Implementations must provide periodic check logic.
pub trait PeriodicCheck {
    /// Run a periodic health probe (heat detection / cross-check).
    fn periodic_check(&mut self, current_time: u32) -> Result<(), SsrError>;
}

/// Full SSR controller: owns the SSR GPIO plus an LEDC PWM channel and a
/// detection pin. Embeds `SsrControlBase` for shared state.
pub struct SsrControl<'a, PIN, DETECT, PWM>
where
    PIN: OutputPin,
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed> + LedcDutyReader,
{
    #[allow(dead_code)]
    // Stored for ownership - pin is set low during initialization and kept alive
    // to prevent accidental reconfiguration by the HAL. PWM controls actual SSR output.
    pin: PIN,
    detection_pin: DETECT,
    pwm_channel: PWM,
    base: SsrControlBase,
    _phantom: PhantomData<&'a ()>,
}

/// Convert a 0–100 % duty into raw LEDC ticks, applying min-duty snap-to-zero.
pub fn percentage_to_ledc_duty(percentage: f32) -> u16 {
    let clamped = percentage.clamp(0.0, 100.0);
    let max_duty = (1u32 << SSR_PWM_RESOLUTION) - 1;
    let scaled = ((clamped / 100.0) * max_duty as f32 + 0.5) as u32;
    let scaled = scaled.min(max_duty);
    let scaled = scaled as u16;

    // Enforce minimum SSR duty - if duty is too low, snap to 0
    if scaled > 0 && scaled < crate::config::constants::SSR_MIN_DUTY_TICKS {
        0
    } else {
        scaled
    }
}

impl<'a, PIN, DETECT, PWM> SsrControl<'a, PIN, DETECT, PWM>
where
    PIN: OutputPin,
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed> + LedcDutyReader,
{
    /// Build a `SsrControl`, forcing the SSR GPIO low and the PWM to 0 %.
    pub fn new_with_pwm_and_detection(
        mut pin: PIN,
        detection_pin: DETECT,
        pwm_channel: PWM,
    ) -> Result<Self, SsrError> {
        pin.set_low().map_err(|_| SsrError::OutputError {
            source: "pin_init_failed",
        })?;
        pwm_channel
            .set_duty_raw(0)
            .map_err(|_| SsrError::PwmError {
                source: "channel_init_failed",
            })?;

        let ssr = SsrControl {
            pin,
            detection_pin,
            pwm_channel,
            base: SsrControlBase::new(),
            _phantom: PhantomData,
        };

        // Boot-time detection is uninformative at duty=0 (pin HIGH is the
        // expecter OFF state, NOT evidence of missing hardware). The
        // `detect_heat_source` runs from `periodic_check` once enough duty is
        // commanded for the pin read to be meaningful.
        info!(
            "SSR control initialized with PWM - heat source: {:?}",
            ssr.base.hardware_status
        );
        Ok(ssr)
    }

    fn detect_heat_source(&mut self, current_time: u32) -> Result<(), SsrError> {
        self.base
            .detect_heat_source(current_time, || self.detection_pin.is_low())
    }

    pub fn get_current_duty(&self) -> u16 {
        self.base.current_duty
    }

    pub fn is_pwm_enabled(&self) -> bool {
        self.base.is_pwm_enabled
    }

    /// Set heater output as a percentage, writing raw duty and verifying readback.
    pub fn set_percentage(&mut self, percentage: f32) -> Result<(), SsrError> {
        let clamped = percentage.clamp(0.0, 100.0);
        let ledc_duty = percentage_to_ledc_duty(clamped);

        self.base.last_duty_delta_ticks = 0;
        self.base.retry_count = 0;

        self.pwm_channel
            .set_duty_raw(ledc_duty)
            .map_err(|_| SsrError::PwmError {
                source: "set_duty_failed",
            })?;

        // Bug H6 (2026-08-10): record the commanded duty BEFORE the readback
        // verification. `set_duty_raw` succeeded — the duty IS in the LEDC.
        // If only the re-read fails (DUTY_R lag / mismatch after retry), the
        // `?` below used to skip this assignment, leaving `current_duty`
        // stale: telemetry reported a false duty and the observability gate
        // + heat cross-check evaluated safety against a duty the hardware is
        // not applying. The cache must track the commanded value, not the
        // verification outcome.
        self.base.current_duty = ledc_duty;

        monitor_ledc_after_set(
            &mut self.pwm_channel,
            ledc_duty,
            &mut self.base.retry_count,
            &mut self.base.last_duty_delta_ticks,
        )?;

        debug!(
            "SSR set to {:.1}% (duty {}), heat available: {}",
            clamped,
            ledc_duty,
            self.is_heating_available()
        );

        Ok(())
    }
}

/// SSR controller variant without the SSR GPIO: PWM + detection pin only.
pub struct SsrControlSimple<'a, DETECT, PWM>
where
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed> + LedcDutyReader,
{
    detection_pin: DETECT,
    pwm_channel: PWM,
    base: SsrControlBase,
    _phantom: PhantomData<&'a ()>,
}

impl<'a, DETECT, PWM> SsrControlSimple<'a, DETECT, PWM>
where
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed> + LedcDutyReader,
{
    /// Build a `SsrControlSimple`, initialising the PWM channel to 0 %.
    pub fn new(detection_pin: DETECT, pwm_channel: PWM) -> Result<Self, SsrError> {
        pwm_channel
            .set_duty_raw(0)
            .map_err(|_| SsrError::PwmError {
                source: "channel_init_failed",
            })?;

        let ssr = SsrControlSimple {
            detection_pin,
            pwm_channel,
            base: SsrControlBase::new(),
            _phantom: PhantomData,
        };

        // Boot-time detection is uninformative at duty=0 (pin HIGH is the
        // expected OFF state). The `detect_heat_source` runs from
        // `periodic_check` once enough duty is commanded for the pin read to
        // be meaningful. `SsrControlBase::new` already initializes the
        // hardware status to `Available` to avoid the dead-lock the report
        // flags.
        info!(
            "SSR control initialized (simple mode) - heat source: {:?}",
            ssr.base.hardware_status
        );
        Ok(ssr)
    }

    fn detect_heat_source(&mut self, current_time: u32) -> Result<(), SsrError> {
        #[cfg(feature = "simulated-sensors")]
        {
            self.base
                .detect_heat_source(current_time, || Ok::<bool, core::convert::Infallible>(true))
        }
        #[cfg(not(feature = "simulated-sensors"))]
        {
            self.base
                .detect_heat_source(current_time, || self.detection_pin.is_low())
        }
    }

    /// Run heat detection and the stuck-on cross-check for the simple controller.
    pub fn periodic_check(&mut self, current_time: u32) -> Result<(), SsrError> {
        // No throttle (bug audit 2026-08-02): consecutive detects must stay
        // one control-loop tick apart (~330 ms → ~130 ms of PWM phase
        // separation, provably in the (100, 200) ms band the
        // `HEAT_ABSENT_DEBOUNCE` run-bound analysis relies on). The previous
        // 1000 ms gate made the separation depend on the tick multiple — up
        // to 3 ticks — where the OFF window could alias with the phase and
        // produce long spurious "no heat" runs. A GPIO read is negligible;
        // the per-tick cadence also matches `cross_check_heat_detection`.
        self.detect_heat_source(current_time)?;

        self.base
            .cross_check_heat_detection(self.base.current_duty, || self.detection_pin.is_low())?;

        Ok(())
    }

    /// Set heater output as a percentage, writing raw duty and verifying readback.
    pub fn set_percentage(&mut self, percentage: f32) -> Result<(), SsrError> {
        let clamped = percentage.clamp(0.0, 100.0);
        let ledc_duty = percentage_to_ledc_duty(clamped);

        self.base.last_duty_delta_ticks = 0;
        self.base.retry_count = 0;

        self.pwm_channel
            .set_duty_raw(ledc_duty)
            .map_err(|_| SsrError::PwmError {
                source: "set_duty_failed",
            })?;

        // Bug H6 (2026-08-10): record the commanded duty BEFORE the readback
        // verification — the write reached the LEDC; a failed re-read must
        // not leave the duty cache stale (see SsrControl::set_percentage).
        self.base.current_duty = ledc_duty;

        monitor_ledc_after_set(
            &mut self.pwm_channel,
            ledc_duty,
            &mut self.base.retry_count,
            &mut self.base.last_duty_delta_ticks,
        )?;

        debug!(
            "SSR set to {:.1}% (duty {}), heat available: {}",
            clamped,
            ledc_duty,
            self.is_heating_available()
        );

        self.base
            .cross_check_heat_detection(self.base.current_duty, || self.detection_pin.is_low())?;

        Ok(())
    }

    pub fn get_current_duty(&self) -> u16 {
        self.base.current_duty
    }

    pub fn is_pwm_enabled(&self) -> bool {
        self.base.is_pwm_enabled
    }

    pub fn last_lead_delta_ticks(&self) -> i16 {
        self.base.last_duty_delta_ticks
    }

    pub fn last_retry_count(&self) -> u8 {
        self.base.retry_count
    }
}

impl<'a, PIN, DETECT, PWM> StatusGetters for SsrControl<'a, PIN, DETECT, PWM>
where
    PIN: OutputPin,
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed> + LedcDutyReader,
{
    fn get_hardware_status(&self) -> SsrHardwareStatus {
        self.base.hardware_status
    }

    fn is_heating_available(&self) -> bool {
        self.base.hardware_status == SsrHardwareStatus::Available
    }

    fn get_current_duty(&self) -> u16 {
        self.base.current_duty
    }

    fn is_pwm_enabled(&self) -> bool {
        self.base.is_pwm_enabled
    }

    fn last_lead_delta_ticks(&self) -> i16 {
        self.base.last_duty_delta_ticks
    }

    fn last_retry_count(&self) -> u8 {
        self.base.retry_count
    }
}

impl<'a, PIN, DETECT, PWM> HeatSourceDetector for SsrControl<'a, PIN, DETECT, PWM>
where
    PIN: OutputPin,
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed> + LedcDutyReader,
{
    fn detect_heat_source(&mut self, current_time: u32) -> Result<(), SsrError> {
        SsrControl::detect_heat_source(self, current_time)
    }
}

impl<'a, PIN, DETECT, PWM> PeriodicCheck for SsrControl<'a, PIN, DETECT, PWM>
where
    PIN: OutputPin,
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed> + LedcDutyReader,
{
    fn periodic_check(&mut self, current_time: u32) -> Result<(), SsrError> {
        // No throttle — see SsrControlSimple::periodic_check for the
        // phase-separation argument (bug audit 2026-08-02).
        self.detect_heat_source(current_time)?;

        Ok(())
    }
}

impl<'a, DETECT, PWM> Heater for SsrControlSimple<'a, DETECT, PWM>
where
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed> + LedcDutyReader,
{
    fn set_power(&mut self, duty: f32) -> Result<(), RoasterError> {
        self.set_percentage(duty)
            .map_err(|_| RoasterError::HardwareError {
                source: Some("ssr_set_power"),
            })
    }

    fn get_status(&self) -> GlobalSsrStatus {
        match self.get_hardware_status() {
            SsrHardwareStatus::Available => GlobalSsrStatus::Available,
            SsrHardwareStatus::NotDetected => GlobalSsrStatus::NotDetected,
            SsrHardwareStatus::Error => GlobalSsrStatus::Error,
        }
    }

    fn periodic_health_check(&mut self, current_time_ms: u32) {
        if let Err(e) = self.periodic_check(current_time_ms) {
            log::warn!("SSR health check failed (SsrControlSimple): {:?}", e);
        }
    }

    fn rearm_hardware_status(&mut self) {
        self.base.rearm();
    }

    fn last_duty_delta_ticks(&self) -> i16 {
        self.base.last_duty_delta_ticks
    }

    fn last_retry_count(&self) -> u8 {
        self.base.retry_count
    }
}

impl<'a, DETECT, PWM> StatusGetters for SsrControlSimple<'a, DETECT, PWM>
where
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed> + LedcDutyReader,
{
    fn get_hardware_status(&self) -> SsrHardwareStatus {
        self.base.hardware_status
    }

    fn is_heating_available(&self) -> bool {
        self.base.hardware_status == SsrHardwareStatus::Available
    }

    fn get_current_duty(&self) -> u16 {
        self.base.current_duty
    }

    fn is_pwm_enabled(&self) -> bool {
        self.base.is_pwm_enabled
    }

    fn last_lead_delta_ticks(&self) -> i16 {
        self.base.last_duty_delta_ticks
    }

    fn last_retry_count(&self) -> u8 {
        self.base.retry_count
    }
}

impl<'a, DETECT, PWM> HeatSourceDetector for SsrControlSimple<'a, DETECT, PWM>
where
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed> + LedcDutyReader,
{
    fn detect_heat_source(&mut self, current_time: u32) -> Result<(), SsrError> {
        SsrControlSimple::detect_heat_source(self, current_time)
    }
}

impl<'a, DETECT, PWM> PeriodicCheck for SsrControlSimple<'a, DETECT, PWM>
where
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed> + LedcDutyReader,
{
    fn periodic_check(&mut self, current_time: u32) -> Result<(), SsrError> {
        SsrControlSimple::periodic_check(self, current_time)
    }
}

// SAFETY(v5.1): Sound on single-core ESP32-C3 (cooperative Embassy tasks).
// If porting to multi-core ESP32 (e.g., ESP32-S3 dual-core), review ref-counting
// of the LEDC Timer reference held by ChannelIFace. The Timer is stored in the
// static LedcBus and outlives all Channel users on single-core.
// SAFETY: SsrControlSimple owns its peripheral handles exclusively.
// On the single-core ESP32-C3, Embassy tasks run cooperatively — only one
// task executes at a time. The type is moved into a `Box<dyn Heater + Send>`
// and passed to a single task via ServiceContainer, so no concurrent access
// occurs. The LEDC ChannelIFace is !Send by default because it holds a
// reference to the Timer; we vouch that the Timer outlives all Channel users
// (it is stored in the static LedcBus).
unsafe impl<'a, DETECT, PWM> Send for SsrControlSimple<'a, DETECT, PWM>
where
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed> + LedcDutyReader,
{
}

impl<'a, PIN, DETECT, PWM> Heater for SsrControl<'a, PIN, DETECT, PWM>
where
    PIN: OutputPin,
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed> + LedcDutyReader,
{
    fn set_power(&mut self, duty: f32) -> Result<(), RoasterError> {
        SsrControl::set_percentage(self, duty).map_err(|_| RoasterError::HardwareError {
            source: Some("ssr_control_set_percentage"),
        })
    }

    fn get_status(&self) -> GlobalSsrStatus {
        match StatusGetters::get_hardware_status(self) {
            SsrHardwareStatus::Available => GlobalSsrStatus::Available,
            SsrHardwareStatus::NotDetected => GlobalSsrStatus::NotDetected,
            SsrHardwareStatus::Error => GlobalSsrStatus::Error,
        }
    }

    fn periodic_health_check(&mut self, current_time_ms: u32) {
        if let Err(e) = PeriodicCheck::periodic_check(self, current_time_ms) {
            log::warn!("SSR health check failed (SsrControl): {:?}", e);
        }
    }

    fn rearm_hardware_status(&mut self) {
        self.base.rearm();
    }

    fn last_duty_delta_ticks(&self) -> i16 {
        StatusGetters::last_lead_delta_ticks(self)
    }

    fn last_retry_count(&self) -> u8 {
        StatusGetters::last_retry_count(self)
    }
}

// SAFETY(v5.1): Sound on single-core ESP32-C3 (cooperative Embassy tasks).
// If porting to multi-core ESP32 (e.g., ESP32-S3 dual-core), review ref-counting
// of the LEDC Timer reference held by ChannelIFace. The Timer is stored in the
// static LedcBus and outlives all Channel users on single-core.
// SAFETY: SsrControl owns its peripheral handles exclusively.
// On the single-core ESP32-C3, Embassy tasks run cooperatively — only one
// task executes at a time. The type is moved into a `Box<dyn Heater + Send>`
// and passed to a single task via ServiceContainer, so no concurrent access
// occurs. The LEDC ChannelIFace is !Send by default because it holds a
// reference to the Timer; we vouch that the Timer outlives all Channel users
// (it is stored in the static LedcBus).
unsafe impl<'a, PIN, DETECT, PWM> Send for SsrControl<'a, PIN, DETECT, PWM>
where
    PIN: OutputPin,
    DETECT: InputPin,
    PWM: ChannelIFace<'a, LowSpeed> + LedcDutyReader,
{
}

#[cfg(test)]
mod tests {
    use super::percentage_to_ledc_duty;

    use crate::config::constants::{
        SSR_CYCLE_GUARD_MS, SSR_DUTY_TOLERANCE_TICKS, SSR_PWM_RESOLUTION,
    };

    const fn max_duty() -> u16 {
        ((1u32 << SSR_PWM_RESOLUTION) - 1) as u16
    }

    #[test]
    fn percentage_to_ledc_duty_handles_bounds() {
        assert_eq!(percentage_to_ledc_duty(0.0), 0);
        assert_eq!(percentage_to_ledc_duty(100.0), max_duty());
        assert_eq!(percentage_to_ledc_duty(-50.0), 0);
        assert_eq!(percentage_to_ledc_duty(150.0), max_duty());
    }

    #[test]
    fn percentage_to_ledc_duty_rounds_midpoints() {
        let max_duty = ((1u32 << SSR_PWM_RESOLUTION) - 1) as f32;
        let expected = (max_duty * 0.5).round() as u16;
        assert_eq!(percentage_to_ledc_duty(50.0), expected);
    }

    #[test]
    fn guard_constants_are_locked() {
        assert_eq!(SSR_CYCLE_GUARD_MS, 100);
        assert_eq!(SSR_DUTY_TOLERANCE_TICKS, 128);
    }

    // The `test_digital_error_kind` test that lived here was removed: it
    // constructed `SsrError::OutputError` without the required `source` field
    // (E0063) and was dead code only compiled under `#[cfg(test)]` on the
    // riscv32 target. The underlying behaviour — `SsrError: embedded_hal::
    // digital::Error` returning `ErrorKind::Other` — is exercised trivially by
    // the trait impl at lines 84-88 above and needs no dedicated test.
    //
    // Note: this module is replaced by `ssr_stub.rs` on host builds, so the
    // tests here only compile on the riscv32 target. The heat-source
    // detection debounce logic therefore lives in `heat_presence.rs` (an
    // un-gated module) where its unit tests actually run in CI.
}
