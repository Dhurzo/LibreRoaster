# LibreRoaster - OpenSource Coffee Bean Roaster

LibreRoaster is a professional-grade open-source coffee bean roaster designed for ESP32-C3. Built with modern embedded Rust using Embassy async framework, featuring precision temperature control, dual thermocouple monitoring, PID-based heating, and WiFi connectivity.

## Features

### 🎯 Core Roasting System
- **Precision Temperature Control**: Coffee roaster optimized PID controller (Kp=2.0, Ki=0.01, Kd=0.5)
- **Dual Thermocouple Support**: 2x MAX31856 Type-K thermocouples for Bean Temp (BT) and Environment Temp (ET)
- **SSR Control**: Solid State Relay control with PWM for ceramic heating elements
- **Safety Systems**: Multi-layer temperature protection with emergency shutdown (250°C limit)
- **Real-time Monitoring**: 10Hz sampling rate with responsive control loop

### ⚡ Technical Architecture
- **Modern Embedded Rust**: Embassy async framework with esp-hal ~1.0
- **WiFi Connectivity**: HTTP server with health endpoints for remote monitoring
- **RISC-V Architecture**: Optimized for ESP32-C3's RISC-V core
- **Memory Management**: 66KB heap with esp-alloc
- **Async/Await**: Non-blocking operations with Embassy concurrency
- **Structured Logging**: Comprehensive debug output and system monitoring

### 🔧 Hardware Features
- **Optimized GPIO Assignment**: SPI on GPIO5-7, CS pins GPIO3-4, SSR control on GPIO2
- **High-Speed SPI**: 1MHz communication with MAX31856 sensors
- **SSR PWM**: 1Hz control frequency suitable for heating elements
- **Temperature Ranges**: 225°C base temperature, 250°C maximum safe limit

## Hardware Requirements

### Required Components
- **ESP32-C3** development board (RISC-V architecture)
- **2x MAX31856** thermocouple amplifier boards (Type-K compatible)
- **2x Type-K** thermocouples (for BT and ET measurements)
- **1x SSR** (Solid State Relay) for heating element control
- **Ceramic heating element** (compatible with your roaster design)
- **USB-C cable** for power and programming
- **WiFi network** (2.4GHz) for remote monitoring

### Wiring Configuration
```
ESP32-C3    →    MAX31856 #1 (BT)    MAX31856 #2 (ET)    SSR
GPIO7       →    SCLK                 SCLK              
GPIO6       →    MISO                 MISO              
GPIO5       →    MOSI                 MOSI              
GPIO4       →    CS                   —                 
GPIO3       →    —                    CS                 
GPIO2       →    —                    —                  Control
3.3V        →    VCC                  VCC               
GND         →    GND                  GND               
```

### Power Requirements
- **ESP32-C3**: 3.3V (500mA minimum)
- **Heating Element**: As per your ceramic element specifications
- **Safety**: Use appropriate fusing and isolation for high-voltage heating circuit


## ⚠️ Safety Warning

**This project involves serious safety risks.**

LibreRoaster works with:

- ⚡ **High voltages**
- 🔥 **Very high temperatures**

Improper handling can result in **severe injury, fire, or death**.

### Please follow these precautions:

- Only work on the hardware if you have **proper electrical knowledge**.
- Always disconnect power before modifying or servicing the device.
- Use appropriate **thermal insulation and heat-resistant materials**.
- **Never leave the roaster unattended while operating.**
- Keep a **fire extinguisher nearby at all times** when using the roaster.
- Operate the roaster in a **well-ventilated and fire-safe area**.

> ⚠️ You build and use this project **at your own risk**.  
> The authors and contributors are **not responsible** for any damage, injury, or loss.

---

## Software Requirements

- Rust stable toolchain (1.92.0+)
- cargo-espflash (for flashing)
- Optional: probe-rs (for debugging)

*All ESP32-C3 dependencies are automatically managed via Cargo.*

## Quick Start

### 1. Install Dependencies

```bash
# Install cargo-espflash for flashing
cargo install cargo-espflash
```

### 2. Build Project

```bash
# Build in release mode
cargo build --release
```

### 3. Connect Hardware

- Connect ESP32-C3 board to computer via USB-C
- Ensure proper power supply
- Verify device detection

### 4. Flash Firmware

```bash
# List available serial ports
cargo espflash list

# Flash the firmware
cargo espflash flash --release

# Flash and monitor serial output
cargo espflash flash --release --monitor
```

### 5. Monitor Serial Output

```bash
# Monitor serial output separately
cargo espflash monitor

# Or specify port manually
cargo espflash monitor --port /dev/ttyUSB0
```

## Current Implementation

LibreRoaster provides a complete coffee roaster control system with:

### 🎛️ Temperature Control System
- **PID Controller**: Coffee roaster optimized with anti-windup protection
- **Dual Sensor Support**: Independent BT and ET thermocouple monitoring
- **MAX31856 Driver**: Async communication with fault detection
- **SSR Control**: PWM output with 0-100% duty cycle control

### 🌐 Network & Monitoring
- **HTTP Server**: Built-in web server with health monitoring
- **WiFi Integration**: Ready for network configuration and remote access
- **API Endpoints**: Health check and system status endpoints

### 🔄 State Machine
- **Roaster States**: Idle → Heating → Stable → Cooling → Emergency
- **Command Processing**: Start/Stop roast, temperature control, emergency shutdown
- **Safety Monitoring**: Over-temperature protection and sensor validation

### 📊 System Features
- **Real-time Control**: 10Hz PID loop with responsive temperature regulation
- **Safety First**: Multiple protection layers including hard limits at 250°C
- **Calibration Support**: Adjustable thermocouple offsets for accuracy
- **Emergency Systems**: Automatic shutdown on fault conditions

### Sample Output

```
INFO  Wake the f*** up samurai we have beans to burn!
INFO  Embassy initialized!
INFO  Roaster is ready!
INFO  Starting roast with target temperature: 225.0°C
INFO  Target temperature reached, entering stable state
```

### HTTP API Endpoints

```
GET /health  → "Wake the f*** up samurai we have beans to burn!"
GET /        → LibreRoaster information and available endpoints
```

The system is ready for:
- Hardware integration with actual thermocouples and SSR
- WiFi configuration for network connectivity
- Advanced roasting profiles and automation
- Artisan+ compatibility implementation
- Real-time data logging and analysis

## Project Structure

```
├── src/
│   ├── main.rs              # Main application entry point
│   ├── lib.rs               # Library interface
│   ├── hardware/            # Hardware abstraction layer
│   │   ├── mod.rs           # Hardware module exports
│   │   ├── max31856.rs      # ✅ MAX31856 thermocouple driver
│   │   ├── ssr.rs           # ✅ SSR control implementation
│   │   └── pid.rs           # ✅ PID controller (coffee roaster optimized)
│   ├── server/              # Web server and API
│   │   ├── mod.rs           # Server module exports
│   │   └── http.rs          # ✅ HTTP server with health endpoints
│   ├── control/             # Roaster control logic
│   │   ├── mod.rs           # Control module exports
│   │   └── roaster.rs       # ✅ Complete roaster state machine
│   └── config/              # Configuration management
│       ├── mod.rs           # Configuration exports
│       └── constants.rs     # ✅ Hardware constants and pin assignments
├── .cargo/
│   └── config.toml          # Cargo target configuration
├── Cargo.toml               # Project dependencies
├── build.rs                 # Build script
├── rust-toolchain.toml      # Rust toolchain specification
└── README.md                # This file
```

### Architecture Overview

#### `hardware/` - Hardware Abstraction Layer
- **`max31856.rs`**: Complete MAX31856 driver with async support, fault detection, and Type-K thermocouple configuration
- **`ssr.rs`**: Solid State Relay control with PWM output capabilities
- **`pid.rs`**: Professional PID controller with coffee roaster optimized parameters and anti-windup protection

#### `server/` - Network & API
- **`http.rs`**: Lightweight HTTP server with routing, error handling, and health endpoints

#### `control/` - Business Logic
- **`roaster.rs`**: Complete state machine implementation with safety monitoring, temperature validation, and command processing

#### `config/` - Configuration
- **`constants.rs`**: All hardware pin assignments, temperature limits, PID parameters, and system constants

## Development

### Build Commands

```bash
# Build in release mode
cargo build --release

# Build in debug mode  
cargo build

# Clean build artifacts
cargo clean
```

### Flash Commands

```bash
# List available ports
cargo espflash list

# Flash firmware
cargo espflash flash --release

# Flash and monitor
cargo espflash flash --release --monitor

# Monitor only
cargo espflash monitor

# Specify port manually
cargo espflash flash --release --port /dev/ttyUSB0
```

### Advanced Options

```bash
# Erase flash completely
cargo espflash erase-flash

# Monitor with specific baud rate
cargo espflash monitor --speed 115200

# List all ports (including unrecognized)
cargo espflash list --list-all-ports
```

## Debugging

### Serial Monitor

```bash
cargo espflash monitor --speed 115200
```

### Common Issues

1. **Flash Write Errors**: 
   - Check USB connection
   - Try different USB port
   - Ensure ESP32-C3 is properly connected

2. **Build Errors**:
   - Update Rust toolchain: `rustup update stable`
   - Clear build artifacts: `cargo clean`
   - Check internet connection for dependency downloads

3. **Serial Port Issues**:
   - List all ports: `cargo espflash list --list-all-ports`
   - Try specifying port manually
   - Check USB drivers for ESP32-C3

### Build Output

After successful build, binary is located at:
```
target/riscv32imc-esp-espidf/release/libreroaster
```

## License

This project is licensed under the APACHE-2 License. See LICENSE file for details.


## Support

For issues and questions:

1. Check the [Issues](../../issues) page
2. Review the [Wiki](../../wiki) documentation
3. Create a new issue with detailed information

---

**Note**: This project requires an ESP32-C3 development board. Ensure proper power supply and USB connection during flashing and operation.
