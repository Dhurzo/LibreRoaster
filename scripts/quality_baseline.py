#!/usr/bin/env python3
"""
Quality Baseline Policy Evaluator
Parses gate outputs, applies tier policy, and emits compact actionable summary.
"""

import argparse
import json
import os
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Optional


# =============================================================================
# Data Models
# =============================================================================

@dataclass
class Finding:
    """A single diagnostic finding from a gate."""
    gate: str          # fmt, clippy, test
    file_path: str     # Path to file with issue
    line: Optional[int]
    column: Optional[int]
    rule: str          # Rule identifier (e.g., "E0432", "clippy::unwrap_used")
    message: str       # Human-readable message
    severity: str      # error, warning
    tier: str         # t1_critical, t2_core, t3_support


@dataclass
class GateResult:
    """Result from a single gate execution."""
    name: str
    passed: bool
    findings: list[Finding]


# =============================================================================
# Policy Configuration
# =============================================================================

# Module to tier mapping (from baseline-policy.toml)
TIER_MAPPING = {
    # Tier 1: Critical - blocking
    "src/safety": "t1_critical",
    "src/control": "t1_critical",
    "src/input/parser.rs": "t1_critical",
    "src/output/artisan.rs": "t1_critical",
    "src/config": "t1_critical",
    
    # Tier 2: Core - informational
    "src/hardware": "t2_core",
    "src/application": "t2_core",
    "src/input/multiplexer.rs": "t2_core",
    "src/output/traits.rs": "t2_core",
    
    # Tier 3: Support - informational
    "src/logging": "t3_support",
    "src/common": "t3_support",
}

# Gate to policy prefix mapping
GATE_POLICY_PREFIX = {
    "fmt": "QG-FMT",
    "clippy": "QG-CLIPPY",
    "test": "QG-TEST",
}


# =============================================================================
# Tier Classification
# =============================================================================

def classify_tier(file_path: str) -> str:
    """
    Classify a file path into a tier based on the tier mapping.
    Default to t3_support if no match.
    """
    for prefix, tier in TIER_MAPPING.items():
        if file_path.startswith(prefix) or prefix in file_path:
            return tier
    return "t3_support"  # Default to lowest tier


def is_blocking_tier(tier: str) -> bool:
    """Check if a tier is blocking (Tier 1 only)."""
    return tier == "t1_critical"


# =============================================================================
# Output Parsing
# =============================================================================

def parse_clippy_json(json_path: str) -> list[Finding]:
    """
    Parse clippy JSON output into Finding objects.
    """
    findings = []
    
    if not os.path.exists(json_path):
        return findings
    
    with open(json_path, 'r') as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            
            try:
                msg = json.loads(line)
            except json.JSONDecodeError:
                continue
            
            # Only process compiler messages (diagnostics)
            if msg.get("reason") != "compiler-message":
                continue
            
            # Extract message content
            message = msg.get("message", {})
            spans = message.get("spans", [])
            
            # Get primary location
            file_path = "unknown"
            line_num = None
            column = None
            
            if spans:
                primary_span = spans[0]
                file_path = primary_span.get("file_name", "unknown")
                line_num = primary_span.get("line_start")
                column = primary_span.get("column_start")
            
            # Extract rule code
            code = message.get("code", {})
            rule = code.get("code", "") if code else ""
            
            # Message text
            msg_text = message.get("message", "")
            
            # Severity
            level = message.get("level", "warning")
            
            # Classify tier
            tier = classify_tier(file_path)
            
            finding = Finding(
                gate="clippy",
                file_path=file_path,
                line=line_num,
                column=column,
                rule=rule,
                message=msg_text,
                severity=level,
                tier=tier
            )
            findings.append(finding)
    
    return findings


def parse_fmt_output(output_path: str) -> list[Finding]:
    """
    Parse fmt output into Finding objects.
    """
    findings = []
    
    if not os.path.exists(output_path):
        return findings
    
    with open(output_path, 'r') as f:
        content = f.read()
    
    # rustfmt outputs list of unformatted files
    # Format: "path/to/file.rs" or "Diff in ..."
    for line in content.splitlines():
        line = line.strip()
        if not line:
            continue
        
        # Match file paths
        if line.endswith(".rs") and not line.startswith("Diff"):
            file_path = line
            tier = classify_tier(file_path)
            
            finding = Finding(
                gate="fmt",
                file_path=file_path,
                line=None,
                column=None,
                rule="E0384",  # rustfmt uses this internally
                message="File not formatted according to rustfmt",
                severity="warning",
                tier=tier
            )
            findings.append(finding)
    
    return findings


def parse_test_output(output_path: str) -> list[Finding]:
    """
    Parse test output into Finding objects.
    """
    findings = []
    
    if not os.path.exists(output_path):
        return findings
    
    with open(output_path, 'r') as f:
        content = f.read()
    
    # Parse test failures
    # Look for test name patterns: "test <name> ... FAILED"
    test_fail_pattern = re.compile(r'^test\s+(.+?)\s+\.\.\.\s+(FAILED|ok)$', re.MULTILINE)
    
    for match in test_fail_pattern.finditer(content):
        test_name = match.group(1)
        status = match.group(2)
        
        if status == "FAILED":
            # Determine file path from test name convention
            # Tests are typically in src/ or tests/
            file_path = "tests/"  # Default to tests dir
            
            tier = classify_tier(file_path)
            
            finding = Finding(
                gate="test",
                file_path=file_path,
                line=None,
                column=None,
                rule="TEST_FAIL",
                message=f"Test failed: {test_name}",
                severity="error",
                tier=tier
            )
            findings.append(finding)
    
    return findings


# =============================================================================
# Summary Generation
# =============================================================================

def generate_compact_summary(
    fmt_status: int,
    clippy_findings: list[Finding],
    test_status: int,
    policy_id: str,
    policy_version: str
) -> tuple[bool, list[Finding]]:
    """
    Generate compact summary and determine final verdict.
    
    Returns:
        (verdict_passed, all_findings)
    """
    all_findings = []
    
    # Add fmt findings
    # If fmt failed, add a finding
    if fmt_status != 0:
        finding = Finding(
            gate="fmt",
            file_path="multiple",
            line=None,
            column=None,
            rule="E0384",
            message="Formatting violations detected",
            severity="warning",
            tier="t1_critical"  # fmt issues are always blocking
        )
        all_findings.append(finding)
    
    # Add clippy findings
    all_findings.extend(clippy_findings)
    
    # Add test findings
    if test_status != 0:
        finding = Finding(
            gate="test",
            file_path="multiple",
            line=None,
            column=None,
            rule="TEST_FAIL",
            message="Test failures detected",
            severity="error",
            tier="t1_critical"  # test failures are always blocking
        )
        all_findings.append(finding)
    
    # Determine verdict: FAIL if any blocking findings exist
    blocking_findings = [f for f in all_findings if is_blocking_tier(f.tier)]
    verdict_passed = len(blocking_findings) == 0
    
    return verdict_passed, all_findings


def print_findings(findings: list[Finding], policy_id: str, policy_version: str) -> None:
    """
    Print findings in compact format grouped by gate.
    """
    if not findings:
        return
    
    print("")
    print("=" * 60)
    print(" FINDINGS")
    print("=" * 60)
    
    # Group by gate
    by_gate = {}
    for f in findings:
        if f.gate not in by_gate:
            by_gate[f.gate] = []
        by_gate[f.gate].append(f)
    
    # Print each gate's findings
    for gate_name, gate_findings in by_gate.items():
        print(f"\n[{policy_id}@{policy_version}] {gate_name.upper()} Findings:")
        print("-" * 50)
        
        for f in gate_findings:
            tier_marker = "T1 BLOCK" if is_blocking_tier(f.tier) else f.tier.upper().replace("_", " ")
            
            # Format: TIER FILE:LINE RULE - message
            location = ""
            if f.file_path and f.file_path != "multiple":
                location = f"{f.file_path}"
                if f.line:
                    location += f":{f.line}"
            
            print(f"  - {tier_marker:10} {location:40} {f.rule}")
            print(f"             {f.message[:60]}...")
    
    print("")


def print_summary(
    verdict_passed: bool,
    findings: list[Finding],
    policy_id: str,
    policy_version: str
) -> None:
    """
    Print the final summary.
    """
    # Count by tier
    t1_count = len([f for f in findings if f.tier == "t1_critical"])
    t2_count = len([f for f in findings if f.tier == "t2_core"])
    t3_count = len([f for f in findings if f.tier == "t3_support"])
    
    print("")
    print("=" * 60)
    print(" QUALITY BASELINE SUMMARY")
    print("=" * 60)
    print(f"Policy:     {policy_id} v{policy_version}")
    print(f"Tier 1 (Blocking):     {t1_count}")
    print(f"Tier 2 (Core):         {t2_count}")
    print(f"Tier 3 (Support):     {t3_count}")
    print("=" * 60)
    
    # Print detailed findings
    print_findings(findings, policy_id, policy_version)
    
    # Final verdict
    if verdict_passed:
        print(f"[{policy_id}@{policy_version}] VERDICT: PASS")
        print("All Tier 1 (blocking) requirements satisfied.")
    else:
        print(f"[{policy_id}@{policy_version}] VERDICT: FAIL")
        print("Tier 1 (blocking) issues must be resolved.")


# =============================================================================
# Fixture Mode (for intentional failure testing)
# =============================================================================

def load_fixture_mode(args) -> Optional[tuple[int, list[Finding], int]]:
    """
    Load from fixture mode if --from-json is provided.
    Returns (fmt_status, clippy_findings, test_status) or None.
    """
    if not args.from_json:
        return None
    
    fixture_path = args.from_json
    
    if not os.path.exists(fixture_path):
        print(f"Error: Fixture file not found: {fixture_path}", file=sys.stderr)
        return None
    
    with open(fixture_path, 'r') as f:
        data = json.load(f)
    
    fmt_status = data.get("fmt_status", 0)
    test_status = data.get("test_status", 0)
    clippy_findings = []
    
    # Parse clippy findings from fixture
    for cf in data.get("clippy_findings", []):
        finding = Finding(
            gate="clippy",
            file_path=cf.get("file_path", "unknown"),
            line=cf.get("line"),
            column=cf.get("column"),
            rule=cf.get("rule", ""),
            message=cf.get("message", ""),
            severity=cf.get("severity", "warning"),
            tier=cf.get("tier", "t3_support")
        )
        clippy_findings.append(finding)
    
    return fmt_status, clippy_findings, test_status


# =============================================================================
# Main
# =============================================================================

def main():
    parser = argparse.ArgumentParser(
        description="Quality Baseline Policy Evaluator",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Live run mode (called by quality-baseline.sh)
  %(prog)s --fmt-status 0 --clippy-json /tmp/clippy.json --test-status 0 \\
           --policy-version 1.0.0 --policy-id QG-POLICY

  # Fixture mode (for intentional failure testing)
  %(prog)s --from-json /path/to/fixture.json
"""
    )
    
    # Live mode arguments
    parser.add_argument("--fmt-status", type=int, help="Exit status of fmt gate (0=pass)")
    parser.add_argument("--clippy-json", type=str, help="Path to clippy JSON output file")
    parser.add_argument("--test-status", type=int, help="Exit status of test gate (0=pass)")
    parser.add_argument("--test-output", type=str, help="Path to test output file")
    parser.add_argument("--policy-version", type=str, default="1.0.0", help="Policy version")
    parser.add_argument("--policy-id", type=str, default="QG-POLICY", help="Policy ID")
    
    # Fixture mode arguments
    parser.add_argument(
        "--from-json", type=str,
        help="Load from JSON fixture instead of running gates (for intentional failure testing)"
    )
    
    args = parser.parse_args()
    
    # Check for fixture mode
    fixture_data = load_fixture_mode(args)
    
    if fixture_data:
        fmt_status, clippy_findings, test_status = fixture_data
    else:
        # Live mode: parse actual gate outputs
        fmt_status = args.fmt_status if args.fmt_status is not None else 0
        test_status = args.test_status if args.test_status is not None else 0
        
        # Parse clippy JSON
        clippy_findings = []
        if args.clippy_json:
            clippy_findings = parse_clippy_json(args.clippy_json)
    
    # Generate summary
    verdict_passed, all_findings = generate_compact_summary(
        fmt_status,
        clippy_findings,
        test_status,
        args.policy_id,
        args.policy_version
    )
    
    # Print summary
    print_summary(
        verdict_passed,
        all_findings,
        args.policy_id,
        args.policy_version
    )
    
    # Exit with appropriate code
    sys.exit(0 if verdict_passed else 1)


if __name__ == "__main__":
    main()
