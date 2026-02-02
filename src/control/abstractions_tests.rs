#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::*;
    use crate::output::traits::OutputError;

    struct MockPidController {
        enabled: bool,
        target: f32,
    }

    impl MockPidController {
        fn new() -> Self {
            Self {
                enabled: false,
                target: DEFAULT_TARGET_TEMP,
            }
        }
    }

    impl PidController for MockPidController {
        type Error = PidError;

        fn set_target(&mut self, target: f32) -> Result<(), Self::Error> {
            self.target = target;
            Ok(())
        }

        fn enable(&mut self) {
            self.enabled = true;
        }

        fn disable(&mut self) {
            self.enabled = false;
        }

        fn compute_output(&mut self, _current_temp: f32, _current_time: u32) -> f32 {
            if self.enabled {
                50.0
            } else {
                0.0
            }
        }

        fn is_enabled(&self) -> bool {
            self.enabled
        }

        fn get_target(&self) -> f32 {
            self.target
        }
    }

    struct MockOutputManager {
        continuous_enabled: bool,
        process_called: bool,
    }

    impl MockOutputManager {
        fn new() -> Self {
            Self {
                continuous_enabled: false,
                process_called: false,
            }
        }
    }

    impl OutputManager for MockOutputManager {
        type Error = OutputError;

        async fn process_status(&mut self, _status: &SystemStatus) -> Result<(), Self::Error> {
            self.process_called = true;
            Ok(())
        }

        fn reset(&mut self) {
            self.continuous_enabled = false;
        }

        fn enable_continuous_output(&mut self) {
            self.continuous_enabled = true;
        }

        fn disable_continuous_output(&mut self) {
            self.continuous_enabled = false;
        }

        fn is_continuous_enabled(&self) -> bool {
            self.continuous_enabled
        }
    }

    #[test]
    fn test_safety_command_handler_priority() {
        let mut handler = SafetyCommandHandler::new();

        let mut status = SystemStatus::default();
        let current_time = embassy_time::Instant::now();

        let result = handler.handle_command(
            RoasterCommand::EmergencyStop,
            current_time,
            &mut status,
        );

        assert!(result.is_err());
        assert!(handler.is_emergency_active());
        assert!(status.fault_condition);
        assert_eq!(status.ssr_output, 0.0);
        assert!(!status.pid_enabled);
    }

    #[test]
    fn test_artisan_command_handler() {
        let mut handler = ArtisanCommandHandler::new();

        let mut status = SystemStatus::default();
        let current_time = embassy_time::Instant::now();

        let result = handler.handle_command(
            RoasterCommand::SetHeaterManual(80),
            current_time,
            &mut status,
        );

        assert!(result.is_ok());
        assert_eq!(handler.get_manual_heater(), 80.0);
        assert!(status.artisan_control);

        let result = handler.handle_command(
            RoasterCommand::SetFanManual(60),
            current_time,
            &mut status,
        );

        assert!(result.is_ok());
        assert_eq!(handler.get_manual_fan(), 60.0);
        assert_eq!(status.fan_output, 60.0);
    }

    #[test]
    fn test_system_command_handler() {
        let mut handler = SystemCommandHandler;

        let mut status = SystemStatus {
            state: RoasterState::Heating,
            bean_temp: 150.0,
            env_temp: 100.0,
            ..Default::default()
        };
        let current_time = embassy_time::Instant::now();

        let result = handler.handle_command(RoasterCommand::Reset, current_time, &mut status);

        assert!(result.is_ok());
        assert_eq!(status.state, RoasterState::Idle);
        assert_eq!(status.bean_temp, 0.0);
        assert_eq!(status.env_temp, 0.0);
    }

    #[test]
    fn test_mock_output_manager() {
        let mut output = MockOutputManager::new();

        assert!(!output.is_continuous_enabled());

        output.enable_continuous_output();
        assert!(output.is_continuous_enabled());

        output.process_called = false;
        let status = SystemStatus::default();

        output.process_called = true;

        assert!(output.process_called);

        output.disable_continuous_output();
        assert!(!output.is_continuous_enabled());
    }
}
