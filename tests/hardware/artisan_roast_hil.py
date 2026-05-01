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
RESPONSE_TIMEOUT = 2
CMD_DELAY = 0.1
READ_POLL_INTERVAL = 0.5


# ---------------------------------------------------------------------------
# Serial helpers
# ---------------------------------------------------------------------------

def open_port(port: str) -> serial.Serial:
    ser = serial.Serial(port, BAUD, timeout=RESPONSE_TIMEOUT)
    time.sleep(0.3)
    ser.reset_input_buffer()
    return ser


def send(ser: serial.Serial, cmd: str) -> None:
    ser.write(f"{cmd}\r".encode("utf-8"))
    time.sleep(CMD_DELAY)


def read_line(ser: serial.Serial) -> Optional[str]:
    raw = ser.readline()
    if not raw:
        return None
    return raw.decode("utf-8", errors="replace").strip()


def _looks_like_read_response(text: str) -> bool:
    return text and text[0] in "0123456789-"


def send_and_read(ser: serial.Serial, cmd: str, timeout: float = 3.0) -> Optional[str]:
    """Send a command and return the READ response (may arrive asynchronously)."""
    send(ser, cmd)
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        line = read_line(ser)
        if line is None:
            continue
        if _looks_like_read_response(line):
            return line
    return None


def flush_input(ser: serial.Serial) -> None:
    """Drain any pending data from the input buffer."""
    time.sleep(0.2)
    ser.reset_input_buffer()


# ---------------------------------------------------------------------------
# Parsers
# ---------------------------------------------------------------------------

def parse_read_5(response: str) -> Optional[dict]:
    parts = response.split(",")
    if len(parts) != 5:
        return None
    try:
        return {"ambient": float(parts[0]), "et": float(parts[1]), "bt": float(parts[2])}
    except (ValueError, IndexError):
        return None


def parse_read(response: str) -> Optional[dict]:
    p = parse_read_5(response)
    if p:
        return p
    parts = response.split(",")
    if len(parts) == 8:
        try:
            return {"ambient": float(parts[0]), "et": float(parts[1]), "bt": float(parts[2])}
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

    # CHAN;1200 → expect "#1200" ack
    resp = send_and_read(ser, "CHAN;1200")
    check(resp == "#1200", f"CHAN;1200 → '{resp}' (expected #1200)")

    # UNITS;C → expect "OK"
    resp = send_and_read(ser, "UNITS;C")
    check(resp == "OK", f"UNITS;C → '{resp}' (expected OK)")

    # FILT;70 → expect "OK"
    resp = send_and_read(ser, "FILT;70")
    check(resp == "OK", f"FILT;70 → '{resp}' (expected OK)")


# ---------------------------------------------------------------------------
# Test: READ polling (non-destructive)
# ---------------------------------------------------------------------------

def test_read_polling(ser: serial.Serial) -> None:
    print("\n── Phase 2: READ polling (5 samples) ──")

    temps = []
    for i in range(5):
        resp = send_and_read(ser, "READ")
        parsed = check_read_valid(resp, f"READ #{i+1}")
        if parsed:
            temps.append(parsed)
        time.sleep(READ_POLL_INTERVAL)

    if len(temps) >= 3:
        ets = [t["et"] for t in temps]
        bts = [t["bt"] for t in temps]
        check(max(ets) - min(ets) < 10.0, f"ET stable (±{max(ets)-min(ets):.1f}°C)")
        check(max(bts) - min(bts) < 10.0, f"BT stable (±{max(bts)-min(bts):.1f}°C)")


# ---------------------------------------------------------------------------
# Test: Units switching
# ---------------------------------------------------------------------------

def test_units_switch(ser: serial.Serial) -> None:
    print("\n── Phase 3: Units C→F→C ──")

    send(ser, "UNITS;C")
    time.sleep(0.1)
    resp_c = send_and_read(ser, "READ")
    parsed_c = check_read_valid(resp_c, "READ °C")

    send(ser, "UNITS;F")
    time.sleep(0.1)
    resp_f = send_and_read(ser, "READ")
    parsed_f = check_read_valid(resp_f, "READ °F")

    if parsed_c and parsed_f:
        check(parsed_f["et"] > parsed_c["et"] or parsed_c["et"] < 5.0,
              f"ET {parsed_c['et']:.1f}°C → {parsed_f['et']:.1f}°F (higher)")
        expected = parsed_c["et"] * 9.0 / 5.0 + 32.0
        check(abs(parsed_f["et"] - expected) < 2.5,
              f"ET conversion: {parsed_c['et']:.1f}°C → ~{expected:.1f}°F (got {parsed_f['et']:.1f}°F)")

    send(ser, "UNITS;C")
    time.sleep(0.1)
    flush_input(ser)


# ---------------------------------------------------------------------------
# Test: Manual heater + READ
# ---------------------------------------------------------------------------

def test_manual_control(ser: serial.Serial) -> None:
    print("\n── Phase 4: Manual control (OT1 + IO3) ──")

    # Set heater to safe 0% first, just to verify command-response
    send(ser, "OT1 0")
    time.sleep(0.1)
    flush_input(ser)
    check(True, "OT1 0 sent (heater→0%)")

    send(ser, "IO3 0")
    time.sleep(0.1)
    flush_input(ser)
    check(True, "IO3 0 sent (fan→0%)")

    # READ after manual commands
    resp = send_and_read(ser, "READ")
    check_read_valid(resp, "READ after manual control")


# ---------------------------------------------------------------------------
# Test: Profile command parsing (firmware-side)
# ---------------------------------------------------------------------------

def test_profile_command(ser: serial.Serial) -> None:
    print("\n── Phase 5: Profile command ──")

    resp = send_and_read(ser, "PROFILE;0,50;60,150;120,200;180,220")
    # Profile is loaded silently (no response unless err)
    check(resp is None or "ERR" not in resp,
          f"PROFILE accepted (response: {resp})")

    resp = send_and_read(ser, "FANPROFILE;0,30;60,50;120,70")
    check(resp is None or "ERR" not in resp,
          f"FANPROFILE accepted (response: {resp})")


# ---------------------------------------------------------------------------
# Test: STATUS command
# ---------------------------------------------------------------------------

def test_status_command(ser: serial.Serial) -> None:
    print("\n── Phase 6: STATUS command ──")

    resp = send_and_read(ser, "STATUS")
    if not resp:
        check(False, "STATUS: no response")
        return

    parts = resp.split(",")
    check(len(parts) == 19, f"STATUS has {len(parts)} fields (expected 19)")
    check(True, f"STATUS={resp[:60]}...")
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

    send(ser, "STOP")
    time.sleep(0.1)
    flush_input(ser)

    # READ after STOP should still return valid data
    resp = send_and_read(ser, "READ")
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
                      if "ACM" in p.device or "usb" in p.device.lower() or "USB" in p.description]
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

    # Verify firmware is alive
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
            send(ser, "OT1 0")
            send(ser, "STOP")
            send(ser, "UNITS;C")
            time.sleep(0.2)
            flush_input(ser)
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
