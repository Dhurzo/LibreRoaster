import serial
import time
import csv
import argparse
import os
import json
from datetime import datetime

def load_manifest(path):
    with open(path, 'r') as f:
        return json.load(f)


def normalize_suffix(suffix):
    if not suffix.startswith('.'):
        return f'.{suffix}'
    return suffix


def build_fieldnames():
    return [
        'timestamp', 'et', 'bt', 'heater', 'fan',
        'watchdog_flag', 'failure_count', 'failure_reason',
        'guard_timeouts', 'regression_flag', 'pv', 'mv',
        'integrator_value', 'derivative_value', 'saturation_flag',
        'integrator_clamp_flag', 'derivative_available_flag',
        'command_latency_us', 'max_command_latency_us'
    ]


def build_read_fieldnames():
    return ['timestamp', 'et', 'bt', 'valid']


def parse_read_response(line):
    """Parse TC4 READ response: AMB,ET,BT,CHAN3,CHAN4"""
    parts = line.split(',')
    if len(parts) == 5:
        try:
            et = float(parts[1])
            bt = float(parts[2])
            return {'et': et, 'bt': bt, 'valid': True}
        except (ValueError, IndexError):
            pass
    return {'et': 0.0, 'bt': 0.0, 'valid': False}


def is_hardware_scenario(scenario_id):
    """Check if scenario ID is a hardware component type."""
    return scenario_id.startswith(('TC-', 'SSR-', 'FAN-', 'GPIO-'))


def send_and_read(ser, command, timeout_chars=100):
    """Send a command and read one response line."""
    ser.write(f"{command}\n".encode('utf-8'))
    line = ser.readline().decode('utf-8', errors='replace').strip()
    return line


def execute_hardware_sequence(ser, scenario_entry, csv_path, args, max_latency_ref):
    """Execute hardware command sequence from manifest entry."""
    command_sequence = scenario_entry.get('command_sequence', '')
    run_dir = os.path.dirname(csv_path)
    read_csv_path = os.path.join(run_dir, 'read_telemetry.csv')

    read_fieldnames = build_read_fieldnames()
    status_fieldnames = build_fieldnames()

    max_latency = max_latency_ref[0] if max_latency_ref else 0.0

    with open(csv_path, 'a', newline='') as status_csvfile, \
         open(read_csv_path, 'a', newline='') as read_csvfile:

        status_writer = csv.DictWriter(status_csvfile, fieldnames=status_fieldnames)
        read_writer = csv.DictWriter(read_csvfile, fieldnames=read_fieldnames)

        if not os.path.isfile(csv_path) or os.path.getsize(csv_path) == 0:
            status_writer.writeheader()
        if not os.path.isfile(read_csv_path) or os.path.getsize(read_csv_path) == 0:
            read_writer.writeheader()

        for cmd in command_sequence.split(';'):
            cmd = cmd.strip()
            if not cmd:
                continue

            if cmd == 'READ':
                print(f"  Sending: READ")
                line = send_and_read(ser, 'READ')
                if not line:
                    print("  No response for READ")
                    continue
                if line.startswith('#') or line.startswith('ERR'):
                    print(f"  Firmware message: {line}")
                    continue

                parsed = parse_read_response(line)
                row = {
                    'timestamp': datetime.utcnow().isoformat(),
                    'et': parsed['et'],
                    'bt': parsed['bt'],
                    'valid': parsed['valid']
                }
                read_writer.writerow(row)
                read_csvfile.flush()
                print(f"  READ → ET={parsed['et']}, BT={parsed['bt']}, valid={parsed['valid']}")

                if args.reference_temp is not None:
                    delta_et = abs(parsed['et'] - args.reference_temp)
                    delta_bt = abs(parsed['bt'] - args.reference_temp)
                    print(f"    Reference delta: ET={delta_et:.2f}, BT={delta_bt:.2f}")

            elif cmd.startswith('OT1') or cmd.startswith('IO3'):
                print(f"  Sending: {cmd}")
                line = send_and_read(ser, cmd)
                if line:
                    print(f"  Response: {line}")
                time.sleep(0.5)

            elif cmd == 'STATUS':
                print(f"  Sending: STATUS")
                line = send_and_read(ser, 'STATUS')
                if not line:
                    print("  No response for STATUS")
                    continue
                if line.startswith('#') or line.startswith('ERR'):
                    print(f"  Firmware message: {line}")
                    continue

                parts = line.split(',')
                if len(parts) == 18:
                    row = {
                        'timestamp': datetime.utcnow().isoformat(),
                        'et': parts[0],
                        'bt': parts[1],
                        'heater': parts[2],
                        'fan': parts[3],
                        'watchdog_flag': parts[4],
                        'failure_count': parts[5],
                        'failure_reason': parts[6],
                        'guard_timeouts': parts[7],
                        'regression_flag': parts[8],
                        'pv': parts[9],
                        'mv': parts[10],
                        'integrator_value': parts[11],
                        'derivative_value': parts[12],
                        'saturation_flag': parts[13],
                        'integrator_clamp_flag': parts[14],
                        'derivative_available_flag': parts[15],
                        'command_latency_us': parts[16],
                        'max_command_latency_us': parts[17]
                    }
                    status_writer.writerow(row)
                    status_csvfile.flush()

                    try:
                        latency = float(row['max_command_latency_us'])
                        if latency > max_latency:
                            max_latency = latency
                    except ValueError:
                        pass

                    print(f"  STATUS → ET={parts[0]}, BT={parts[1]}, Heater={parts[2]}, Fan={parts[3]}")
                else:
                    print(f"  Unexpected STATUS format (length {len(parts)}): {line}")

                time.sleep(args.interval)

            elif cmd == '[manual_disconnect_tc]':
                if args.pause_for_manual:
                    print("\n" + "=" * 60)
                    print("  MANUAL INTERVENTION REQUIRED")
                    print("  Please disconnect the thermocouple now.")
                    print("=" * 60)
                    input("  Press Enter to continue...")
                else:
                    print("  [manual_disconnect_tc] skipped (use --pause-for-manual to enable)")

            else:
                print(f"  Unknown command in sequence: {cmd}")

    if max_latency_ref:
        max_latency_ref[0] = max_latency


def write_metadata(path, metadata):
    try:
        with open(path, 'w') as f:
            json.dump(metadata, f, indent=2)
        print(f"Metadata written: {path}")
    except Exception as exc:
        print(f"Failed to write metadata {path}: {exc}")


def main():
    parser = argparse.ArgumentParser(description='LibreRoaster Validation Runner')
    parser.add_argument('--port', type=str, required=True, help='Serial port (e.g. /dev/ttyACM0 or COM3)')
    parser.add_argument('--baud', type=int, default=115200, help='Baud rate (default: 115200)')
    parser.add_argument('--interval', type=float, default=1.0, help='Polling interval in seconds (default: 1.0)')
    parser.add_argument('--manifest', type=str, default='tests/hardware/scenario_manifest.json', help='Manifest describing HIL scenarios')
    parser.add_argument('--scenario', type=str, default='all', help='Scenario ID from manifest or "all" for unfiltered capture')
    parser.add_argument('--runs-dir', type=str, default='tests/hardware/runs', help='Directory to store run artifacts')
    parser.add_argument('--metadata-suffix', type=str, default='.json', help='Suffix appended to telemetry CSV when emitting metadata (default: .json)')
    parser.add_argument('--hardware-mode', action='store_true', help='Enable hardware scenario mode: execute command sequences from manifest instead of STATUS polling')
    parser.add_argument('--reference-temp', type=float, default=None, help='Known reference temperature for thermocouple validation (optional)')
    parser.add_argument('--pause-for-manual', action='store_true', help='Pause for manual intervention (e.g. TC-03 thermocouple disconnect)')

    args = parser.parse_args()

    manifest = load_manifest(args.manifest)
    manifest_map = {entry['id']: entry for entry in manifest if 'id' in entry}

    scenario_id = args.scenario
    scenario_entry = None
    if scenario_id.lower() != 'all':
        scenario_id = scenario_id.upper()
        if scenario_id not in manifest_map:
            parser.error(f"Scenario '{args.scenario}' not found in manifest {args.manifest}")
        scenario_entry = manifest_map[scenario_id]

    run_timestamp = datetime.utcnow().strftime('%Y%m%dT%H%M%SZ')
    run_dir = os.path.join(args.runs_dir, scenario_id, run_timestamp)
    os.makedirs(run_dir, exist_ok=True)
    csv_path = os.path.join(run_dir, 'telemetry.csv')
    metadata_suffix = normalize_suffix(args.metadata_suffix)
    metadata_path = f"{csv_path}{metadata_suffix}"
    run_id = f"{scenario_id}-{run_timestamp}"

    print(f"Opening port {args.port} at {args.baud} baud...")
    try:
        ser = serial.Serial(args.port, args.baud, timeout=2)
    except Exception as e:
        print(f"Error opening serial port: {e}")
        return

    print(f"Logging to {csv_path}. Press Ctrl+C to stop.")

    file_exists = os.path.isfile(csv_path)
    max_latency = 0.0
    metadata_entry = scenario_entry.copy() if scenario_entry else None
    start_time = datetime.utcnow().isoformat()

    # Hardware mode: execute command sequence from manifest, then exit
    if args.hardware_mode and scenario_entry and is_hardware_scenario(scenario_id):
        print(f"Hardware mode: executing command sequence for {scenario_id}")
        max_latency_box = [0.0]
        try:
            execute_hardware_sequence(ser, scenario_entry, csv_path, args, max_latency_box)
        except KeyboardInterrupt:
            print("\nHardware sequence interrupted.")
        finally:
            ser.close()
            metadata = {
                'run_id': run_id,
                'scenario_id': scenario_id,
                'manifest_entry': metadata_entry,
                'max_command_latency_us': max_latency_box[0],
                'run_directory': run_dir,
                'started_at': start_time,
                'manifest_path': args.manifest,
                'hardware_mode': True,
                'reference_temp': args.reference_temp
            }
            write_metadata(metadata_path, metadata)
            print(f"Hardware sequence complete. Max latency: {max_latency_box[0]}us")
        return

    with open(csv_path, 'a', newline='') as csvfile:
        fieldnames = build_fieldnames()
        writer = csv.DictWriter(csvfile, fieldnames=fieldnames)

        if not file_exists or os.path.getsize(csv_path) == 0:
            writer.writeheader()

        try:
            while True:
                ser.write(b"STATUS\n")
                line = ser.readline().decode('utf-8', errors='replace').strip()

                if line:
                    if line.startswith("#") or line.startswith("ERR"):
                        print(f"Firmware message: {line}")
                        time.sleep(args.interval)
                        continue

                    parts = line.split(',')
                    if len(parts) == 18:
                        row = {
                            'timestamp': datetime.utcnow().isoformat(),
                            'et': parts[0],
                            'bt': parts[1],
                            'heater': parts[2],
                            'fan': parts[3],
                            'watchdog_flag': parts[4],
                            'failure_count': parts[5],
                            'failure_reason': parts[6],
                            'guard_timeouts': parts[7],
                            'regression_flag': parts[8],
                            'pv': parts[9],
                            'mv': parts[10],
                            'integrator_value': parts[11],
                            'derivative_value': parts[12],
                            'saturation_flag': parts[13],
                            'integrator_clamp_flag': parts[14],
                            'derivative_available_flag': parts[15],
                            'command_latency_us': parts[16],
                            'max_command_latency_us': parts[17]
                        }
                        writer.writerow(row)
                        csvfile.flush()

                        try:
                            latency = float(row['max_command_latency_us'])
                            if latency > max_latency:
                                max_latency = latency
                        except ValueError:
                            pass

                        print(f"[{row['timestamp']}] Latency: {row['command_latency_us']}us, Max: {row['max_command_latency_us']}us")
                    else:
                        print(f"Unexpected response format (length {len(parts)}): {line}")

                time.sleep(args.interval)
        except KeyboardInterrupt:
            print("\nStopping capture...")
        finally:
            ser.close()
            metadata = {
                'run_id': run_id,
                'scenario_id': scenario_id,
                'manifest_entry': metadata_entry,
                'max_command_latency_us': max_latency,
                'run_directory': run_dir,
                'started_at': start_time,
                'manifest_path': args.manifest
            }
            write_metadata(metadata_path, metadata)

if __name__ == "__main__":
    main()
