# Technology Stack

**Analysis Date:** 2026-02-04

## Languages

**Primary:**
- **Rust** (1.88 stable) - Primary language for all firmware code
  - Edition: 2021
  - Target: `riscv32imc-unknown-none-elf` (ESP32-C3 RISC-V)
  - No-std environment for embedded systems

## Runtime

**Embedded Runtime:**
- **Embassy** (async executor)
  - `embassy-executor` v0.9.1 - Task spawning and async runtime
  - `embassy-time` v0.5.0 - Time management and delays
  - `embassy-sync` v0.6.1 - Synchronization primitives
  - `embassy-net` v0.7.1 - Network stack (Ethernet support)
  - Architecture: RISC-V 32-bit (`arch-riscv32` feature)

**Hardware Platform:**
- **ESP32-C3** (RISC-V architecture)
  - Vendor: Espressif
  - Cores: Single-core 32-bit RISC-V
  - Frequency: Configurable (default max)
  - Flash: External (via ESP-IDF bootloader)

## Frameworks

**Core Embedded:**
- **esp-hal** (~1.0) - Hardware abstraction layer for ESP32-C3
  - GPIO, SPI, UART, LEDC, I2C peripherals
  - Feature: `esp32c3` with `log-04` and `unstable`
- **embedded-hal** (1.0.0) - Embedded hardware abstraction interface
- **embedded-hal-02** (0.2.7, optional) - Legacy HAL compatibility
- **embedded-hal-bus** (0.2.0) - Bus abstraction for SPI devices
- **embedded-io** (0.7.1) - I/O trait abstractions

**RTOS and Scheduling:**
- **esp-rtos** (0.2.0) - ESP Real-Time Operating System
  - Features: `embassy`, `esp-alloc`, `esp-radio`, `esp32c3`, `log-04`

**Build and Linker:**
- **esp-backtrace** (0.18.1) - Stack trace recovery
- **esp-println** (0.16.1) - Printf-style logging
- **esp-bootloader-esp-idf** (0.4.0) - Bootloader support
- **static_cell** (2.1.1) - Static memory allocation

## Key Dependencies

**Core Embedded:**
- **heapless** (0.8.0) - Heap-free data structures (Vec, String, etc.)
- **critical-section** (1.2.0) - Critical section management for concurrency
- **log** (0.4.27) - Logging facade

**Sensor Drivers:**
- **MAX31856** - Custom driver for thermocouple amplifiers
  - SPI communication at 1MHz
  - Type-K thermocouple support
  - Fault detection and cold-junction compensation

**Wireless/Networking:**
- **esp-radio** (0.17.0) - WiFi and radio support
  - Features: `esp-alloc`, `esp32c3`, `log-04`, `smoltcp`, `wifi`
- **smoltcp** (0.12.0) - SMAll TCP/IP stack
  - Protocols: DHCP, DNS, ICMP, TCP, UDP
  - Socket APIs for network communication

**Peripheral Interfaces:**
- **SPI Bus** - Shared between two MAX31856 sensors
  - Pins: GPIO5 (MOSI), GPIO6 (MISO), GPIO7 (SCLK)
  - Chip selects: GPIO3 (ET), GPIO4 (BT)
- **UART** - Artisan+ communication
  - Pins: GPIO20 (TX), GPIO21 (RX)
  - Baud rate: Configurable (default 115200)

**PWM Control:**
- **LEDC** - LED Control hardware for PWM
  - Channel 0: Fan control (25kHz)
  - Channel 1: SSR control (1Hz)

## Configuration

**Build Configuration:**
- Target: `riscv32imc-unknown-none-elf`
- Custom linker script: `linkall.x`
- Runner: `espflash flash --monitor --chip esp32c3`

**Rust Toolchain:**
- Channel: stable
- Components: rust-src
- Targets: riscv32imc-unknown-none-elf
- Unstable features: build-std for alloc, core, test

**Compiler Flags:**
- Debug: Optimization level "s" (size optimization)
- Release: LTO enabled, codegen-units=1, opt-level="s"
- Force frame pointers (for backtraces)
- Debug assertions disabled in release

**Memory:**
- Heap allocation: 66,320 bytes via esp-alloc
- Static allocation via `static_cell!` macro

**Environment Variables:**
- `ESP_LOG="info"` - ESP logging level
- `TEST_ENV="1"` - Test environment flag

## Platform Requirements

**Development:**
- Rust 1.88+ with riscv32imc-unknown-none-elf target
- cargo-espflash (for flashing)
- USB-to-UART drivers (for ESP32-C3)
- Optional: probe-rs (for debugging)

**Production:**
- ESP32-C3 development board
- USB power (3.3V, 500mA minimum)
- External flash storage (if needed)
- Proper heat shielding and ventilation

**Target Hardware:**
- 2x MAX31856 thermocouple amplifiers
- 2x Type-K thermocouples
- 1x Solid State Relay (SSR)
- 1x Ceramic heating element
- 1x DC fan (25kHz PWM compatible)
- USB-to-UART adapter for Artisan+

---

*Stack analysis: 2026-02-04*
