// Temporary stub for PID controller
// This will be replaced with the actual implementation

pub struct CoffeeRoasterPid {
    enabled: bool,
    target: f32,
}

#[derive(Debug)]
pub enum PidError {
    InitializationError,
    ComputationError,
}

impl CoffeeRoasterPid {
    pub fn new() -> Result<Self, PidError> {
        Ok(Self {
            enabled: false,
            target: 0.0,
        })
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_target(&mut self, target: f32) -> Result<(), PidError> {
        self.target = target;
        Ok(())
    }

    pub fn compute_output(&mut self, current_temp: f32, _timestamp: u32) -> f32 {
        if !self.enabled {
            return 0.0;
        }

        let error = self.target - current_temp;
        (error * 2.0).clamp(0.0, 100.0)
    }
}
