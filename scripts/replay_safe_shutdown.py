#!/usr/bin/env python3
"""Replay a safe-shutdown artifact and verify the guard metadata."""

from __future__ import annotations

import argparse
import csv
import io
import json
import sys
import tempfile
import zipfile
from pathlib import Path
from typing import Dict, List

ROOT = Path(__file__).resolve().parent.parent
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from scripts.traceability_matrix import (
    TRACE_COLUMNS,
    TraceSummary,
    _update_summary,
    parse_trace_line,
    summarize_trace,
)


Metadata = Dict[str, str | None]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="""Replay safe-shutdown artifacts and confirm guard metadata."""
    )
    parser.add_argument(
        "--artifact",
        type=Path,
        default=Path("logs/traceability/safe-shutdown-replay.zip"),
        help="Zipped safe-shutdown artifact to replay.",
    )
    parser.add_argument(
        "--report",
        type=Path,
        help="Path to emit a JSON report describing the replay results.",
    )
    return parser.parse_args()


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


def serialize_csv(rows: List[List[str]]) -> str:
    buffer = io.StringIO()
    writer = csv.writer(buffer)
    writer.writerow(TRACE_COLUMNS)
    writer.writerows(rows)
    return buffer.getvalue()


def extract_guard_metadata(traces: Dict[str, TraceSummary]) -> Metadata:
    for trace_id, summary in traces.items():
        if not summary.guard:
            continue
        guard = summary.guard
        return {
            "TraceId": trace_id,
            "watchdog_failure": guard.get("watchdog_failure"),
            "error_category": guard.get("error_category"),
            "error_source": guard.get("error_source"),
        }
    return {
        "TraceId": "",
        "watchdog_failure": None,
        "error_category": None,
        "error_source": None,
    }


def normalize(value: str | None) -> str:
    return "" if value is None else value


def compare_metadata(
    replay_metadata: Metadata, artifact_metadata: Dict[str, str | None]
) -> List[str]:
    mismatches: List[str] = []
    for key in ["TraceId", "watchdog_failure", "error_category", "error_source"]:
        replay_value = normalize(replay_metadata.get(key))
        artifact_value = normalize(artifact_metadata.get(key))
        if replay_value != artifact_value:
            mismatches.append(
                f"{key}: replay={replay_value!r} artifact={artifact_value!r}"
            )
    return mismatches


def ensure_report_path(report_path: Path) -> None:
    report_path.parent.mkdir(parents=True, exist_ok=True)


def main() -> None:
    args = parse_args()
    artifact_path = args.artifact

    if not artifact_path.exists():
        raise FileNotFoundError(f"Artifact not found: {artifact_path}")

    target_csv = artifact_path.parent / "traceability-replay.csv"

    with tempfile.TemporaryDirectory() as tempdir:
        tempdir_path = Path(tempdir)
        with zipfile.ZipFile(artifact_path) as archive:
            archive.extractall(tempdir_path)

        log_path = tempdir_path / "sample-safe-shutdown.log"
        metadata_path = tempdir_path / "metadata.json"

        if not log_path.exists():
            raise FileNotFoundError("sample-safe-shutdown.log missing inside artifact")
        if not metadata_path.exists():
            raise FileNotFoundError("metadata.json missing inside artifact")

        traces = build_trace_summaries(log_path)
        if not traces:
            raise RuntimeError("No TRACE entries found in the replay log.")

        rows = summarize_trace(traces)
        if not rows:
            raise RuntimeError("Trace summaries did not produce any matrix rows.")

        csv_payload = serialize_csv(rows)
        artifact_metadata = json.loads(metadata_path.read_text(encoding="utf-8"))

    target_csv.write_text(csv_payload, encoding="utf-8")
    replay_metadata = extract_guard_metadata(traces)
    mismatches = compare_metadata(replay_metadata, artifact_metadata)

    if args.report:
        ensure_report_path(args.report)
        report_data = {
            "artifact": str(artifact_path),
            "csv": str(target_csv),
            "report": str(args.report),
            "metadata_match": not mismatches,
            "replayed_metadata": replay_metadata,
            "artifact_metadata": {
                key: normalize(artifact_metadata.get(key))
                for key in ["TraceId", "watchdog_failure", "error_category", "error_source"]
            },
            "mismatch_details": mismatches,
        }
        args.report.write_text(json.dumps(report_data, indent=2), encoding="utf-8")

    if mismatches:
        detail_text = "; ".join(mismatches)
        raise SystemExit(f"Guard metadata mismatch: {detail_text}")

    print("Guard metadata replay successful:")
    print(f"  TraceId: {replay_metadata['TraceId']}")
    print(f"  watchdog_failure: {normalize(replay_metadata.get('watchdog_failure'))}")
    print(f"  error_category: {normalize(replay_metadata.get('error_category'))}")
    print(f"  error_source: {normalize(replay_metadata.get('error_source'))}")
    print(f"Regenerated matrix written to: {target_csv}")
    if args.report:
        print(f"JSON report available at: {args.report}")


if __name__ == "__main__":
    main()
