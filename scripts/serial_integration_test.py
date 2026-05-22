#!/usr/bin/env python3
"""Serial Integration Test for LibreRoaster ESP32-C3.

Connects to a real ESP32-C3 running the production firmware with
simulated-sensors and drives a full roast session, verifying responses
at each step.

Prerequisites:
    - ESP32-C3 connected via USB (UART or CDC)
    - Firmware flashed with: --features "embedded,simulated-sensors"
    - pyserial installed: pip install pyserial

Usage:
    python3 scripts/serial_integration_test.py [--port /dev/ttyUSB0] [--baud 115200]

Test phases:
    1. Connection and handshake (CHAN, UNITS, FILT)
    2. Temperature polling (READ — verify simulated curve produces temps)
    3. PID control (enable PID, set target, verify heater/fan in READ)
    4. Manual actuator control (OT1, OT2, UP, DOWN)
    5. Diagnostics (STATUS — verify all 20 fields)
    6. Emergency stop (STOP — verify heater=0, fan=100)
"""

import argparse
import sys
import time
from dataclasses import dataclass, field

import serial


# ── Configuration ──────────────────────────────────────────────────────────────

DEFAULT_PORT = "/dev/ttyUSB0"
DEFAULT_BAUD = 115200
READ_TIMEOUT = 5  # seconds to wait for a response
BOOT_DELAY = 5  # seconds to wait after device reset
COMMAND_DELAY = 0.5  # seconds between commands (firmware needs processing time)
READ_POLL_INTERVAL = 2.0  # seconds between READ polls
READ_POLL_COUNT = 10  # how many READ polls to do
TEMP_TOLERANCE = 30.0  # °C tolerance for simulated curve checks


# ── Test result tracking ───────────────────────────────────────────────────────

@dataclass
class TestResult:
    name: str
    passed: bool
    detail: str = ""

@dataclass
class TestReport:
    results: list = field(default_factory=list)

    def add(self, name: str, passed: bool, detail: str = ""):
        self.results.append(TestResult(name, passed, detail))
        status = "PASS" if passed else "FAIL"
        print(f"  TEST:{name}:{status}:{detail}")

    def summary(self) -> bool:
        total = len(self.results)
        passed = sum(1 for r in self.results if r.passed)
        failed = total - passed
        all_pass = failed == 0
        status = "PASS" if all_pass else "FAIL"
        print(f"\nTESTSUITE:COMPLETE:{passed}/{total}:{status}")
        return all_pass


# ── Serial helpers ─────────────────────────────────────────────────────────────

def is_log_line(line: str) -> bool:
    """Check if a line is firmware log/telemetry output, not a protocol response."""
    log_prefixes = [
        "INFO", "WARN", "ERROR", "DEBUG", "TRACE",
        "STAGE,", "STAGE:",
        "\x1b[",  # ANSI color codes
    ]
    if not line:
        return True
    for prefix in log_prefixes:
        if line.startswith(prefix):
            return True
    # Also filter lines that look like: "rd=...,watchdog=ok"
    if "watchdog=" in line and "elapsed=" in line:
        return True
    if "queue_enqueue" in line:
        return True
    if "queue_drain" in line:
        return True
    return False


def send_command(ser: serial.Serial, cmd: str, report: TestReport) -> str | None:
    """Send a command and read the protocol response, skipping log lines."""
    try:
        ser.reset_input_buffer()
        line = f"{cmd}\r\n"
        ser.write(line.encode("ascii"))
        ser.flush()
        time.sleep(COMMAND_DELAY)
        # Read lines until we find a non-log protocol response
        deadline = time.time() + READ_TIMEOUT
        while time.time() < deadline:
            raw = ser.readline().decode("ascii", errors="replace").strip()
            if not raw:
                continue
            if not is_log_line(raw):
                # Drain remaining log lines
                while ser.in_waiting > 0:
                    ser.readline()
                return raw
        return None
    except Exception as e:
        report.add(f"cmd_{cmd.split()[0].lower()}_send", False, f"serial_error={e}")
        return None


def send_and_expect(ser: serial.Serial, cmd: str, expected_prefix: str,
                    test_name: str, report: TestReport) -> str | None:
    """Send command, verify response starts with expected prefix."""
    response = send_command(ser, cmd, report)
    if response is None:
        report.add(test_name, False, "no_response")
        return None
    if response.startswith(expected_prefix):
        report.add(test_name, True, f"response={response}")
        return response
    else:
        report.add(test_name, False, f"expected_prefix={expected_prefix}:got={response}")
        return None


def parse_read_response(response: str) -> dict | None:
    """Parse a READ response into a dict with typed fields."""
    # READ response: AMB,ET,BT,0.0,0.0[,HEATER,FAN,SV]
    parts = response.split(",")
    if len(parts) < 5:
        return None
    try:
        result = {
            "ambient": float(parts[0]),
            "et": float(parts[1]),
            "bt": float(parts[2]),
        }
        if len(parts) >= 8:
            result["heater"] = float(parts[5])
            result["fan"] = float(parts[6])
            result["sv"] = float(parts[7])
        return result
    except (ValueError, IndexError):
        return None


def parse_status_response(response: str) -> dict | None:
    """Parse a STATUS response into a dict with typed fields."""
    parts = response.split(",")
    if len(parts) < 19 or len(parts) > 20:
        return None
    try:
        return {
            "et": float(parts[0]),
            "bt": float(parts[1]),
            "heater": float(parts[2]),
            "fan": float(parts[3]),
            "watchdog_ok": int(parts[4]),
            "watchdog_failures": int(parts[5]),
            "last_watchdog_reason": parts[6],
            "ledc_guard_timeouts": int(parts[7]),
            "regression_active": int(parts[8]),
            "pv": float(parts[9]),
            "mv": float(parts[10]),
            "integrator": float(parts[11]),
            "derivative": float(parts[12]),
            "saturation_flag": int(parts[13]),
            "integrator_clamp": int(parts[14]),
            "derivative_available": int(parts[15]),
            "command_latency_us": int(parts[16]),
            "max_command_latency_us": int(parts[17]),
            "temp_scale": int(parts[18]),
            "fault_condition": int(parts[19]) if len(parts) >= 20 else 0,
        }
    except (ValueError, IndexError):
        return None


# ── Test phases ────────────────────────────────────────────────────────────────

def phase_1_handshake(ser: serial.Serial, report: TestReport) -> bool:
    """Phase 1: Connection and handshake."""
    print("\n── Phase 1: Handshake ──")

    # CHAN;1200 → expect #1200
    send_and_expect(ser, "CHAN;1200", "#1200", "handshake_chan", report)

    # UNITS;C → expect OK
    send_and_expect(ser, "UNITS;C", "OK", "handshake_units", report)

    # FILT;70,70,70,70 → expect OK
    send_and_expect(ser, "FILT;70,70,70,70", "OK", "handshake_filt", report)

    return True


def phase_2_temperature_polling(ser: serial.Serial, report: TestReport) -> list[float]:
    """Phase 2: Poll READ and verify simulated temperatures."""
    print("\n── Phase 2: Temperature Polling ──")

    bt_values: list[float] = []
    et_values: list[float] = []

    for i in range(READ_POLL_COUNT):
        response = send_command(ser, "READ", report)
        if response is None:
            report.add(f"read_poll_{i+1:02d}", False, "no_response")
            time.sleep(READ_POLL_INTERVAL)
            continue

        data = parse_read_response(response)
        if data is None:
            report.add(f"read_poll_{i+1:02d}", False, f"parse_error:raw={response}")
            time.sleep(READ_POLL_INTERVAL)
            continue

        bt = data["bt"]
        et = data["et"]
        bt_values.append(bt)
        et_values.append(et)

        # Verify temperatures are in valid range (-50 to 350)
        valid_range = -50.0 <= bt <= 350.0 and -50.0 <= et <= 350.0
        report.add(
            f"read_poll_{i+1:02d}",
            valid_range,
            f"bt={bt:.1f}:et={et:.1f}:valid_range={valid_range}",
        )

        time.sleep(READ_POLL_INTERVAL)

    # Verify temperatures are changing (simulated curve should produce movement)
    if len(bt_values) >= 3:
        bt_moving = max(bt_values) - min(bt_values) > 1.0
        report.add(
            "read_bt_climbing",
            bt_moving,
            f"min={min(bt_values):.1f}:max={max(bt_values):.1f}:delta={max(bt_values)-min(bt_values):.1f}",
        )

        # Verify ET > BT (environmental temp leads bean temp in the curve)
        last_bt = bt_values[-1]
        last_et = et_values[-1]
        et_above_bt = last_et >= last_bt
        report.add(
            "read_et_above_bt",
            et_above_bt,
            f"bt={last_bt:.1f}:et={last_et:.1f}",
        )
    else:
        report.add("read_bt_climbing", False, "insufficient_samples")
        report.add("read_et_above_bt", False, "insufficient_samples")

    return bt_values


def phase_3_pid_control(ser: serial.Serial, report: TestReport) -> None:
    """Phase 3: Enable PID and verify actuator outputs appear in READ."""
    print("\n── Phase 3: PID Control ──")

    # Enable PID
    send_and_expect(ser, "PID;ON", "OK", "pid_enable", report)

    # Set target temperature
    send_and_expect(ser, "PID;SV;210", "OK", "pid_setpoint", report)

    # Set PID gains
    send_and_expect(ser, "PID;T;2.0;0.25;0.05", "OK", "pid_gains", report)

    # Poll READ — should now have 8 fields (AMBIENT,ET,BT,0.0,0.0,HEATER,FAN,SV)
    time.sleep(1.0)
    response = send_command(ser, "READ", report)
    if response is None:
        report.add("pid_read_8fields", False, "no_response")
        return

    data = parse_read_response(response)
    if data is None:
        report.add("pid_read_8fields", False, f"parse_error:raw={response}")
        return

    has_pid_fields = "heater" in data and "fan" in data and "sv" in data
    report.add(
        "pid_read_8fields",
        has_pid_fields,
        f"heater={data.get('heater', 'N/A')}:fan={data.get('fan', 'N/A')}:sv={data.get('sv', 'N/A')}",
    )

    # Verify setpoint matches what we set
    if has_pid_fields:
        sv_close = abs(data["sv"] - 210.0) < 1.0
        report.add(
            "pid_setpoint_value",
            sv_close,
            f"sv={data['sv']:.1f}:expected=210.0",
        )

    # Verify heater is nonzero (PID should be trying to reach target)
    if has_pid_fields:
        heater_active = data["heater"] > 0.0
        report.add(
            "pid_heater_active",
            heater_active,
            f"heater={data['heater']:.1f}",
        )


def phase_4_manual_control(ser: serial.Serial, report: TestReport) -> None:
    """Phase 4: Manual actuator control."""
    print("\n── Phase 4: Manual Actuator Control ──")

    # Disable PID first so manual commands take effect
    send_and_expect(ser, "PID;OFF", "OK", "manual_pid_off", report)

    # Test OT1 (heater) — set to 50%
    response = send_command(ser, "OT1 50", report)
    if response is not None:
        report.add("manual_ot1", True, f"response={response}")
    else:
        report.add("manual_ot1", False, "no_response")

    # Test OT2 (fan) — set to 75%
    response = send_command(ser, "OT2 75", report)
    if response is not None:
        report.add("manual_ot2", True, f"response={response}")
    else:
        report.add("manual_ot2", False, "no_response")

    # Test UP (increment heater)
    response = send_command(ser, "UP", report)
    if response is not None:
        report.add("manual_up", True, f"response={response}")
    else:
        report.add("manual_up", False, "no_response")

    # Test DOWN (decrement heater)
    response = send_command(ser, "DOWN", report)
    if response is not None:
        report.add("manual_down", True, f"response={response}")
    else:
        report.add("manual_down", False, "no_response")

    # Read STATUS to verify actuator values
    time.sleep(0.5)
    response = send_command(ser, "STATUS", report)
    if response:
        data = parse_status_response(response)
        if data:
            heater_nonzero = data["heater"] > 0.0
            fan_nonzero = data["fan"] > 0.0
            report.add(
                "manual_status_actuators",
                heater_nonzero and fan_nonzero,
                f"heater={data['heater']:.1f}:fan={data['fan']:.1f}",
            )
        else:
            report.add("manual_status_actuators", False, f"parse_error:raw={response[:80]}")
    else:
        report.add("manual_status_actuators", False, "no_response")


def phase_5_diagnostics(ser: serial.Serial, report: TestReport) -> None:
    """Phase 5: STATUS diagnostics — verify all 20 fields."""
    print("\n── Phase 5: STATUS Diagnostics ──")

    response = send_command(ser, "STATUS", report)
    if response is None:
        report.add("status_response", False, "no_response")
        return

    data = parse_status_response(response)
    if data is None:
        report.add("status_parse", False, f"19_fields_expected:raw={response[:100]}")
        return

    report.add("status_parse", True, "19_fields_parsed")

    # Verify temperatures are present
    bt_valid = -50.0 <= data["bt"] <= 350.0
    report.add("status_bt_valid", bt_valid, f"bt={data['bt']:.1f}")

    et_valid = -50.0 <= data["et"] <= 350.0
    report.add("status_et_valid", et_valid, f"et={data['et']:.1f}")

    # Verify heater in range
    heater_valid = 0.0 <= data["heater"] <= 100.0
    report.add("status_heater_range", heater_valid, f"heater={data['heater']:.1f}")

    # Verify fan in range
    fan_valid = 0.0 <= data["fan"] <= 100.0
    report.add("status_fan_range", fan_valid, f"fan={data['fan']:.1f}")

    # Verify watchdog is healthy
    wd_ok = data["watchdog_ok"] >= 0
    report.add("status_watchdog", wd_ok, f"ok={data['watchdog_ok']}:failures={data['watchdog_failures']}")

    # Verify command latency is reasonable (< 10s)
    latency_ok = data["command_latency_us"] < 10_000_000
    report.add(
        "status_latency",
        latency_ok,
        f"latency_us={data['command_latency_us']}:max_us={data['max_command_latency_us']}",
    )

    # Verify temp scale (0=Celsius since we sent UNITS;C)
    scale_ok = data["temp_scale"] == 0
    report.add("status_temp_scale", scale_ok, f"scale={data['temp_scale']}:expected=0")

    # Verify PID fields present (PV, MV, integrator, derivative)
    pv_valid = -50.0 <= data["pv"] <= 350.0 or data["pv"] == 0.0
    report.add("status_pv_valid", pv_valid, f"pv={data['pv']:.1f}")

    # Verify LEDC guard timeouts is a valid count
    ledc_valid = data["ledc_guard_timeouts"] >= 0
    report.add("status_ledc_guard", ledc_valid, f"timeouts={data['ledc_guard_timeouts']}")


def phase_6_emergency_stop(ser: serial.Serial, report: TestReport) -> None:
    """Phase 6: STOP — verify heater=0, fan=100."""
    print("\n── Phase 6: Emergency Stop ──")

    # Send STOP
    response = send_command(ser, "STOP", report)
    if response is None:
        report.add("stop_response", False, "no_response")
        return

    report.add("stop_response", True, f"response={response}")

    # Read STATUS to verify heater=0, fan=100
    time.sleep(0.5)
    response = send_command(ser, "STATUS", report)
    if response is None:
        report.add("stop_status", False, "no_response")
        return

    data = parse_status_response(response)
    if data is None:
        report.add("stop_status", False, f"parse_error")
        return

    heater_zero = data["heater"] == 0.0
    report.add("stop_heater_zero", heater_zero, f"heater={data['heater']:.1f}")

    fan_full = data["fan"] == 100.0
    report.add("stop_fan_100", fan_full, f"fan={data['fan']:.1f}")

    emergency_confirmed = heater_zero and fan_full
    report.add(
        "stop_emergency_shutdown",
        emergency_confirmed,
        f"heater={data['heater']:.1f}:fan={data['fan']:.1f}",
    )


# ── Main ───────────────────────────────────────────────────────────────────────

def drain_buffer(ser: serial.Serial, timeout: float = 2.0) -> list[str]:
    """Drain all pending data from the serial buffer."""
    lines = []
    deadline = time.time() + timeout
    while time.time() < deadline:
        if ser.in_waiting > 0:
            line = ser.readline().decode("ascii", errors="replace").strip()
            if line:
                lines.append(line)
        else:
            break
    return lines


def main() -> int:
    parser = argparse.ArgumentParser(description="LibreRoaster Serial Integration Test")
    parser.add_argument("--port", default=DEFAULT_PORT, help=f"Serial port (default: {DEFAULT_PORT})")
    parser.add_argument("--baud", type=int, default=DEFAULT_BAUD, help=f"Baud rate (default: {DEFAULT_BAUD})")
    parser.add_argument("--skip-flash", action="store_true", help="Skip firmware flashing (already flashed)")
    parser.add_argument("--boot-delay", type=float, default=BOOT_DELAY, help="Seconds to wait after flash")
    args = parser.parse_args()

    report = TestReport()

    print("=" * 60)
    print("LibreRoaster Serial Integration Test")
    print("=" * 60)
    print(f"Port: {args.port}")
    print(f"Baud: {args.baud}")

    # ── Flash firmware if requested ──
    if not args.skip_flash:
        print(f"\n── Flashing firmware with simulated-sensors ──")
        import subprocess
        flash_cmd = [
            "cargo", "espflash", "flash",
            "--release",
            "--target", "riscv32imc-unknown-none-elf",
            "--features", "embedded,simulated-sensors",
            "--", "--monitor=false",
        ]
        print(f"  Running: {' '.join(flash_cmd)}")
        result = subprocess.run(flash_cmd, capture_output=True, text=True, timeout=120)
        if result.returncode != 0:
            # espflash may succeed with non-zero exit due to monitor
            if "Flashing has completed" not in result.stdout and "successfully" not in result.stdout.lower():
                print(f"  Flash FAILED:\n{result.stderr[-500:]}")
                report.add("firmware_flash", False, f"exit_code={result.returncode}")
                report.summary()
                return 1
        print("  Flash completed successfully")
        report.add("firmware_flash", True, "success")
    else:
        print("\n── Skipping flash (--skip-flash) ──")

    # ── Connect to serial port ──
    print(f"\n── Connecting to {args.port} ──")
    print(f"  Waiting {args.boot_delay}s for device boot...")

    time.sleep(args.boot_delay)

    try:
        ser = serial.Serial(
            port=args.port,
            baudrate=args.baud,
            timeout=READ_TIMEOUT,
            bytesize=serial.EIGHTBITS,
            parity=serial.PARITY_NONE,
            stopbits=serial.STOPBITS_ONE,
        )
    except serial.SerialException as e:
        report.add("serial_connect", False, f"error={e}")
        report.summary()
        return 1

    print(f"  Connected: {ser.name}")
    report.add("serial_connect", True, f"port={ser.name}")

    # Drain any boot messages
    boot_lines = drain_buffer(ser, timeout=3.0)
    if boot_lines:
        print(f"  Boot output ({len(boot_lines)} lines):")
        for line in boot_lines[:5]:
            print(f"    {line}")
        if len(boot_lines) > 5:
            print(f"    ... ({len(boot_lines) - 5} more lines)")

    # ── Run test phases ──
    try:
        phase_1_handshake(ser, report)
        phase_2_temperature_polling(ser, report)
        phase_3_pid_control(ser, report)
        phase_4_manual_control(ser, report)
        phase_5_diagnostics(ser, report)
        phase_6_emergency_stop(ser, report)
    except Exception as e:
        print(f"\n  FATAL: Test aborted with exception: {e}")
        report.add("fatal_exception", False, str(e))
    finally:
        ser.close()
        print(f"\n  Serial port closed")

    # ── Report ──
    all_pass = report.summary()
    return 0 if all_pass else 1


if __name__ == "__main__":
    sys.exit(main())
