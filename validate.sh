#!/bin/bash

# ESP32 NodeMCU Project Validation Script
# Validates the project setup and provides next steps

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

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

# Check file existence
check_file() {
    if [ -f "$1" ]; then
        print_success "✓ $1 exists"
        return 0
    else
        print_error "✗ $1 missing"
        return 1
    fi
}

# Check directory existence
check_dir() {
    if [ -d "$1" ]; then
        print_success "✓ $1 directory exists"
        return 0
    else
        print_error "✗ $1 directory missing"
        return 1
    fi
}

# Main validation
main() {
    echo "ESP32 NodeMCU Project Validation"
    echo "================================="
    echo
    
    local errors=0
    
    print_status "Checking project structure..."
    
    # Check root files
    check_file "Cargo.toml" || ((errors++))
    check_file "build.rs" || ((errors++))
    check_file "sdkconfig.defaults" || ((errors++))
    check_file "rust-toolchain.toml" || ((errors++))
    check_file "build.sh" || ((errors++))
    check_file "README.md" || ((errors++))
    
    # Check .cargo directory
    check_dir ".cargo" || ((errors++))
    if [ -f ".cargo/config.toml" ]; then
        print_success "✓ .cargo/config.toml exists"
    else
        print_error "✗ .cargo/config.toml missing"
        ((errors++))
    fi
    
    # Check source directory structure
    check_dir "src" || ((errors++))
    if [ -d "src" ]; then
        check_file "src/main.rs" || ((errors++))
        check_file "src/config.rs" || ((errors++))
        check_file "src/error.rs" || ((errors++))
        check_file "src/led.rs" || ((errors++))
        check_file "src/server.rs" || ((errors++))
        check_file "src/wifi.rs" || ((errors++))
    fi
    
    # Check static directory
    check_dir "static" || ((errors++))
    if [ -d "static" ]; then
        check_file "static/index.html" || ((errors++))
    fi
    
    echo
    
    # Check file contents (basic validation)
    print_status "Validating key configuration files..."
    
    # Check Cargo.toml for required dependencies
    if grep -q "esp-idf-hal" Cargo.toml; then
        print_success "✓ esp-idf-hal dependency found"
    else
        print_error "✗ esp-idf-hal dependency missing"
        ((errors++))
    fi
    
    # Check for proper target in .cargo/config.toml
    if grep -q "xtensa-esp32-espidf" .cargo/config.toml; then
        print_success "✓ ESP32 target configured"
    else
        print_error "✗ ESP32 target not configured"
        ((errors++))
    fi
    
    # Check toolchain file
    if grep -q "nightly" rust-toolchain.toml; then
        print_success "✓ Nightly toolchain specified"
    else
        print_error "✗ Nightly toolchain not specified"
        ((errors++))
    fi
    
    # Check for GPIO2 configuration (NodeMCU LED)
    if grep -q "Gpio2" src/led.rs; then
        print_success "✓ NodeMCU LED GPIO configured"
    else
        print_error "✗ NodeMCU LED GPIO not configured"
        ((errors++))
    fi
    
    echo
    
    # Check permissions
    print_status "Checking file permissions..."
    if [ -x "build.sh" ]; then
        print_success "✓ build.sh is executable"
    else
        print_warning "⚠ build.sh is not executable"
        print_status "Fixing permissions..."
        chmod +x build.sh
    fi
    
    echo
    
    # Summary
    if [ $errors -eq 0 ]; then
        print_success "✅ Project validation passed! All files are in place."
        echo
        print_status "Next steps:"
        echo "1. Configure WiFi credentials in src/config.rs"
        echo "2. Install dependencies: ./build.sh check"
        echo "3. Build and flash: ./build.sh all"
        echo "4. Monitor: ./build.sh monitor"
        echo
        print_status "Access the web interface at http://<esp32-ip> after flashing"
    else
        print_error "❌ Project validation failed with $errors errors"
        echo
        print_status "Please fix the missing files or configurations above."
        exit 1
    fi
}

# Run validation
main