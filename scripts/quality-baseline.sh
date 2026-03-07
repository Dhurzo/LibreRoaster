#!/bin/bash
# Quality Baseline Runner
# Deterministic gate execution with policy-aware output
# Policy: QG-POLICY v1.0.0

set -o pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
POLICY_FILE="${SCRIPT_DIR}/../.planning/quality/baseline-policy.toml"
EVALUATOR_SCRIPT="${SCRIPT_DIR}/quality_baseline.py"

# Detect toolchain and context
TOOLCHAIN=$(cat rust-toolchain.toml 2>/dev/null | grep 'channel' | cut -d'"' -f2 || echo "stable")
RUST_VERSION=$(rustc --version 2>/dev/null | cut -d' ' -f2 || echo "unknown")

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Temporary files for gate outputs
TEMP_DIR=$(mktemp -d)
FMT_OUTPUT="${TEMP_DIR}/fmt.output"
CLIPPY_OUTPUT="${TEMP_DIR}/clippy.output"
TEST_OUTPUT="${TEMP_DIR}/test.output"

# Gate exit codes
FMT_EXIT=0
CLIPPY_EXIT=0
TEST_EXIT=0

# Cleanup on exit
trap 'rm -rf "${TEMP_DIR}"' EXIT

print_header() {
    echo "=============================================="
    echo "  LibreRoaster Quality Baseline Runner"
    echo "=============================================="
    echo "Policy:     QG-POLICY"
    echo "Version:    1.0.0"
    echo "Toolchain:  ${TOOLCHAIN}"
    echo "Rust:       ${RUST_VERSION}"
    echo "Date:       $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
    echo "=============================================="
    echo ""
}

print_gate_header() {
    echo -e "${YELLOW}[GATE]${NC} $1"
    echo "----------------------------------------------"
}

print_gate_result() {
    local gate_name=$1
    local exit_code=$2
    
    if [ $exit_code -eq 0 ]; then
        echo -e "${GREEN}[PASS]${NC} ${gate_name}"
    else
        echo -e "${RED}[FAIL]${NC} ${gate_name}"
    fi
}

# Parse policy version from TOML
get_policy_version() {
    if [ -f "$POLICY_FILE" ]; then
        grep 'policy_version' "$POLICY_FILE" | cut -d'"' -f2
    else
        echo "1.0.0"
    fi
}

# =============================================================================
# GATE 1: Format Check (rustfmt)
# =============================================================================
run_fmt_gate() {
    print_gate_header "Format Check (cargo fmt --check)"
    
    # Run fmt with locked dependencies
    if cargo fmt --all -- --check > "${FMT_OUTPUT}" 2>&1; then
        echo "✓ Code formatting OK"
        FMT_EXIT=0
    else
        echo "✗ Code formatting violations found"
        FMT_EXIT=1
    fi
    
    print_gate_result "fmt" $FMT_EXIT
    echo ""
    return $FMT_EXIT
}

# =============================================================================
# GATE 2: Clippy Lint Check
# =============================================================================
run_clippy_gate() {
    print_gate_header "Clippy Lint Check"
    
    # Run clippy with curated flags and JSON output for machine parsing
    # Using -W (warn) to collect all findings, then evaluator applies tier policy
    # Note: --message-format=json is a cargo flag, not clippy flag
    if cargo clippy --locked --message-format=json --all-targets -- -W clippy::unwrap_used -W clippy::expect_used -W clippy::panic > "${CLIPPY_OUTPUT}" 2>&1; then
        echo "✓ Clippy passed (no blocking findings)"
        CLIPPY_EXIT=0
    else
        # Capture exit code - clippy returns non-zero if warnings found
        CLIPPY_EXIT=$?
        echo "✗ Clippy found lint issues"
    fi
    
    print_gate_result "clippy" $CLIPPY_EXIT
    echo ""
    return $CLIPPY_EXIT
}

# =============================================================================
# GATE 3: Test Execution
# =============================================================================
run_test_gate() {
    print_gate_header "Test Execution (cargo test)"
    
    # Run tests with locked deps, host-safe scope, no-fail-fast
    if cargo test --locked --lib --tests --no-fail-fast > "${TEST_OUTPUT}" 2>&1; then
        echo "✓ All tests passed"
        TEST_EXIT=0
    else
        TEST_EXIT=$?
        echo "✗ Test failures detected"
    fi
    
    print_gate_result "test" $TEST_EXIT
    echo ""
    return $TEST_EXIT
}

# =============================================================================
# Run Evaluator for Compact Summary
# =============================================================================
run_evaluator() {
    local policy_version
    policy_version=$(get_policy_version)
    
    # Run Python evaluator to produce compact summary
    python3 "${EVALUATOR_SCRIPT}" \
        --fmt-status "${FMT_EXIT}" \
        --clippy-json "${CLIPPY_OUTPUT}" \
        --test-status "${TEST_EXIT}" \
        --test-output "${TEST_OUTPUT}" \
        --policy-version "${policy_version}" \
        --policy-id "QG-POLICY"
}

# =============================================================================
# Main Execution
# =============================================================================
main() {
    print_header
    
    # Get policy version for display
    POLICY_VERSION=$(get_policy_version)
    
    # Execute gates in fixed order (fmt -> clippy -> test)
    # Do NOT stop at first failure - run all gates
    
    run_fmt_gate
    FMT_STATUS=$?
    
    run_clippy_gate
    CLIPPY_STATUS=$?
    
    run_test_gate
    TEST_STATUS=$?
    
    # Always run evaluator to produce summary
    run_evaluator
    EVALUATOR_EXIT=$?
    
    # Determine final verdict based on policy
    # Policy: Block on fmt failure, Tier 1 clippy findings, or test failure
    # Note: Final verdict is determined by evaluator
    
    if [ $EVALUATOR_EXIT -ne 0 ]; then
        echo ""
        echo -e "${RED}=============================================="
        echo -e "  FINAL VERDICT: FAIL"
        echo -e "==============================================${NC}"
        echo "same input, same verdict - QG-POLICY v${POLICY_VERSION}"
        exit 1
    else
        echo ""
        echo -e "${GREEN}=============================================="
        echo -e "  FINAL VERDICT: PASS"
        echo -e "==============================================${NC}"
        echo "same input, same verdict - QG-POLICY v${POLICY_VERSION}"
        exit 0
    fi
}

# Run main
main "$@"
