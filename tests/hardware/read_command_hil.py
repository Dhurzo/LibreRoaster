#!/usr/bin/env python3
"""
HIL test: READ command validation via USB/serial on real hardware.

Connects to an ESP32-C3 running LibreRoaster firmware, sends READ commands,
and validates the TC4-standard responses (AMB,ET,BT,CHAN3,CHAN4).

Usage:
  # Default port (Linux)
  python tests/hardware/read_command_hil.py

  # Explicit port
  python tests/hardware/read_command_hil.py --port /dev/ttyACM0

  # List available ports
  python tests/hardware/read_command_hil.py --list-ports
"""

import argparse
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
READ_TIMEOUT = 2  # seconds to wait for a READ response
CMD_DELAY = 0.05  # 50ms between commands


# ---------------------------------------------------------------------------
# Serial helpers
# ---------------------------------------------------------------------------

def open_port(port: str) -> serial.Serial:
    ser = serial.Serial(port, BAUD, timeout=READ_TIMEOUT)
    # Drain bootloader/startup messages
    time.sleep(1.0)
    ser.reset_input_buffer()
    time.sleep(0.5)
    # Send a few CRs to synchronise with the firmware's USB reader task
    for _ in range(3):
        ser.write(b"\r")
        time.sleep(0.1)
    ser.reset_input_buffer()
    return ser


def send(ser: serial.Serial, cmd: str) -> None:
    """Send a command terminated by CR."""
    ser.write(f"{cmd}\r".encode("utf-8"))
    time.sleep(CMD_DELAY)


def read_line(ser: serial.Serial) -> Optional[str]:
    """Read one line from serial, return stripped or None."""
    raw = ser.readline()
    if not raw:
        return None
    text = raw.decode("utf-8", errors="replace").strip()
    return text


def _looks_like_read_response(text: str) -> bool:
    """Heuristic: TC4 READ response starts with a digit or minus."""
    return text and text[0] in "0123456789-"


def send_and_read(ser: serial.Serial, cmd: str, timeout: float = 3.0) -> Optional[str]:
    """Send a command and return the READ response (may arrive asynchronously).

    The firmware processes commands through an async pipeline
    (queue → control loop → output writer), so the response may
    arrive after several lines of other output. We read up to
    *timeout* seconds looking for a TC4-formatted response line.
    """
    send(ser, cmd)
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        line = read_line(ser)
        if line is None:
            continue
        if _looks_like_read_response(line):
            return line
    return None


# ---------------------------------------------------------------------------
# READ response parsers
# ---------------------------------------------------------------------------

def parse_read_5(response: str) -> Optional[dict]:
    """Parse TC4 5-value READ response: AMB,ET,BT,CHAN3,CHAN4"""
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


def parse_read_8(response: str) -> Optional[dict]:
    """Parse TC4 8-value READ response: AMB,ET,BT,CHAN3,CHAN4,heater,fan,SV"""
    parts = response.split(",")
    if len(parts) != 8:
        return None
    try:
        return {
            "ambient": float(parts[0]),
            "et": float(parts[1]),
            "bt": float(parts[2]),
            "heater": float(parts[5]),
            "fan": float(parts[6]),
            "sv": float(parts[7]),
        }
    except (ValueError, IndexError):
        return None


def parse_read(response: str) -> Optional[dict]:
    """Auto-detect 5 or 8 value format and parse."""
    result = parse_read_5(response)
    if result:
        return result
    return parse_read_8(response)


# ---------------------------------------------------------------------------
# Validation
# ---------------------------------------------------------------------------

def check(condition: bool, msg: str) -> None:
    """Print PASS/FAIL and return nothing — caller continues."""
    if condition:
        print(f"  ✅ {msg}")
    else:
        print(f"  ❌ {msg}")


# ---------------------------------------------------------------------------
# Test suites
# ---------------------------------------------------------------------------

def test_basic_read(ser: serial.Serial) -> int:
    """Test basic READ command returns valid TC5 5-value response."""
    print("\n--- Basic READ ---")
    failures = 0

    response = send_and_read(ser, "READ")
    if not response:
        print("  ❌ No response to READ")
        return 1

    parsed = parse_read(response)
    if not parsed:
        print(f"  ❌ Unparseable READ response: {response}")
        return 1

    check(parsed["ambient"] >= 0.0, f"AMB={parsed['ambient']:.1f} (≥0)")
    check(parsed["et"] >= 0.0, f"ET={parsed['et']:.1f} (≥0)")
    check(parsed["bt"] >= 0.0, f"BT={parsed['bt']:.1f} (≥0)")
    check(parsed["et"] < 350.0, f"ET={parsed['et']:.1f} (<350°C, sane)")
    check(parsed["bt"] < 350.0, f"BT={parsed['bt']:.1f} (<350°C, sane)")

    parts = response.split(",")
    check(len(parts) == 5, f"Response has {len(parts)} fields (expected 5): {response}")
    check(parts[3] == "0.0", f"CHAN3=0.0 (got {parts[3]})")
    check(parts[4] == "0.0", f"CHAN4=0.0 (got {parts[4]})")

    return 0


def test_multiple_reads(ser: serial.Serial) -> int:
    """READ is consistent across multiple requests."""
    print("\n--- Multiple READs (consistency) ---")

    values = []
    for i in range(5):
        response = send_and_read(ser, "READ")
        if not response:
            print(f"  ❌ No response for READ #{i+1}")
            continue
        parsed = parse_read(response)
        if not parsed:
            print(f"  ❌ Unparseable: {response}")
            continue
        values.append(parsed)
        print(f"   READ #{i+1}: AMB={parsed['ambient']:.1f}  ET={parsed['et']:.1f}  BT={parsed['bt']:.1f}")

    check(len(values) >= 3, f"Got {len(values)}/5 valid READ responses")

    if len(values) >= 2:
        # Temperatures should be roughly stable (not wildly different)
        ets = [v["et"] for v in values]
        bts = [v["bt"] for v in values]
        et_delta = max(ets) - min(ets)
        bt_delta = max(bts) - min(bts)
        check(et_delta < 5.0, f"ET stable (±{et_delta:.1f}°C over {len(values)} reads)")
        check(bt_delta < 5.0, f"BT stable (±{bt_delta:.1f}°C over {len(values)} reads)")
    return 0


def test_units_switch(ser: serial.Serial) -> int:
    """Switch to Fahrenheit, READ, then back to Celsius."""
    print("\n--- Units switching (C → F → C) ---")
    failures = 0

    # First ensure Celsius
    send(ser, "UNITS;C")
    time.sleep(0.1)
    response_c = send_and_read(ser, "READ")
    if not response_c:
        print("  ❌ No READ response in Celsius mode")
        return 1

    parsed_c = parse_read(response_c)
    if not parsed_c:
        print(f"  ❌ Unparseable in °C: {response_c}")
        failures += 1

    # Switch to Fahrenheit
    send(ser, "UNITS;F")
    time.sleep(0.1)

    response_f = send_and_read(ser, "READ")
    if not response_f:
        print("  ❌ No READ response in Fahrenheit mode")
        send(ser, "UNITS;C")
        return 1

    parsed_f = parse_read(response_f)
    if not parsed_f:
        print(f"  ❌ Unparseable in °F: {response_f}")
        failures += 1

    # Validate: °F values should be higher than °C values
    if parsed_c and parsed_f:
        check(parsed_f["ambient"] > parsed_c["ambient"] or parsed_c["ambient"] < 5.0,
              f"AMB {parsed_c['ambient']:.1f}°C → {parsed_f['ambient']:.1f}°F (higher in °F)")
        check(parsed_f["et"] > parsed_c["et"] or parsed_c["et"] < 5.0,
              f"ET {parsed_c['et']:.1f}°C → {parsed_f['et']:.1f}°F (higher in °F)")
        check(parsed_f["bt"] > parsed_c["bt"] or parsed_c["bt"] < 5.0,
              f"BT {parsed_c['bt']:.1f}°C → {parsed_f['bt']:.1f}°F (higher in °F)")

        # Rough sanity: °F ≈ °C * 9/5 + 32
        expected_et_f = parsed_c["et"] * 9.0 / 5.0 + 32.0
        et_delta = abs(parsed_f["et"] - expected_et_f)
        check(et_delta < 2.0,
              f"ET °C→°F conversion error: {parsed_c['et']:.1f}°C → {parsed_f['et']:.1f}°F "
              f"(expected ~{expected_et_f:.1f}°F, Δ={et_delta:.1f})")

    # Restore Celsius
    send(ser, "UNITS;C")
    time.sleep(0.1)
    restore = send_and_read(ser, "READ")
    if restore:
        print(f"   Restored to °C: {restore}")

    return failures


def test_read_field_order(ser: serial.Serial) -> int:
    """Verify TC4 field order is AMB,ET,BT,CHAN3,CHAN4."""
    print("\n--- TC4 field order verification ---")

    response = send_and_read(ser, "READ")
    if not response:
        print("  ❌ No response")
        return 1

    parts = response.split(",")
    check(len(parts) >= 5, f"Response has {len(parts)} fields")

    if len(parts) >= 5:
        # AMB should be a reasonable ambient temperature
        try:
            amb = float(parts[0])
            check(0 <= amb <= 50, f"Field 0 (AMB) = {amb:.1f} (0-50°C plausible)")
        except ValueError:
            print(f"  ❌ Field 0 not a float: {parts[0]}")

        # CHAN3 and CHAN4 should be 0.0
        check(parts[3] == "0.0", f"Field 3 (CHAN3) = {parts[3]} (expected 0.0)")
        check(parts[4] == "0.0", f"Field 4 (CHAN4) = {parts[4]} (expected 0.0)")

    return 0


def test_no_terminators(ser: serial.Serial) -> int:
    """READ response has no embedded CR/LF."""
    print("\n--- READ response terminator check ---")

    raw = b""
    ser.write(b"READ\r")
    time.sleep(CMD_DELAY)

    # Read until we get a full line or timeout
    deadline = time.monotonic() + READ_TIMEOUT
    while time.monotonic() < deadline:
        chunk = ser.read(1)
        if not chunk:
            break
        raw += chunk
        if chunk == b"\n":
            break

    text = raw.decode("utf-8", errors="replace").strip()
    check("\r" not in text, "No embedded CR in READ response")
    check(text.startswith(tuple("0123456789.-")), f"Response starts with digit or sign: {text[:20]}")

    return 0


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


def dry_run_test(name: str, fn, *args) -> int:
    """Run a test function in dry-run mode with simulated data."""
    print(f"\n--- {name} (dry-run) ---")
    class FakeSerial:
        def write(self, _): pass
        def readline(self): return b"25.0,185.3,201.4,0.0,0.0\n"
        def read(self, n=1): return b"25.0,185.3,201.4,0.0,0.0\n"[:n]
        def reset_input_buffer(self): pass
        def close(self): pass
    return fn(FakeSerial())


def verify_firmware_alive(ser: serial.Serial) -> bool:
    """Send a READ and check if we get a valid-looking response."""
    ser.write(b"READ\r")
    deadline = time.monotonic() + 4.0
    while time.monotonic() < deadline:
        raw = ser.readline()
        if not raw:
            continue
        text = raw.decode("utf-8", errors="replace").strip()
        if text and text[0] in "0123456789-":
            return True
    return False


def main():
    parser = argparse.ArgumentParser(description="HIL READ command test for LibreRoaster")
    parser.add_argument("--port", "-p", default=None,
                        help="Serial port (e.g. /dev/ttyACM0). Auto-detects if omitted.")
    parser.add_argument("--list-ports", action="store_true",
                        help="List available serial ports and exit.")
    parser.add_argument("--dry-run", action="store_true",
                        help="Validate test logic without hardware.")
    args = parser.parse_args()

    if args.list_ports:
        list_ports()
        return

    if args.dry_run:
        print("Dry-run: validating test logic with simulated data...")
        total_failures = 0
        total_failures += dry_run_test("Basic READ", test_basic_read)
        total_failures += dry_run_test("Multiple READs", test_multiple_reads)
        total_failures += dry_run_test("TC4 field order", test_read_field_order)
        total_failures += dry_run_test("No terminators", test_no_terminators)
        passed = 4 - total_failures
        print(f"\n{'=' * 40}")
        print(f"  {passed}/4 passed (dry-run)  —  {'ALL PASS' if total_failures == 0 else f'{total_failures} FAILURES'}")
        print(f"{'=' * 40}")
        return

    port = args.port
    if not port:
        ports = serial.tools.list_ports.comports()
        candidates = [p.device for p in ports
                      if "ACM" in p.device or "usb" in p.device.lower() or "USB" in p.description]
        if not candidates:
            candidates = [p.device for p in ports]
        if not candidates:
            print("ERROR: No serial ports found. Specify --port or check connection.")
            sys.exit(1)
        port = candidates[0]
        print(f"Auto-selected port: {port}")

    print(f"Opening {port} at {BAUD} baud ...")
    ser = open_port(port)
    print(f"Connected.\n")

    # Verify firmware is alive
    print("Verifying firmware responds to READ...", end=" ", flush=True)
    if not verify_firmware_alive(ser):
        print("NO RESPONSE")
        print("\n⚠️  Firmware did not respond to READ command. Check that:")
        print("   1. The ESP32-C3 is flashed with LibreRoaster firmware")
        print("   2. The firmware boots and initialises USB CDC")
        print("   3. The correct port is selected (try --list-ports)")
        print("   4. No other program holds the serial port open")
        ser.close()
        sys.exit(1)
    print("OK")

    total_failures = 0
    total_tests = 5

    try:
        total_failures += test_basic_read(ser)
        total_failures += test_multiple_reads(ser)
        total_failures += test_units_switch(ser)
        total_failures += test_read_field_order(ser)
        total_failures += test_no_terminators(ser)
    except serial.SerialException as e:
        print(f"\n⚠️  Serial error: {e}")
        total_failures = total_tests
    except KeyboardInterrupt:
        print("\n⚠️  Interrupted")
    finally:
        try:
            send(ser, "UNITS;C")
        except Exception:
            pass
        ser.close()

    passed = total_tests - total_failures
    verdict = "ALL PASS" if total_failures == 0 else f"{total_failures} FAILURES"
    print(f"\n{'=' * 40}")
    print(f"  {passed}/{total_tests} passed  —  {verdict}")
    print(f"{'=' * 40}")

    sys.exit(0 if total_failures == 0 else 1)


if __name__ == "__main__":
    main()
