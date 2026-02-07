use crate::config::ArtisanCommand;

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
    if let Some((cmd, args)) = trimmed.split_once(';') {
        return match cmd.to_ascii_uppercase().as_str() {
            "CHAN" => args
                .trim()
                .parse::<u16>()
                .map(ArtisanCommand::Chan)
                .map_err(|_| ParseError::InvalidValue),
            "UNITS" => match args.trim() {
                "C" | "c" => Ok(ArtisanCommand::Units(false)),
                "F" | "f" => Ok(ArtisanCommand::Units(true)),
                _ => Err(ParseError::InvalidValue),
            },
            "FILT" => args
                .trim()
                .parse::<u8>()
                .map(ArtisanCommand::Filt)
                .map_err(|_| ParseError::InvalidValue),
            _ => Err(ParseError::UnknownCommand),
        };
    }

    // Fall back to space delimiter for operational commands
    let parts: heapless::Vec<&str, 4> = trimmed.split_whitespace().collect();

    match parts.as_slice() {
        ["READ"] => Ok(ArtisanCommand::ReadStatus),

        ["START"] => Ok(ArtisanCommand::StartRoast),

        ["OT1", value_str] => {
            let value = parse_percentage(value_str)?;
            Ok(ArtisanCommand::SetHeater(value))
        }

        ["IO3", value_str] => {
            let value = parse_percentage(value_str)?;
            Ok(ArtisanCommand::SetFan(value))
        }

        ["OT2" | "ot2"] => Ok(ArtisanCommand::SetFanSpeed(0, false)),

        ["OT2" | "ot2", value_str] => {
            let (value, was_clamped) = parse_ot2_value(value_str)?;
            Ok(ArtisanCommand::SetFanSpeed(value, was_clamped))
        }

        ["STOP"] => Ok(ArtisanCommand::EmergencyStop),

        ["UP" | "up"] => Ok(ArtisanCommand::IncreaseHeater),

        ["DOWN" | "down"] => Ok(ArtisanCommand::DecreaseHeater),

        // Partial commands (commands that require a value but don't have one)
        ["OT1"] | ["IO3"] => Err(ParseError::InvalidValue),

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

/// Parse OT2 fan speed value with decimal support
/// - Decimals are rounded to nearest integer
/// - Values are silently clamped to 0-100 range
/// - Negative values clamp to 0
/// - Returns (clamped_value, was_clamped)
fn parse_ot2_value(value_str: &str) -> Result<(u8, bool), ParseError> {
    let value = value_str
        .parse::<f32>()
        .map_err(|_| ParseError::InvalidValue)?;

    let was_clamped = value < 0.0 || value > 100.0;

    // Round to nearest integer (0.5 rounds up)
    let rounded = if value >= 0.0 {
        (value + 0.5) as i32
    } else {
        (value - 0.5) as i32
    };

    let clamped = rounded.clamp(0, 100) as u8;
    Ok((clamped, was_clamped))
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
    fn test_parse_filt_command_invalid_value() {
        let result = parse_artisan_command("FILT;abc");
        assert!(matches!(result, Err(ParseError::InvalidValue)));
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
}
