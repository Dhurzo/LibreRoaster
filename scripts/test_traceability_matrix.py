import unittest
from pathlib import Path

from scripts.traceability_matrix import (
    TraceSummary,
    parse_trace_line,
    _format_queue_depth,
    _update_summary,
    summarize_trace,
)


def build_trace_summaries(lines: list[str]) -> dict[str, TraceSummary]:
    traces: dict[str, TraceSummary] = {}
    for line in lines:
        entry = parse_trace_line(line)
        if not entry:
            continue
        summary = traces.setdefault(entry.trace_id, TraceSummary())
        _update_summary(summary, entry.step, entry.data)
    return traces


class TestTraceabilityMatrix(unittest.TestCase):
    def test_parse_queue_enqueue(self) -> None:
        line = "TRACE,1,queue_enqueue,cmd=ReadStatus,channel=Uart,depth=5,fallback=0"
        entry = parse_trace_line(line)
        self.assertIsNotNone(entry)
        assert entry is not None
        self.assertEqual(entry.step, "queue_enqueue")
        self.assertEqual(entry.data.get("cmd"), "ReadStatus")
        self.assertEqual(_format_queue_depth(entry.data), "depth=5 channel=Uart")

    def test_parse_queue_fallback(self) -> None:
        line = "TRACE,2,queue_fallback,cmd=SetPoint,channel=Uart,depth=4,fallback=1"
        entry = parse_trace_line(line)
        self.assertIsNotNone(entry)
        assert entry is not None
        self.assertEqual(entry.step, "queue_fallback")
        self.assertEqual(
            _format_queue_depth(entry.data),
            "depth=4 channel=Uart fallback=true",
        )

    def test_parse_actuation(self) -> None:
        line = (
            "TRACE,1,actuation,cmd=ReadStatus,channel=Uart,ssr=33.5,fan=12,latency_us=150,saturation_active=1"
        )
        entry = parse_trace_line(line)
        self.assertIsNotNone(entry)
        assert entry is not None
        self.assertEqual(entry.step, "actuation")
        self.assertEqual(entry.data.get("ssr"), "33.5")
        self.assertEqual(entry.data.get("fan"), "12")
        self.assertEqual(entry.data.get("latency_us"), "150")

    def test_parse_telemetry(self) -> None:
        line = "TRACE,1,telemetry,guard_timeout=0,guard_timeouts=0,watchdog=ok"
        entry = parse_trace_line(line)
        self.assertIsNotNone(entry)
        assert entry is not None
        self.assertEqual(entry.step, "telemetry")
        self.assertEqual(entry.data.get("watchdog"), "ok")

    def test_parse_guard(self) -> None:
        line = (
            "TRACE,1,guard,guard_timeout=0,guard_timeouts=0,watchdog=ok,error_category=control,error_source=pid_error"
        )
        entry = parse_trace_line(line)
        self.assertIsNotNone(entry)
        assert entry is not None
        self.assertEqual(entry.step, "guard")
        self.assertEqual(entry.data.get("error_category"), "control")
        self.assertEqual(entry.data.get("error_source"), "pid_error")

    def test_complete_trace_flow(self) -> None:
        lines = [
            "TRACE,1,queue_enqueue,cmd=ReadStatus,channel=Uart,depth=5,fallback=0",
            "TRACE,1,queue_dequeue,cmd=ReadStatus,channel=Uart,depth=4",
            "TRACE,1,actuation,cmd=ReadStatus,channel=Uart,ssr=33.5,fan=12,latency_us=150,saturation_active=1",
            "TRACE,1,telemetry,guard_timeout=0,guard_timeouts=0,watchdog=ok,ET=150,BT=142,PV=1450,MV=1600",
            "TRACE,1,guard,guard_timeout=0,guard_timeouts=0,watchdog=ok",
        ]
        rows = summarize_trace(build_trace_summaries(lines))
        self.assertEqual(len(rows), 1)
        trace_id, command, queue_depth, actuator, telemetry, guard = rows[0]
        self.assertEqual(trace_id, "1")
        self.assertEqual(command, "ReadStatus")
        self.assertEqual(queue_depth, "depth=4 channel=Uart")
        self.assertIn("ssr=33.5", actuator)
        self.assertIn("ET=150", telemetry)
        self.assertIn("watchdog=ok", guard)

    def test_mixed_log_lines(self) -> None:
        lines = [
            "STATUS,2026-03-20T13:25:00,Ready,channel=USB,queue=0",
            "TRACE,1,queue_enqueue,cmd=ReadStatus,channel=Uart,depth=5,fallback=0",
            "TRACE,1,queue_dequeue,cmd=ReadStatus,channel=Uart,depth=4",
            "TRACE,1,actuation,cmd=ReadStatus,channel=Uart,ssr=33.5,fan=12,latency_us=150,saturation_active=1",
            "TRACE,1,telemetry,guard_timeout=0,guard_timeouts=0,watchdog=ok",
            "TRACE,1,guard,guard_timeout=0,guard_timeouts=0,watchdog=ok",
            "TRACE,2,queue_enqueue,cmd=SetPoint,channel=Uart,depth=4,fallback=0",
            "TRACE,2,queue_fallback,cmd=SetPoint,channel=Uart,depth=4,fallback=1",
            "STATUS,2026-03-20T13:25:10,Busy,channel=USB,queue=1",
            "TRACE,3,queue_enqueue,cmd=ReadStatus,channel=Uart,depth=3,fallback=0",
            "TRACE,3,telemetry,guard_timeout=1,guard_timeouts=3,watchdog=fail,error_category=control,error_source=pid_error",
        ]
        rows = summarize_trace(build_trace_summaries(lines))
        self.assertEqual(len(rows), 3)
        _, _, queue_depth_2, _, _, _ = rows[1]
        self.assertEqual(queue_depth_2, "depth=4 channel=Uart fallback=true")
        _, _, _, _, telemetry_3, _ = rows[2]
        self.assertIn("error_category=control", telemetry_3)

    def test_debug_format_cmd(self) -> None:
        line = "TRACE,4,queue_enqueue,cmd=ArtisanCommand::STATUS,channel=Uart,depth=2,fallback=0"
        entry = parse_trace_line(line)
        self.assertIsNotNone(entry)
        assert entry is not None
        summary = TraceSummary()
        _update_summary(summary, entry.step, entry.data)
        self.assertEqual(summary.command, "STATUS")

    def test_safe_shutdown_log_replays_guard_failure(self) -> None:
        log_path = (
            Path(__file__)
            .resolve()
            .parent
            .parent
            / "logs"
            / "traceability"
            / "sample-safe-shutdown.log"
        )
        lines = log_path.read_text().splitlines()
        rows = summarize_trace(build_trace_summaries(lines))
        self.assertEqual(len(rows), 1)
        guard = rows[0][5]
        self.assertIn("watchdog_failure=init_error_failure", guard)
        self.assertIn("watchdog=fail", guard)
        self.assertIn("error_category=initialization", guard)
        self.assertIn("error_source=hardware_init_failed", guard)


if __name__ == "__main__":
    unittest.main()
