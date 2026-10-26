#!/usr/bin/env bash
# Build and run Wokwi scenarios from sim/wokwi/scenarios/manifest.json.
#
# USAGE:
#   ./scripts/sim-wokwi-check.sh --list
#   ./scripts/sim-wokwi-check.sh --build-only
#   ./scripts/sim-wokwi-check.sh --scenario f1-smoke --scenario f2-spi
#   ./scripts/sim-wokwi-check.sh --all
#
# Wokwi execution requires wokwi-cli and WOKWI_CLI_TOKEN. `--build-only`
# prepares the images without touching Wokwi and is safe in ordinary CI.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if ! command -v python3 >/dev/null 2>&1; then
    echo "python3 is required to read the scenario manifest." >&2
    exit 2
fi

exec python3 "${ROOT}/scripts/sim-wokwi-check.py" "$@"
