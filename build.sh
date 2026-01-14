#!/bin/bash

# ESP32 NodeMCU Project Build Script
# This script builds and flashes the ESP32 firmware with proper error handling

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

# Function to check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Check and source ESP environment
setup_esp_environment() {
    print_status "Setting up ESP environment..."
    
    # Check if export script exists
    if [ -f "/home/juan/export-esp.sh" ]; then
        source /home/juan/export-esp.sh
        print_success "ESP environment loaded"
    else
        print_warning "ESP export script not found, using current environment"
    fi
    
    # Set environment variables
    export ESP_IDF_TOOLS_INSTALL_DIR=global
    export ESP_IDF_VERSION=v5.2.2
    export MCU=esp32
}

# Check if required tools are installed
check_dependencies() {
    print_status "Checking dependencies..."
    
    local missing_deps=()
    
    # Check for cargo
    if ! command_exists cargo; then
        missing_deps+=("cargo")
    fi
    
    # Check for cargo-espflash
    if ! cargo espflash --version >/dev/null 2>&1; then
        print_warning "cargo-espflash not found. Installing..."
        cargo install cargo-espflash
    fi
    
    # Check for ldproxy
    if ! command_exists ldproxy; then
        print_warning "ldproxy not found. Installing..."
        cargo install ldproxy
    fi
    
    if [ ${#missing_deps[@]} -ne 0 ]; then
        print_error "Missing dependencies: ${missing_deps[*]}"
        exit 1
    fi
    
    print_success "Dependencies check completed"
}

# Clean previous builds
clean_build() {
    print_status "Cleaning previous builds..."
    cargo clean
    rm -rf target/xtensa-esp32-espidf/release/ 2>/dev/null || true
    rm -rf target/xtensa-esp32-espidf/debug/ 2>/dev/null || true
    print_success "Build cleaned"
}

# Build the project
build_project() {
    local build_mode="${1:-release}"
    print_status "Building ESP32 firmware in $build_mode mode..."
    
    # Build with proper flags
    if [ "$build_mode" = "debug" ]; then
        cargo build --target xtensa-esp32-espidf
    else
        cargo build --target xtensa-esp32-espidf --release
    fi
    
    print_success "Build completed successfully"
}

# Monitor serial output
monitor_device() {
    print_status "Starting serial monitor..."
    cargo espflash monitor --target xtensa-esp32-espidf
}

# Flash the device
flash_device() {
    local build_mode="${1:-release}"
    print_status "Flashing ESP32 device..."
    
    # Flash with appropriate build
    if [ "$build_mode" = "debug" ]; then
        cargo espflash flash --target xtensa-esp32-espidf --monitor
    else
        cargo espflash flash --target xtensa-esp32-espidf --release --monitor
    fi
    
    print_success "Device flashed successfully"
}

# Get device information
device_info() {
    print_status "Getting device information..."
    cargo espflash board-info
}

# Validate build files
validate_project() {
    print_status "Validating project structure..."
    
    local missing_files=()
    
    if [ ! -f "Cargo.toml" ]; then
        missing_files+=("Cargo.toml")
    fi
    
    if [ ! -f "src/main.rs" ]; then
        missing_files+=("src/main.rs")
    fi
    
    if [ ! -d ".cargo" ]; then
        missing_files+=(".cargo directory")
    fi
    
    if [ ${#missing_files[@]} -ne 0 ]; then
        print_error "Missing files: ${missing_files[*]}"
        exit 1
    fi
    
    print_success "Project structure validation passed"
}

# Show usage
show_usage() {
    echo "ESP32 NodeMCU Build Script"
    echo ""
    echo "Usage: $0 [COMMAND] [OPTIONS]"
    echo ""
    echo "Commands:"
    echo "  validate      Validate project structure"
    echo "  clean         Clean build artifacts"
    echo "  build         Build the project (default: release)"
    echo "  build-debug   Build in debug mode"
    echo "  flash         Flash the device (default: release)"
    echo "  flash-debug   Flash debug build"
    echo "  monitor       Monitor serial output"
    echo "  info          Show device information"
    echo "  all           Clean, build, and flash (default)"
    echo "  help          Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0              # Run full build and flash cycle"
    echo "  $0 build        # Build release version"
    echo "  $0 build-debug  # Build debug version"
    echo "  $0 flash        # Flash release version"
    echo "  $0 monitor      # Monitor serial output"
}

# Main script logic
main() {
    print_status "ESP32 NodeMCU Project Build Script"
    
    # Setup ESP environment first
    setup_esp_environment
    
    case "${1:-all}" in
        "validate")
            validate_project
            ;;
        "clean")
            clean_build
            ;;
        "build")
            check_dependencies
            validate_project
            build_project release
            ;;
        "build-debug")
            check_dependencies
            validate_project
            build_project debug
            ;;
        "flash")
            check_dependencies
            validate_project
            flash_device release
            ;;
        "flash-debug")
            check_dependencies
            validate_project
            flash_device debug
            ;;
        "monitor")
            monitor_device
            ;;
        "info")
            device_info
            ;;
        "all")
            check_dependencies
            validate_project
            clean_build
            build_project release
            flash_device release
            print_success "Full build and flash cycle completed!"
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