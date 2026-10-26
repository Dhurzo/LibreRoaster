#!/usr/bin/env python3
"""Verify a trace produced by `sim-wokwi-artisan-bridge.py --trace`.

Trace lines have the form::

    12345.678 S b'0.0,25.0,25.0\\r\\n'

`S` is device -> Artisan and `A` is Artisan -> device. The script decodes
both byte streams and checks them with regular expressions, so a live Artisan
session can be turned into a CI-able assertion without a desktop GUI.

Examples:
    ./scripts/verify-artisan-trace.py --trace artisan.trace \\
        --expect '^#OK' --min-read-responses 10

    ./scripts/verify-artisan-trace.py --trace artisan.trace \\
        --forbid 'ERR safety_fault' --json-out trace-check.json
"""

from __future__ import annotations

import argparse
import ast
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path

TRACE_LINE = re.compile(r"^(?P<time>\d+(?:\.\d+)?)\s+(?P<dir>[A-Z])\s+(?P<data>b.*)$")
READ_RESPONSE = re.compile(
    r"^\d+(?:\.\d+)?,\d+(?:\.\d+)?,\d+(?:\.\d+)?(?:,.*)?$", re.MULTILINE
)


@dataclass
class Trace:
    device: bytes = b""
    host: bytes = b""
    lines: int = 0


def parse_trace(path: Path) -> Trace:
    trace = Trace()
    for lineno, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not raw.strip():
            continue
        match = TRACE_LINE.match(raw)
        if match is None:
            raise ValueError(f"{path}:{lineno}: malformed trace line")
        try:
            data = ast.literal_eval(match.group("data"))
        except (SyntaxError, ValueError) as exc:
            raise ValueError(f"{path}:{lineno}: invalid bytes literal: {exc}") from exc
        if not isinstance(data, bytes):
            raise ValueError(f"{path}:{lineno}: trace payload is not bytes")
        if match.group("dir") == "S":
            trace.device += data
        elif match.group("dir") == "A":
            trace.host += data
        else:
            raise ValueError(f"{path}:{lineno}: unknown direction {match.group('dir')!r}")
        trace.lines += 1
    return trace


def decode(data: bytes) -> str:
    return data.decode("utf-8", errors="replace")


def run_checks(
    trace: Trace, expect: list[str], forbid: list[str], min_reads: int
) -> dict:
    device = decode(trace.device)
    host = decode(trace.host)
    combined = device + "\n" + host
    line_text = device.replace("\r", "\n")
    read_count = len(READ_RESPONSE.findall(line_text))

    expect_results = []
    for pattern in expect:
        try:
            compiled = re.compile(pattern, re.MULTILINE)
        except re.error as exc:
            raise ValueError(f"invalid --expect regex {pattern!r}: {exc}") from exc
        expect_results.append(
            {
                "pattern": pattern,
                "matched": compiled.search(combined) is not None,
            }
        )

    forbid_results = []
    for pattern in forbid:
        try:
            compiled = re.compile(pattern, re.MULTILINE)
        except re.error as exc:
            raise ValueError(f"invalid --forbid regex {pattern!r}: {exc}") from exc
        matched = compiled.search(combined) is not None
        forbid_results.append({"pattern": pattern, "matched": matched})

    missing = [item["pattern"] for item in expect_results if not item["matched"]]
    forbidden_hits = [item["pattern"] for item in forbid_results if item["matched"]]
    if min_reads and read_count < min_reads:
        missing.append(f"minimum READ responses ({read_count} < {min_reads})")

    ok = trace.lines > 0 and not missing and not forbidden_hits
    return {
        "ok": ok,
        "trace_lines": trace.lines,
        "device_bytes": len(trace.device),
        "host_bytes": len(trace.host),
        "read_responses": read_count,
        "expect": expect_results,
        "forbid": forbid_results,
        "missing": missing,
        "forbidden_hits": forbidden_hits,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--trace", required=True, type=Path, help="bridge trace file")
    parser.add_argument("--expect", action="append", default=[], help="required regex")
    parser.add_argument("--forbid", action="append", default=[], help="forbidden regex")
    parser.add_argument(
        "--min-read-responses",
        type=int,
        default=0,
        help="minimum number of TC4-style device readings",
    )
    parser.add_argument("--json-out", type=Path, help="write machine-readable results")
    parser.add_argument("--quiet", action="store_true", help="only set the exit code")
    args = parser.parse_args(argv)

    try:
        trace = parse_trace(args.trace)
        result = run_checks(trace, args.expect, args.forbid, args.min_read_responses)
    except (OSError, ValueError) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 2

    if args.json_out is not None:
        args.json_out.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")

    if not args.quiet:
        print(
            f"trace lines={result['trace_lines']} "
            f"device={result['device_bytes']}B host={result['host_bytes']}B "
            f"reads={result['read_responses']}"
        )
        for item in result["expect"]:
            print(f"{'PASS' if item['matched'] else 'FAIL'} expect {item['pattern']!r}")
        for item in result["forbid"]:
            print(f"{'FAIL' if item['matched'] else 'PASS'} forbid {item['pattern']!r}")
        for item in result["missing"]:
            print(f"missing: {item}")
        if result["ok"]:
            print("OK: Artisan trace verified")
        else:
            print("FAIL: Artisan trace verification failed")

    return 0 if result["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
