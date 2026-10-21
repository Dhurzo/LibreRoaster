#!/bin/bash
# sim-esp-emu.sh — Experimental firmware run in esp-emulator (no hardware).
#
# USAGE:
#   ./scripts/sim-esp-emu.sh
#   ESP_EMU=/path/to/esp-emu ./scripts/sim-esp-emu.sh
#   CONSOLE=uart0 ./scripts/sim-esp-emu.sh
#
# STATUS: EXPERIMENTAL / BLOCKED. esp-emulator v0.50.0 loads the bootloader and
# the application, but the firmware then hangs in the esp-hal software
# interrupt path used by esp-rtos:
#
#   esp_hal::interrupt::software::SoftwareInterrupt::raise
#     <- esp_rtos::run_queue::RunQueue::mark_task_ready
#     <- SchedulerState::resume_task
#
# `raise()` waits for the interrupt pending bit, which this emulator never sets;
# the watchdog then resets the chip in a loop. There is no CLI workaround and no
# public esp-emulator core source to patch. Use Wokwi (scripts/sim-wokwi-prepare.sh)
# for actual no-hardware validation.
#
# The script still builds the merged image and runs the emulator so the boot
# progress and the limitation can be reproduced. It is not part of the quality
# gates and is expected to time out or reset.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

FEATURES="${FEATURES:-embedded,simulated-sensors}"
TARGET="riscv32imc-unknown-none-elf"
ELF="target/${TARGET}/release/libreroaster"
OUT_DIR="target/esp-emu"
BIN="${OUT_DIR}/libreroaster-merged.bin"
ESP_EMU="${ESP_EMU:-esp-emu}"
CONSOLE="${CONSOLE:-usb-jtag}"
TIMEOUT="${TIMEOUT:-60}"

YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${YELLOW}==> WARNING: esp-emu support is experimental and currently blocked.${NC}"
echo "    The firmware boots and loads, then esp-rtos hits an unsupported"
echo "    software-interrupt path and the RWDT resets the chip."
echo

if ! command -v "$ESP_EMU" >/dev/null 2>&1 && [ ! -x "$ESP_EMU" ]; then
    echo -e "${RED}ERROR: esp-emu not found.${NC}"
    echo "Download a release from https://github.com/esp-emulator/esp-emulator/releases"
    echo "and set ESP_EMU=/path/to/esp-emu"
    exit 1
fi

echo -e "${YELLOW}==> Building firmware (${FEATURES})${NC}"
cargo build --release --target "$TARGET" --features "$FEATURES"

mkdir -p "$OUT_DIR"

echo -e "${YELLOW}==> Creating merged flash image${NC}"
espflash save-image --chip esp32c3 --merge "$ELF" "$BIN"

echo -e "${YELLOW}==> Running esp-emu (console: ${CONSOLE}, timeout: ${TIMEOUT}s)${NC}"
echo "    Expected: bootloader/app load, then a software-interrupt hang and RWDT reset."
echo

set +e
timeout "${TIMEOUT}s" "$ESP_EMU" \
    --chip esp32c3 \
    --firmware "$BIN" \
    --elf "$ELF" \
    --console "$CONSOLE"
status=$?
set -e

echo
if [ "$status" -eq 124 ]; then
    echo -e "${YELLOW}Timed out (expected for the current blocker).${NC}"
else
    echo "esp-emu exited with status ${status} (see docs/SIMULATION.md)."
fi
