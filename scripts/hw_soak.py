#!/usr/bin/env python3
"""Hardware soak test for LibreRoaster ESP32-C3 (no sensors needed).

Captures the 1 Hz spontaneous `#` telemetry stream from a real ESP32-C3
running the simulated-sensors firmware over the full default roast curve
(~10 min) and checks stream integrity end to end:

    - every telemetry line is well-formed `#t,ET,BT,ROR,Gas`
    - 1 Hz cadence (no gaps), timestamps strictly increasing (no resets)
    - no `ERR safety_fault` latch at any point (curve stays sub-threshold)
    - temperatures finite, in range, with no sample-to-sample glitches
    - the roast actually traverses the curve (BT span check)
    - device still responsive afterwards (STATUS fault clear)

With `--reset` (default) the device is rebooted via esptool first so the
soak always starts at curve t=0.

Session keepalive: the firmware drops the active transport after 60 s
without commands (CommandMultiplexer IDLE_TIMEOUT_SECS anti-hijack
failover — see src/input/multiplexer.rs), which would silently stop
telemetry routing even with STREAM;ON. Like Artisan (which polls READ
continuously), this script sends STATUS every 25 s to hold the session.
Those keepalive responses are tolerated by the analyzer.

Prerequisites:
    - ESP32-C3 flashed with: --features "embedded,simulated-sensors"
    - pyserial installed: pip install pyserial
    - esptool installed (only for --reset): pip install esptool
    - Prefer only the UART cable connected (see docs/DEVELOPMENT.md §9:
      with native USB plugged, esp-println's auto printer may route lines
      to the unread USB side once the host enumerates it).

Usage:
    python3 scripts/hw_soak.py [--port /dev/ttyUSB0] [--duration 660] [--no-reset]

Exit code: 0 when every check passes, 1 otherwise.
"""

import argparse
import re
import subprocess
import sys
import time

import serial


DEFAULT_PORT = "/dev/ttyUSB0"
DEFAULT_BAUD = 115200
DEFAULT_DURATION = 660  # seconds: 600 s default curve + margin
MAX_GAP_S = 3.0
MAX_STEP_C = 6.0
TEMP_MIN, TEMP_MAX = -10.0, 300.0
MIN_BT_SPAN = 20.0

_ANSI = re.compile(r"\x1b\[[0-9;]*m")
_STREAM_RE = re.compile(r"^#(\d+\.\d+),(-?\d+\.\d+),(-?\d+\.\d+),(-?\d+\.\d+),(\d+\.\d+)$")


def is_csv_response(line):
    """READ (5/8 all-numeric fields) or STATUS (20 fields, index 6 is the
    string watchdog-reason token) response to our keepalives."""
    parts = line.split(",")
    if len(parts) in (5, 8):
        try:
            [float(x) for x in parts]
            return True
        except ValueError:
            return False
    if len(parts) == 20:
        try:
            [float(x) for i, x in enumerate(parts) if i != 6]
            return bool(parts[6])
        except ValueError:
            return False
    return False


class Report:
    def __init__(self):
        self.results = []

    def add(self, name, passed, detail=""):
        self.results.append((name, passed))
        print(f"TEST:{name}:{'PASS' if passed else 'FAIL'}:{detail}", flush=True)

    def summary(self):
        total = len(self.results)
        passed = sum(1 for _, ok in self.results if ok)
        print(f"TESTSUITE:COMPLETE:{passed}/{total}:{'PASS' if passed == total else 'FAIL'}",
              flush=True)
        return passed == total


def main():
    ap = argparse.ArgumentParser(description="LibreRoaster hardware soak test")
    ap.add_argument("--port", default=DEFAULT_PORT)
    ap.add_argument("--baud", type=int, default=DEFAULT_BAUD)
    ap.add_argument("--duration", type=float, default=DEFAULT_DURATION,
                    help="telemetry capture seconds (default: %(default)s)")
    ap.add_argument("--no-reset", action="store_true",
                    help="skip esptool reboot (curve continues wherever it is)")
    args = ap.parse_args()
    rep = Report()

    if not args.no_reset:
        print(f"── rebooting {args.port} via esptool ──", flush=True)
        try:
            r = subprocess.run(["esptool", "--port", args.port, "run"],
                               capture_output=True, text=True, timeout=60)
            ok = "Hard resetting" in (r.stdout + r.stderr)
            rep.add("device_reset", ok, "esptool_run")
            if not ok:
                print((r.stdout + r.stderr)[-500:], flush=True)
                rep.summary()
                return 1
        except FileNotFoundError:
            print("esptool not found; continuing without reset", flush=True)
            rep.add("device_reset", True, "skipped_no_esptool")
        time.sleep(6)

    try:
        ser = serial.Serial(args.port, args.baud, timeout=1)
    except Exception as e:
        rep.add("serial_connect", False, str(e))
        rep.summary()
        return 1
    time.sleep(0.5)
    ser.reset_input_buffer()
    rep.add("serial_connect", True, f"port={args.port}")

    ser.write(b"STREAM;ON\r\n")
    ser.flush()

    print(f"── capturing {args.duration:.0f}s of telemetry ──", flush=True)
    tele, other, faults, keepalives, raw_log = [], [], [], [], []
    end = time.time() + args.duration
    buf = b""
    last_print = time.time()
    next_keepalive = time.time() + 25.0
    while time.time() < end:
        if time.time() >= next_keepalive:
            # Hold the mux session (see module docstring): a lightweight
            # STATUS poll, exactly like an Artisan client would send.
            ser.write(b"STATUS\r\n")
            ser.flush()
            next_keepalive = time.time() + 25.0
        n = ser.in_waiting
        if n:
            buf += ser.read(n)
            while b"\n" in buf:
                raw, buf = buf.split(b"\n", 1)
                line = _ANSI.sub("", raw.decode(errors="replace")).strip("\r").strip()
                if not line:
                    continue
                raw_log.append(line)
                m = _STREAM_RE.match(line)
                if m:
                    t, et, bt, ror, gas = (float(m.group(i)) for i in range(1, 6))
                    tele.append((time.time(), t, et, bt, ror, gas))
                elif "safety_fault" in line:
                    faults.append(line)
                elif line in ("#OK",) or line.startswith("ERR"):
                    other.append(line)
                elif is_csv_response(line):
                    keepalives.append(line)
                else:
                    other.append(line)
        else:
            time.sleep(0.05)
        if time.time() - last_print >= 60:
            print(f"  ... {len(tele)} telemetry lines so far", flush=True)
            last_print = time.time()

    ser.write(b"STREAM;OFF\r\n")
    ser.flush()
    time.sleep(0.5)
    ser.reset_input_buffer()
    ser.write(b"STATUS\r\n")
    ser.flush()
    time.sleep(1.5)
    status_raw = ser.read(ser.in_waiting).decode(errors="replace")
    ser.close()

    print(f"── analyzing {len(tele)} telemetry lines ──", flush=True)
    rep.add("stream_flowing", len(tele) >= int(args.duration * 0.8),
            f"lines={len(tele)}:expected~{int(args.duration)}")
    rep.add("keepalive_answered", len(keepalives) >= int(args.duration / 25) - 1,
            f"status_responses={len(keepalives)}")

    if len(tele) >= 2:
        gaps = [b[0] - a[0] for a, b in zip(tele, tele[1:])]
        rep.add("stream_cadence_1hz",
                max(gaps) <= MAX_GAP_S and 0.5 <= sum(gaps) / len(gaps) <= 1.6,
                f"median_gap={sorted(gaps)[len(gaps)//2]:.2f}s:max_gap={max(gaps):.2f}s")
        mono = all(b[1] > a[1] for a, b in zip(tele, tele[1:]))
        rep.add("stream_time_monotonic", mono, "no_reboots" if mono else "time_went_backwards")
        steps = [abs(b[3] - a[3]) for a, b in zip(tele, tele[1:])]
        rep.add("stream_no_glitches", max(steps) <= MAX_STEP_C,
                f"max_bt_step={max(steps):.1f}C")
        span = max(x[3] for x in tele) - min(x[3] for x in tele)
        rep.add("stream_curve_traversed", span >= MIN_BT_SPAN, f"bt_span={span:.1f}C")
    else:
        for name in ("stream_cadence_1hz", "stream_time_monotonic",
                     "stream_no_glitches", "stream_curve_traversed"):
            rep.add(name, False, "insufficient_lines")

    sane = all(TEMP_MIN <= x[2] <= TEMP_MAX and TEMP_MIN <= x[3] <= TEMP_MAX
               for x in tele)
    rep.add("stream_temps_sane", sane, f"lines={len(tele)}")
    rep.add("stream_no_safety_fault", not faults, f"faults={len(faults)}")
    unexpected = [l for l in other if not (l == "#OK" or l.startswith("ERR OT2_CLAMPED")
                                           or "WARN" in l or "warn" in l.lower())]
    rep.add("stream_no_garbage", not unexpected, f"unexpected={len(unexpected)}")

    m = re.search(r"(-?\d+\.\d+,){3}[\d.\-,a-z]+,(\d+)\s*$", status_raw.strip().splitlines()[-1]
                  if status_raw.strip() else "")
    fault_clear = m is not None and m.group(2) == "0"
    rep.add("device_alive_after_soak", fault_clear,
            status_raw.strip().splitlines()[-1][:60] if status_raw.strip() else "no_status")

    import os
    os.makedirs("logs", exist_ok=True)
    stamp = time.strftime("%Y%m%d_%H%M%S")
    path = f"logs/hw_soak_{stamp}.log"
    with open(path, "w") as f:
        f.write("\n".join(raw_log) + "\n")
    print(f"raw capture saved to {path} ({len(raw_log)} lines)", flush=True)

    return 0 if rep.summary() else 1


if __name__ == "__main__":
    sys.exit(main())
