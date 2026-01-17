#[derive(Debug)]
pub enum FanError {
    InitializationError,
    InvalidSpeed,
    PwmError,
}

pub struct FanController {
    current_speed: f32,
}

impl FanController {
    pub fn new(_io: ()) -> Result<Self, FanError> {
        Ok(Self { current_speed: 0.0 })
    }

    pub fn set_speed(&mut self, speed_percent: f32) -> Result<(), FanError> {
        self.current_speed = speed_percent.clamp(0.0, 100.0);
        log::info!("Fan speed set to: {}%", self.current_speed);
        Ok(())
    }

    pub fn get_speed(&self) -> f32 {
        self.current_speed
    }

    pub fn enable(&mut self) {
        self.current_speed = 100.0;
        log::info!("Fan enabled");
    }

    pub fn disable(&mut self) {
        self.current_speed = 0.0;
        log::info!("Fan disabled");
    }

    pub fn is_enabled(&self) -> bool {
        self.current_speed > 0.0
    }
}
