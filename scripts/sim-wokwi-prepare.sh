#!/bin/bash
# sim-wokwi-prepare.sh — Build the firmware and prepare the Wokwi project.
#
# USAGE:
#   ./scripts/sim-wokwi-prepare.sh
#   FEATURES=embedded,heat-sense ./scripts/sim-wokwi-prepare.sh
#   FEATURES=embedded,simulated-typical-roast ./scripts/sim-wokwi-prepare.sh
#
# Produces in sim/wokwi/:
#   libreroaster.bin  merged flash image (bootloader + partitions + app)
#   libreroaster.elf  ELF with symbols for the Wokwi debugger
#   chips/max31856.chip.wasm  compiled custom MAX31856 chip
#
# Then open sim/wokwi/ in VS Code with the Wokwi extension and run
# "Wokwi: Start Simulator". The first run asks for a free license.
#
# PREREQUISITES:
#   - Rust target riscv32imc-unknown-none-elf installed
#   - espflash on PATH
#   - wokwi-cli on PATH (optional; only needed to rebuild the custom chip)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

FEATURES="${FEATURES:-embedded,simulated-sensors}"
TARGET="riscv32imc-unknown-none-elf"
ELF="target/${TARGET}/release/libreroaster"
PROJECT_DIR="sim/wokwi"
BIN="${PROJECT_DIR}/libreroaster.bin"
PROJECT_ELF="${PROJECT_DIR}/libreroaster.elf"
CHIP_SRC="${PROJECT_DIR}/chips/max31856.chip.c"
CHIP_WASM="${PROJECT_DIR}/chips/max31856.chip.wasm"
WOKWI_CLI="${WOKWI_CLI:-wokwi-cli}"

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${YELLOW}==> Building firmware (${FEATURES})${NC}"
cargo build --release --target "$TARGET" --features "$FEATURES"

if ! command -v espflash >/dev/null 2>&1; then
    echo -e "${RED}ERROR: espflash not found on PATH.${NC}"
    echo "Install it with: cargo install espflash --locked"
    exit 1
fi

mkdir -p "$PROJECT_DIR/chips"

echo -e "${YELLOW}==> Creating merged flash image${NC}"
espflash save-image --chip esp32c3 --merge "$ELF" "$BIN"

echo -e "${YELLOW}==> Copying ELF symbols${NC}"
cp "$ELF" "$PROJECT_ELF"

if command -v "$WOKWI_CLI" >/dev/null 2>&1; then
    echo -e "${YELLOW}==> Compiling custom MAX31856 chip${NC}"
    "$WOKWI_CLI" chip compile "$CHIP_SRC" -o "$CHIP_WASM"
else
    echo -e "${YELLOW}WARNING: ${WOKWI_CLI} not found; using the existing ${CHIP_WASM}.${NC}"
    echo "Install wokwi-cli to compile the custom chip (see docs/SIMULATION.md)."
fi

if [ ! -f "$CHIP_WASM" ]; then
    echo -e "${RED}ERROR: ${CHIP_WASM} is missing; the custom chip cannot load.${NC}"
    exit 1
fi

echo
echo -e "${GREEN}Wokwi project ready: ${PROJECT_DIR}/${NC}"
echo "  1. Open ${PROJECT_DIR}/ in VS Code (Wokwi extension installed)."
echo "  2. Run 'Wokwi: Start Simulator' (accept the free license prompt)."
echo "  3. Open the serial monitor at 115200 baud and send: READ"
echo "  4. In F2 (FEATURES=embedded,heat-sense), move the MAX31856 sliders"
echo "     to exercise the real SPI driver and the GPIO1 heat switch."
echo "  5. For a guard-safe START/Heating check, build F3 with"
echo "     FEATURES=embedded,simulated-typical-roast."
