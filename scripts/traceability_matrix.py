#!/usr/bin/env python3
"""Parse TRACE logs into a command → queue → actuator → telemetry → guard matrix.

Columns: TraceId, Command, QueueDepth, Actuator (SSR/fan outputs), Telemetry (ET/BT/PV/MV), Guard (watchdog/guard flags).
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass, field
from typing import Dict, List, Mapping, Sequence

TRACE_COLUMNS = ["TraceId", "Command", "QueueDepth", "Actuator", "Telemetry", "Guard"]


@dataclass
class TraceSummary:
    command: str = ""
    queue_depth: str = ""
    actuator: Dict[str, str] = field(default_factory=dict)
    telemetry: Dict[str, str] = field(default_factory=dict)
    guard: Dict[str, str] = field(default_factory=dict)


@dataclass
class TraceEntry:
    trace_id: str
    step: str
    data: Dict[str, str]


def parse_trace_line(line: str) -> TraceEntry | None:
    line = line.strip()
    if not line or not line.startswith("TRACE,"):
        return None

    parts = [segment.strip() for segment in line.split(",")]
    if len(parts) < 4:
        return None

    trace_id = parts[1]
    step = parts[2]
    data: Dict[str, str] = {}
    for segment in parts[3:]:
        if "=" not in segment:
            continue
        key, value = segment.split("=", 1)
        data[key.strip()] = value.strip()

    return TraceEntry(trace_id=trace_id, step=step, data=data)


def summarize_trace(traces: Mapping[str, TraceSummary]) -> List[List[str]]:
    rows: List[List[str]] = []
    for trace_id in sorted(traces.keys(), key=_numeric_key):
        summary = traces[trace_id]
        rows.append([
            trace_id,
            summary.command or "<unknown>",
            summary.queue_depth or "",
            _kv_to_str(summary.actuator),
            _kv_to_str(summary.telemetry, ["ET", "BT", "PV", "MV"]),
            _kv_to_str(summary.guard),
        ])
    return rows


def _numeric_key(value: str) -> int:
    try:
        return int(value)
    except ValueError:
        return float("inf")  # type: ignore[return-value]


def _kv_to_str(data: Mapping[str, str], key_order: Sequence[str] | None = None) -> str:
    if not data:
        return ""
    if key_order:
        ordered = [f"{key}={data[key]}" for key in key_order if key in data]
        if ordered:
            return ", ".join(ordered)
    return ", ".join(f"{key}={value}" for key, value in sorted(data.items()))


def _format_queue_depth(data: Mapping[str, str]) -> str:
    depth = data.get("queue_depth")
    channel = data.get("channel")
    parts: List[str] = []
    if depth:
        parts.append(f"depth={depth}")
    if channel:
        parts.append(f"channel={channel}")
    return " ".join(parts)


def _update_summary(summary: TraceSummary, step: str, data: Mapping[str, str]) -> None:
    if step == "command_enqueue":
        summary.command = data.get("command", summary.command)
        queue_info = _format_queue_depth(data)
        if queue_info:
            summary.queue_depth = queue_info
    elif step == "queue_dequeue":
        queue_info = _format_queue_depth(data)
        if queue_info:
            summary.queue_depth = queue_info
    elif step == "actuator_output":
        summary.actuator.update(data)
    elif step == "telemetry_emit":
        summary.telemetry.update(data)
    elif step == "guard_report":
        summary.guard.update(data)


def _print_table(rows: List[List[str]]) -> None:
    widths = [len(header) for header in TRACE_COLUMNS]
    for row in rows:
        for index, cell in enumerate(row):
            if len(cell) > widths[index]:
                widths[index] = len(cell)
    header_line = " | ".join(header.ljust(width) for header, width in zip(TRACE_COLUMNS, widths))
    separator = "-+-".join("-" * width for width in widths)
    print(header_line)
    print(separator)
    if not rows:
        print("No TRACE entries found in the log.")
        return
    for row in rows:
        print(" | ".join(cell.ljust(widths[idx]) for idx, cell in enumerate(row)))


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Parse TRACE logs into a regression triage matrix.",
        epilog="Columns: TraceId, Command, QueueDepth, Actuator, Telemetry, Guard.",
    )
    parser.add_argument("log_path", help="Path to the TRACE log to summarize.")
    args = parser.parse_args()

    traces: Dict[str, TraceSummary] = {}
    try:
        with open(args.log_path, encoding="utf-8") as input_file:
            for line in input_file:
                entry = parse_trace_line(line)
                if not entry:
                    continue
                summary = traces.setdefault(entry.trace_id, TraceSummary())
                _update_summary(summary, entry.step, entry.data)
    except FileNotFoundError as exc:
        parser.error(f"Cannot read log: {exc}")

    rows = summarize_trace(traces)
    _print_table(rows)


if __name__ == "__main__":
    main()
