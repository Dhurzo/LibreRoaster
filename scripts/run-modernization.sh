#!/usr/bin/env sh
set -euo pipefail

RUN_ID=${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)-$RANDOM}
LOG_DIR="logs/modernization/$RUN_ID"
mkdir -p "$LOG_DIR"

SKIP_REASON=${SKIP_REASON:-}

dump_step() {
  STEP_NAME=$1
  shift
  LOG_PATH="$LOG_DIR/step-$(printf "%02d" "$STEP_COUNTER")-$STEP_NAME.log"
  printf "[%s] Running %s\n" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$STEP_NAME" | tee "$LOG_PATH"
  "$@" 2>&1 | tee -a "$LOG_PATH"
  STEP_COUNTER=$((STEP_COUNTER + 1))
}

STEP_COUNTER=1

dump_step fmt cargo fmt

dump_step quality_baseline scripts/quality-baseline.sh

dump_step fix cargo fix --allow-dirty --allow-staged

dump_step clippy cargo clippy --all-targets --all-features -- -D warnings -D clippy::pedantic

SUMMARY_FILE="$LOG_DIR/summary.txt"
{
  printf 'run_id = "%s"\n' "$RUN_ID"
  printf 'log_path = "%s"\n' "$LOG_DIR"
  printf 'unsafe_register_changes = ""\n'
  if [ -n "$SKIP_REASON" ]; then
    printf 'skip_reason = "%s"\n' "$SKIP_REASON"
  fi
} > "$SUMMARY_FILE"

echo "Modernization run $RUN_ID logged to $SUMMARY_FILE"
