#!/usr/bin/env python3
"""
LibreRoaster Hardware Test Runner

CLI tool that automates Tier 2 HIL testing:
  build → flash → capture → parse → validate → report

Usage:
  python tests/hardware/hardware_test_runner.py --port /dev/ttyACM0 --example hil_tc
  python tests/hardware/hardware_test_runner.py --list
  python tests/hardware/hardware_test_runner.py --dry-run --example hil_ssr
"""

import argparse
import json
import os
import subprocess
import sys
import time
from datetime import datetime

import serial

from hardware_test_helpers import (
    HardwareTestReport,
    HardwareTestResult,
    load_thresholds,
    parse_test_output,
)

TARGET = 'riscv32imc-unknown-none-elf'
EXAMPLES = ['hil_tc', 'hil_ssr', 'hil_c1', 'hil_fan', 'hil_gpio']
BAUD = 115200
SUITE_COMPLETE_PREFIX = 'TESTSUITE:COMPLETE'
BUILD_ARTIFACT_DIR = os.path.join('target', TARGET, 'release', 'examples')


def _fail(msg: str, code: int = 1) -> None:
    print(f'ERROR: {msg}', file=sys.stderr)
    sys.exit(code)


def _run_cmd(cmd: list, label: str, timeout: int = 300) -> None:
    """Run *cmd* via subprocess. Exits on non-zero return code."""
    print(f'[build] {label}: {" ".join(cmd)}')
    proc = None
    try:
        proc = subprocess.run(cmd, timeout=timeout)
    except FileNotFoundError:
        _fail(f'{label}: command not found — is {" ".join(cmd[:2])} installed?')
    except subprocess.TimeoutExpired:
        _fail(f'{label}: timed out after {timeout}s')
    if proc is not None and proc.returncode != 0:
        _fail(f'{label}: exited with code {proc.returncode}')


def build_example(example: str) -> str:
    """Build the firmware example and return the binary path."""
    artifact = os.path.join(BUILD_ARTIFACT_DIR, example)
    _run_cmd(
        [
            'cargo', 'build', '--release',
            '--target', TARGET,
            '--example', example,
            '--features', 'embedded',
        ],
        f'Build {example}',
    )
    if not os.path.isfile(artifact):
        _fail(f'Build succeeded but artifact not found at {artifact}')
    return artifact


def flash_firmware(artifact: str, port: str) -> None:
    """Flash *artifact* to the ESP32-C3 via *port*."""
    _run_cmd(
        ['espflash', 'flash', '--port', port, artifact],
        f'Flash {os.path.basename(artifact)}',
    )


def capture_serial(port: str, timeout: int) -> list:
    """Open serial port and collect lines until TESTSUITE:COMPLETE or timeout.

    Returns a list of raw line strings (stripped).
    """
    lines: list = []
    deadline = time.monotonic() + timeout
    print(f'[capture] Opening {port} at {BAUD} baud, timeout={timeout}s ...')

    try:
        ser = serial.Serial(port, BAUD, timeout=2)
    except serial.SerialException as exc:
        _fail(f'Cannot open serial port {port}: {exc}')
        return []  # unreachable — _fail exits, but satisfies type checker

    try:
        while time.monotonic() < deadline:
            raw = ser.readline()
            if not raw:
                continue
            text = raw.decode('utf-8', errors='replace').strip()
            if not text:
                continue
            lines.append(text)
            print(f'  {text}')
            if text.startswith(SUITE_COMPLETE_PREFIX):
                break
    finally:
        ser.close()

    return lines


def parse_results(lines: list) -> list:
    """Extract ``HardwareTestResult`` objects from captured serial *lines*."""
    results = []
    for line in lines:
        parsed = parse_test_output(line)
        if parsed is None:
            continue
        name, passed, detail = parsed
        results.append(HardwareTestResult(
            name=name,
            passed=passed,
            detail=detail,
            timestamp=datetime.utcnow().isoformat(),
        ))
    return results


def validate_against_thresholds(results: list, thresholds: dict) -> list:
    """Apply threshold checks to parsed results. Returns a list of
    ``(result, warnings)`` pairs where *warnings* is a list of strings
    for any threshold violations detected from result details.
    """
    validated = []
    for r in results:
        warnings = []
        if not thresholds:
            validated.append((r, warnings))
            continue

        detail = r.detail

        temp_min = thresholds.get('ambient_temp_min')
        temp_max = thresholds.get('ambient_temp_max')
        if temp_min is not None and temp_max is not None:
            for token in detail.split(','):
                if '=' not in token:
                    continue
                key, _, val = token.partition('=')
                try:
                    v = float(val)
                except ValueError:
                    continue
                if key in ('temp', 'et', 'bt', 'ambient'):
                    if v < temp_min or v > temp_max:
                        warnings.append(
                            f'{key}={v} outside [{temp_min}, {temp_max}]'
                        )

        if not r.passed:
            warnings.append('Firmware reported FAIL')

        validated.append((r, warnings))
    return validated


def write_run_artifacts(
    report: HardwareTestReport,
    example: str,
    runs_dir: str,
) -> str:
    """Write JSON + Markdown artifacts to ``<runs_dir>/<example>/<timestamp>/``.

    Returns the run directory path.
    """
    ts = datetime.utcnow().strftime('%Y%m%dT%H%M%SZ')
    run_dir = os.path.join(runs_dir, example, ts)
    os.makedirs(run_dir, exist_ok=True)

    json_path = os.path.join(run_dir, 'results.json')
    with open(json_path, 'w') as f:
        f.write(report.to_json())

    md_path = os.path.join(run_dir, 'report.md')
    with open(md_path, 'w') as f:
        f.write(report.to_markdown())

    print(f'[report] Artifacts written to {run_dir}')
    return run_dir


def list_examples() -> None:
    print('Available HIL test firmware examples:')
    for ex in EXAMPLES:
        print(f'  {ex}')


def run(args: argparse.Namespace) -> None:
    thresholds = {}
    if args.thresholds:
        thresholds = load_thresholds(args.thresholds)

    example = args.example

    artifact = build_example(example)
    flash_firmware(artifact, args.port)

    if args.dry_run:
        print('[dry-run] Skipping serial capture and validation.')
        return

    lines = capture_serial(args.port, args.timeout)
    if not lines:
        _fail('No output received from firmware before timeout')

    results = parse_results(lines)
    if not results:
        _fail(
            'Serial output captured but no TEST: lines found. '
            'Raw output:\n  ' + '\n  '.join(lines[:20])
        )

    validated = validate_against_thresholds(results, thresholds)

    report = HardwareTestReport(example_name=example)
    for r, warnings in validated:
        report.add_result(r)
        for w in warnings:
            print(f'  WARN: {r.name}: {w}')

    run_dir = write_run_artifacts(report, example, args.runs_dir)

    print(f'\n{"=" * 60}')
    print(f'  {report.summary()}')
    print(f'  Run dir: {run_dir}')
    print(f'{"=" * 60}')

    if not report.all_passed:
        sys.exit(1)


def main() -> None:
    parser = argparse.ArgumentParser(
        description='LibreRoaster Tier 2 HIL Test Runner',
    )
    parser.add_argument(
        '--port', type=str,
        help='Serial port for ESP32-C3 (e.g. /dev/ttyACM0)',
    )
    parser.add_argument(
        '--example', type=str, choices=EXAMPLES,
        help='Test firmware example to build, flash, and validate',
    )
    parser.add_argument(
        '--thresholds', type=str,
        default='tests/hardware/hardware_thresholds.json',
        help='Path to hardware_thresholds.json',
    )
    parser.add_argument(
        '--runs-dir', type=str, default='tests/hardware/runs',
        help='Directory to store run artifacts',
    )
    parser.add_argument(
        '--timeout', type=int, default=60,
        help='Serial capture timeout in seconds (default: 60)',
    )
    parser.add_argument(
        '--dry-run', action='store_true',
        help='Only build and flash — skip serial capture and validation',
    )
    parser.add_argument(
        '--list', action='store_true', dest='list_examples',
        help='List available test firmware examples and exit',
    )

    args = parser.parse_args()

    if args.list_examples:
        list_examples()
        return

    if not args.example:
        parser.error('--example is required (use --list to see options)')
    if not args.port:
        parser.error('--port is required')

    run(args)


if __name__ == '__main__':
    main()
