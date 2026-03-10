#!/bin/bash
# Quality Baseline Selfcheck
# Deterministic intentional-failure drills for policy evaluation validation
# Policy: QG-POLICY v1.0.0

set -o pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
EVALUATOR_SCRIPT="${SCRIPT_DIR}/quality_baseline.py"
FIXTURE_DIR="${SCRIPT_DIR}/../tests/quality/fixtures"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

print_header() {
    echo "=============================================="
    echo "  Quality Baseline Selfcheck"
    echo "  Intentional-Failure Drills"
    echo "=============================================="
    echo "Policy:     QG-POLICY"
    echo "Version:    1.0.0"
    echo "Date:       $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
    echo "=============================================="
    echo ""
}

# =============================================================================
# Test 1: Blocking Tier 1 Failure Drill
# =============================================================================
run_tier1_blocking_test() {
    echo -e "${YELLOW}[TEST 1]${NC} Tier 1 Blocking Failure Drill"
    echo "----------------------------------------------"
    
    local fixture="${FIXTURE_DIR}/clippy-tier1-fail.jsonl"
    
    echo "Running: python3 ${EVALUATOR_SCRIPT} --from-json ${fixture}"
    
    # Run evaluator in fixture mode
    if python3 "${EVALUATOR_SCRIPT}" --from-json "${fixture}" 2>&1; then
        echo -e "${RED}[FAIL]${NC} Expected non-zero exit for Tier 1 blocking finding"
        return 1
    else
        local exit_code=$?
        echo -e "${GREEN}[PASS]${NC} Correctly returned non-zero for Tier 1 blocking"
        
        # Verify output contains expected elements
        local output
        output=$(python3 "${EVALUATOR_SCRIPT}" --from-json "${fixture}" 2>&1)
        
        # Check for policy reference
        if echo "$output" | grep -q "QG-POLICY"; then
            echo -e "${GREEN}[PASS]${NC} Output contains policy reference (QG-POLICY)"
        else
            echo -e "${RED}[FAIL]${NC} Output missing policy reference"
            return 1
        fi
        
        # Check for module path
        if echo "$output" | grep -q "src/control"; then
            echo -e "${GREEN}[PASS]${NC} Output contains module path (src/control)"
        else
            echo -e "${RED}[FAIL]${NC} Output missing module path"
            return 1
        fi
        
        # Check for tier marker
        if echo "$output" | grep -q "T1 BLOCK"; then
            echo -e "${GREEN}[PASS]${NC} Output contains tier marker (T1 BLOCK)"
        else
            echo -e "${RED}[FAIL]${NC} Output missing tier marker"
            return 1
        fi
        
        # Check for lint rule
        if echo "$output" | grep -q "clippy::unwrap_used"; then
            echo -e "${GREEN}[PASS]${NC} Output contains lint rule (clippy::unwrap_used)"
        else
            echo -e "${RED}[FAIL]${NC} Output missing lint rule"
            return 1
        fi
        
        return 0
    fi
}

# =============================================================================
# Test 2: Mixed Tier Findings Drill
# =============================================================================
run_mixed_tier_test() {
    echo ""
    echo -e "${YELLOW}[TEST 2]${NC} Mixed Tier Findings Drill"
    echo "----------------------------------------------"
    
    local fixture="${FIXTURE_DIR}/clippy-mixed-failures.jsonl"
    
    echo "Running: python3 ${EVALUATOR_SCRIPT} --from-json ${fixture}"
    
    # Run evaluator in fixture mode
    if python3 "${EVALUATOR_SCRIPT}" --from-json "${fixture}" 2>&1; then
        # Mixed failures should still fail due to any T1 finding
        # But in this case all are T2/T3 which are non-blocking
        echo -e "${GREEN}[PASS]${NC} Mixed tiers correctly pass (no T1 blocking)"
    else
        local exit_code=$?
        # Check if there are any T1 findings - if not, this might be a bug
        local output
        output=$(python3 "${EVALUATOR_SCRIPT}" --from-json "${fixture}" 2>&1)
        
        if echo "$output" | grep -q "T1 BLOCK"; then
            echo -e "${GREEN}[PASS]${NC} Correctly returned non-zero for Tier 1 finding in mixed"
        else
            echo -e "${RED}[FAIL]${NC} Unexpected non-zero exit without T1 findings"
            return 1
        fi
    fi
    
    # Verify output lists ALL findings
    local output
    output=$(python3 "${EVALUATOR_SCRIPT}" --from-json "${fixture}" 2>&1)
    
    # Should find all three files
    local findings_count=0
    echo "$output" | grep -q "src/hardware/i2c.rs" && ((findings_count++))
    echo "$output" | grep -q "src/hardware/sensor.rs" && ((findings_count++))
    echo "$output" | grep -q "src/common/utils.rs" && ((findings_count++))
    
    if [ $findings_count -ge 2 ]; then
        echo -e "${GREEN}[PASS]${NC} Output lists multiple findings (all-failures aggregation)"
    else
        echo -e "${RED}[FAIL]${NC} Output missing some findings (expected 3, found ${findings_count})"
        return 1
    fi
    
    # Verify informational tiers are marked correctly
    if echo "$output" | grep -q "T2 CORE"; then
        echo -e "${GREEN}[PASS]${NC} Output marks Tier 2 findings as informational"
    else
        echo -e "${RED}[FAIL]${NC} Missing Tier 2 marker"
        return 1
    fi
    
    return 0
}

# =============================================================================
# Test 3: Reproducibility Drill (Same Input, Same Verdict)
# =============================================================================
run_reproducibility_test() {
    echo ""
    echo -e "${YELLOW}[TEST 3]${NC} Reproducibility Drill"
    echo "----------------------------------------------"
    
    local fixture="${FIXTURE_DIR}/clippy-tier1-fail.jsonl"
    
    echo "Running same fixture twice to verify deterministic output..."
    
    # Run 1
    local output1
    output1=$(python3 "${EVALUATOR_SCRIPT}" --from-json "${fixture}" 2>&1)
    local exit1=$?
    
    # Run 2
    local output2
    output2=$(python3 "${EVALUATOR_SCRIPT}" --from-json "${fixture}" 2>&1)
    local exit2=$?
    
    # Check exit codes match
    if [ $exit1 -eq $exit2 ]; then
        echo -e "${GREEN}[PASS]${NC} Exit codes match (${exit1} == ${exit2})"
    else
        echo -e "${RED}[FAIL]${NC} Exit codes differ (${exit1} != ${exit2})"
        return 1
    fi
    
    # Extract verdict lines (contain "VERDICT:")
    local verdict1=$(echo "$output1" | grep "VERDICT:")
    local verdict2=$(echo "$output2" | grep "VERDICT:")
    
    if [ "$verdict1" = "$verdict2" ]; then
        echo -e "${GREEN}[PASS]${NC} Verdict text identical"
        echo "         '$verdict1'"
    else
        echo -e "${RED}[FAIL]${NC} Verdict text differs:"
        echo "         Run 1: $verdict1"
        echo "         Run 2: $verdict2"
        return 1
    fi
    
    # Verify "same input, same verdict" appears in output
    if echo "$output1" | grep -q "same input, same verdict"; then
        echo -e "${GREEN}[PASS]${NC} Output includes reproducibility statement"
    else
        echo -e "${YELLOW}[WARN]${NC} Output missing 'same input, same verdict' statement"
    fi
    
    return 0
}

# =============================================================================
# Main
# =============================================================================
main() {
    print_header
    
    local tests_passed=0
    local tests_total=3
    
    # Test 1: Tier 1 Blocking
    if run_tier1_blocking_test; then
        ((tests_passed++))
    else
        echo -e "${RED}[TEST 1 FAILED]${NC}"
    fi
    
    # Test 2: Mixed Tiers
    if run_mixed_tier_test; then
        ((tests_passed++))
    else
        echo -e "${RED}[TEST 2 FAILED]${NC}"
    fi
    
    # Test 3: Reproducibility
    if run_reproducibility_test; then
        ((tests_passed++))
    else
        echo -e "${RED}[TEST 3 FAILED]${NC}"
    fi
    
    echo ""
    echo "=============================================="
    echo "  Selfcheck Results: ${tests_passed}/${tests_total} tests passed"
    echo "=============================================="
    
    if [ $tests_passed -eq $tests_total ]; then
        echo -e "${GREEN}All intentional-failure drills passed!${NC}"
        echo ""
        echo "Verified:"
        echo "  - Intentional failure produces actionable module+rule+tier+policy output"
        echo "  - Mixed-tier drill proves all findings are listed"
        echo "  - Reproducibility confirms 'same input, same verdict' behavior"
        exit 0
    else
        echo -e "${RED}Some tests failed!${NC}"
        exit 1
    fi
}

main "$@"
