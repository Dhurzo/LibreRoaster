"""
LibreRoaster Hardware Test Helpers

Shared utilities for parsing Tier 2 firmware serial output and building
hardware integration test reports. Used by hardware_test_runner.py and
any standalone validation scripts.

All parsers are pure-stdlib — no pytest, no unittest, no third-party deps.
"""

import json
import os
import re
from dataclasses import dataclass, field, asdict
from datetime import datetime
from typing import Optional, Tuple, Dict


# ---------------------------------------------------------------------------
# Line parsers
# ---------------------------------------------------------------------------

_TEST_RE = re.compile(r'^TEST:([^:]+):(?:(PASS|FAIL))(?::(.+))?$')

def parse_test_output(line: str) -> Optional[Tuple[str, bool, str]]:
    """Parse a ``TEST:<name>:<PASS|FAIL>[:<detail>]`` line from firmware output.

    Returns ``(test_name, passed, detail)`` or ``None`` if the line does not
    match the expected format.
    """
    m = _TEST_RE.match(line.strip())
    if not m:
        return None
    name, status, detail = m.group(1), m.group(2), m.group(3) or ''
    return (name, status == 'PASS', detail)


def parse_read_response(line: str) -> Optional[Dict[str, float]]:
    """Parse a TC4-style ``AMB,ET,BT,0.0,0.0`` READ response.

    Returns ``{ambient, et, bt}`` or ``None`` on parse failure.
    """
    parts = line.strip().split(',')
    if len(parts) != 5:
        return None
    try:
        return {
            'ambient': float(parts[0]),
            'et': float(parts[1]),
            'bt': float(parts[2]),
        }
    except ValueError:
        return None


_STATUS_FIELDS = [
    'et', 'bt', 'heater', 'fan', 'watchdog_flag', 'failure_count',
    'failure_reason', 'guard_timeouts', 'regression_flag', 'pv', 'mv',
    'integrator_value', 'derivative_value', 'saturation_flag',
    'integrator_clamp_flag', 'derivative_available_flag',
    'command_latency_us', 'max_command_latency_us',
]

def parse_status_response(line: str) -> Optional[Dict[str, str]]:
    """Parse the 18-field STATUS CSV produced by the firmware.

    Returns a dict keyed by field name (all values are strings, matching the
    existing validation_runner convention), or ``None`` on parse failure.
    """
    parts = line.strip().split(',')
    if len(parts) != 18:
        return None
    return dict(zip(_STATUS_FIELDS, parts))


# ---------------------------------------------------------------------------
# Validation helpers
# ---------------------------------------------------------------------------

def check_temp_in_range(temp: float, min_temp: float, max_temp: float) -> bool:
    """Return True if *temp* falls within [min_temp, max_temp] inclusive."""
    return min_temp <= temp <= max_temp


def check_duty_in_range(actual: float, expected: float, tolerance: float) -> bool:
    """Return True if *actual* is within ±*tolerance* of *expected*."""
    return abs(actual - expected) <= tolerance


# ---------------------------------------------------------------------------
# Data classes
# ---------------------------------------------------------------------------

@dataclass
class HardwareTestResult:
    """Single test result from firmware serial output."""
    name: str
    passed: bool
    detail: str
    timestamp: str = field(default_factory=lambda: datetime.utcnow().isoformat())


class HardwareTestReport:
    """Collects :class:`HardwareTestResult` instances and can serialize to
    JSON or Markdown.
    """

    def __init__(self, example_name: str = '', firmware_version: str = ''):
        self.example_name = example_name
        self.firmware_version = firmware_version
        self.results: list = []
        self.created_at: str = datetime.utcnow().isoformat()

    # -- mutation -----------------------------------------------------------

    def add_result(self, result: HardwareTestResult) -> None:
        self.results.append(result)

    # -- query --------------------------------------------------------------

    @property
    def total(self) -> int:
        return len(self.results)

    @property
    def passed_count(self) -> int:
        return sum(1 for r in self.results if r.passed)

    @property
    def failed_count(self) -> int:
        return self.total - self.passed_count

    @property
    def all_passed(self) -> bool:
        return self.total > 0 and self.failed_count == 0

    def summary(self) -> str:
        """Return compact verdict, e.g. ``6/6 PASS`` or ``4/6 FAIL``."""
        status = 'PASS' if self.all_passed else 'FAIL'
        return f'{self.passed_count}/{self.total} {status}'

    # -- serialization ------------------------------------------------------

    def to_dict(self) -> dict:
        return {
            'example_name': self.example_name,
            'firmware_version': self.firmware_version,
            'created_at': self.created_at,
            'summary': self.summary(),
            'total': self.total,
            'passed': self.passed_count,
            'failed': self.failed_count,
            'results': [asdict(r) for r in self.results],
        }

    def to_json(self, indent: int = 2) -> str:
        return json.dumps(self.to_dict(), indent=indent)

    def to_markdown(self) -> str:
        """Render a human-readable Markdown report."""
        lines = [
            f'# HIL Test Report: {self.example_name or "unknown"}',
            '',
            f'- **Date:** {self.created_at}',
            f'- **Firmware:** {self.firmware_version or "unknown"}',
            f'- **Verdict:** {self.summary()}',
            '',
            '## Results',
            '',
            '| # | Test | Status | Detail | Timestamp |',
            '|---|------|--------|--------|-----------|',
        ]
        for i, r in enumerate(self.results, 1):
            status = 'PASS' if r.passed else 'FAIL'
            lines.append(
                f'| {i} | {r.name} | {status} | {r.detail} | {r.timestamp} |'
            )
        lines.append('')
        return '\n'.join(lines)


# ---------------------------------------------------------------------------
# Threshold I/O
# ---------------------------------------------------------------------------

def load_thresholds(path: str) -> dict:
    """Load a thresholds JSON file. Returns an empty dict if the file is
    missing.
    """
    if not os.path.isfile(path):
        return {}
    with open(path, 'r') as f:
        return json.load(f)


def save_thresholds(thresholds: dict, path: str) -> None:
    """Persist *thresholds* dict to *path* as pretty-printed JSON."""
    os.makedirs(os.path.dirname(path) or '.', exist_ok=True)
    with open(path, 'w') as f:
        json.dump(thresholds, f, indent=2)
        f.write('\n')
