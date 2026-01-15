#!/bin/bash

# Build LibreRoaster firmware for Wokwi simulation
# This script builds the project and generates the binary for ESP32-C3

set -e  # Exit on any error

echo "🔨 Building LibreRoaster firmware..."

# Build in release mode
echo "📦 Building in release mode..."
cargo build --release

echo "📋 Generating binary firmware..."

# Generate ESP32-C3 binary
espflash save-image --chip esp32c3 target/riscv32imc-unknown-none-elf/release/libreroaster libreroaster.bin

# Check if binary was created successfully
if [ -f "libreroaster.bin" ]; then
    echo "✅ Firmware generated successfully!"
    echo "📁 File: libreroaster.bin ($(stat -f%z libreroaster.bin 2>/dev/null || stat -c%s libreroaster.bin) bytes)"
    echo "🎯 Ready for Wokwi simulation!"
    echo ""
    echo "To use in Wokwi:"
    echo "1. Upload libreroaster.bin to your Wokwi project"
    echo "2. Use wokwi.toml and diagram.json for configuration"
    echo ""
    echo "Flash to device:"
    echo "espflash flash --chip esp32c3 libreroaster.bin --monitor"
else
    echo "❌ Failed to generate firmware binary!"
    exit 1
fi