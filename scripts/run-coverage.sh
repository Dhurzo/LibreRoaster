#!/usr/bin/env bash
# ── Code coverage runner ──────────────────────────────────────────────
# Runs cargo-llvm-cov on the host test suite and generates reports.
#
# Usage:
#   ./scripts/run-coverage.sh            # HTML report + stdout summary
#   ./scripts/run-coverage.sh --ci       # Lcov report for CI upload
#   ./scripts/run-coverage.sh --html     # HTML report only
#
# Requires: cargo-llvm-cov (install: cargo install cargo-llvm-cov)
# ──────────────────────────────────────────────────────────────────────

set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

COV="cargo llvm-cov --target x86_64-unknown-linux-gnu --features test --no-fail-fast"

case "${1:-}" in
  --ci)
    echo "=== Generating lcov coverage report for CI ==="
    $COV --lcov --output-path target/coverage/lcov.info
    echo "=== Report saved to target/coverage/lcov.info ==="
    ;;
  --html)
    echo "=== Generating HTML coverage report ==="
    $COV --html --output-dir target/coverage/html
    echo "=== Report saved to target/coverage/html/index.html ==="
    ;;
  *)
    echo "=== Running coverage with stdout summary ==="
    $COV --summary-only
    ;;
esac
