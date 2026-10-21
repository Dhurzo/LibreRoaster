#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Relative to the Wokwi project root (sim/wokwi), per --scenario semantics.
SCENARIO="${1:-scenarios/f3-roast.yaml}"
LOG="${2:-$ROOT/target/wokwi-f3-roast.log}"

if [[ ! -f sim/wokwi/libreroaster.bin ]]; then
    echo "Missing sim/wokwi/libreroaster.bin." >&2
    echo "Build the F3 image first:" >&2
    echo "  FEATURES=embedded,simulated-typical-roast ./scripts/sim-wokwi-prepare.sh" >&2
    exit 2
fi

if ! command -v wokwi-cli >/dev/null 2>&1; then
    echo "wokwi-cli is not on PATH." >&2
    echo "Install it from https://docs.wokwi.com/wokwi-ci/cli-installation" >&2
    exit 2
fi

if [[ -z "${WOKWI_CLI_TOKEN:-}" ]]; then
    echo "WOKWI_CLI_TOKEN is not set." >&2
    echo "Create a CI token at https://wokwi.com/dashboard/ci and export it." >&2
    exit 2
fi

mkdir -p "$(dirname "$LOG")"

echo "Running $SCENARIO (F3 image; 90 s simulation timeout)"
if ! wokwi-cli sim/wokwi \
    --scenario "$SCENARIO" \
    --timeout 90000 \
    --fail-text "safety_fault" \
    --serial-log-file "$LOG"; then
    echo "Wokwi scenario failed. Serial log: $LOG" >&2
    exit 1
fi

if grep -Eq 'safety_fault|Emergency shutdown|BT RoR|Control update error' "$LOG"; then
    echo "Safety/latch text found in the serial log: $LOG" >&2
    grep -En 'safety_fault|Emergency shutdown|BT RoR|Control update error' "$LOG" >&2 || true
    exit 1
fi

echo "Scenario passed (no latch; green LED high in Heating, low after PID;OFF)."
echo "Serial log: $LOG"
