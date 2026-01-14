# LibreRoaster - OpenSource Coffee Bean Roaster

LibreRoaster is an openSource coffee bean roaster compatible with Artisan+. Built with Rust and ESP32, featuring WiFi connectivity, HTTP server, sensor integration, and real-time monitoring capabilities.

## ⚠️ **Configuration Required Before Building**

This project requires local configuration. Copy the template and update with your settings:

```bash
cp config.template.json config.json
# Edit config.json with your WiFi credentials and device settings
```

## Features

- **Artisan+ Compatibility**: Full integration with Artisan+ coffee roasting software
- **Temperature Sensing**: High-precision temperature monitoring for bean and environment
- **WiFi Connectivity**: Automatic connection with reconnection logic and network scanning
- **HTTP Server**: RESTful API with CORS support and web interface
- **Real-time Monitoring**: Live temperature data streaming and roast profile tracking
- **System Monitoring**: Real-time device status, heap monitoring, and system information
- **OTA Updates**: Over-the-air firmware update capability
- **Structured Logging**: Comprehensive logging with configurable levels
- **Web Interface**: Responsive control panel with real-time roast monitoring

## Hardware Requirements

- NodeMCU ESP32 development board
- USB-C cable for power and programming
- WiFi network (2.4GHz)
- Temperature sensors (thermocouples)
- Solid-state relays for heating element control
- Cooling fan system
- Roasting chamber and bean agitator

## Software Requirements

- Rust nightly toolchain
- ESP-IDF v5.2.2
- cargo-espflash
- cargo-binutils

## Quick Start

### 1. Install Dependencies

```bash
# Install Rust nightly with ESP32 target
rustup toolchain install nightly-2024-04-1
rustup component add rust-src --toolchain nightly-2024-04-1

# Install ESP flashing tools
cargo install cargo-espflash
cargo install cargo-binutils
rustup component add llvm-tools-preview

# Install ESP-IDF (automated)
export ESP_IDF_TOOLS_INSTALL_DIR=global
export ESP_IDF_VERSION=v5.2.2
```

### 2. Configure WiFi

**Important**: Copy the template configuration first:

```bash
cp config.template.json config.json
```

Then edit `config.json` and update the WiFi credentials:

```json
{
  "wifi": {
    "ssid": "YOUR_WIFI_SSID",
    "password": "YOUR_WIFI_PASSWORD"
  }
}
```

### 3. Build and Flash

```bash
# Configure ESP-IDF path if needed (update in .cargo/config.toml)
# See .cargo/config.toml for LIBCLANG_PATH configuration

# Using the provided build script
./build.sh all        # Build all
./build.sh build      # Build firmware
./build.sh flash       # Build + flash
./build.sh monitor     # Serial monitor
./build.sh clean      # Clean build artifacts

# Or manual commands
cargo build --target xtensa-esp32-espidf --release
cargo-espflash --target xtensa-esp32-espidf --release
```

### 4. Flash Firmware (Working Solution)

**Use the manual flash script for working firmware:**

```bash
# Flash firmware and start serial monitor
./flash.sh

# Or flash only
./flash.sh flash

# Or monitor only
./flash.sh monitor
```

### 5. Monitor Serial Output

```bash
cargo-espflash monitor
```

## Web Interface

Once connected to WiFi, access the control panel at:
- **IP Address**: Check serial output for the assigned IP
- **Port**: 80 (HTTP)
- **URL**: `http://<esp32-ip-address>`

### Web Interface Features

- **Roast Dashboard**: Real-time temperature graphs and roast progress
- **Temperature Monitoring**: Live bean and environment temperature readings
- **Roaster Control**: Heating, cooling, and agitator controls
- **Profile Manager**: Upload and manage roast profiles
- **System Status**: Real-time monitoring of heap, uptime, CPU frequency
- **Safety Systems**: Temperature limits and emergency controls
- **WiFi Status**: Connection info, network scanning
- **Artisan+ Sync**: Export/import roast profiles with Artisan+

## API Endpoints

### Temperature Monitoring
- `GET /api/temperature` - Get current temperature readings
- `GET /api/temperature/history` - Get temperature history
- `POST /api/temperature/calibrate` - Calibrate temperature sensors

### Roaster Control
- `GET /api/roaster/status` - Get roaster system status
- `POST /api/roaster/heating` - Control heating element
- `POST /api/roaster/cooling` - Control cooling system
- `POST /api/roaster/agitator` - Control bean agitation

### Roast Management
- `GET /api/roast/status` - Get current roast status
- `POST /api/roast/start` - Start new roast
- `POST /api/roast/pause` - Pause current roast
- `POST /api/roast/stop` - Stop current roast
- `GET /api/roast/profiles` - Get available roast profiles
- `POST /api/roast/profiles` - Upload new roast profile

### Artisan+ Integration
- `GET /api/artisan/data` - Get Artisan+ compatible data stream
- `POST /api/artisan/profile` - Upload profile from Artisan+
- `GET /api/artisan/events` - Get roasting events log

### System Information
- `GET /api/status` - Full system status
- `GET /api/system/info` - Device information
- `GET /api/system/health` - Health check
- `GET /api/system/safety` - Safety system status

### WiFi Management
- `GET /api/wifi/status` - WiFi connection status
- `GET /api/wifi/scan` - Scan available networks

### Root Endpoint
- `GET /` - Web interface control panel

## Project Structure

```
├── src/
│   ├── main.rs          # Main application entry point
│   ├── config.rs        # Configuration management
│   ├── error.rs         # Error handling
│   ├── temperature.rs   # Temperature sensor management
│   ├── roaster.rs       # Roaster control implementation
│   ├── artisan.rs       # Artisan+ integration
│   ├── server.rs        # HTTP server and API routes
│   └── wifi.rs          # WiFi connection management
├── static/
│   └── index.html       # Web interface for roast monitoring
├── .cargo/
│   └── config.toml      # Cargo target configuration
├── Cargo.toml           # Project dependencies
├── build.rs             # Build script
├── sdkconfig.defaults   # ESP-IDF configuration
├── rust-toolchain.toml  # Rust toolchain specification
├── build.sh             # Build and flash script
├── flash.sh             # Manual flash script
├── config.template.json # Configuration template
└── README.md            # This file
```

## Configuration

### ESP-IDF Configuration (sdkconfig.defaults)

- Target: ESP32
- WiFi enabled with static/dynamic buffers
- HTTP server with WebSocket support
- FreeRTOS configuration
- Memory and performance optimizations

### Rust Configuration (.cargo/config.toml)

- Target: xtensa-esp32-espidf
- Linker: ldproxy
- Runner: cargo-espflash
- Optimization flags

**Note**: Update `LIBCLANG_PATH` in `.cargo/config.toml` to match your ESP-IDF installation path.

## Development Workflow

### Build Commands

```bash
# Clean build artifacts
./build.sh clean

# Build in debug mode
./build.sh build-debug

# Build in release mode
./build.sh build

# Flash debug build
./build.sh flash-debug

# Flash release build
./build.sh flash

# Monitor serial output
./build.sh monitor

# Full cycle (clean, build, flash)
./build.sh all
```

### Manual Build

```bash
# Set environment variables
export ESP_IDF_TOOLS_INSTALL_DIR=global
export ESP_IDF_VERSION=v5.2.2

# Build
cargo build --target xtensa-esp32-espidf --release

# Flash (use working solution)
./flash.sh

# Monitor
cargo-espflash monitor
```

## Roaster Control

### Temperature Monitoring

- **Bean Temperature**: Primary temperature sensor for roasting progress
- **Environment Temperature**: Ambient temperature tracking
- **Thermocouple Support**: Type K thermocouple interface
- **High-Precision ADC**: 24-bit temperature measurement

### Roasting Control

- **Heating Element**: PWM-controlled heating via solid-state relays
- **Cooling System**: Automated cooling profile management
- **Agitator Control**: Bean stirring motor control
- **Safety Features**: Over-temperature protection and emergency stop

### Artisan+ Integration

LibreRoaster is fully compatible with Artisan+ coffee roasting software:

- **Real-time Data Streaming**: Live temperature data via serial/HTTP
- **Profile Management**: Upload and download roast profiles
- **Event Logging**: Comprehensive roasting event tracking
- **Bluetooth Support**: Wireless connection to Artisan+ desktop app

### Roaster API Examples

```bash
# Get current temperature readings
curl -X GET http://<esp32-ip>/api/temperature

# Set heating level (0-100%)
curl -X POST http://<esp32-ip>/api/heating \
  -H "Content-Type: application/json" \
  -d '{"level": 75}'

# Start roast profile
curl -X POST http://<esp32-ip>/api/roast/start \
  -H "Content-Type: application/json" \
  -d '{"profile_name": "Medium Roast", "duration": 720}'

# Emergency stop
curl -X POST http://<esp32-ip>/api/roast/stop' \
  -H "Content-Type: application/json" \
  -d '{"emergency": true}'
```

## WiFi Features

### Connection Management

- Automatic connection on startup
- Retry logic with configurable attempts
- Background monitoring and auto-reconnection
- Network scanning capabilities

### Configuration Options

```json
{
  "wifi": {
    "ssid": "YOUR_WIFI_SSID",
    "password": "YOUR_WIFI_PASSWORD",
    "max_retries": 5,
    "retry_delay_ms": 5000
  }
}
```

## Error Handling

The project uses a comprehensive error handling system:

- Custom error types for different subsystems
- Proper error propagation
- Panic handler with LED indication
- Logging for debugging

## Memory Optimization

- Release builds optimized for size (`opt-level = "s"`)
- Link-time optimization (LTO)
- Single codegen unit
- Panic mode: abort (reduces binary size)

## Debugging

### Serial Monitor

```bash
cargo-espflash monitor --speed 115200
```

### Build Analysis

```bash
# Analyze binary size
cargo size --target xtensa-esp32-espidf --release

# Disassembly analysis
cargo objdump --target xtensa-esp32-espidf --release -- -d
```

### Common Issues

1. **Flash Write Errors**: 
   - Check USB connection
   - Try lower baud rate (`--baud 115200`)
   - Hold BOOT button during flash

2. **WiFi Connection Issues**:
   - Verify SSID and password in `config.json`
   - Check 2.4GHz availability
   - Monitor serial logs for errors

3. **Build Errors**:
   - Update Rust toolchain
   - Clear build artifacts (`./build.sh clean`)
   - Check ESP-IDF environment
   - Verify `LIBCLANG_PATH` in `.cargo/config.toml`

## Firmware Files

After successful build, firmware is located at:

```
target/xtensa-esp32-espidf/release/deploy/
├── bootloader.bin       # 26KB - ESP32 bootloader
├── partition-table.bin  # 3KB  - Partition layout  
└── libreroaster.bin    # 162KB - LibreRoaster application
```

## Artisan+ Compatibility

LibreRoaster is designed specifically for coffee roasting enthusiasts and professionals using Artisan+:

- **Data Format**: Compatible with Artisan+ temperature logging format
- **Serial Communication**: Direct connection to Artisan+ via serial/USB
- **Wireless Streaming**: Real-time data over WiFi for remote monitoring
- **Profile Transfer**: Seamless roast profile import/export
- **Event Synchronization**: Automatic sync of roast events and annotations

### Artisan+ Setup

1. Connect LibreRoaster to your computer via USB or WiFi
2. In Artisan+, add new device with "LibreRoaster" profile
3. Configure temperature channels:
   - **BT**: Bean Temperature (primary sensor)
   - **ET**: Environment Temperature (secondary sensor)
4. Start roasting with real-time data logging

## Coffee Roasting Features

### Temperature Precision

- **High-Resolution ADC**: 24-bit temperature conversion
- **Multiple Sensors**: Bean, environment, and safety temperature monitoring
- **Calibration System**: Built-in sensor calibration routines
- **Data Logging**: 10Hz temperature sampling rate

### Roast Profiles

- **Profile Storage**: Store multiple roast profiles internally
- **Dynamic Control**: Real-time profile modification during roasting
- **Repeatability**: Consistent roasting results with profile-based control
- **Learning Curve**: Profile optimization based on roast results

### Safety Systems

- **Over-Temperature Protection**: Automatic shutdown at dangerous temperatures
- **Hardware Watchdog**: System reset on malfunction
- **Temperature Limits**: Configurable safety thresholds
- **Emergency Stop**: Physical and software emergency controls

## OTA Updates

The project supports OTA updates (implementation in progress):

- Configuration enabled in sdkconfig.defaults
- HTTP-based firmware download
- Rollback support
- Secure update verification

## License

This project is licensed under the MIT License. See LICENSE file for details.

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## Support

For issues and questions:

1. Check the [Issues](../../issues) page
2. Review the [Wiki](../../wiki) documentation
3. Create a new issue with detailed information

---

**Note**: This project requires a NodeMCU ESP32 board. Ensure proper power supply and USB connection during flashing and operation.

**Configuration Required**: Remember to copy `config.template.json` to `config.json` and update with your WiFi credentials before building.