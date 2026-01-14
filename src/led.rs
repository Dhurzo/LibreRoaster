use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use esp_idf_hal::gpio::{Gpio2, PinDriver, Output, Level};
use log::{debug, error, info};

use crate::error::AppError;

#[derive(Clone)]
pub struct LedController {
    pin: Arc<PinDriver<'static, Gpio2, Output>>,
    state: Arc<AtomicBool>,
}

impl LedController {
    pub fn new(pin: Gpio2) -> Result<Self, AppError> {
        let driver = PinDriver::output(pin)?;
        
        Ok(Self {
            pin: Arc::new(driver),
            state: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn set_state(&self, state: bool) -> Result<(), AppError> {
        debug!("Setting LED state to: {}", state);
        
        // NodeMCU ESP32 built-in LED is active low (GPIO2)
        let level = if state { Level::Low } else { Level::High };
        
        self.pin.set_level(level)?;
        self.state.store(state, Ordering::Relaxed);
        
        Ok(())
    }

    pub fn get_state(&self) -> bool {
        self.state.load(Ordering::Relaxed)
    }

    pub fn toggle(&self) -> Result<(), AppError> {
        let current_state = self.get_state();
        self.set_state(!current_state)
    }

    pub fn blink(&self, count: u32, on_ms: u64, off_ms: u64) -> Result<(), AppError> {
        info!("Blinking LED {} times", count);
        
        for _ in 0..count {
            self.set_state(true)?;
            std::thread::sleep(std::time::Duration::from_millis(on_ms));
            self.set_state(false)?;
            std::thread::sleep(std::time::Duration::from_millis(off_ms));
        }
        
        Ok(())
    }

    pub fn blink_pattern(&self, pattern: &[bool], duration_ms: u64) -> Result<(), AppError> {
        info!("Blinking LED with pattern: {:?}", pattern);
        
        for &state in pattern {
            self.set_state(state)?;
            std::thread::sleep(std::time::Duration::from_millis(duration_ms));
        }
        
        Ok(())
    }

    pub fn set_brightness(&self, brightness: f32) -> Result<(), AppError> {
        // Simple PWM simulation using rapid blinking for NodeMCU LED
        let clamped_brightness = brightness.clamp(0.0, 1.0);
        
        if clamped_brightness == 0.0 {
            return self.set_state(false);
        } else if clamped_brightness >= 1.0 {
            return self.set_state(true);
        }
        
        // Simulate PWM with 10ms period
        let on_time = (clamped_brightness * 10.0) as u64;
        let off_time = 10 - on_time;
        
        for _ in 0..10 {
            if on_time > 0 {
                self.set_state(true)?;
                std::thread::sleep(std::time::Duration::from_millis(on_time));
            }
            if off_time > 0 {
                self.set_state(false)?;
                std::thread::sleep(std::time::Duration::from_millis(off_time));
            }
        }
        
        Ok(())
    }

    pub async fn async_blink(&self, count: u32, on_ms: u64, off_ms: u64) -> Result<(), AppError> {
        info!("Async blinking LED {} times", count);
        
        for _ in 0..count {
            self.set_state(true)?;
            tokio::time::sleep(std::time::Duration::from_millis(on_ms)).await;
            self.set_state(false)?;
            tokio::time::sleep(std::time::Duration::from_millis(off_ms)).await;
        }
        
        Ok(())
    }

    pub fn get_status(&self) -> serde_json::Value {
        serde_json::json!({
            "state": self.get_state(),
            "gpio": 2,
            "active_low": true,
            "controller": "NodeMCU-LED"
        })
    }
}