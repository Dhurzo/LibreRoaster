use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub wifi: WifiConfig,
    pub server: ServerConfig,
    pub device: DeviceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WifiConfig {
    pub ssid: String,
    pub password: String,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
    pub max_connections: usize,
    pub enable_cors: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
    pub name: String,
    pub led_gpio: u32,
    pub log_level: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            wifi: WifiConfig {
                ssid: "YOUR_WIFI_SSID".to_string(),
                password: "YOUR_WIFI_PASSWORD".to_string(),
                max_retries: 5,
                retry_delay_ms: 5000,
            },
            server: ServerConfig {
                port: 80,
                max_connections: 10,
                enable_cors: true,
            },
            device: DeviceConfig {
                name: "ESP32-NodeMCU".to_string(),
                led_gpio: 2,
                log_level: "info".to_string(),
            },
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = "/spiffs/config.json";
        
        if Path::new(config_path).exists() {
            let config_str = fs::read_to_string(config_path)?;
            let config: Config = serde_json::from_str(&config_str)?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let config_path = "/spiffs/config.json";
        let config_str = serde_json::to_string_pretty(self)?;
        fs::write(config_path, config_str)?;
        Ok(())
    }

    pub fn get_device_info(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.device.name,
            "version": env!("CARGO_PKG_VERSION"),
            "build_time": env!("VERGEN_BUILD_TIMESTAMP"),
            "led_gpio": self.device.led_gpio,
            "log_level": self.device.log_level
        })
    }
}