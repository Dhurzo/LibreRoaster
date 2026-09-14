#!/usr/bin/env python3
"""Hardware virtual-roast test for LibreRoaster ESP32-C3 (no sensors needed).

Drives a complete scripted roast on a real ESP32-C3 running the
simulated-sensors firmware and asserts the control state machine at
every phase: idle -> preheat -> roast (profile following) -> stop ->
recovered idle.

What it verifies on hardware (beyond serial_integration_test.py):
    - PROFILE/FANPROFILE load + linear-interpolation following: the SV
      reported by READ and the fan output track the loaded profiles
      against roast time (proves the profile engine runs on device).
    - Full roast lifecycle state transitions with fault-flag asserts.
    - STOP operator path (heater cut, fan 100 %, latch) + PID;OFF recovery.

Bare-board note: with nothing attached, GPIO1 (heat sense) reads
no-heat, so at sustained heater duty >= 50 % the heat-absent guard may
legitimately clamp heater output to 0 until the next explicit operator
command. Heater asserts are therefore range checks, never exact values.

Prerequisites:
    - ESP32-C3 flashed with: --features "embedded,simulated-sensors"
    - pyserial installed: pip install pyserial
    - Only the UART cable preferred (see docs/DEVELOPMENT.md §9: with the
      native-USB cable plugged, esp-println's auto printer may route
      output to the unread USB side once the host enumerates it).

Usage:
    python3 scripts/hw_virtual_roast.py [--port /dev/ttyUSB0]
    python3 scripts/hw_virtual_roast.py --port /dev/ttyUSB0 --profile "0,100;120,160;300,200"

Exit code: 0 when every check passes, 1 otherwise.
"""

import argparse
import re
import sys
import time

import serial


DEFAULT_PORT = "/dev/ttyUSB0"
DEFAULT_BAUD = 115200
DEFAULT_PROFILE = "0,100;120,160;300,200"
DEFAULT_FAN_PROFILE = "0,20;120,40"
SV_TOLERANCE = 8.0
FAN_TOLERANCE = 20.0


# ── Result tracking (matches repo HIL line format) ───────────────────────────

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


# ── Serial helpers ───────────────────────────────────────────────────────────

def read_lines(ser, duration):
    """Collect raw lines for `duration` seconds."""
    out = []
    end = time.time() + duration
    buf = b""
    while time.time() < end:
        n = ser.in_waiting
        if n:
            buf += ser.read(n)
            while b"\n" in buf:
                raw, buf = buf.split(b"\n", 1)
                out.append(raw.decode(errors="replace").strip("\r").strip())
        else:
            time.sleep(0.05)
    return [l for l in out if l]


_ANSI = re.compile(r"\x1b\[[0-9;]*m")


def clean(line):
    return _ANSI.sub("", line).strip()


def cmd(ser, text, wait=2.0):
    """Send a command, return all response lines (cleaned)."""
    ser.reset_input_buffer()
    ser.write((text + "\r\n").encode())
    ser.flush()
    return [clean(l) for l in read_lines(ser, wait)]


def first_match(lines, predicate):
    for line in lines:
        if predicate(line):
            return line
    return None


def is_read(line, pid_on=None):
    p = line.split(",")
    try:
        vals = [float(x) for x in p]
    except ValueError:
        return False
    if len(vals) == 5 and pid_on is not True:
        return True
    if len(vals) == 8 and pid_on is not False:
        return True
    return False


def parse_read(line):
    p = [float(x) for x in line.split(",")]
    d = {"amb": p[0], "et": p[1], "bt": p[2]}
    if len(p) >= 8:
        d["heater"], d["fan"], d["sv"] = p[5], p[6], p[7]
    return d


def parse_status(line):
    p = line.split(",")
    if len(p) < 20:
        return None
    try:
        return {
            "et": float(p[0]), "bt": float(p[1]),
            "heater": float(p[2]), "fan": float(p[3]),
            "fault": int(float(p[19])),
        }
    except ValueError:
        return None


def parse_stream(line):
    """Parse '#t,ET,BT,ROR,Gas' line, or return None."""
    if not line.startswith("#"):
        return None
    p = line[1:].split(",")
    if len(p) != 5:
        return None
    try:
        return {"t": float(p[0]), "et": float(p[1]), "bt": float(p[2]),
                "ror": float(p[3]), "gas": float(p[4])}
    except ValueError:
        return None


# ── Profile math (mirrors firmware linear interpolation + terminal hold) ─────

def parse_points(spec):
    pts = []
    for item in spec.split(";"):
        t, v = item.split(",")
        pts.append((float(t), float(v)))
    return sorted(pts)


def interp(pts, t):
    if t <= pts[0][0]:
        return pts[0][1]
    for (t0, v0), (t1, v1) in zip(pts, pts[1:]):
        if t <= t1:
            frac = (t - t0) / (t1 - t0) if t1 > t0 else 0.0
            return v0 + frac * (v1 - v0)
    return pts[-1][1]


# ── Phases ───────────────────────────────────────────────────────────────────

def main():
    ap = argparse.ArgumentParser(description="LibreRoaster hardware virtual roast")
    ap.add_argument("--port", default=DEFAULT_PORT)
    ap.add_argument("--baud", type=int, default=DEFAULT_BAUD)
    ap.add_argument("--profile", default=DEFAULT_PROFILE,
                    help='Roast profile "t1,T1;t2,T2;..." in degC')
    ap.add_argument("--fan-profile", default=DEFAULT_FAN_PROFILE,
                    help='Fan profile "t1,s1;t2,s2;..." in percent')
    args = ap.parse_args()

    rep = Report()
    profile = parse_points(args.profile)
    fan_profile = parse_points(args.fan_profile)

    try:
        ser = serial.Serial(args.port, args.baud, timeout=1)
    except Exception as e:
        rep.add("serial_connect", False, str(e))
        rep.summary()
        return 1
    time.sleep(0.5)

    # ── Phase 0: normalize + idle asserts ──
    print("── Phase 0: idle ──", flush=True)
    cmd(ser, "PID;OFF")
    cmd(ser, "STREAM;OFF")
    cmd(ser, "UNITS;C")
    lines = cmd(ser, "READ")
    r = first_match(lines, lambda l: is_read(l, pid_on=False))
    rep.add("idle_read_5field", r is not None, r or "no_response")
    lines = cmd(ser, "STATUS")
    s = first_match(lines, lambda l: parse_status(l) is not None)
    st = parse_status(s) if s else None
    rep.add("idle_status_fault_clear", st is not None and st["fault"] == 0,
            s or "no_response")
    if st:
        rep.add("idle_temps_finite",
                -50 < st["et"] < 350 and -50 < st["bt"] < 350,
                f"et={st['et']:.1f}:bt={st['bt']:.1f}")

    # ── Phase 1: load profiles (silent accept, no ERR) ──
    print("── Phase 1: profiles ──", flush=True)
    prof_cmd = "PROFILE;" + ";".join(f"{t:g},{v:g}" for t, v in profile)
    lines = cmd(ser, prof_cmd)
    rep.add("profile_accepted", not any("ERR" in l for l in lines), "|".join(lines))
    fan_cmd = "FANPROFILE;" + ";".join(f"{t:g},{v:g}" for t, v in fan_profile)
    lines = cmd(ser, fan_cmd)
    rep.add("fanprofile_accepted", not any("ERR" in l for l in lines), "|".join(lines))

    # ── Phase 2: preheat ──
    print("── Phase 2: preheat ──", flush=True)
    cmd(ser, "PREHEAT 120")
    time.sleep(4)
    lines = cmd(ser, "STATUS")
    s = first_match(lines, lambda l: parse_status(l) is not None)
    st = parse_status(s) if s else None
    rep.add("preheat_no_fault", st is not None and st["fault"] == 0, s or "no_response")

    # ── Phase 3: roast + profile following ──
    print("── Phase 3: roast ──", flush=True)
    cmd(ser, "START")
    time.sleep(2)
    lines = cmd(ser, "READ")
    r = first_match(lines, lambda l: is_read(l, pid_on=True))
    rep.add("roast_read_8field", r is not None, r or "no_response")

    cmd(ser, "STREAM;ON", wait=1.0)
    stream = [parse_stream(l) for l in read_lines(ser, 12)]
    stream = [x for x in stream if x]
    cmd(ser, "STREAM;OFF")
    rep.add("roast_stream_flowing", len(stream) >= 8, f"lines={len(stream)}")

    ok_sv, ok_fan, sv_detail, fan_detail = False, False, "no_stream", "no_stream"
    if stream:
        sample = stream[-1]
        exp_sv = interp(profile, sample["t"])
        lines = cmd(ser, "READ")
        r = first_match(lines, lambda l: is_read(l, pid_on=True))
        if r:
            sv = parse_read(r)["sv"]
            # Recompute expectation at stream time (READ follows within ~2 s).
            ok_sv = abs(sv - exp_sv) <= SV_TOLERANCE
            sv_detail = f"sv={sv:.1f}:expected={exp_sv:.1f}:t={sample['t']:.1f}"
            exp_fan = interp(fan_profile, sample["t"])
            fan = parse_read(r)["fan"]
            ok_fan = abs(fan - exp_fan) <= FAN_TOLERANCE
            fan_detail = f"fan={fan:.1f}:expected={exp_fan:.1f}"
    rep.add("roast_sv_follows_profile", ok_sv, sv_detail)
    rep.add("roast_fan_follows_profile", ok_fan, fan_detail)

    # ── Phase 4: STOP operator path ──
    print("── Phase 4: stop ──", flush=True)
    cmd(ser, "STOP")
    time.sleep(1)
    lines = cmd(ser, "STATUS")
    s = first_match(lines, lambda l: parse_status(l) is not None)
    st = parse_status(s) if s else None
    if st:
        rep.add("stop_heater_cut", st["heater"] == 0.0, f"heater={st['heater']:.1f}")
        rep.add("stop_fan_full", st["fan"] == 100.0, f"fan={st['fan']:.1f}")
        rep.add("stop_latch_set", st["fault"] == 1, f"fault={st['fault']}")
    else:
        rep.add("stop_status", False, "no_response")

    # ── Phase 5: recovery to idle ──
    print("── Phase 5: recovery ──", flush=True)
    cmd(ser, "PID;OFF")
    lines = cmd(ser, "STATUS")
    s = first_match(lines, lambda l: parse_status(l) is not None)
    st = parse_status(s) if s else None
    rep.add("recover_fault_clear", st is not None and st["fault"] == 0,
            s or "no_response")
    lines = cmd(ser, "READ")
    r = first_match(lines, lambda l: is_read(l, pid_on=False))
    rep.add("recover_read_5field", r is not None, r or "no_response")

    ser.close()
    return 0 if rep.summary() else 1


if __name__ == "__main__":
    sys.exit(main())
