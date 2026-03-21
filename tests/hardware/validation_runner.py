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
