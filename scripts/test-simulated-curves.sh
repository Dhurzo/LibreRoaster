#!/bin/bash
# test-simulated-curves.sh — Interactive verification of simulated roast curves
#
# USAGE (2 terminals):
#   Terminal A:  picocom /dev/ttyACM0 -b 115200
#   Terminal B:  ./scripts/test-simulated-curves.sh
#
# The script sends Artisan+ commands to a running LibreRoaster ESP32-C3 flashed
# with simulated sensors. Terminal A (picocom) shows the boot log and telemetry output.
#
# PREREQUISITES:
#   - ESP32-C3 flashed with: cargo espflash flash --release --target riscv32imc-unknown-none-elf --features "embedded,simulated-sensors"
#   - picocom running in another terminal BEFORE running this script
#   - /dev/ttyACM0 present and not held by espflash monitor

set -euo pipefail

PORT="${1:-/dev/ttyACM0}"
BAUD="${2:-115200}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# ---------------------------------------------------------------------------
# Sanity checks
# ---------------------------------------------------------------------------
if [ ! -e "$PORT" ]; then
    echo -e "${RED}ERROR: $PORT not found. Is the ESP32-C3 connected?${NC}"
    exit 1
fi

if [ ! -w "$PORT" ]; then
    echo -e "${RED}ERROR: $PORT not writable. Check permissions (sudo chmod 666 $PORT) or kill espflash monitor.${NC}"
    exit 1
fi

cat << 'EOF'

 ╔═══════════════════════════════════════════════════════════╗
 ║       LibreRoaster — Simulated Curves Test               ║
 ║                                                           ║
 ║  WATCH Terminal A (picocom) for telemetry output.         ║
 ║  This script only sends commands — verification is        ║
 ║  visual. You should see READ/STATUS responses and         ║
 ║  continuous telemetry lines every ~100ms.                 ║
 ╚═══════════════════════════════════════════════════════════╝

EOF

send_cmd() {
    local desc="$1"
    local cmd="$2"
    printf "${CYAN}→ %s${NC}  %s\n" "$desc" "$cmd"
    echo "$cmd" > "$PORT"
}

wait_tick() {
    local secs="${1:-1}"
    sleep "$secs"
}

# ---------------------------------------------------------------------------
# Phase 1: Handshake
# ---------------------------------------------------------------------------
echo -e "\n${YELLOW}═══ Phase 1: Handshake ═══${NC}"

send_cmd "Set USB channel"  "CHAN;0"
wait_tick 0.5

send_cmd "Set Celsius"      "UNITS;C"
wait_tick 0.5

# ---------------------------------------------------------------------------
# Phase 2: Single-shot readings (no continuous output yet)
# ---------------------------------------------------------------------------
echo -e "\n${YELLOW}═══ Phase 2: Single-shot readings ═══${NC}"

send_cmd "Read status (5-field)"  "READ"
wait_tick 0.5

send_cmd "Read diagnostics (19-field)"  "STATUS"
wait_tick 0.5

# ---------------------------------------------------------------------------
# Phase 3: SETTARGET + START (PID roast with continuous telemetry)
# ---------------------------------------------------------------------------
echo -e "\n${YELLOW}═══ Phase 3: SETTARGET + START (PID roast) ═══${NC}"
echo -e "${GREEN}After START, Terminal A should show continuous telemetry every ~100ms${NC}"

send_cmd "Set target to 200°C"  "SETTARGET 200"
wait_tick 1

send_cmd "Start roast (enables PID + continuous output)"  "START"
wait_tick 1

# Let simulated curve run for a few seconds — observe telemetry in picocom
echo -e "\n${CYAN}Observing telemetry for 5 seconds...${NC}"
echo    "Watch Terminal A for lines like:  120.0,180.5,150.3,3.2,0.0"
for i in $(seq 1 5); do
    sleep 1
    printf "  t=%ds\n" "$i"
done

# ---------------------------------------------------------------------------
# Phase 4: READ and STATUS during active roast
# ---------------------------------------------------------------------------
echo -e "\n${YELLOW}═══ Phase 4: READ / STATUS during active roast ═══${NC}"

send_cmd "READ during roast"  "READ"
wait_tick 0.5

send_cmd "STATUS during roast"  "STATUS"
wait_tick 0.5

# ---------------------------------------------------------------------------
# Phase 5: Manual actuation (OT1 / IO3) — verify manual override
# ---------------------------------------------------------------------------
echo -e "\n${YELLOW}═══ Phase 5: Manual actuation ═══${NC}"

send_cmd "Heater 50% (OT1)"  "OT1 50"
wait_tick 1

send_cmd "Fan 75% (IO3)"  "IO3 75"
wait_tick 1

send_cmd "STATUS after OT1/IO3"  "STATUS"
wait_tick 0.5

# ---------------------------------------------------------------------------
# Phase 6: STOP
# ---------------------------------------------------------------------------
echo -e "\n${YELLOW}═══ Phase 6: STOP ═══${NC}"

send_cmd "STOP roast"  "STOP"
wait_tick 1

send_cmd "Final READ (should show idle state)"  "READ"
wait_tick 0.5

send_cmd "Final STATUS"  "STATUS"
wait_tick 0.5

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
echo    ""
echo -e "${YELLOW}════════════════════════════════════════════════════════════${NC}"
echo -e "${YELLOW}  Commands sent. Now verify MANUALLY in Terminal A (picocom):${NC}"
echo    ""
echo    "  [ ] Boot log appeared ('LibreRoaster v5.1 starting...')"
echo    "  [ ] Handshake: CHAN;0 and UNITS;C acknowledged"
echo    "  [ ] READ returned 5-field TC4 response (AMB,ET,BT,...)"
echo    "  [ ] STATUS returned 19-field diagnostic line"
echo    "  [ ] SETTARGET 200: 'PID control re-enabled with target: 200.0°C'"
echo    "  [ ] START: 'Artisan+ roast started with target ...°C'"
echo    "  [ ] Continuous telemetry lines appeared (~100ms cadence)"
echo    "  [ ] OT1/IO3: heater/fan values changed in STATUS response"
echo    "  [ ] STOP: PID disabled, heater at 0%"
echo    ""
echo    "  If telemetry did NOT appear after START:"
echo    "    - Run 'cargo espflash flash ... --monitor' to check boot log"
echo    "    - Ensure GPIO1 pull-up is present (heat source detection)"
echo    "    - Send 'OT1 50' manually:  echo 'OT1 50' > /dev/ttyACM0"
echo -e "${YELLOW}════════════════════════════════════════════════════════════${NC}"
