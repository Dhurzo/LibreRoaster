#!/usr/bin/env python3
"""Verify a Wokwi serial log against expected and forbidden regexes.

The scenario runner uses this after wokwi-cli exits. Keeping the log checks
outside the YAML lets positive and negative scenarios share one mechanism:
happy-path scenarios forbid safety latches, while latch scenarios require one.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ANSI_RE = re.compile(r"\x1b\[[0-9;]*m")


def _compile(pattern: str) -> re.Pattern[str]:
    try:
        return re.compile(pattern, re.MULTILINE)
    except re.error as exc:
        raise SystemExit(f"invalid regex {pattern!r}: {exc}") from exc


def _search(pattern: str, text: str) -> dict[str, object]:
    regex = _compile(pattern)
    match = regex.search(text)
    if match is None:
        return {"pattern": pattern, "matched": False}
    line_no = text.count("\n", 0, match.start()) + 1
    line_start = text.rfind("\n", 0, match.start()) + 1
    line_end = text.find("\n", match.end())
    if line_end == -1:
        line_end = len(text)
    line = text[line_start:line_end].strip()
    return {
        "pattern": pattern,
        "matched": True,
        "line": line_no,
        "text": line[:300],
    }


def verify(log_path: Path, expect: list[str], forbid: list[str]) -> dict[str, object]:
    try:
        raw = log_path.read_text(encoding="utf-8", errors="replace")
    except OSError as exc:
        return {
            "ok": False,
            "log": str(log_path),
            "error": str(exc),
            "expect": [],
            "forbid": [],
        }

    text = ANSI_RE.sub("", raw.replace("\r\n", "\n").replace("\r", "\n"))
    expect_results = [_search(pattern, text) for pattern in expect]
    forbid_results = [_search(pattern, text) for pattern in forbid]
    missing = [item["pattern"] for item in expect_results if not item["matched"]]
    forbidden_hits = [item for item in forbid_results if item["matched"]]
    return {
        "ok": not missing and not forbidden_hits,
        "log": str(log_path),
        "line_count": text.count("\n") + (1 if text else 0),
        "expect": expect_results,
        "forbid": forbid_results,
        "missing": missing,
        "forbidden_hits": [item["pattern"] for item in forbidden_hits],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--log", required=True, type=Path)
    parser.add_argument("--expect", action="append", default=[], metavar="REGEX")
    parser.add_argument("--forbid", action="append", default=[], metavar="REGEX")
    parser.add_argument("--json-out", type=Path)
    args = parser.parse_args()

    result = verify(args.log, args.expect, args.forbid)
    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(
            json.dumps(result, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

    if not args.log.exists():
        print(f"log missing: {args.log}", file=sys.stderr)
        return 2
    if result.get("error"):
        print(result["error"], file=sys.stderr)
        return 2
    if result["ok"]:
        print(f"OK: {args.log}")
        return 0

    for pattern in result["missing"]:
        print(f"missing expected pattern: {pattern}", file=sys.stderr)
    for pattern in result["forbidden_hits"]:
        print(f"forbidden pattern present: {pattern}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
