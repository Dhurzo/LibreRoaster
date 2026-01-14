#!/bin/bash

# ESP32 Flash Script - Manual Flashing Solution
# This script flashes the manually built ESP32 firmware

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if ESP32 is connected
check_device() {
    print_status "Checking for ESP32 device..."
    if ! /home/juan/.cargo/bin/cargo espflash board-info >/dev/null 2>&1; then
        print_warning "Could not detect ESP32 device. Make sure it's connected and in bootloader mode."
    else
        print_success "ESP32 device detected"
    fi
}

# Flash firmware
flash_firmware() {
    local deploy_dir="/home/juan/Repos/LibreRoaster/target/xtensa-esp32-espidf/release/deploy"
    
    if [ ! -d "$deploy_dir" ]; then
        print_error "Deployment directory not found: $deploy_dir"
        print_error "Please run the build process first to generate firmware files."
        exit 1
    fi
    
    print_status "Flashing ESP32 firmware..."
    
    # Flash bootloader
    print_status "Flashing bootloader..."
    /home/juan/.cargo/bin/cargo espflash flash --partition-table off --bootloader off --target xtensa-esp32-espidf "$deploy_dir/bootloader.bin"
    
    # Flash partition table
    print_status "Flashing partition table..."
    /home/juan/.cargo/bin/cargo espflash flash --partition-table off --bootloader off --target xtensa-esp32-espidf --partition-table-offset 0x8000 "$deploy_dir/partition-table.bin"
    
    # Flash main application
    print_status "Flashing main application..."
    /home/juan/.cargo/bin/cargo espflash flash --partition-table off --bootloader off --target xtensa-esp32-espidf --app-offset 0x10000 "$deploy_dir/libreroaster.bin"
    
    print_success "Firmware flashed successfully!"
}

# Monitor serial output
monitor_serial() {
    print_status "Starting serial monitor (press Ctrl+C to exit)..."
    /home/juan/.cargo/bin/cargo espflash monitor --speed 115200
}

# Show usage
show_usage() {
    echo "ESP32 Manual Flash Script"
    echo ""
    echo "Usage: $0 [COMMAND]"
    echo ""
    echo "Commands:"
    echo "  flash    Flash the firmware (default)"
    echo "  monitor  Start serial monitor only"
    echo "  help     Show this help message"
    echo ""
    echo "Firmware files must be present in:"
    echo "  /home/juan/Repos/LibreRoaster/target/xtensa-esp32-espidf/release/deploy/"
}

# Main script logic
main() {
    print_status "ESP32 Manual Flash Script"
    
    case "${1:-flash}" in
        "flash")
            check_device
            flash_firmware
            print_status "Starting serial monitor automatically..."
            monitor_serial
            ;;
        "monitor")
            monitor_serial
            ;;
        "help"|"-h"|"--help")
            show_usage
            ;;
        *)
            print_error "Unknown command: $1"
            show_usage
            exit 1
            ;;
    esac
}

# Run main function with all arguments
main "$@"