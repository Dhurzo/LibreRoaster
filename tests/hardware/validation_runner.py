import serial
import time
import csv
import argparse
import os
from datetime import datetime

def main():
    parser = argparse.ArgumentParser(description='LibreRoaster Validation Runner')
    parser.add_argument('--port', type=str, required=True, help='Serial port (e.g. /dev/ttyACM0 or COM3)')
    parser.add_argument('--baud', type=int, default=115200, help='Baud rate (default: 115200)')
    parser.add_argument('--interval', type=float, default=1.0, help='Polling interval in seconds (default: 1.0)')
    parser.add_argument('--output', type=str, default='validation_run.csv', help='Output CSV file')
    
    args = parser.parse_args()
    
    print(f"Opening port {args.port} at {args.baud} baud...")
    try:
        ser = serial.Serial(args.port, args.baud, timeout=2)
    except Exception as e:
        print(f"Error opening serial port: {e}")
        return

    print(f"Logging to {args.output}. Press Ctrl+C to stop.")
    
    file_exists = os.path.isfile(args.output)
    
    with open(args.output, 'a', newline='') as csvfile:
        fieldnames = [
            'timestamp', 'et', 'bt', 'heater', 'fan', 
            'watchdog_flag', 'failure_count', 'failure_reason', 
            'guard_timeouts', 'regression_flag', 'pv', 'mv', 
            'integrator_value', 'derivative_value', 'saturation_flag', 
            'integrator_clamp_flag', 'derivative_available_flag', 
            'command_latency_us', 'max_command_latency_us'
        ]
        writer = csv.DictWriter(csvfile, fieldnames=fieldnames)
        
        if not file_exists or os.path.getsize(args.output) == 0:
            writer.writeheader()
            
        try:
            while True:
                # Send STATUS command
                ser.write(b"STATUS\n")
                line = ser.readline().decode('utf-8', errors='replace').strip()
                
                if line:
                    # Filter out non-CSV lines if any (e.g. boot logs)
                    if line.startswith("#") or line.startswith("ERR"):
                        print(f"Firmware message: {line}")
                        continue
                        
                    parts = line.split(',')
                    if len(parts) == 18:
                        row = {
                            'timestamp': datetime.now().isoformat(),
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
                        print(f"[{row['timestamp']}] Latency: {row['command_latency_us']}us, Max: {row['max_command_latency_us']}us")
                    else:
                        print(f"Unexpected response format (length {len(parts)}): {line}")
                
                time.sleep(args.interval)
        except KeyboardInterrupt:
            print("\nStopping capture...")
        finally:
            ser.close()

if __name__ == "__main__":
    main()
