#!/usr/bin/env python3
"""Package safe-shutdown trace captures into a replay artifact."""

from __future__ import annotations

import argparse
import csv
import io
import json
import sys
import zipfile
from pathlib import Path
from typing import Dict, List

ROOT = Path(__file__).resolve().parent.parent
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from scripts.traceability_matrix import TraceSummary, parse_trace_line, summarize_trace, TRACE_COLUMNS, _update_summary


def build_trace_summaries(log_path: Path) -> Dict[str, TraceSummary]:
    traces: Dict[str, TraceSummary] = {}
    with log_path.open(encoding="utf-8") as handle:
        for line in handle:
            entry = parse_trace_line(line)
            if not entry:
                continue
            summary = traces.setdefault(entry.trace_id, TraceSummary())
            _update_summary(summary, entry.step, entry.data)
    return traces


def rows_from_traces(traces: Dict[str, TraceSummary]) -> List[List[str]]:
    return summarize_trace(traces)


def find_guard_metadata(traces: Dict[str, TraceSummary], artifact_name: str, log_name: str) -> Dict[str, str | None]:
    for trace_id, summary in traces.items():
        if not summary.guard:
            continue
        guard = summary.guard
        return {
            "TraceId": trace_id,
            "watchdog_failure": guard.get("watchdog_failure"),
            "error_category": guard.get("error_category"),
            "error_source": guard.get("error_source"),
            "artifact_name": artifact_name,
            "source_log": log_name,
        }
    return {
        "TraceId": "",
        "watchdog_failure": None,
        "error_category": None,
        "error_source": None,
        "artifact_name": artifact_name,
        "source_log": log_name,
    }


def serialize_csv(rows: List[List[str]]) -> str:
    output = io.StringIO()
    writer = csv.writer(output)
    writer.writerow(TRACE_COLUMNS)
    writer.writerows(rows)
    return output.getvalue()


def build_readme(metadata: Dict[str, str | None]) -> str:
    return """Safe-Shutdown Replay Artifact

Artifact name: {artifact_name}
Source log: {source_log}
TraceId: {TraceId}
Guard failure: {watchdog_failure}
Error category: {error_category}
Error source: {error_source}

Contents:
- Original TRACE log (copy of {source_log})
- traceability.csv (queue → actuator → telemetry → guard matrix)
- metadata.json (guard diagnostics for auditing)
- README.txt (this summary)

Replay the guard matrix:
1. Unzip the artifact: `unzip {artifact_name}.zip`
2. Run `python scripts/traceability_matrix.py {source_log}`
3. Validate the guard row shows `watchdog_failure=init_error_failure`, `error_category=initialization`, `error_source=hardware_init_failed`

This artifact keeps the failure path reproducible without hardware.
""".format(**{k: metadata.get(k, "") for k in metadata})


def create_artifact(log_path: Path, output_path: Path, artifact_name: str, force: bool) -> None:
    if not log_path.exists():
        raise FileNotFoundError(f"Log not found: {log_path}")
    if output_path.exists():
        if force:
            output_path.unlink()
        else:
            raise FileExistsError(f"Artifact already exists: {output_path}. Pass --force to overwrite.")
    output_path.parent.mkdir(parents=True, exist_ok=True)

    traces = build_trace_summaries(log_path)
    rows = rows_from_traces(traces)
    if not rows:
        raise RuntimeError(f"No TRACE entries found in {log_path}")

    metadata = find_guard_metadata(traces, artifact_name, log_path.name)
    csv_payload = serialize_csv(rows)
    readme = build_readme(metadata)
    metadata_bytes = json.dumps(metadata, indent=2)

    with zipfile.ZipFile(output_path, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        archive.write(log_path, log_path.name)
        archive.writestr("traceability.csv", csv_payload)
        archive.writestr("metadata.json", metadata_bytes)
        archive.writestr("README.txt", readme)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Bundle a safe-shutdown TRACE log, traceability matrix, and guard metadata into one artifact.",
    )
    parser.add_argument("--log", dest="log_path", type=Path, help="TRACE log file to package.")
    parser.add_argument(
        "log_path_positional",
        nargs="?",
        type=Path,
        help="TRACE log file to package (alternative to --log).",
    )
    parser.add_argument("--output", type=Path, required=True, help="Destination zip artifact path.")
    parser.add_argument("--artifact-name", default="safe-shutdown-replay", help="Label to embed inside the artifact.")
    parser.add_argument("--force", action="store_true", help="Overwrite the output if it already exists.")
    args = parser.parse_args()
    if args.log_path is None and args.log_path_positional is None:
        parser.error("Specify the TRACE log with --log or as the first positional argument.")
    if args.log_path is None:
        args.log_path = args.log_path_positional
    return args


def main() -> None:
    args = parse_args()
    assert args.log_path is not None  # guard for type checkers
    create_artifact(args.log_path, args.output, args.artifact_name, args.force)
    print(f"Created artifact: {args.output}")


if __name__ == "__main__":
    main()
