use crate::config::ArtisanCommand;

#[derive(Debug)]
pub enum ParseError {
    InvalidCommand,
    InvalidValue,
    EmptyCommand,
}

pub fn parse_artisan_command(command: &str) -> Result<ArtisanCommand, ParseError> {
    let trimmed = command.trim();

    if trimmed.is_empty() {
        return Err(ParseError::EmptyCommand);
    }

    let parts: heapless::Vec<&str, 4> = trimmed.split_whitespace().collect();

    match parts.as_slice() {
        ["READ"] => Ok(ArtisanCommand::ReadStatus),

        ["START"] => Ok(ArtisanCommand::StartRoast),

        ["OT1", value_str] => {
            let value = parse_percentage(value_str)?;
            if value <= 100 {
                Ok(ArtisanCommand::SetHeater(value))
            } else {
                Err(ParseError::InvalidValue)
            }
        }

        ["IO3", value_str] => {
            let value = parse_percentage(value_str)?;
            if value <= 100 {
                Ok(ArtisanCommand::SetFan(value))
            } else {
                Err(ParseError::InvalidValue)
            }
        }

        ["STOP"] => Ok(ArtisanCommand::EmergencyStop),

        _ => Err(ParseError::InvalidCommand),
    }
}

fn parse_percentage(value_str: &str) -> Result<u8, ParseError> {
    value_str
        .parse::<u8>()
        .map_err(|_| ParseError::InvalidValue)
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
        assert!(matches!(result, Err(ParseError::InvalidCommand)));
    }

    #[test]
    fn test_invalid_value() {
        let result = parse_artisan_command("OT1 150");
        assert!(matches!(result, Err(ParseError::InvalidValue)));
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
        assert!(matches!(result, Err(ParseError::InvalidValue)));
    }
}
