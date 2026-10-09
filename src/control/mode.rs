//! CORE-2 (plan CORE-2026-10-09): who drives the heater, as ONE value.
//!
//! The core still STORES the legacy flags (`SystemStatus::pid_enabled`,
//! `SystemStatus::artisan_control`, the TUNE state and the RoR follower).
//! `ControlMode::derive` turns them into one enum so that decisions read a
//! single value instead of re-combining flags by hand — the combination
//! errors of the last audits (N1, M-4, M-7, R-1) all came from a predicate
//! that looked at one flag too few.
//!
//! Pure logic: no hardware, no clock. The emergency latch is NOT part of the
//! mode (it is an override on top of any mode, see `SafetyController`).
//!
//! | `pid_enabled` | `artisan_control` | extra | mode |
//! |---|---|---|---|
//! | false | false | — | `Off` |
//! | false | true | step test queued/running | `ManualTune` |
//! | false | true | — | `Manual` |
//! | true | false | RoR follower present | `RorFollow` |
//! | true | false | — | `FirmwarePid` |
//! | true | true | — | `Transitional` (inside a handler only; never at the end of a tick) |

/// Who drives the heater.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlMode {
    /// Nobody: the heater is held at 0 (Idle, after STOP).
    Off,
    /// Operator sliders (`OT1`/`UP`/`DOWN`).
    Manual,
    /// Manual mode with a step test (TUNE) queued or running.
    ManualTune,
    /// Firmware PID toward a fixed or profile setpoint.
    FirmwarePid,
    /// Firmware PID whose setpoint RoR-follow generates (armed or ramping).
    RorFollow,
    /// Both flags set. Only exists between two writes inside a handler.
    Transitional,
}

/// The stored flags `ControlMode` is derived from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeFlags {
    pub pid_enabled: bool,
    pub artisan_control: bool,
    pub tune_active: bool,
    pub follower_present: bool,
}

impl ControlMode {
    /// Derive the mode from the stored flags (table in the module docs).
    pub fn derive(f: ModeFlags) -> Self {
        match (f.pid_enabled, f.artisan_control) {
            (false, false) => ControlMode::Off,
            (false, true) if f.tune_active => ControlMode::ManualTune,
            (false, true) => ControlMode::Manual,
            (true, false) if f.follower_present => ControlMode::RorFollow,
            (true, false) => ControlMode::FirmwarePid,
            (true, true) => ControlMode::Transitional,
        }
    }

    /// The firmware PID owns the heater: exactly `pid_enabled && !artisan_control`.
    pub fn firmware_pid(self) -> bool {
        matches!(self, ControlMode::FirmwarePid | ControlMode::RorFollow)
    }

    /// The operator owns the heater: exactly `artisan_control && !pid_enabled`.
    pub fn operator_manual(self) -> bool {
        matches!(self, ControlMode::Manual | ControlMode::ManualTune)
    }

    /// The operator does NOT own the heater: exactly `!artisan_control`.
    pub fn firmware_in_control(self) -> bool {
        matches!(
            self,
            ControlMode::Off | ControlMode::FirmwarePid | ControlMode::RorFollow
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exhaustive proof that every helper is EXACTLY the flag expression it
    /// replaces in `roaster_control.rs` (task C2 relies on this).
    #[test]
    fn helpers_equal_the_flag_expressions_for_all_16_combinations() {
        for bits in 0u8..16 {
            let f = ModeFlags {
                pid_enabled: bits & 1 != 0,
                artisan_control: bits & 2 != 0,
                tune_active: bits & 4 != 0,
                follower_present: bits & 8 != 0,
            };
            let m = ControlMode::derive(f);
            let (pid, art) = (f.pid_enabled, f.artisan_control);
            assert_eq!(m.firmware_pid(), pid && !art, "{f:?}");
            assert_eq!(m.operator_manual(), art && !pid, "{f:?}");
            assert_eq!(m.firmware_in_control(), !art, "{f:?}");
            assert_eq!(
                m == ControlMode::RorFollow,
                f.follower_present && pid && !art,
                "{f:?}"
            );
            assert_eq!(
                m == ControlMode::ManualTune,
                f.tune_active && art && !pid,
                "{f:?}"
            );
            assert_eq!(m == ControlMode::Transitional, pid && art, "{f:?}");
        }
    }
}
