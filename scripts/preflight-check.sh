#!/usr/bin/env bash
# Preflight check — run before flashing firmware to ESP32-C3.
# Validates pin assignments, wiring consistency, and pin behaviour
# without requiring real hardware.
set -euo pipefail

PASS=0
FAIL=0

run_check() {
    local name="$1"
    shift
    if "$@" > /dev/null 2>&1; then
        echo "  ✅ $name"
        ((PASS++))
    else
        echo "  ❌ $name"
        ((FAIL++))
    fi
}

run_check_verbose() {
    local name="$1"
    shift
    if "$@"; then
        ((PASS++))
    else
        ((FAIL++))
    fi
}

echo ""
echo "╔══════════════════════════════════════════════════════════╗"
echo "║         LibreRoaster ESP32-C3 Preflight Check           ║"
echo "╚══════════════════════════════════════════════════════════╝"
echo ""

HOST_TARGET="x86_64-unknown-linux-gnu"
ESP_TARGET="riscv32imc-unknown-none-elf"

echo "1. Pinout validation tests"
run_check "No duplicate GPIO assignments"      cargo test --target "$HOST_TARGET" --test pinout_validation -- no_duplicate_gpio_assignments
run_check "No forbidden pins used (GPIO2)"    cargo test --target "$HOST_TARGET" --test pinout_validation -- no_forbidden_pins_used
run_check "Strapping pins have constraints"   cargo test --target "$HOST_TARGET" --test pinout_validation -- all_strapping_pins_have_required_constraints
run_check "SPI MISO is input"                 cargo test --target "$HOST_TARGET" --test pinout_validation -- spi_miso_is_input
run_check "UART RX=input TX=output"           cargo test --target "$HOST_TARGET" --test pinout_validation -- uart_rx_is_input_tx_is_output
run_check "LEDC PWM pins are output"          cargo test --target "$HOST_TARGET" --test pinout_validation -- all_ledc_pwm_pins_are_output
run_check "Heat detection input w/ pull-up"   cargo test --target "$HOST_TARGET" --test pinout_validation -- heat_detection_is_input_with_pullup
run_check "Fan PWM GPIO9 has flyback diode"   cargo test --target "$HOST_TARGET" --test pinout_validation -- fan_pwm_gpio9_has_flyback_constraint
run_check "Pin constants match pin table"     cargo test --target "$HOST_TARGET" --test pinout_validation -- pin_constants_match_pin_table
run_check "Wiring manifest ↔ pin table"       cargo test --target "$HOST_TARGET" --test pinout_validation -- wiring_manifest_and_pin_table_are_bidirectionally_consistent
run_check "Init uses correct pins"            cargo test --target "$HOST_TARGET" --test pinout_validation -- init_rs_uses_pins_from_constants

echo ""
echo "2. Digital twin (pin behaviour simulation)"
run_check "SSR ON → GPIO1 reads LOW"          cargo test --target "$HOST_TARGET" --test pin_digital_twin -- ssr_conducting_means_heat_detection_reads_low
run_check "SSR OFF → GPIO1 reads HIGH"        cargo test --target "$HOST_TARGET" --test pin_digital_twin -- ssr_off_means_heat_detection_reads_high_via_pullup
run_check "Emergency stop: SSR LOW fan HIGH"  cargo test --target "$HOST_TARGET" --test pin_digital_twin -- emergency_stop_drives_ssr_low_and_fan_high
run_check "SPI CS mutual exclusion"           cargo test --target "$HOST_TARGET" --test pin_digital_twin -- both_spi_cs_are_never_low_simultaneously
run_check "GPIO1 pull-up holds HIGH floating" cargo test --target "$HOST_TARGET" --test pin_digital_twin -- heat_detection_pullup_holds_high_when_floating
run_check "GPIO2 never used"                  cargo test --target "$HOST_TARGET" --test pin_digital_twin -- gpio2_is_never_touched
run_check "All output pins toggle"            cargo test --target "$HOST_TARGET" --test pin_digital_twin -- all_output_pins_can_toggle

echo ""
echo "3. Embedded target compilation"
run_check "ESP32-C3 firmware compiles"        cargo check --target "$ESP_TARGET" --features embedded

echo ""
echo "════════════════════════════════════════════════════════════"
echo "  Results: $PASS passed, $FAIL failed"
echo "════════════════════════════════════════════════════════════"

if [ "$FAIL" -eq 0 ]; then
    echo ""
    echo "  🟢 PREFLIGHT PASSED — safe to flash to ESP32-C3"
    echo ""
    exit 0
else
    echo ""
    echo "  🔴 PREFLIGHT FAILED — fix errors before flashing"
    echo ""
    exit 1
fi
