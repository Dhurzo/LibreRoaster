# LibreRoaster - OpenSource Coffee Bean Roaster Firmware ☕🔥

LibreRoaster is a open-source (hackable) coffee bean roaster designed for ESP32-C3 (firmware & hardware). Built with modern embedded Rust using Embassy async framework, featuring temperature control, dual thermocouple monitoring, proportional-based heating, fan control, heat source detection, and **Artisan+ compatibility via UART communication**.

## Project Philosophy

The project aims to enable anyone with intermediate technical skills to build their own affordable coffee roaster. Due to the cost-focused approach, certain components are chosen over more expensive alternatives - this is evident in the (future) hardware section where even recycled components are utilized.

The project is adaptable to both more expensive and more budget-friendly components. The design has also been kept simple, which means the roaster is dependent on ARTISAN+ and does not function in "standalone" mode without ARTISAN+ (a standalone version with a different controller could be considered if there is community interest).

## Core Value

Artisan can read temperatures and control heater/fan during a roast session via serial connection.

## Supported Artisan Commands

| Command | Description |
|---------|-------------|
| `READ` | Request telemetry (ET, BT, ambient, fan%, heater%) |
| `OT1 [0-100]` | Set heater power percentage |
| `IO3 [0-100]` | Set fan speed percentage |
| `UP` | Increase heater by 5% |
| `DOWN` | Decrease heater by 5% |
| `START` | Begin roasting, enable continuous output |
| `STOP` | Emergency stop, disable outputs |

## Quick Start

### 1. Install Dependencies

```bash
cargo install cargo-espflash
```

### 2. Build

```bash
cargo build --release
```

### 3. Flash

```bash
cargo espflash flash --release --monitor
```

### 4. Connect to Artisan

1. USB CDC: Board appears as serial port
2. UART: GPIO20 (TX), GPIO21 (RX) at 115200 baud

## Hardware Requirements

| Component | Description |
|-----------|-------------|
| ESP32-C3 | RISC-V development board |
| 2x MAX31856 | Thermocouple amplifier boards |
| 2x Type-K Thermocouples | Bean Temp and Environment Temp |
| SSR | Solid State Relay for heater control |
| Fan | Variable speed fan (PWM controlled) |

## Pinout

| GPIO | Function |
|------|----------|
| 3 | MAX31856 #1 CS (BT) |
| 4 | MAX31856 #2 CS (ET) |
| 5-7 | SPI (MOSI, MISO, SCLK) |
| 9 | Fan PWM |
| 10 | SSR PWM |
| 20 | UART TX (to Artisan) |
| 21 | UART RX (from Artisan) |

## Artisan Connection

LibreRoaster supports dual-channel communication:

- **USB CDC**: Native USB, no adapter needed
- **UART0**: GPIO20/21 at 115200 baud

Artisan connects to either channel. The multiplexer automatically routes commands.

## Protocol

### READ Response Format

```
ET,BT,ET2,BT2,ambient,fan,heater
```

Example: `185.2,192.3,-1,-1,24.5,45,75`

| Field | Description |
|-------|-------------|
| ET | Environment temperature (°C) |
| BT | Bean temperature (°C) |
| ET2 | Extra channel (-1 if unused) |
| BT2 | Extra channel (-1 if unused) |
| ambient | Ambient temperature |
| fan | Fan output % |
| heater | Heater output % |

### Initialization

Artisan sends commands without handshake or Artisan sends handshake sequence (CHAN, UNITS, FILT). LibreRoaster responds with `#` acknowledgment. 

## License

Apache 2.0

## Project Structure

```
├── src/
│   ├── main.rs              # Main application entry point
│   ├── lib.rs               # Library interface
│   ├── application/         # Application architecture
│   │   ├── mod.rs          # Application module exports
│   │   ├── app_builder.rs  # Service container and dependency injection
│   │   ├── service_container.rs # Service management
│   │   └── tasks.rs        # Application tasks
│   ├── hardware/           # Hardware abstraction layer
│   │   ├── mod.rs         # Hardware module exports
│   │   ├── max31856.rs    # MAX31856 thermocouple driver
│   │   ├── ssr.rs         # SSR control with LEDC PWM and heat detection
│   │   ├── fan.rs         # Fan control with LEDC PWM
│   │   ├── shared_spi.rs  # Shared SPI bus implementation
│   │   └── uart.rs        # UART communication
│   ├── control/            # Roaster control logic
│   │   ├── mod.rs         # Control module exports
│   │   ├── roaster_refactored.rs # State machine and command processing
│   │   └── handlers.rs     # Control handlers
│   ├── input/              # Input processing
│   │   ├── mod.rs         # Input module exports
│   │   └── parser.rs      # Artisan command parsing
│   ├── output/             # Output and formatting
│   │   ├── mod.rs         # Output module exports
│   │   ├── artisan.rs     # Artisan protocol formatter
│   │   └── uart.rs        # UART output
│   ├── config/             # Configuration
│   │   └── constants.rs    # Hardware constants and pin assignments
│   └── error/              # Error handling
│       └── app_error.rs    # Custom error types
├── examples/
│   └── artisan_test.rs     # Artisan protocol example
├── .cargo/
│   └── config.toml         # Cargo target configuration
├── Cargo.toml               # Project dependencies
├── build.rs                # Build script
├── rust-toolchain.toml     # Rust toolchain specification
└── README.md               # This file
```

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

## 📜 License

This project is open source under APACHE-2 LICENCE.  
See the `LICENSE` file for more information.
