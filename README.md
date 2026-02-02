# LibreRoaster - OpenSource Coffee Bean Roaster

LibreRoaster is a professional-grade open-source coffee bean roaster designed for ESP32-C3. Built with modern embedded Rust using Embassy async framework, featuring temperature control, dual thermocouple monitoring, proportional-based heating, fan control, heat source detection, and **Artisan+ compatibility via UART communication**.

## Project Philosophy

The project aims to enable anyone with intermediate technical skills to build their own affordable coffee roaster. Due to the cost-focused approach, certain components are chosen over more expensive alternatives - this is evident in the (future) hardware section where even recycled components are utilized.

The project is adaptable to both more expensive and more budget-friendly components. The design has also been kept simple, which means the roaster is dependent on ARTISAN+ and does not function in "standalone" mode without ARTISAN+ (a standalone version with a different controller could be considered if there is community interest).

## Features

### 🎯 Core Roasting System
- **Simple Temperature Control**: Proportional control loop for heating regulation
- **Dual Thermocouple Support**: 2x MAX31856 Type-K thermocouples for Bean Temp (BT) and Environment Temp (ET)
- **SSR Control with PWM**: Solid State Relay control with LEDC PWM for ceramic heating elements
- **Fan Control**: Variable speed fan control using LEDC PWM (25kHz)
- **Heat Source Detection**: Automatic detection of connected heating element (GPIO1)
- **Safety Systems**: Multi-layer temperature protection with emergency shutdown (250°C limit)
- **Real-time Monitoring**: 10Hz sampling rate with responsive control loop

### ⚡ Technical Architecture
- **Modern Embedded Rust**: Embassy async framework with esp-hal ~1.0
- **Artisan+ Compatibility**: Standard UART protocol for integration with Artisan coffee roasting software
- **RISC-V Architecture**: Optimized for ESP32-C3's RISC-V core
- **Memory Management**: 66KB heap with esp-alloc
- **Async/Await**: Non-blocking operations with Embassy concurrency
- **Service Container Pattern**: Modular dependency injection and error handling
- **Structured Logging**: Comprehensive debug output and system monitoring
- **Trait-Based Hardware**: Abstractions for Thermometer, Heater, and Fan

### 🔧 Hardware Features
- **Optimized GPIO Assignment**: SPI on GPIO5-7, CS pins GPIO3-4, SSR control on GPIO10, Fan on GPIO9
- **High-Speed SPI**: 1MHz communication with MAX31856 sensors using shared SPI bus
- **UART Communication**: Serial interface for Artisan+ protocol (GPIO20/21)
- **LEDC PWM**: Dual-channel PWM for SSR (1Hz) and Fan (25kHz)
- **Temperature Ranges**: 225°C base temperature, 250°C maximum safe limit

## Hardware Requirements

### Required Components
- **ESP32-C3** development board (RISC-V architecture)
- **2x MAX31856** thermocouple amplifier boards (Type-K compatible)
- **2x Type-K** thermocouples (for BT and ET measurements)
- **1x SSR** (Solid State Relay) for heating element control
- **Ceramic heating element** (compatible with your roaster design)
- **USB-C cable** for power and programming
- **USB-to-UART adapter** (for Artisan+ connection to computer)

### Wiring Configuration
```
ESP32-C3    →    MAX31856 #1 (BT)    MAX31856 #2 (ET)    SSR         Fan         UART (to PC)
GPIO7       →    SCLK                 SCLK              —            —           —
GPIO6       →    MISO                 MISO              —            —           —
GPIO5       →    MOSI                 MOSI              —            —           —
GPIO4       →    CS                   —                 —            —           —
GPIO3       →    —                    CS                —            —           —
GPIO10      →    —                    —                 PWM          —           —
GPIO9       →    —                    —                 —            PWM        —
GPIO1       →    —                    —                 Detect*      —           —
GPIO20      →    —                    —                 —            —           TX
GPIO21      →    —                    —                 —            —           RX
3.3V        →    VCC                  VCC               —            —           —
GND         →    GND                  GND               —            —           GND
```

*GPIO1 is an input with internal pull-up for heat source detection (active low)

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

- Rust stable toolchain (1.88+)
- cargo-espflash (for flashing)
- Optional: probe-rs (for debugging)
- Artisan software (for roasting control and logging)
- USB-to-UART drivers for your operating system

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
- **Temperature Control**: Proportional control loop for heating regulation
- **Dual Sensor Support**: Independent BT and ET thermocouple monitoring via shared SPI
- **MAX31856 Driver**: Async communication with fault detection and Type-K support
- **SSR Control with Heat Detection**: PWM output with 0-100% duty cycle and automatic heat source detection

### 🌬️ Fan Control System
- **SimpleLedcFan**: LEDC-based PWM control (25kHz) for variable speed fan
- **Fan Trait**: Abstraction for fan control with speed 0-100%
- **Channel0 LEDC**: Dedicated channel on GPIO9 for fan PWM output

### 📡 Artisan+ Integration
- **UART Communication**: Standard Artisan protocol over serial (time,ET,BT,ROR,Gas)
- **Real-time Data Streaming**: 10Hz output rate for smooth plotting
- **ArtisanFormatter**: Built-in CSV protocol formatter
- **Rate of Rise (ROR)**: Automatic calculation using 5-sample moving average

### 🏗️ Modular Architecture
- **Service Container**: Dependency injection pattern with AppBuilder
- **Hardware Abstractions**: Traits for Thermometer, Heater, and Fan
- **Shared SPI**: Multiple MAX31856 sensors on single SPI bus with chip select
- **Error Handling**: Comprehensive error management with custom error types
- **Input/Output System**: Modular data flow from sensors to Artisan output
- **Task Management**: Embassy async tasks for concurrent operations

### 🔄 State Machine
- **Roaster States**: Idle → Heating → Stable → Cooling → Emergency
- **Command Processing**: Start/Stop roast, temperature control, emergency shutdown
- **Safety Monitoring**: Over-temperature protection and sensor validation

### 📊 System Features
- **Real-time Control**: 10Hz control loop with responsive temperature regulation
- **Safety First**: Multiple protection layers including hard limits at 250°C
- **Heat Source Detection**: Automatic detection of connected heating element via GPIO1
- **Calibration Support**: Adjustable thermocouple offsets for accuracy
- **Emergency Systems**: Automatic shutdown on fault conditions


### Artisan+ Protocol Output

The system outputs CSV data in Artisan standard format:
```
0.0,25.1,24.8,0.0,0
0.1,25.3,25.0,0.2,0
0.2,26.1,25.8,0.8,5
0.3,27.4,27.1,1.3,12
...
```

Fields: `time,ET,BT,ROR,Gas`
- **time**: Seconds since roast start
- **ET**: Environment temperature (°C)
- **BT**: Bean temperature (°C)  
- **ROR**: Rate of rise (°C/s)
- **Gas**: SSR output percentage (0-100)

The system is ready for:
- Hardware integration with actual thermocouples and SSR
- Direct connection to Artisan software via UART
- Advanced roasting profiles and automation
- Real-time data logging and analysis in Artisan

## Project Structure

```
├── src/
│   ├── main.rs              # Main application entry point
│   ├── lib.rs               # Library interface
│   ├── application/         # Application architecture
│   │   ├── mod.rs           # Application module exports
│   │   ├── app_builder.rs   # Service container and dependency injection
│   │   ├── service_container.rs # Service management
│   │   └── tasks.rs         # Application tasks
│   ├── hardware/            # Hardware abstraction layer
│   │   ├── mod.rs           # Hardware module exports
│   │   ├── max31856.rs      # MAX31856 thermocouple driver
│   │   ├── ssr.rs           # SSR control with LEDC PWM and heat detection
│   │   ├── fan.rs           # Fan control with LEDC PWM
│   │   ├── shared_spi.rs    # Shared SPI bus implementation
│   │   └── board.rs         # Board-specific hardware types
│   ├── control/             # Roaster control logic
│   │   ├── mod.rs           # Control module exports
│   │   ├── roaster_refactored.rs # Refactored control logic
│   │   ├── command_handler.rs # Command processing
│   │   ├── handlers.rs      # Control handlers
│   │   ├── abstractions.rs  # Control abstractions
│   │   ├── abstractions_tests.rs # Control tests
│   │   └── traits.rs        # Hardware traits (Thermometer, Heater, Fan)
│   ├── input/               # Input processing
│   │   ├── mod.rs           # Input module exports
│   │   └── parser.rs        # Command parsing
│   ├── output/              # Output and formatting
│   │   ├── mod.rs           # Output module exports
│   │   ├── artisan.rs       # Artisan+ CSV formatter
│   │   ├── serial.rs        # Serial output management
│   │   ├── uart.rs          # UART output implementation
│   │   ├── scheduler.rs     # Output scheduling
│   │   ├── manager.rs       # Output manager
│   │   └── traits.rs        # Output trait definitions
│   ├── server/              # Communication server (placeholder)
│   │   └── mod.rs           # Server module exports (empty)
│   ├── config/              # Configuration management
│   │   ├── mod.rs           # Configuration exports
│   │   └── constants.rs     # Hardware constants and pin assignments
│   └── error/               # Error handling
│       ├── mod.rs           # Error module exports
│       └── app_error.rs     # Custom error types
├── examples/
│   └── artisan_test.rs     # Artisan+ protocol example
├── .cargo/
│   └── config.toml          # Cargo target configuration
├── Cargo.toml               # Project dependencies
├── build.rs                 # Build script
├── rust-toolchain.toml      # Rust toolchain specification
└── README.md                # This file
```

### Architecture Overview

#### `application/` - Core Architecture
- **`app_builder.rs`**: Service container pattern with dependency injection and clean initialization
- **`service_container.rs`**: Service management and lifetime handling
- **`tasks.rs`**: Main application task orchestration

#### `hardware/` - Hardware Abstraction Layer
- **`max31856.rs`**: Complete MAX31856 driver with async support, fault detection, and Type-K thermocouple configuration
- **`ssr.rs`**: Solid State Relay control with LEDC PWM, heat source detection (GPIO1), and simple mode implementation
- **`fan.rs`**: Fan control with LEDC PWM (25kHz), including FanController and SimpleLedcFan implementations
- **`shared_spi.rs`**: Shared SPI bus for multiple MAX31856 sensors with chip select
- **`board.rs`**: Board-specific hardware type definitions

#### `control/` - Business Logic
- **`roaster_refactored.rs`**: State machine implementation with safety monitoring and command processing
- **`command_handler.rs`**: Command processing and response handling
- **`handlers.rs`**: Control operation handlers
- **`abstractions.rs`**: Control system abstractions and interfaces
- **`traits.rs`**: Hardware traits (Thermometer, Heater, Fan)

#### `input/` & `output/` - Data Flow
- **`parser.rs`**: Command parsing and validation
- **`artisan.rs`**: Artisan+ CSV protocol formatter with ROR calculation
- **`uart.rs`**: UART output implementation and management
- **`traits.rs`**: Output abstraction interfaces

#### `error/` - Error Management
- **`app_error.rs`**: Comprehensive error types and handling

#### `config/` - Configuration
- **`constants.rs`**: All hardware pin assignments, temperature limits, and system constants

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
target/riscv32imc-unknown-none-elf/release/libreroaster
```

## License

This project is licensed under the APACHE-2 License. See LICENSE file for details.


## Support

For issues and questions:

1. Check the [Issues](../../issues) page
2. Review the [Wiki](../../wiki) documentation
3. Create a new issue with detailed information

## Examples

### Artisan+ Test

Run the Artisan+ protocol example to test the data formatting:

```bash
# Build and run the example (requires host target)
cargo run --example artisan_test --features std
```

This example demonstrates the CSV output format that will be sent to Artisan software during actual roasting.

---

**Note**: This project requires an ESP32-C3 development board. Ensure proper power supply and USB connection during flashing and operation. Connect the UART pins (GPIO20/TX, GPIO21/RX) to a USB-to-UART adapter for Artisan+ integration.
