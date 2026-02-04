# External Integrations

**Analysis Date:** 2026-02-04

## Hardware Peripherals

**Temperature Sensors:**
- **MAX31856** (2x units) - Thermocouple Amplifier
  - Interface: SPI (shared bus)
  - Chip Select: GPIO4 (Bean Temp), GPIO3 (Environment Temp)
  - Communication: 1MHz SPI
  - Thermocouple Type: Type-K
  - Driver: Custom implementation in `src/hardware/max31856.rs`
  - Measurements: Cold-junction compensated temperature
  - Fault Detection: Open circuit, short to GND/VCC, over/under voltage

**Solid State Relay (SSR):**
- **Heating Element Control**
  - Interface: LEDC PWM on GPIO10
  - Frequency: 1Hz (slow PWM for heating element)
  - Resolution: 8-bit (0-255 duty levels)
  - Safety: Heat source detection via GPIO1 (input with pull-up)

**Fan Control:**
- **DC Fan Motor**
  - Interface: LEDC PWM on GPIO9
  - Frequency: 25kHz (inaudible range for DC motors)
  - Control: Variable speed 0-100%
  - Implementation: `SimpleLedcFan` in `src/hardware/fan.rs`

**SPI Bus:**
- **Shared Peripheral Bus**
  - SCLK: GPIO7
  - MOSI: GPIO5
  - MISO: GPIO6
  - Devices: 2x MAX31856 with separate chip selects
  - Implementation: `SpiDeviceWithCs` wrapper in `src/hardware/shared_spi.rs`

## Software & Communication

**Artisan+ Coffee Roasting Software:**
- **Primary Integration** - Main control interface
  - Protocol: Serial CSV over UART
  - Baud Rate: 115200 (configurable)
  - Connection: USB-to-UART adapter
  - Pins: GPIO20 (TX), GPIO21 (RX)
  - Format: `time,ET,BT,ROR,Gas`
    - time: Seconds since roast start (decimal)
    - ET: Environment temperature (°C)
    - BT: Bean temperature (°C)
    - ROR: Rate of Rise (°C/s)
    - Gas: SSR output percentage (0-100)
  - Implementation: `ArtisanFormatter` in `src/output/artisan.rs`
  - Output Rate: 10Hz (configurable)

**Artisan+ Commands (Received):**
- **READ** - Request current status (ET, BT, Power, Fan)
- **START** - Begin roasting with continuous output
- **OT1 x** - Set heater output (0-100%)
- **IO3 x** - Set fan speed (0-100%)
- **STOP** - Emergency stop
- Implementation: `ArtisanInput` parser in `src/input/parser.rs`

## Network Stack

**TCP/IP (Available):**
- **smoltcp** v0.12.0 - Embedded TCP/IP stack
  - Protocols: DHCP, DNS, ICMP, TCP, UDP
  - Medium: Ethernet (via embassy-net)
  - Socket APIs: DNS, ICMP, Raw, TCP, UDP
  - Features: DHCPv4, multicast support

**WiFi (Available):**
- **esp-radio** v0.17.0 - WiFi radio driver
  - Chip: ESP32-C3 integrated WiFi
  - Features: Station mode, access point
  - Dependencies: smoltcp networking stack

**Note:** Network stack is available but not actively used in current firmware. Template configuration exists in `config.template.json` for WiFi credentials.

## Data Storage

**Configuration Storage:**
- **JSON Configuration File** (`config.template.json`)
  - WiFi credentials (SSID, password)
  - Roaster settings (pins, limits, calibration)
  - Artisan communication settings
  - Device settings (LED, log level)
  - Note: This is a template; actual config may vary per deployment

**Runtime Configuration:**
- **Constants** - Hardcoded in `src/config/constants.rs`
  - GPIO pin assignments
  - Temperature limits (MAX_SAFE_TEMP: 250°C)
  - PWM frequencies (Fan: 25kHz, SSR: 1Hz)
  - Thermocouple offsets (calibration)
  - PID parameters and timeouts

## Development & Debugging

**Flashing:**
- **cargo-espflash** - Primary flashing tool
  - Chip: ESP32-C3
  - Monitor: Serial output via USB
  - Speed: Default 115200 baud

**Logging:**
- **esp_println::logger** - Serial logging
  - Level: Configurable (default: info)
  - Output: UART0 serial console
  - Implementation: `log` crate facade

**Debugging:**
- **probe-rs** - Optional JTAG/SWD debugging
- **esp-backtrace** - Stack trace recovery on panic

## Security Considerations

**No External APIs:**
- This is a standalone embedded system
- No cloud services, no telemetry
- No authentication providers
- No third-party data collection

**Safety Systems:**
- Multi-layer temperature protection
- Over-temperature emergency shutdown (260°C threshold)
- Temperature sensor timeout detection (1 second)
- Heat source presence detection (GPIO1)
- SSR hardware status monitoring

**Configuration Security:**
- WiFi credentials should be stored securely
- No hardcoded secrets in firmware
- Template config indicates user-provided credentials

## Environment Configuration

**Required Configuration:**
- WiFi credentials (if networking enabled)
- Thermocouple calibration offsets (optional)
- Custom temperature limits (safety)

**Runtime Parameters:**
- Default target temperature: 225°C
- Maximum safe temperature: 250°C
- Emergency threshold: 260°C
- PID sample time: 100ms (10Hz)

**Build-Time Settings:**
- ESP-IDF bootloader integration
- Heap allocator enabled (66KB)
- Static allocation for drivers

---

*Integration audit: 2026-02-04*
