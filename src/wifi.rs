use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use embedded_svc::wifi::{ClientConfiguration, Configuration, Wifi, AuthMethod};
use esp_idf_hal::modem::Modem;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{BlockingWifi, EspWifi};
use esp_idf_sys::EspError;
use log::{debug, error, info, warn};

use crate::config::Config;
use crate::error::AppError;

pub type WifiStack = Arc<EspWifi<'static>>;

pub async fn initialize(
    modem: Modem,
    sys_loop: EspSystemEventLoop,
    nvs: EspDefaultNvsPartition,
    config: &Config,
) -> Result<(WifiStack, Arc<BlockingWifi<EspWifi<'static>>>), AppError> {
    info!("Initializing WiFi...");

    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(modem, sys_loop.clone(), Some(nvs))?,
        sys_loop.clone(),
    );

    // Configure WiFi
    let wifi_config = Configuration::Client(ClientConfiguration {
        ssid: config.wifi.ssid.as_str().try_into().map_err(|e| {
            AppError::wifi(format!("Invalid SSID: {}", e))
        })?,
        password: config.wifi.password.as_str().try_into().map_err(|e| {
            AppError::wifi(format!("Invalid password: {}", e))
        })?,
        auth_method: AuthMethod::WPA2Personal,
        ..Default::default()
    });

    wifi.set_configuration(&wifi_config)?;

    // Start WiFi
    wifi.start()?;
    info!("WiFi started");

    // Connect to network with retry logic
    let mut retry_count = 0;
    let max_retries = config.wifi.max_retries;
    let retry_delay = Duration::from_millis(config.wifi.retry_delay_ms);

    while retry_count < max_retries {
        match wifi.connect() {
            Ok(_) => {
                info!("Connected to WiFi network: {}", config.wifi.ssid);
                break;
            }
            Err(e) => {
                warn!("WiFi connection attempt {} failed: {:?}", retry_count + 1, e);
                retry_count += 1;
                
                if retry_count < max_retries {
                    info!("Retrying in {}ms...", retry_delay.as_millis());
                    tokio::time::sleep(retry_delay).await;
                }
            }
        }
    }

    if retry_count >= max_retries {
        return Err(AppError::wifi(format!(
            "Failed to connect after {} attempts",
            max_retries
        )));
    }

    // Wait for IP configuration
    wifi.wait_netif_up()?;
    let ip_info = wifi.wifi().sta_netif().get_ip_info()?;
    info!("WiFi connected. IP: {}", ip_info.ip);

    let wifi_stack = Arc::new(wifi.into_wifi());
    let blocking_wifi = Arc::new(BlockingWifi::wrap(
        (*wifi_stack).clone(),
        sys_loop,
    ));

    // Start WiFi monitoring task
    start_wifi_monitor(wifi_stack.clone(), config.clone()).await;

    Ok((wifi_stack, blocking_wifi))
}

async fn start_wifi_monitor(
    wifi_stack: WifiStack,
    config: Config,
) {
    tokio::spawn(async move {
        let mut connection_lost = false;
        
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            
            // Check WiFi status
            match wifi_stack.wifi().sta_netif().get_ip_info() {
                Ok(ip_info) => {
                    if ip_info.ip.is_unspecified() {
                        if !connection_lost {
                            warn!("WiFi connection lost");
                            connection_lost = true;
                        }
                        
                        // Try to reconnect
                        for attempt in 1..=config.wifi.max_retries {
                            debug!("Reconnection attempt {}", attempt);
                            
                            match wifi_stack.connect() {
                                Ok(_) => {
                                    info!("WiFi reconnected successfully");
                                    connection_lost = false;
                                    break;
                                }
                                Err(e) => {
                                    debug!("Reconnection attempt {} failed: {:?}", attempt, e);
                                    if attempt < config.wifi.max_retries {
                                        tokio::time::sleep(Duration::from_millis(config.wifi.retry_delay_ms)).await;
                                    }
                                }
                            }
                        }
                        
                        if connection_lost {
                            error!("Failed to reconnect WiFi after maximum retries");
                        }
                    } else {
                        if connection_lost {
                            info!("WiFi connection restored. IP: {}", ip_info.ip);
                            connection_lost = false;
                        }
                        debug!("WiFi status: IP: {}, Gateway: {}", ip_info.ip, ip_info.gateway);
                    }
                }
                Err(e) => {
                    error!("Failed to get WiFi status: {:?}", e);
                    connection_lost = true;
                }
            }
        }
    });
}

pub async fn scan_networks(wifi_stack: &WifiStack) -> Result<Vec<serde_json::Value>, AppError> {
    let scan_config = embedded_svc::wifi::ScanConfig {
        ssid: None,
        channel: None,
        bssid: None,
    };

    let networks = wifi_stack.wifi().scan_scan_sync(
        Some(scan_config),
        esp_idf_sys::wifi_scan_time_t_default(),
        false,
        embedded_svc::wifi::scan_type_t_WIFI_SCAN_TYPE_ACTIVE,
        3, // Scan 3 times for better reliability
        0,  // No specific channel
    )?;

    let mut result = Vec::new();
    for network in networks {
        result.push(serde_json::json!({
            "ssid": String::from_utf8_lossy(&network.ssid),
            "rssi": network.rssi,
            "channel": network.channel,
            "auth_mode": format!("{:?}", network.auth_method),
            "bssid": network.bssid.iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>().join(":"),
        }));
    }

    Ok(result)
}

pub fn get_wifi_status(wifi_stack: &WifiStack) -> Result<serde_json::Value, AppError> {
    let wifi = wifi_stack.wifi();
    let ip_info = wifi.sta_netif().get_ip_info()?;
    
    let status = match wifi.get_status()? {
        esp_idf_svc::wifi::WifiStatus::Started => "started",
        esp_idf_svc::wifi::WifiStatus::Stopped => "stopped",
        esp_idf_svc::wifi::WifiStatus::Connecting => "connecting",
        esp_idf_svc::wifi::WifiStatus::Connected => "connected",
        esp_idf_svc::wifi::WifiStatus::Disconnected => "disconnected",
    };

    Ok(serde_json::json!({
        "status": status,
        "ip": ip_info.ip.to_string(),
        "netmask": ip_info.netmask.to_string(),
        "gateway": ip_info.gateway.to_string(),
        "primary_dns": ip_info.dns.to_string(),
        "mode": "station",
        "connection_attempts": 1,
    }))
}