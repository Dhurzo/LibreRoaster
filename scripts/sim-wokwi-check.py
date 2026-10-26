#!/usr/bin/env python3
"""Build and run LibreRoaster Wokwi scenarios from the manifest.

Usage:
  ./scripts/sim-wokwi-check.sh --list
  ./scripts/sim-wokwi-check.sh --build-only
  ./scripts/sim-wokwi-check.sh --scenario f1-smoke --scenario f2-spi
  ./scripts/sim-wokwi-check.sh --all

The runner builds each required feature set once, verifies that the generated
image matches the manifest, runs the scenario, and delegates log checks to
verify-wokwi-log.py.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_MANIFEST = ROOT / "sim" / "wokwi" / "scenarios" / "manifest.json"
DEFAULT_LOG_DIR = ROOT / "target" / "wokwi"
PROJECT_DIR = ROOT / "sim" / "wokwi"
BUILD_META = PROJECT_DIR / "build-meta.json"
VERIFIER = ROOT / "scripts" / "verify-wokwi-log.py"
PREPARE = ROOT / "scripts" / "sim-wokwi-prepare.sh"


@dataclass(frozen=True)
class Scenario:
    id: str
    file: str
    features: str
    timeout_ms: int
    kind: str
    expect: tuple[str, ...]
    forbid: tuple[str, ...]
    notes: str


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _load_manifest(path: Path) -> list[Scenario]:
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise SystemExit(f"cannot read manifest {path}: {exc}") from exc

    entries = raw.get("scenarios")
    if not isinstance(entries, list) or not entries:
        raise SystemExit(f"manifest {path} has no scenarios")

    scenarios: list[Scenario] = []
    seen: set[str] = set()
    for entry in entries:
        if not isinstance(entry, dict):
            raise SystemExit(f"manifest {path}: scenario entry is not an object")
        required = ("id", "file", "features", "timeout_ms", "kind")
        missing = [key for key in required if key not in entry]
        if missing:
            raise SystemExit(f"manifest {path}: scenario missing {', '.join(missing)}")
        scenario_id = str(entry["id"])
        if scenario_id in seen:
            raise SystemExit(f"manifest {path}: duplicate scenario id {scenario_id}")
        seen.add(scenario_id)
        scenario_file = PROJECT_DIR / str(entry["file"])
        if not scenario_file.is_file():
            raise SystemExit(f"scenario {scenario_id}: file not found: {scenario_file}")
        if entry["kind"] not in ("happy", "latch"):
            raise SystemExit(f"scenario {scenario_id}: kind must be happy or latch")
        scenarios.append(
            Scenario(
                id=scenario_id,
                file=str(entry["file"]),
                features=str(entry["features"]),
                timeout_ms=int(entry["timeout_ms"]),
                kind=str(entry["kind"]),
                expect=tuple(str(pattern) for pattern in entry.get("expect", [])),
                forbid=tuple(str(pattern) for pattern in entry.get("forbid", [])),
                notes=str(entry.get("notes", "")),
            )
        )
    return scenarios


def _select(scenarios: list[Scenario], requested: list[str]) -> list[Scenario]:
    if not requested:
        return scenarios
    by_id = {scenario.id: scenario for scenario in scenarios}
    unknown = [scenario_id for scenario_id in requested if scenario_id not in by_id]
    if unknown:
        raise SystemExit(f"unknown scenario(s): {', '.join(unknown)}")
    return [by_id[scenario_id] for scenario_id in requested]


def _build(features: str, wokwi_cli: str) -> dict[str, Any]:
    env = os.environ.copy()
    env["FEATURES"] = features
    env["WOKWI_CLI"] = wokwi_cli
    print(f"\n==> Building {features}")
    result = subprocess.run(
        [str(PREPARE)],
        cwd=ROOT,
        env=env,
        text=True,
        capture_output=True,
        timeout=1200,
    )
    if result.stdout:
        print(result.stdout, end="")
    if result.stderr:
        print(result.stderr, end="", file=sys.stderr)
    if result.returncode != 0:
        raise SystemExit(f"build failed for {features} (exit {result.returncode})")

    if not BUILD_META.is_file():
        raise SystemExit(f"prepare did not write {BUILD_META}")
    try:
        meta = json.loads(BUILD_META.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise SystemExit(f"cannot read build metadata: {exc}") from exc
    if meta.get("features") != features:
        raise SystemExit(
            f"stale image: build-meta features={meta.get('features')!r}, "
            f"scenario requires {features!r}"
        )
    firmware = PROJECT_DIR / "libreroaster.bin"
    if not firmware.is_file():
        raise SystemExit(f"missing firmware image: {firmware}")
    if meta.get("firmware_sha256") not in (None, "unavailable"):
        if _sha256(firmware) != meta["firmware_sha256"]:
            raise SystemExit("firmware hash does not match build-meta.json")
    return meta


def _run_verifier(
    scenario: Scenario, log: Path, json_out: Path
) -> tuple[int, dict[str, Any] | None, str]:
    command = [
        sys.executable,
        str(VERIFIER),
        "--log",
        str(log),
        "--json-out",
        str(json_out),
    ]
    for pattern in scenario.expect:
        command.extend(["--expect", pattern])
    for pattern in scenario.forbid:
        command.extend(["--forbid", pattern])
    result = subprocess.run(
        command,
        cwd=ROOT,
        text=True,
        capture_output=True,
        timeout=60,
    )
    details: dict[str, Any] | None = None
    if json_out.is_file():
        try:
            details = json.loads(json_out.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            details = None
    output = (result.stdout or "") + (result.stderr or "")
    return result.returncode, details, output


def _run_scenario(
    scenario: Scenario,
    wokwi_cli: str,
    log_dir: Path,
    build_meta: dict[str, Any],
) -> dict[str, Any]:
    log_dir.mkdir(parents=True, exist_ok=True)
    log = log_dir / f"{scenario.id}.log"
    verify_json = log_dir / f"{scenario.id}.verify.json"
    result_json = log_dir / f"{scenario.id}.json"
    log.unlink(missing_ok=True)
    verify_json.unlink(missing_ok=True)
    started = time.monotonic()

    command = [
        wokwi_cli,
        str(PROJECT_DIR),
        "--scenario",
        scenario.file,
        "--timeout",
        str(scenario.timeout_ms),
        "--serial-log-file",
        str(log),
    ]
    print(f"\n==> Running {scenario.id} ({scenario.features}, {scenario.timeout_ms} ms)")
    try:
        run = subprocess.run(
            command,
            cwd=ROOT,
            text=True,
            capture_output=True,
            timeout=(scenario.timeout_ms / 1000.0) + 60.0,
        )
        run_output = (run.stdout or "") + (run.stderr or "")
        wokwi_exit = run.returncode
    except subprocess.TimeoutExpired as exc:
        run_output = f"wokwi-cli process timed out: {exc}"
        wokwi_exit = 124

    if run_output.strip():
        print(run_output.rstrip())

    log_has_data = log.is_file() and log.stat().st_size > 0
    if log_has_data:
        verify_exit, verify_details, verify_output = _run_verifier(
            scenario, log, verify_json
        )
        if verify_output.strip():
            print(verify_output.rstrip())
        passed = wokwi_exit == 0 and verify_exit == 0
        status = "PASS" if passed else "FAIL"
        infra_error = None
    else:
        verify_exit = None
        verify_details = None
        passed = False
        status = "INFRA_ERROR"
        infra_error = f"wokwi-cli produced no serial log (exit {wokwi_exit})"
        print(f"INFRA_ERROR: {scenario.id}: {infra_error}")

    result = {
        "id": scenario.id,
        "features": scenario.features,
        "kind": scenario.kind,
        "status": status,
        "wokwi_exit": wokwi_exit,
        "verify_exit": verify_exit,
        "passed": passed,
        "infra_error": infra_error,
        "duration_s": round(time.monotonic() - started, 3),
        "log": str(log.relative_to(ROOT)) if log.is_relative_to(ROOT) else str(log),
        "verify": verify_details,
        "build": build_meta,
    }
    result_json.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")

    print(f"{status}: {scenario.id} -> {log}")
    return result


def _print_list(scenarios: list[Scenario]) -> None:
    print(f"{'ID':<18} {'KIND':<6} {'FEATURES':<44} FILE")
    for scenario in scenarios:
        print(
            f"{scenario.id:<18} {scenario.kind:<6} {scenario.features:<44} "
            f"{scenario.file}"
        )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--scenario", action="append", default=[], metavar="ID")
    parser.add_argument("--all", action="store_true", help="run every scenario")
    parser.add_argument("--list", action="store_true", help="list scenarios and exit")
    parser.add_argument(
        "--build-only",
        action="store_true",
        help="prepare each required image but do not run Wokwi",
    )
    parser.add_argument("--log-dir", type=Path, default=DEFAULT_LOG_DIR)
    parser.add_argument(
        "--wokwi-cli",
        default=os.environ.get("WOKWI_CLI", "wokwi-cli"),
        help="wokwi-cli executable (default: WOKWI_CLI or wokwi-cli)",
    )
    args = parser.parse_args()

    scenarios = _load_manifest(args.manifest)
    if args.list:
        _print_list(scenarios)
        return 0

    if args.all and args.scenario:
        raise SystemExit("use either --all or --scenario, not both")
    selected = _select(scenarios, [] if args.all else args.scenario)

    if not args.build_only:
        if not os.environ.get("WOKWI_CLI_TOKEN"):
            raise SystemExit(
                "WOKWI_CLI_TOKEN is not set. Create a CI token at "
                "https://wokwi.com/dashboard/ci, or use --build-only."
            )
        if shutil.which(args.wokwi_cli) is None:
            raise SystemExit(f"{args.wokwi_cli} is not on PATH")

    groups: dict[str, list[Scenario]] = {}
    for scenario in selected:
        groups.setdefault(scenario.features, []).append(scenario)

    results: list[dict[str, Any]] = []
    for features, group in groups.items():
        meta = _build(features, args.wokwi_cli)
        if args.build_only:
            print(f"BUILT: {features} -> {BUILD_META}")
            continue
        for scenario in group:
            results.append(_run_scenario(scenario, args.wokwi_cli, args.log_dir, meta))
            # Give the simulator a moment to release its local RFC2217 port
            # before starting the next process.
            time.sleep(1.0)

    if args.build_only:
        return 0

    functional_failures = [
        result for result in results if result["status"] == "FAIL"
    ]
    infra_errors = [
        result for result in results if result["status"] == "INFRA_ERROR"
    ]
    summary = {
        "ok": bool(results) and not functional_failures and not infra_errors,
        "total": len(results),
        "passed": sum(1 for result in results if result["status"] == "PASS"),
        "failed": len(functional_failures),
        "infra_errors": len(infra_errors),
        "scenarios": results,
        "generated_at_utc": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "git_commit": subprocess.run(
            ["git", "rev-parse", "--short", "HEAD"],
            cwd=ROOT,
            text=True,
            capture_output=True,
        ).stdout.strip(),
    }
    args.log_dir.mkdir(parents=True, exist_ok=True)
    summary_path = args.log_dir / "summary.json"
    summary_path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")

    print(
        f"\nSummary: {summary['passed']}/{summary['total']} passed, "
        f"{summary['failed']} failed, {summary['infra_errors']} infra error(s) "
        f"({summary_path})"
    )
    if infra_errors:
        print(
            "INFRA_ERROR results are not scenario failures; check Wokwi access, "
            "token/quota, and the CLI output."
        )
    return 0 if summary["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
