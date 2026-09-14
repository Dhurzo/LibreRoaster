#!/bin/bash
# Dead-code removal batch runner
# Applies trusted modules by batch while gating on test/baseline evidence.

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

BATCH_NAME="${BATCH_NAME:-}"
MODULES="${MODULES:-}"

if [ -z "$BATCH_NAME" ]; then
  echo "ERROR: BATCH_NAME is required"
  exit 1
fi

if [ -z "$MODULES" ]; then
  echo "ERROR: MODULES list is required"
  exit 1
fi

BATCH_ROOT="$REPO_ROOT/quality/dead-code/batches"
BATCH_FILE="$BATCH_ROOT/${BATCH_NAME}.md"
TEST_LOG="$BATCH_ROOT/${BATCH_NAME}-cargo-test.log"
BASELINE_LOG="$BATCH_ROOT/${BATCH_NAME}-quality-baseline.log"

mkdir -p "$BATCH_ROOT"

declare -a MODULE_ARRAY
read -r -a MODULE_ARRAY <<< "$MODULES"

write_module_snapshot() {
  local title="$1"
  {
    echo "## ${title}"
    for module in "${MODULE_ARRAY[@]}"; do
      echo "- $module"
    done
    echo ""
  } >> "$BATCH_FILE"
}

echo "# Dead-code batch: $BATCH_NAME" > "$BATCH_FILE"
echo "- Recorded at: $(date -u +'%Y-%m-%dT%H:%M:%SZ')" >> "$BATCH_FILE"
echo "" >> "$BATCH_FILE"

write_module_snapshot "Pre-removal modules"

echo "== Removal stage: candidates referenced from MODULES list" >> "$BATCH_FILE"
echo "" >> "$BATCH_FILE"

run_command() {
  local cmd_description="$1"
  local log_path="$2"
  shift 2
  echo "Running: ${cmd_description}"
  if "$@" > "$log_path" 2>&1; then
    echo "${cmd_description} succeeded (logs: ${log_path})"
    return 0
  else
    local rc=$?
    echo "${cmd_description} failed with exit ${rc} (logs: ${log_path})"
    return $rc
  fi
}

run_command "cargo test --locked --lib --tests --no-fail-fast" "$TEST_LOG" cargo test --locked --lib --tests --no-fail-fast
TEST_EXIT=$?

run_command "scripts/quality-baseline.sh" "$BASELINE_LOG" bash "$SCRIPT_DIR/quality-baseline.sh"
BASELINE_EXIT=$?

write_module_snapshot "Post-removal modules"

SUMMARY_STATUS="PASS"
if [ $TEST_EXIT -ne 0 ] || [ $BASELINE_EXIT -ne 0 ]; then
  SUMMARY_STATUS="FAIL"
fi

{
  echo "## Gate summary"
  echo "- Status: $SUMMARY_STATUS"
  echo "- Cargo test log: $TEST_LOG"
  echo "- Quality baseline log: $BASELINE_LOG"
  echo "- Quality baseline exit code: $BASELINE_EXIT"
  echo ""
} >> "$BATCH_FILE"

echo "Batch $BATCH_NAME summary: status=$SUMMARY_STATUS, test_log=$TEST_LOG, baseline_exit=$BASELINE_EXIT"

if [ "$SUMMARY_STATUS" = "PASS" ]; then
  exit 0
else
  exit 1
fi
