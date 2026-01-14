use std::net::IpAddr;
use std::sync::Arc;
use std::convert::Infallible;

use axum::{
    extract::State,
    http::{HeaderValue, Method, StatusCode},
    response::{Html, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
    trace::TraceLayer,
};
use log::{debug, error, info, warn};

use crate::led::LedController;
use crate::wifi::WifiStack;
use crate::error::AppError;
use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub led_controller: Arc<LedController>,
    pub wifi_stack: WifiStack,
    pub config: Arc<Config>,
}

#[derive(Debug, Deserialize)]
pub struct LedRequest {
    state: Option<bool>,
    blink_count: Option<u32>,
    blink_on_ms: Option<u64>,
    blink_off_ms: Option<u64>,
    brightness: Option<f32>,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
        }
    }
}

pub async fn start_server(
    wifi_stack: WifiStack,
    led_controller: Arc<LedController>,
) -> Result<tokio::task::JoinHandle<()>, AppError> {
    let config = Config::default();
    let app_state = AppState {
        led_controller: led_controller.clone(),
        wifi_stack: wifi_stack.clone(),
        config: Arc::new(config),
    };

    // Get IP address
    let ip_info = wifi_stack.wifi().sta_netif().get_ip_info()?;
    let ip_address = ip_info.ip;

    // Create CORS layer
    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers(Any)
        .allow_origin(Any);

    // Create router
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/status", get(status_handler))
        .route("/api/led", get(led_status_handler))
        .route("/api/led", post(led_control_handler))
        .route("/api/wifi/status", get(wifi_status_handler))
        .route("/api/wifi/scan", get(wifi_scan_handler))
        .route("/api/system/info", get(system_info_handler))
        .route("/api/system/health", get(health_handler))
        .nest_service("/static", ServeDir::new("static"))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(cors),
        )
        .with_state(app_state);

    // Create server
    let listener = tokio::net::TcpListener::bind(format!("{}:80", ip_address)).await
        .map_err(|e| AppError::server(format!("Failed to bind to {}:80 - {}", ip_address, e)))?;

    info!("HTTP server started on http://{}:80", ip_address);

    let handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            error!("HTTP server error: {}", e);
        }
    });

    Ok(handle)
}

async fn index_handler() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

async fn status_handler(State(state): State<AppState>) -> Json<ApiResponse<serde_json::Value>> {
    let led_status = state.led_controller.get_status();
    let wifi_status = crate::wifi::get_wifi_status(&state.wifi_stack)
        .unwrap_or_else(|_| serde_json::json!({"error": "Failed to get WiFi status"}));
    
    let system_info = serde_json::json!({
        "uptime": get_uptime(),
        "free_heap": get_free_heap(),
        "device": state.config.get_device_info(),
    });

    let response = serde_json::json!({
        "led": led_status,
        "wifi": wifi_status,
        "system": system_info,
    });

    Json(ApiResponse::success(response))
}

async fn led_status_handler(State(state): State<AppState>) -> Json<ApiResponse<serde_json::Value>> {
    let status = state.led_controller.get_status();
    Json(ApiResponse::success(status))
}

async fn led_control_handler(
    State(state): State<AppState>,
    Json(request): Json<LedRequest>,
) -> Json<ApiResponse<String>> {
    debug!("LED control request: {:?}", request);

    if let Some(blink_count) = request.blink_count {
        let on_ms = request.blink_on_ms.unwrap_or(200);
        let off_ms = request.blink_off_ms.unwrap_or(200);
        
        if let Err(e) = state.led_controller.blink(blink_count, on_ms, off_ms) {
            error!("LED blink error: {}", e);
            return Json(ApiResponse::error(format!("LED blink failed: {}", e)));
        }
        
        return Json(ApiResponse::success("LED blinked successfully".to_string()));
    }

    if let Some(brightness) = request.brightness {
        if let Err(e) = state.led_controller.set_brightness(brightness) {
            error!("LED brightness error: {}", e);
            return Json(ApiResponse::error(format!("LED brightness failed: {}", e)));
        }
        
        return Json(ApiResponse::success("LED brightness set successfully".to_string()));
    }

    if let Some(led_state) = request.state {
        if let Err(e) = state.led_controller.set_state(led_state) {
            error!("LED state error: {}", e);
            return Json(ApiResponse::error(format!("LED state failed: {}", e)));
        }
        
        let state_str = if led_state { "on" } else { "off" };
        return Json(ApiResponse::success(format!("LED turned {}", state_str)));
    }

    Json(ApiResponse::error("No valid LED control parameter provided".to_string()))
}

async fn wifi_status_handler(State(state): State<AppState>) -> Json<ApiResponse<serde_json::Value>> {
    match crate::wifi::get_wifi_status(&state.wifi_stack) {
        Ok(status) => Json(ApiResponse::success(status)),
        Err(e) => {
            error!("WiFi status error: {}", e);
            Json(ApiResponse::error(format!("Failed to get WiFi status: {}", e)))
        }
    }
}

async fn wifi_scan_handler(State(state): State<AppState>) -> Json<ApiResponse<Vec<serde_json::Value>>> {
    match crate::wifi::scan_networks(&state.wifi_stack).await {
        Ok(networks) => Json(ApiResponse::success(networks)),
        Err(e) => {
            error!("WiFi scan error: {}", e);
            Json(ApiResponse::error(format!("WiFi scan failed: {}", e)))
        }
    }
}

async fn system_info_handler(State(state): State<AppState>) -> Json<ApiResponse<serde_json::Value>> {
    let info = serde_json::json!({
        "device": state.config.get_device_info(),
        "system": {
            "esp_idf_version": esp_idf_sys::ESP_IDF_VERSION_MAJOR,
            "rust_version": "1.75.0",
            "heap_free": get_free_heap(),
            "heap_min_free": get_min_free_heap(),
            "cpu_freq": esp_idf_hal::cpu::get_cpu_freq(),
        },
        "uptime": get_uptime(),
    });

    Json(ApiResponse::success(info))
}

async fn health_handler() -> Json<ApiResponse<String>> {
    Json(ApiResponse::success("OK".to_string()))
}

// Helper functions
fn get_uptime() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn get_free_heap() -> usize {
    unsafe { esp_idf_sys::esp_get_free_heap_size() as usize }
}

fn get_min_free_heap() -> usize {
    unsafe { esp_idf_sys::esp_get_minimum_free_heap_size() as usize }
}