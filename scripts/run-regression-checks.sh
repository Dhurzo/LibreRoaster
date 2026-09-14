#!/usr/bin/env bash
set -euo pipefail

RUN_ID=${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)-$RANDOM}
LOG_DIR="logs/regression/$RUN_ID"
mkdir -p "$LOG_DIR"

echo "Running quality baseline checks before regression tests..."
scripts/quality-baseline.sh

log_test() {
  NAME=$1
  shift
  LOG_PATH="$LOG_DIR/$NAME.log"
  printf "[%s] Running %s\n" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$NAME" | tee "$LOG_PATH"
  "$@" 2>&1 | tee -a "$LOG_PATH"
  local statuses=("${PIPESTATUS[@]}")
  printf "return_code = %s\n" "${statuses[0]}" >> "$LOG_PATH" || true
}

tests="command_idempotence command_multiplexer_concurrency artisan_integration_test mock_uart_integration"
for test in $tests; do
  case $test in
    artisan_integration_test)
      # Regression fixtures need the `regression` feature.
      # All four suites are gated on `feature = "test"` (see the
      # `#![cfg(all(test, feature = "test", ...))]` header in each test
      # file); without it they compile to 0 tests and the pipe goes
      # silently green. `--target` matches quality-baseline.sh so the
      # host Embassy time driver is linked.
      CMD="cargo test --test $test --features test,regression --target x86_64-unknown-linux-gnu"
      ;;
    *)
      CMD="cargo test --test $test --features test --target x86_64-unknown-linux-gnu"
      ;;
  esac
  log_test "$test" sh -c "$CMD"
done

SUMMARY_FILE="$LOG_DIR/summary.txt"
{
  printf 'run_id = "%s"\n' "$RUN_ID"
  printf 'log_path = "%s"\n' "$LOG_DIR"
  printf 'tests = "%s"\n' "$tests"
} > "$SUMMARY_FILE"

echo "Regression suite $RUN_ID logged to $SUMMARY_FILE"
