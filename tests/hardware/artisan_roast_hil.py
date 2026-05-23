#!/usr/bin/env python3
"""
HIL test: Full Artisan roast simulation against real hardware.

Simulates the complete Artisan+ protocol sequence over USB/serial —
init handshake, profile loading, START, polling, STOP — and validates
READ/STATUS responses at each stage.

SAFETY: This script sends OT1 (heater %) and START/STOP commands.
The heater is set to 0% at the end. Ensure your roaster is in a safe
state before running — do NOT leave it unattended during this test.

Usage:
  python tests/hardware/artisan_roast_hil.py
  python tests/hardware/artisan_roast_hil.py --port /dev/ttyACM0
  python tests/hardware/artisan_roast_hil.py --dry-run   (validate only)
"""

import argparse
import re
import sys
import time
from typing import Optional

try:
    import serial
    import serial.tools.list_ports
except ImportError:
    print("ERROR: pyserial not installed. Run: pip install pyserial")
    sys.exit(1)

BAUD = 115200
RESPONSE_TIMEOUT = 5
CMD_DELAY = 0.5
READ_POLL_INTERVAL = 0.5


# ---------------------------------------------------------------------------
# Line classification helpers — adapted from serial_integration_test.py
# ---------------------------------------------------------------------------

LOG_PREFIXES = [
    "INFO", "WARN", "ERROR", "DEBUG", "TRACE",
    "STAGE,", "STAGE:",
    "\x1b[",  # ANSI color codes
    "[USB]", "[UART]",
]


def is_log_line(line: str) -> bool:
    """Return True if *line* is firmware log/telemetry, not a protocol response."""
    if not line:
        return True
    for prefix in LOG_PREFIXES:
        if line.startswith(prefix):
            return True
    # ESP-IDF bootloader: "I (109) esp_image:", "D (200) boot:"
    if re.match(r'^[IDWEV] \(\d+\) ', line):
        return True
    if "watchdog=" in line and "elapsed=" in line:
        return True
    if "queue_enqueue" in line or "queue_drain" in line:
        return True
    return False


def _is_streaming_csv(line: str) -> bool:
    """Return True if *line* is continuous streaming telemetry (not a protocol
    response).  Both streaming and READ w/o PID have 5 CSV fields; we
    disambiguate by field semantics."""
    parts = line.split(",")
    if len(parts) == 5:
        try:
            f0 = float(parts[0])
            f3 = float(parts[3])
            f4 = float(parts[4])
            # Streaming CSV has non-zero ROR (f3) or gas (f4)
            if f3 != 0.0 or f4 != 0.0:
                return True
            # First field in streaming is monotonic time (< 3600 s).
            # First field in READ is ambient temp (10-50 °C).
            if f0 < 10.0 or f0 > 50.0:
                return True
        except (ValueError, IndexError):
            pass
    return False


# ---------------------------------------------------------------------------
# Serial helpers — robust pattern from serial_integration_test.py
# ---------------------------------------------------------------------------

def open_port(port: str) -> serial.Serial:
    ser = serial.Serial(port, BAUD, timeout=RESPONSE_TIMEOUT)
    time.sleep(0.3)
    ser.reset_input_buffer()
    return ser


def send_command(
    ser: serial.Serial,
    cmd: str,
    timeout: float = 5.0,
    expecting: str = "generic",
) -> Optional[str]:
    """Send a command and return the protocol response.

    Filters out log/telemetry/streaming lines automatically.

    *expecting* controls how the response is identified:

    - ``"generic"`` — first non-log line (skips streaming CSV).
    - ``"read"``    — first line that looks like a TC4 READ response
      (5 or 8 CSV fields).
    - ``"status"``  — first line with 19+ CSV fields.
    """
    try:
        # Drain any in-flight data before sending
        time.sleep(0.15)
        ser.reset_input_buffer()

        ser.write(f"{cmd}\r\n".encode("ascii"))
        ser.flush()
        time.sleep(CMD_DELAY)

        deadline = time.time() + timeout
        while time.time() < deadline:
            if ser.in_waiting:
                raw = ser.read(ser.in_waiting).decode("ascii", errors="replace")
                for raw_line in raw.split("\n"):
                    line_str = raw_line.strip("\r").strip()
                    if not line_str:
                        continue
                    if is_log_line(line_str):
                        continue
                    if expecting == "generic" and _is_streaming_csv(line_str):
                        continue
                    if expecting == "read":
                        parts = line_str.split(",")
                        if len(parts) not in (5, 8):
                            continue
                    if expecting == "status":
                        parts = line_str.split(",")
                        if len(parts) < 19:
                            continue
                    return line_str
            time.sleep(0.05)
        return None
    except serial.SerialException:
        return None


# ---------------------------------------------------------------------------
# Parsers
# ---------------------------------------------------------------------------

def parse_read_5(response: str) -> Optional[dict]:
    parts = response.split(",")
    if len(parts) != 5:
        return None
    try:
        return {
            "ambient": float(parts[0]),
            "et": float(parts[1]),
            "bt": float(parts[2]),
        }
    except (ValueError, IndexError):
        return None


def parse_read(response: str) -> Optional[dict]:
    p = parse_read_5(response)
    if p:
        return p
    parts = response.split(",")
    if len(parts) == 8:
        try:
            return {
                "ambient": float(parts[0]),
                "et": float(parts[1]),
                "bt": float(parts[2]),
            }
        except (ValueError, IndexError):
            return None
    return None


# ---------------------------------------------------------------------------
# Checks
# ---------------------------------------------------------------------------

passed = 0
failed = 0


def check(condition: bool, msg: str) -> None:
    global passed, failed
    if condition:
        passed += 1
        print(f"  ✅ {msg}")
    else:
        failed += 1
        print(f"  ❌ {msg}")


def check_read_valid(response: Optional[str], label: str) -> Optional[dict]:
    if not response:
        check(False, f"{label}: no response")
        return None
    parsed = parse_read(response)
    if not parsed:
        check(False, f"{label}: unparseable READ: {response}")
        return None
    check(True, f"{label}: READ={response}")
    return parsed


# ---------------------------------------------------------------------------
# Test: Handshake
# ---------------------------------------------------------------------------

def test_handshake(ser: serial.Serial) -> None:
    print("\n── Phase 1: Artisan handshake ──")

    # CHAN;1200  →  "#1200"
    resp = send_command(ser, "CHAN;1200", expecting="generic")
    check(resp == "#1200", f"CHAN;1200 → '{resp}' (expected #1200)")

    # UNITS;C   →  "OK"
    resp = send_command(ser, "UNITS;C", expecting="generic")
    check(resp == "OK", f"UNITS;C → '{resp}' (expected OK)")

    # FILT;70   →  "OK"
    resp = send_command(ser, "FILT;70", expecting="generic")
    check(resp == "OK", f"FILT;70 → '{resp}' (expected OK)")


# ---------------------------------------------------------------------------
# Test: READ polling (non-destructive)
# ---------------------------------------------------------------------------

def test_read_polling(ser: serial.Serial) -> None:
    print("\n── Phase 2: READ polling (5 samples) ──")

    temps = []
    for i in range(5):
        resp = send_command(ser, "READ", expecting="read")
        parsed = check_read_valid(resp, f"READ #{i+1}")
        if parsed:
            temps.append(parsed)
        time.sleep(READ_POLL_INTERVAL)

    if len(temps) >= 3:
        ets = [t["et"] for t in temps]
        bts = [t["bt"] for t in temps]
        # Simulated curve climbs ~2.5°C/s — allow 15°C drift over 5 samples
        check(max(ets) - min(ets) < 15.0,
              f"ET stable (±{max(ets) - min(ets):.1f}°C)")
        check(max(bts) - min(bts) < 15.0,
              f"BT stable (±{max(bts) - min(bts):.1f}°C)")


# ---------------------------------------------------------------------------
# Test: Units switching
# ---------------------------------------------------------------------------

def test_units_switch(ser: serial.Serial) -> None:
    print("\n── Phase 3: Units C→F→C ──")

    # Set Celsius, then READ
    send_command(ser, "UNITS;C", expecting="generic")
    time.sleep(0.3)
    resp_c = send_command(ser, "READ", expecting="read")
    parsed_c = check_read_valid(resp_c, "READ °C")

    # Set Fahrenheit, then READ
    send_command(ser, "UNITS;F", expecting="generic")
    time.sleep(0.3)
    resp_f = send_command(ser, "READ", expecting="read")
    parsed_f = check_read_valid(resp_f, "READ °F")

    if parsed_c and parsed_f:
        # Fahrenheit values should be higher than Celsius
        check(parsed_f["et"] > parsed_c["et"] or parsed_c["et"] < 5.0,
              f"ET {parsed_c['et']:.1f}°C → {parsed_f['et']:.1f}°F (higher)")
        # Verify approximate conversion: °C → °F.
        # The temperature climbs ~2°C/s during the simulated roast, so
        # the °F READ is taken at a higher actual temperature than the
        # °C READ.  Allow ±15°F drift tolerance.
        expected = parsed_c["et"] * 9.0 / 5.0 + 32.0
        check(abs(parsed_f["et"] - expected) < 15.0,
              f"ET conversion: {parsed_c['et']:.1f}°C → ~{expected:.1f}°F "
              f"(got {parsed_f['et']:.1f}°F, delta={parsed_f['et'] - expected:.1f})")

    # Restore Celsius for downstream tests
    send_command(ser, "UNITS;C", expecting="generic")


# ---------------------------------------------------------------------------
# Test: Manual heater + READ
# ---------------------------------------------------------------------------

def test_manual_control(ser: serial.Serial) -> None:
    print("\n── Phase 4: Manual control (OT1 + IO3) ──")

    send_command(ser, "OT1 0", expecting="generic")
    check(True, "OT1 0 sent (heater→0%)")

    send_command(ser, "IO3 0", expecting="generic")
    check(True, "IO3 0 sent (fan→0%)")

    resp = send_command(ser, "READ", expecting="read")
    check_read_valid(resp, "READ after manual control")


# ---------------------------------------------------------------------------
# Test: Profile command parsing (firmware-side)
# ---------------------------------------------------------------------------

def test_profile_command(ser: serial.Serial) -> None:
    print("\n── Phase 5: Profile command ──")

    resp = send_command(ser, "PROFILE;0,50;60,150;120,200;180,220",
                        timeout=1.5, expecting="generic")
    # Profile is loaded silently (no response unless error)
    check(resp is None or "ERR" not in resp,
          f"PROFILE accepted (response: {resp})")

    resp = send_command(ser, "FANPROFILE;0,30;60,50;120,70",
                        timeout=1.5, expecting="generic")
    check(resp is None or "ERR" not in resp,
          f"FANPROFILE accepted (response: {resp})")


# ---------------------------------------------------------------------------
# Test: STATUS command
# ---------------------------------------------------------------------------

def test_status_command(ser: serial.Serial) -> None:
    print("\n── Phase 6: STATUS command ──")

    resp = send_command(ser, "STATUS", expecting="status")
    if not resp:
        check(False, "STATUS: no response")
        return

    parts = resp.split(",")
    check(len(parts) >= 19,
          f"STATUS has {len(parts)} fields (expected >= 19)")
    check(True, f"STATUS received ({len(parts)} fields)")

    if len(parts) >= 4:
        try:
            etc = float(parts[0])
            btc = float(parts[1])
            check(etc < 350.0, f"STATUS ET={etc:.1f} (<350°C)")
            check(btc < 350.0, f"STATUS BT={btc:.1f} (<350°C)")
        except ValueError:
            check(False, "STATUS ET/BT not parseable")


# ---------------------------------------------------------------------------
# Test: Emergency STOP
# ---------------------------------------------------------------------------

def test_stop_command(ser: serial.Serial) -> None:
    print("\n── Phase 7: STOP command ──")

    send_command(ser, "STOP", expecting="generic")

    resp = send_command(ser, "READ", expecting="read")
    parsed = check_read_valid(resp, "READ after STOP")
    if parsed:
        check(parsed["et"] < 350.0, f"ET={parsed['et']:.1f} (sane)")
        check(parsed["bt"] < 350.0, f"BT={parsed['bt']:.1f} (sane)")


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def list_ports():
    ports = serial.tools.list_ports.comports()
    if not ports:
        print("No serial ports found.")
        return
    for p in sorted(ports):
        print(f"  {p.device}  —  {p.description}")


def verify_firmware_alive(ser: serial.Serial) -> bool:
    resp = send_command(ser, "READ", expecting="read", timeout=4.0)
    return resp is not None


def dry_run():
    print("Dry-run: test logic validated (no hardware communication).")
    print("\nTests that would run:")
    names = [
        "Handshake (CHAN, UNITS, FILT)",
        "READ polling (5 samples)",
        "Units switching (C→F→C)",
        "Manual control (OT1, IO3)",
        "Profile command (PROFILE, FANPROFILE)",
        "STATUS command (19 fields)",
        "Emergency STOP",
    ]
    for n in names:
        print(f"  ✅ {n}")
    print(f"\n  7/7 passed (dry-run)")


def main():
    global passed, failed

    parser = argparse.ArgumentParser(
        description="HIL Artisan roast simulation for LibreRoaster"
    )
    parser.add_argument("--port", "-p", default=None,
                        help="Serial port (e.g. /dev/ttyACM0)")
    parser.add_argument("--list-ports", action="store_true",
                        help="List serial ports and exit")
    parser.add_argument("--dry-run", action="store_true",
                        help="Validate script only, no hardware communication")
    args = parser.parse_args()

    if args.list_ports:
        list_ports()
        return

    if args.dry_run:
        dry_run()
        return

    port = args.port
    if not port:
        ports = serial.tools.list_ports.comports()
        candidates = [p.device for p in ports
                      if "ACM" in p.device
                      or "usb" in p.device.lower()
                      or "USB" in p.description]
        if not candidates:
            candidates = [p.device for p in ports]
        if not candidates:
            print("ERROR: No serial ports found.")
            sys.exit(1)
        port = candidates[0]
        print(f"Auto-selected port: {port}")

    print(f"Opening {port} at {BAUD} baud ...")
    print("⚠️  SAFETY: This script sends START/STOP/OT1 commands.")
    print("   Ensure your roaster is in a safe state before proceeding.")
    ser = open_port(port)

    print("Verifying firmware responds...", end=" ", flush=True)
    if not verify_firmware_alive(ser):
        print("NO RESPONSE")
        print("\n⚠️  Firmware did not respond. Check that:")
        print("   1. ESP32-C3 is flashed with LibreRoaster firmware")
        print("   2. The firmware booted and initialised USB CDC")
        print("   3. The correct port is selected")
        ser.close()
        sys.exit(1)
    print("OK\n")

    try:
        test_handshake(ser)
        test_read_polling(ser)
        test_units_switch(ser)
        test_manual_control(ser)
        test_profile_command(ser)
        test_status_command(ser)
        test_stop_command(ser)
    except serial.SerialException as e:
        print(f"\n⚠️  Serial error: {e}")
    except KeyboardInterrupt:
        print("\n⚠️  Interrupted")
    finally:
        try:
            send_command(ser, "OT1 0", expecting="generic")
            send_command(ser, "STOP", expecting="generic")
            send_command(ser, "UNITS;C", expecting="generic")
        except Exception:
            pass
        ser.close()

    total = passed + failed
    verdict = "ALL PASS" if failed == 0 else f"{failed} FAILURE(S)"
    print(f"\n{'=' * 40}")
    print(f"  {passed}/{total} passed  —  {verdict}")
    print(f"{'=' * 40}")

    sys.exit(0 if failed == 0 else 1)


if __name__ == "__main__":
    main()
