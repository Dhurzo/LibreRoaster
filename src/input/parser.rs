use crate::config::{ArtisanCommand, FanProfile, ProfileSetpoint, RoastProfile, MAX_PROFILE_SETPOINTS};
use core::cell::RefCell;
use critical_section::Mutex;

static PARSED_PROFILE: Mutex<RefCell<Option<RoastProfile>>> = Mutex::new(RefCell::new(None));

/// Store a parsed profile for the command handler to consume.
pub fn store_profile(profile: RoastProfile) {
    critical_section::with(|cs| {
        *PARSED_PROFILE.borrow(cs).borrow_mut() = Some(profile);
    });
}

/// Consume the stored profile, returning it and clearing the slot.
pub fn take_profile() -> Option<RoastProfile> {
    critical_section::with(|cs| {
        PARSED_PROFILE.borrow(cs).borrow_mut().take()
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    UnknownCommand,
    InvalidValue,
    OutOfRange,
    EmptyCommand,
}

impl ParseError {
    pub fn code(&self) -> &'static str {
        match self {
            ParseError::UnknownCommand => "unknown_command",
            ParseError::InvalidValue => "invalid_value",
            ParseError::OutOfRange => "out_of_range",
            ParseError::EmptyCommand => "invalid_value",
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            ParseError::UnknownCommand => "unknown_command",
            ParseError::InvalidValue => "invalid_value",
            ParseError::OutOfRange => "out_of_range",
            ParseError::EmptyCommand => "empty_command",
        }
    }
}

pub fn parse_artisan_command(command: &str) -> Result<ArtisanCommand, ParseError> {
    let trimmed = command.trim();

    if trimmed.is_empty() {
        return Err(ParseError::EmptyCommand);
    }

    // Try semicolon delimiter first (Artisan standard for init commands)
    // If the command isn't CHAN/UNITS/FILT, fall through to space parsing
    // to handle typos like "OT1;75" → "OT1 75"
    if let Some((cmd, args)) = trimmed.split_once(';') {
        let init_result: Option<Result<ArtisanCommand, ParseError>> =
            match cmd.to_ascii_uppercase().as_str() {
                "CHAN" => Some(args
                    .trim()
                    .parse::<u16>()
                    .map(ArtisanCommand::Chan)
                    .map_err(|_| ParseError::InvalidValue)),
                "UNITS" => Some(match args.trim() {
                    "C" | "c" => Ok(ArtisanCommand::Units(false)),
                    "F" | "f" => Ok(ArtisanCommand::Units(true)),
                    _ => Err(ParseError::InvalidValue),
                }),
                "FILT" => {
                    // Artisan sends comma-separated filter values (e.g., "FILT;70,70,70,70")
                    // or a single value (e.g., "FILT;5"). The value is acknowledged but
                    // not used by the firmware — just extract the first token.
                    let val = args
                        .trim()
                        .split(',')
                        .next()
                        .unwrap_or("0")
                        .trim()
                        .parse::<u8>()
                        .unwrap_or(0);
                    Some(Ok(ArtisanCommand::Filt(val)))
                }
                "PROFILE" => Some(parse_profile_args(args.trim())),
                "FANPROFILE" => Some(parse_fan_profile_args(args.trim())),
                "PID" => Some(parse_pid_subcommand(args.trim())),
                // Unknown init command → fall through to space parsing
                _ => None,
            };

        if let Some(result) = init_result {
            return result;
        }
    }

    // Fall back to space delimiter for operational commands
    let parts: heapless::Vec<&str, 4> = trimmed.split_whitespace().collect();

    if parts.is_empty() {
        return Err(ParseError::UnknownCommand);
    }

    let cmd = parts[0];

    if (cmd.eq_ignore_ascii_case("STATUS") || cmd.eq_ignore_ascii_case("STAT")) && parts.len() == 1
    {
        Ok(ArtisanCommand::StatusReport)
    } else if cmd.eq_ignore_ascii_case("READ") && parts.len() == 1 {
        Ok(ArtisanCommand::ReadStatus)
    } else if cmd.eq_ignore_ascii_case("START") && parts.len() == 1 {
        Ok(ArtisanCommand::StartRoast)
    } else if cmd.eq_ignore_ascii_case("STOP") && parts.len() == 1 {
        Ok(ArtisanCommand::EmergencyStop)
    } else if cmd.eq_ignore_ascii_case("UP") && parts.len() == 1 {
        Ok(ArtisanCommand::IncreaseHeater)
    } else if cmd.eq_ignore_ascii_case("DOWN") && parts.len() == 1 {
        Ok(ArtisanCommand::DecreaseHeater)
    } else if cmd.eq_ignore_ascii_case("REG") && parts.len() == 1 {
        Ok(ArtisanCommand::RunRegression)
    } else if cmd.eq_ignore_ascii_case("OT1") {
        if parts.len() == 2 {
            let value = parse_percentage(parts[1])?;
            Ok(ArtisanCommand::SetHeater(value))
        } else {
            Err(ParseError::InvalidValue)
        }
    } else if cmd.eq_ignore_ascii_case("IO3") {
        if parts.len() == 2 {
            let value = parse_percentage(parts[1])?;
            Ok(ArtisanCommand::SetFan(value))
        } else {
            Err(ParseError::InvalidValue)
        }
    } else if cmd.eq_ignore_ascii_case("OT2") {
        if parts.len() == 2 {
            let (value, was_clamped) = parse_ot2_value(parts[1])?;
            Ok(ArtisanCommand::SetFanSpeed(value, was_clamped))
        } else {
            Err(ParseError::InvalidValue)
        }
    } else if cmd.eq_ignore_ascii_case("PIDGAIN") {
        if parts.len() == 4 {
            let kp = parse_float(parts[1])?;
            let ki = parse_float(parts[2])?;
            let kd = parse_float(parts[3])?;
            Ok(ArtisanCommand::SetPidGain(kp, ki, kd))
        } else {
            Err(ParseError::InvalidValue)
        }
    } else if cmd.eq_ignore_ascii_case("#DUMP") && parts.len() == 1 {
        Ok(ArtisanCommand::DumpLog)
    } else if cmd.eq_ignore_ascii_case("PID,ON") {
        Ok(ArtisanCommand::StartRoast)
    } else if cmd.eq_ignore_ascii_case("PID,OFF") {
        Ok(ArtisanCommand::EmergencyStop)
    } else if cmd.to_ascii_uppercase().starts_with("PID,SV,") {
        // PID,SV,150 → same as SETTARGET 150
        let sv_str = &cmd[7..]; // After "PID,SV,"
        let target = sv_str.trim().parse::<f32>().map_err(|_| ParseError::InvalidValue)?;
        if !(50.0..=300.0).contains(&target) { return Err(ParseError::OutOfRange); }
        Ok(ArtisanCommand::SetTargetTemp(target))
    } else if cmd.eq_ignore_ascii_case("PREHEAT") {
        if parts.len() == 2 {
            let temp = parse_float(parts[1])?;
            if !(50.0..=300.0).contains(&temp) { return Err(ParseError::OutOfRange); }
            Ok(ArtisanCommand::Preheat(temp))
        } else {
            Err(ParseError::InvalidValue)
        }
    } else if cmd.eq_ignore_ascii_case("SETTARGET") {
        if parts.len() == 2 {
            let target = parse_float(parts[1])?;
            if !(50.0..=300.0).contains(&target) {
                return Err(ParseError::OutOfRange);
            }
            Ok(ArtisanCommand::SetTargetTemp(target))
        } else {
            Err(ParseError::InvalidValue)
        }
    } else {
        Err(ParseError::UnknownCommand)
    }
}

fn parse_pid_subcommand(args: &str) -> Result<ArtisanCommand, ParseError> {
    let parts: heapless::Vec<&str, 8> = args.split(';').collect();
    if parts.is_empty() {
        return Err(ParseError::InvalidValue);
    }

    match parts[0].trim().to_ascii_uppercase().as_str() {
        "ON" => Ok(ArtisanCommand::StartRoast),
        "OFF" => Ok(ArtisanCommand::EmergencyStop),
        "SV" => {
            if parts.len() < 2 {
                return Err(ParseError::InvalidValue);
            }
            let target = parts[1]
                .trim()
                .parse::<f32>()
                .map_err(|_| ParseError::InvalidValue)?;
            if !(50.0..=300.0).contains(&target) {
                return Err(ParseError::OutOfRange);
            }
            Ok(ArtisanCommand::SetTargetTemp(target))
        }
        "T" => {
            if parts.len() < 4 {
                return Err(ParseError::InvalidValue);
            }
            let kp = parts[1].trim().parse::<f32>().map_err(|_| ParseError::InvalidValue)?;
            let ki = parts[2].trim().parse::<f32>().map_err(|_| ParseError::InvalidValue)?;
            let kd = parts[3].trim().parse::<f32>().map_err(|_| ParseError::InvalidValue)?;
            if kp < 0.0 || ki < 0.0 || kd < 0.0 {
                return Err(ParseError::OutOfRange);
            }
            Ok(ArtisanCommand::SetPidGain(kp, ki, kd))
        }
        "CHAN" => {
            if parts.len() < 2 {
                return Err(ParseError::InvalidValue);
            }
            let ch = parts[1]
                .trim()
                .parse::<u8>()
                .map_err(|_| ParseError::InvalidValue)?;
            if !(1..=4).contains(&ch) {
                return Err(ParseError::OutOfRange);
            }
            Ok(ArtisanCommand::SetPidChannel(ch))
        }
        "CT" => {
            if parts.len() < 2 {
                return Err(ParseError::InvalidValue);
            }
            let ms = parts[1]
                .trim()
                .parse::<u32>()
                .map_err(|_| ParseError::InvalidValue)?;
            if ms < 10 {
                return Err(ParseError::OutOfRange);
            }
            Ok(ArtisanCommand::SetPidCycleTime(ms))
        }
        "LIMIT" => {
            if parts.len() < 3 {
                return Err(ParseError::InvalidValue);
            }
            let min = parts[1]
                .trim()
                .parse::<f32>()
                .map_err(|_| ParseError::InvalidValue)?;
            let max = parts[2]
                .trim()
                .parse::<f32>()
                .map_err(|_| ParseError::InvalidValue)?;
            Ok(ArtisanCommand::SetPidOutputLimits(min, max))
        }
        _ => Err(ParseError::UnknownCommand),
    }
}

fn parse_percentage(value_str: &str) -> Result<u8, ParseError> {
    let value = value_str
        .parse::<u8>()
        .map_err(|_| ParseError::InvalidValue)?;

    if value <= 100 {
        Ok(value)
    } else {
        Err(ParseError::OutOfRange)
    }
}

fn parse_float(value_str: &str) -> Result<f32, ParseError> {
    value_str
        .parse::<f32>()
        .map_err(|_| ParseError::InvalidValue)
}

/// Parse OT2 fan speed value with decimal support.
///
/// OT2 semantics differ from OT1/IO3: the MAX31856-style protocol specifies that
/// out-of-range OT2 values indicate a sensor fault, so the heater is cut when
/// clamping occurs. Values outside [0,100] are rounded and clamped to that range,
/// and the caller receives `was_clamped=true` to trigger the heater safety cutoff.
///
/// - Decimals are rounded to nearest integer
/// - Values are silently clamped to 0-100 range
/// - Negative values clamp to 0
/// - Returns (clamped_value, was_clamped)
fn parse_ot2_value(value_str: &str) -> Result<(u8, bool), ParseError> {
    let value = value_str
        .parse::<f32>()
        .map_err(|_| ParseError::InvalidValue)?;

    let was_clamped = !(0.0..=100.0).contains(&value);

    // Round to nearest integer (0.5 rounds up)
    let rounded = if value >= 0.0 {
        (value + 0.5) as i32
    } else {
        (value - 0.5) as i32
    };

    let clamped = rounded.clamp(0, 100) as u8;
    Ok((clamped, was_clamped))
}

fn parse_profile_args(args: &str) -> Result<ArtisanCommand, ParseError> {
    let mut profile = RoastProfile::new();
    for segment in args.split(';') {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        let mut parts = segment.splitn(2, ',');
        let time_str = parts.next().ok_or(ParseError::InvalidValue)?;
        let temp_str = parts.next().ok_or(ParseError::InvalidValue)?;

        let time_secs: u32 = time_str.trim().parse().map_err(|_| ParseError::InvalidValue)?;
        let temperature: f32 = temp_str.trim().parse().map_err(|_| ParseError::InvalidValue)?;

        if !(50.0..=300.0).contains(&temperature) {
            return Err(ParseError::OutOfRange);
        }

        profile.setpoints.push(ProfileSetpoint { time_secs, temperature })
            .map_err(|_| ParseError::OutOfRange)?;
    }

    if profile.setpoints.is_empty() {
        return Err(ParseError::EmptyCommand);
    }

    store_profile(profile);
    Ok(ArtisanCommand::SetProfile)
}

fn parse_fan_profile_args(args: &str) -> Result<ArtisanCommand, ParseError> {
    use crate::config::FanSetpoint;
    let mut profile = FanProfile::new();
    for segment in args.split(';') {
        let segment = segment.trim();
        if segment.is_empty() { continue; }
        let mut parts = segment.splitn(2, ',');
        let time_secs: u32 = parts.next().ok_or(ParseError::InvalidValue)?
            .trim().parse().map_err(|_| ParseError::InvalidValue)?;
        let fan_speed: u8 = parts.next().ok_or(ParseError::InvalidValue)?
            .trim().parse().map_err(|_| ParseError::InvalidValue)?;
        if fan_speed > 100 { return Err(ParseError::OutOfRange); }
        profile.setpoints.push(FanSetpoint { time_secs, fan_speed })
            .map_err(|_| ParseError::OutOfRange)?;
    }
    if profile.setpoints.is_empty() { return Err(ParseError::EmptyCommand); }
    crate::input::parser::fan_profile_store(profile);
    Ok(ArtisanCommand::SetFanProfile)
}

static PARSED_FAN_PROFILE: Mutex<RefCell<Option<FanProfile>>> = Mutex::new(RefCell::new(None));
pub fn fan_profile_store(profile: FanProfile) {
    critical_section::with(|cs| *PARSED_FAN_PROFILE.borrow(cs).borrow_mut() = Some(profile));
}
pub fn fan_profile_take() -> Option<FanProfile> {
    critical_section::with(|cs| PARSED_FAN_PROFILE.borrow(cs).borrow_mut().take())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_read_command() {
        let result = parse_artisan_command("READ");
        assert!(matches!(result, Ok(ArtisanCommand::ReadStatus)));
    }

    #[test]
    fn test_parse_status_command() {
        let result = parse_artisan_command("STATUS");
        assert!(matches!(result, Ok(ArtisanCommand::StatusReport)));
    }

    #[test]
    fn test_parse_stat_command_alias() {
        let result = parse_artisan_command("STAT");
        assert!(matches!(result, Ok(ArtisanCommand::StatusReport)));
    }

    #[test]
    fn test_parse_start_command() {
        let result = parse_artisan_command("START");
        assert!(matches!(result, Ok(ArtisanCommand::StartRoast)));
    }

    #[test]
    fn test_parse_ot1_command() {
        let result = parse_artisan_command("OT1 75");
        assert!(matches!(result, Ok(ArtisanCommand::SetHeater(75))));
    }

    #[test]
    fn test_parse_io3_command() {
        let result = parse_artisan_command("IO3 50");
        assert!(matches!(result, Ok(ArtisanCommand::SetFan(50))));
    }

    #[test]
    fn test_parse_stop_command() {
        let result = parse_artisan_command("STOP");
        assert!(matches!(result, Ok(ArtisanCommand::EmergencyStop)));
    }

    #[test]
    fn test_parse_regression_command() {
        let result = parse_artisan_command("REG");
        assert!(matches!(result, Ok(ArtisanCommand::RunRegression)));
    }

    #[test]
    fn test_invalid_command() {
        let result = parse_artisan_command("INVALID");
        assert!(matches!(result, Err(ParseError::UnknownCommand)));
    }

    #[test]
    fn test_invalid_value() {
        let result = parse_artisan_command("OT1 abc");
        assert!(matches!(result, Err(ParseError::InvalidValue)));
    }

    #[test]
    fn test_out_of_range_value() {
        let result = parse_artisan_command("OT1 150");
        assert!(matches!(result, Err(ParseError::OutOfRange)));
    }

    #[test]
    fn test_empty_command() {
        let result = parse_artisan_command("");
        assert!(matches!(result, Err(ParseError::EmptyCommand)));
    }

    #[test]
    fn test_parse_ot1_zero() {
        let result = parse_artisan_command("OT1 0");
        assert!(matches!(result, Ok(ArtisanCommand::SetHeater(0))));
    }

    #[test]
    fn test_parse_ot1_max() {
        let result = parse_artisan_command("OT1 100");
        assert!(matches!(result, Ok(ArtisanCommand::SetHeater(100))));
    }

    #[test]
    fn test_parse_io3_zero() {
        let result = parse_artisan_command("IO3 0");
        assert!(matches!(result, Ok(ArtisanCommand::SetFan(0))));
    }

    #[test]
    fn test_parse_io3_max() {
        let result = parse_artisan_command("IO3 100");
        assert!(matches!(result, Ok(ArtisanCommand::SetFan(100))));
    }

    #[test]
    fn test_parse_io3_invalid_above() {
        let result = parse_artisan_command("IO3 150");
        assert!(matches!(result, Err(ParseError::OutOfRange)));
    }

    // Initialization handshake command tests (Phase 17)

    #[test]
    fn test_parse_chan_command() {
        let result = parse_artisan_command("CHAN;1200");
        assert!(matches!(result, Ok(ArtisanCommand::Chan(1200))));
    }

    #[test]
    fn test_parse_chan_command_lowercase() {
        let result = parse_artisan_command("chan;1200");
        assert!(matches!(result, Ok(ArtisanCommand::Chan(1200))));
    }

    #[test]
    fn test_parse_chan_command_mixed_case() {
        let result = parse_artisan_command("Chan;9999");
        assert!(matches!(result, Ok(ArtisanCommand::Chan(9999))));
    }

    #[test]
    fn test_parse_chan_command_invalid_value() {
        let result = parse_artisan_command("CHAN;abc");
        assert!(matches!(result, Err(ParseError::InvalidValue)));
    }

    #[test]
    fn test_parse_units_command_celsius() {
        let result = parse_artisan_command("UNITS;C");
        assert!(matches!(result, Ok(ArtisanCommand::Units(false))));
    }

    #[test]
    fn test_parse_units_command_fahrenheit() {
        let result = parse_artisan_command("UNITS;F");
        assert!(matches!(result, Ok(ArtisanCommand::Units(true))));
    }

    #[test]
    fn test_parse_units_command_lowercase() {
        let result = parse_artisan_command("units;f");
        assert!(matches!(result, Ok(ArtisanCommand::Units(true))));
    }

    #[test]
    fn test_parse_units_command_invalid() {
        let result = parse_artisan_command("UNITS;K");
        assert!(matches!(result, Err(ParseError::InvalidValue)));
    }

    #[test]
    fn test_parse_filt_command() {
        let result = parse_artisan_command("FILT;5");
        assert!(matches!(result, Ok(ArtisanCommand::Filt(5))));
    }

    #[test]
    fn test_parse_filt_command_lowercase() {
        let result = parse_artisan_command("filt;3");
        assert!(matches!(result, Ok(ArtisanCommand::Filt(3))));
    }

    #[test]
    fn test_parse_filt_command_non_numeric_falls_back_to_zero() {
        // Non-numeric FILT values gracefully fall back to 0
        let result = parse_artisan_command("FILT;abc");
        assert!(matches!(result, Ok(ArtisanCommand::Filt(0))));
    }

    #[test]
    fn test_parse_filt_command_multi_value() {
        // Artisan sends comma-separated filter values: "FILT;70,70,70,70"
        let result = parse_artisan_command("FILT;70,70,70,70");
        assert!(matches!(result, Ok(ArtisanCommand::Filt(70))));
    }

    #[test]
    fn test_parse_filt_command_multi_value_extracts_first() {
        // Only the first comma-separated value matters
        let result = parse_artisan_command("FILT;80,90,100,110");
        assert!(matches!(result, Ok(ArtisanCommand::Filt(80))));
    }

    #[test]
    fn test_parse_filt_command_with_whitespace() {
        let result = parse_artisan_command("FILT; 7 ");
        assert!(matches!(result, Ok(ArtisanCommand::Filt(7))));
    }

    #[test]
    fn test_parse_chan_unknown_command() {
        let result = parse_artisan_command("UNKNOWN;123");
        assert!(matches!(result, Err(ParseError::UnknownCommand)));
    }

    #[test]
    fn test_semicolon_with_space_fallback() {
        let result = parse_artisan_command("CHAN;1200");
        assert!(matches!(result, Ok(ArtisanCommand::Chan(1200))));

        let result = parse_artisan_command("OT1 75");
        assert!(matches!(result, Ok(ArtisanCommand::SetHeater(75))));
    }

    #[test]
    fn test_parse_up_command() {
        let result = parse_artisan_command("UP");
        assert!(matches!(result, Ok(ArtisanCommand::IncreaseHeater)));
    }

    #[test]
    fn test_parse_up_command_case_insensitive() {
        let result = parse_artisan_command("up");
        assert!(matches!(result, Ok(ArtisanCommand::IncreaseHeater)));
    }

    #[test]
    fn test_parse_down_command() {
        let result = parse_artisan_command("DOWN");
        assert!(matches!(result, Ok(ArtisanCommand::DecreaseHeater)));
    }

    #[test]
    fn test_parse_down_command_case_insensitive() {
        let result = parse_artisan_command("down");
        assert!(matches!(result, Ok(ArtisanCommand::DecreaseHeater)));
    }

    #[test]
    fn test_empty_command_returns_empty_command_error() {
        let result = parse_artisan_command("");
        assert!(matches!(result, Err(ParseError::EmptyCommand)));
    }

    /// TEST-18-05b: Verify whitespace-only command returns EmptyCommand error
    #[test]
    fn test_whitespace_command_returns_empty_command_error() {
        let result = parse_artisan_command("   ");
        assert!(matches!(result, Err(ParseError::EmptyCommand)));
    }

    #[test]
    fn test_partial_ot1_command_returns_invalid_value() {
        let result = parse_artisan_command("OT1");
        assert!(matches!(result, Err(ParseError::InvalidValue)));
    }

    #[test]
    fn test_partial_io3_command_returns_invalid_value() {
        let result = parse_artisan_command("IO3");
        assert!(matches!(result, Err(ParseError::InvalidValue)));
    }

    #[test]
    fn test_extra_whitespace_handled_correctly() {
        let result = parse_artisan_command("OT1  50");
        assert!(matches!(result, Ok(ArtisanCommand::SetHeater(50))));
    }

    #[test]
    fn test_parse_ot1_zero_value() {
        let result = parse_artisan_command("OT1 0");
        assert!(matches!(result, Ok(ArtisanCommand::SetHeater(0))));
    }

    #[test]
    fn test_parse_ot1_max_value() {
        let result = parse_artisan_command("OT1 100");
        assert!(matches!(result, Ok(ArtisanCommand::SetHeater(100))));
    }

    #[test]
    fn test_parse_ot1_out_of_range() {
        let result = parse_artisan_command("OT1 150");
        assert!(matches!(result, Err(ParseError::OutOfRange)));
    }

    #[test]
    fn test_parse_io3_zero_value() {
        let result = parse_artisan_command("IO3 0");
        assert!(matches!(result, Ok(ArtisanCommand::SetFan(0))));
    }

    #[test]
    fn test_parse_io3_max_value() {
        let result = parse_artisan_command("IO3 100");
        assert!(matches!(result, Ok(ArtisanCommand::SetFan(100))));
    }

    #[test]
    fn test_parse_io3_out_of_range() {
        let result = parse_artisan_command("IO3 150");
        assert!(matches!(result, Err(ParseError::OutOfRange)));
    }

    // OT2 Command Tests

    #[test]
    fn test_parse_ot2_command_basic() {
        let result = parse_artisan_command("OT2 75");
        assert!(matches!(result, Ok(ArtisanCommand::SetFanSpeed(75, false))));
    }

    #[test]
    fn test_parse_ot2_command_lowercase() {
        let result = parse_artisan_command("ot2 50");
        assert!(matches!(result, Ok(ArtisanCommand::SetFanSpeed(50, false))));
    }

    #[test]
    fn test_parse_ot2_decimal_rounds_up() {
        let result = parse_artisan_command("OT2 50.5");
        assert!(matches!(result, Ok(ArtisanCommand::SetFanSpeed(51, false))));
    }

    #[test]
    fn test_parse_ot2_decimal_rounds_down() {
        let result = parse_artisan_command("OT2 50.4");
        assert!(matches!(result, Ok(ArtisanCommand::SetFanSpeed(50, false))));
    }

    #[test]
    fn test_parse_ot2_clamped_above_max() {
        let result = parse_artisan_command("OT2 150");
        assert!(matches!(result, Ok(ArtisanCommand::SetFanSpeed(100, true))));
    }

    #[test]
    fn test_parse_ot2_clamped_negative() {
        let result = parse_artisan_command("OT2 -5");
        assert!(matches!(result, Ok(ArtisanCommand::SetFanSpeed(0, true))));
    }

    #[test]
    fn test_parse_ot2_zero() {
        let result = parse_artisan_command("OT2 0");
        assert!(matches!(result, Ok(ArtisanCommand::SetFanSpeed(0, false))));
    }

    #[test]
    fn test_parse_ot2_max() {
        let result = parse_artisan_command("OT2 100");
        assert!(matches!(
            result,
            Ok(ArtisanCommand::SetFanSpeed(100, false))
        ));
    }

    #[test]
    fn test_parse_ot2_invalid_value() {
        let result = parse_artisan_command("OT2 abc");
        assert!(matches!(result, Err(ParseError::InvalidValue)));
    }

    #[test]
    fn test_parse_ot2_partial_command() {
        let result = parse_artisan_command("OT2");
        assert!(matches!(result, Err(ParseError::InvalidValue)));
    }

    #[test]
    fn test_parse_pidgain_command() {
        let result = parse_artisan_command("PIDGAIN 2.0 0.25 0.05");
        assert!(matches!(result, Ok(ArtisanCommand::SetPidGain(2.0, 0.25, 0.05))));
    }

    #[test]
    fn test_parse_pidgain_case_insensitive() {
        let result = parse_artisan_command("pidgain 1.5 0.3 0.1");
        assert!(matches!(result, Ok(ArtisanCommand::SetPidGain(1.5, 0.3, 0.1))));
    }

    #[test]
    fn test_parse_pidgain_invalid_value() {
        let result = parse_artisan_command("PIDGAIN abc 0.25 0.05");
        assert!(matches!(result, Err(ParseError::InvalidValue)));
    }

    #[test]
    fn test_parse_pidgain_partial() {
        let result = parse_artisan_command("PIDGAIN 2.0 0.25");
        assert!(matches!(result, Err(ParseError::InvalidValue)));
    }

    #[test]
    fn test_parse_settarget_command() {
        let result = parse_artisan_command("SETTARGET 200");
        assert!(matches!(result, Ok(ArtisanCommand::SetTargetTemp(200.0))));
    }

    #[test]
    fn test_parse_settarget_decimal() {
        let result = parse_artisan_command("SETTARGET 210.5");
        assert!(matches!(result, Ok(ArtisanCommand::SetTargetTemp(210.5))));
    }

    #[test]
    fn test_parse_settarget_out_of_range() {
        let result = parse_artisan_command("SETTARGET 350");
        assert!(matches!(result, Err(ParseError::OutOfRange)));
    }

    #[test]
    fn test_parse_settarget_too_low() {
        let result = parse_artisan_command("SETTARGET 40");
        assert!(matches!(result, Err(ParseError::OutOfRange)));
    }

    // ── PREHEAT command edge cases ────────────

    #[test]
    fn test_preheat_basic() {
        assert!(matches!(parse_artisan_command("PREHEAT 180"), Ok(ArtisanCommand::Preheat(180.0))));
    }

    #[test]
    fn test_preheat_decimal() {
        assert!(matches!(parse_artisan_command("PREHEAT 210.5"), Ok(ArtisanCommand::Preheat(210.5))));
    }

    #[test]
    fn test_preheat_min() {
        assert!(matches!(parse_artisan_command("PREHEAT 50"), Ok(ArtisanCommand::Preheat(50.0))));
    }

    #[test]
    fn test_preheat_max() {
        assert!(matches!(parse_artisan_command("PREHEAT 300"), Ok(ArtisanCommand::Preheat(300.0))));
    }

    #[test]
    fn test_preheat_too_low() {
        assert!(matches!(parse_artisan_command("PREHEAT 40"), Err(ParseError::OutOfRange)));
    }

    #[test]
    fn test_preheat_too_high() {
        assert!(matches!(parse_artisan_command("PREHEAT 350"), Err(ParseError::OutOfRange)));
    }

    #[test]
    fn test_preheat_no_value() {
        assert!(matches!(parse_artisan_command("PREHEAT"), Err(ParseError::InvalidValue)));
    }

    #[test]
    fn test_preheat_invalid() {
        assert!(matches!(parse_artisan_command("PREHEAT abc"), Err(ParseError::InvalidValue)));
    }

    // ── FANPROFILE command edge cases ──────────

    #[test]
    fn test_fanprofile_basic() {
        assert!(matches!(parse_artisan_command("FANPROFILE;0,20;60,50;120,100"),
            Ok(ArtisanCommand::SetFanProfile)));
    }

    #[test]
    fn test_fanprofile_single_setpoint() {
        assert!(matches!(parse_artisan_command("FANPROFILE;0,30"), Ok(ArtisanCommand::SetFanProfile)));
    }

    #[test]
    fn test_fanprofile_empty() {
        assert!(matches!(parse_artisan_command("FANPROFILE;"), Err(ParseError::EmptyCommand)));
    }

    #[test]
    fn test_fanprofile_out_of_range() {
        assert!(matches!(parse_artisan_command("FANPROFILE;0,150"), Err(ParseError::OutOfRange)));
    }

    #[test]
    fn test_fanprofile_invalid_format() {
        assert!(matches!(parse_artisan_command("FANPROFILE;abc,def"), Err(ParseError::InvalidValue)));
    }

    // ── TC4 PID commands ──────────────────────

    #[test]
    fn test_pid_on_maps_to_start() {
        assert!(matches!(parse_artisan_command("PID,ON"), Ok(ArtisanCommand::StartRoast)));
    }

    #[test]
    fn test_pid_off_maps_to_stop() {
        assert!(matches!(parse_artisan_command("PID,OFF"), Ok(ArtisanCommand::EmergencyStop)));
    }

    #[test]
    fn test_pid_sv_maps_to_settarget() {
        assert!(matches!(parse_artisan_command("PID,SV,150"), Ok(ArtisanCommand::SetTargetTemp(150.0))));
        assert!(matches!(parse_artisan_command("PID,SV,210.5"), Ok(ArtisanCommand::SetTargetTemp(210.5))));
    }

    #[test]
    fn test_pid_sv_case_insensitive() {
        assert!(matches!(parse_artisan_command("pid,sv,200"), Ok(ArtisanCommand::SetTargetTemp(200.0))));
    }

    #[test]
    fn test_pid_sv_out_of_range() {
        assert!(matches!(parse_artisan_command("PID,SV,40"), Err(ParseError::OutOfRange)));
        assert!(matches!(parse_artisan_command("PID,SV,350"), Err(ParseError::OutOfRange)));
    }

    // ── PID semicolon command tests ──────────

    #[test]
    fn test_pid_semicolon_on() {
        assert!(matches!(parse_artisan_command("PID;ON"), Ok(ArtisanCommand::StartRoast)));
    }

    #[test]
    fn test_pid_semicolon_off() {
        assert!(matches!(parse_artisan_command("PID;OFF"), Ok(ArtisanCommand::EmergencyStop)));
    }

    #[test]
    fn test_pid_semicolon_sv() {
        assert!(matches!(
            parse_artisan_command("PID;SV;150"),
            Ok(ArtisanCommand::SetTargetTemp(v)) if (v - 150.0).abs() < f32::EPSILON
        ));
    }

    #[test]
    fn test_pid_semicolon_sv_decimal() {
        assert!(matches!(
            parse_artisan_command("PID;SV;210.5"),
            Ok(ArtisanCommand::SetTargetTemp(v)) if (v - 210.5).abs() < f32::EPSILON
        ));
    }

    #[test]
    fn test_pid_semicolon_sv_out_of_range() {
        assert!(matches!(parse_artisan_command("PID;SV;40"), Err(ParseError::OutOfRange)));
    }

    #[test]
    fn test_pid_semicolon_t() {
        let result = parse_artisan_command("PID;T;2.0;0.5;1.0");
        assert!(matches!(result, Ok(ArtisanCommand::SetPidGain(kp, ki, kd))
            if (kp - 2.0).abs() < f32::EPSILON && (ki - 0.5).abs() < f32::EPSILON && (kd - 1.0).abs() < f32::EPSILON
        ));
    }

    #[test]
    fn test_pid_semicolon_t_invalid() {
        assert!(matches!(parse_artisan_command("PID;T;abc"), Err(ParseError::InvalidValue)));
    }

    #[test]
    fn test_pid_semicolon_t_negative() {
        assert!(matches!(parse_artisan_command("PID;T;-1;0.5;1.0"), Err(ParseError::OutOfRange)));
    }

    #[test]
    fn test_pid_semicolon_chan() {
        assert!(matches!(parse_artisan_command("PID;CHAN;2"), Ok(ArtisanCommand::SetPidChannel(2))));
    }

    #[test]
    fn test_pid_semicolon_chan_et() {
        assert!(matches!(parse_artisan_command("PID;CHAN;1"), Ok(ArtisanCommand::SetPidChannel(1))));
    }

    #[test]
    fn test_pid_semicolon_chan_invalid() {
        assert!(matches!(parse_artisan_command("PID;CHAN;5"), Err(ParseError::OutOfRange)));
    }

    #[test]
    fn test_pid_semicolon_ct() {
        assert!(matches!(parse_artisan_command("PID;CT;1000"), Ok(ArtisanCommand::SetPidCycleTime(1000))));
    }

    #[test]
    fn test_pid_semicolon_ct_too_low() {
        assert!(matches!(parse_artisan_command("PID;CT;5"), Err(ParseError::OutOfRange)));
    }

    #[test]
    fn test_pid_semicolon_limit() {
        assert!(matches!(
            parse_artisan_command("PID;LIMIT;0;100"),
            Ok(ArtisanCommand::SetPidOutputLimits(min, max)) if min == 0.0 && max == 100.0
        ));
    }

    #[test]
    fn test_pid_semicolon_limit_custom() {
        assert!(matches!(
            parse_artisan_command("PID;LIMIT;20;80"),
            Ok(ArtisanCommand::SetPidOutputLimits(min, max)) if min == 20.0 && max == 80.0
        ));
    }

    #[test]
    fn test_pid_semicolon_unknown_sub() {
        assert!(matches!(parse_artisan_command("PID;UNKNOWN"), Err(ParseError::UnknownCommand)));
    }

    #[test]
    fn test_pid_comma_still_works_on() {
        assert!(matches!(parse_artisan_command("PID,ON"), Ok(ArtisanCommand::StartRoast)));
    }

    #[test]
    fn test_pid_comma_still_works_off() {
        assert!(matches!(parse_artisan_command("PID,OFF"), Ok(ArtisanCommand::EmergencyStop)));
    }
}
