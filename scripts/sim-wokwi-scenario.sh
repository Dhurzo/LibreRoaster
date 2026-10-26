#!/usr/bin/env bash
# Backwards-compatible wrapper for the manifest-based scenario runner.
#
# USAGE:
#   ./scripts/sim-wokwi-scenario.sh
#   ./scripts/sim-wokwi-scenario.sh scenarios/f1-smoke.yaml
#   ./scripts/sim-wokwi-scenario.sh scenarios/f1-smoke.yaml /tmp/f1.log
#
# The second argument is treated as a log directory override; the runner names
# the file <scenario-id>.log inside it. New scripts should call
# sim-wokwi-check.sh directly.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCENARIO="${1:-scenarios/f3-roast.yaml}"
SCENARIO_ID="$(basename "$SCENARIO" .yaml)"
LOG="${2:-}"

ARGS=(--scenario "$SCENARIO_ID")
if [[ -n "$LOG" ]]; then
    ARGS+=(--log-dir "$(dirname "$LOG")")
fi

exec "${ROOT}/scripts/sim-wokwi-check.sh" "${ARGS[@]}"
