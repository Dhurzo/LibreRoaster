import json
import csv
import argparse
import sys
import os
from datetime import datetime

def load_thresholds(path):
    with open(path, 'r') as f:
        return json.load(f)

def analyze_csv(csv_path, thresholds):
    results = {
        'max_command_latency_ms': {'value': 0.0, 'pass': True, 'threshold': thresholds['max_command_latency_ms']},
        'avg_command_latency_ms': {'value': 0.0, 'pass': True, 'threshold': thresholds['avg_command_latency_ms']},
        'max_watchdog_consecutive_fails': {'value': 0, 'pass': True, 'threshold': thresholds['max_watchdog_consecutive_fails']},
        'max_ledc_guard_timeouts': {'value': 0, 'pass': True, 'threshold': thresholds['max_ledc_guard_timeouts']}
    }
    
    latencies = []
    watchdog_fails = []
    guard_timeouts = 0
    
    try:
        with open(csv_path, 'r') as csvfile:
            reader = csv.DictReader(csvfile)
            for row in reader:
                # Convert us to ms
                lat = float(row['max_command_latency_us']) / 1000.0
                latencies.append(lat)
                
                wd_fails = int(row['failure_count'])
                watchdog_fails.append(wd_fails)
                
                guard_timeouts = int(row['guard_timeouts'])
                
    except Exception as e:
        print(f"Error reading CSV {csv_path}: {e}")
        return None

    if not latencies:
        print("No data found in CSV.")
        return None

    # Latency checks
    max_lat = max(latencies)
    avg_lat = sum(latencies) / len(latencies)
    
    results['max_command_latency_ms']['value'] = max_lat
    results['max_command_latency_ms']['pass'] = max_lat <= thresholds['max_command_latency_ms']
    
    results['avg_command_latency_ms']['value'] = avg_lat
    results['avg_command_latency_ms']['pass'] = avg_lat <= thresholds['avg_command_latency_ms']
    
    # Watchdog checks
    max_wd = max(watchdog_fails)
    results['max_watchdog_consecutive_fails']['value'] = max_wd
    results['max_watchdog_consecutive_fails']['pass'] = max_wd <= thresholds['max_watchdog_consecutive_fails']
    
    # Guard timeout checks (last recorded value is the cumulative count)
    results['max_ledc_guard_timeouts']['value'] = guard_timeouts
    results['max_ledc_guard_timeouts']['pass'] = guard_timeouts <= thresholds['max_ledc_guard_timeouts']
    
    return results

def generate_report(results, csv_path, template_path, output_path):
    if not os.path.exists(template_path):
        print(f"Template not found: {template_path}")
        return
        
    with open(template_path, 'r') as f:
        template = f.read()
        
    overall_pass = all(v['pass'] for v in results.values())
    sign_off = "PASSED" if overall_pass else "FAILED"
    
    # Build summary table
    table_rows = []
    for metric, data in results.items():
        status = "✅ PASS" if data['pass'] else "❌ FAIL"
        table_rows.append(f"| {metric} | {data['value']:.2f} | {data['threshold']} | {status} |")
    
    summary_table = "\n".join(table_rows)
    
    report = template.replace("{{DATE}}", datetime.now().strftime("%Y-%m-%d %H:%M:%S"))
    report = report.replace("{{RUN_ID}}", os.path.basename(csv_path))
    report = report.replace("{{SIGN_OFF}}", sign_off)
    report = report.replace("{{SUMMARY_TABLE}}", summary_table)
    
    os.makedirs(os.path.dirname(output_path), exist_ok=True)
    with open(output_path, 'w') as f:
        f.write(report)
    print(f"Report generated: {output_path}")

def main():
    parser = argparse.ArgumentParser(description='LibreRoaster Threshold Analysis')
    parser.add_argument('--csv', type=str, required=True, help='Path to the validation run CSV')
    parser.add_argument('--thresholds', type=str, default='tests/hardware/thresholds.json', help='Path to thresholds.json')
    parser.add_argument('--template', type=str, default='tests/hardware/report_template.md', help='Path to report template')
    parser.add_argument('--output', type=str, help='Path to save the generated report')
    
    args = parser.parse_args()
    
    if not os.path.exists(args.thresholds):
        print(f"Thresholds file not found: {args.thresholds}")
        sys.exit(1)
        
    thresholds = load_thresholds(args.thresholds)
    results = analyze_csv(args.csv, thresholds)
    
    if results:
        overall_pass = all(v['pass'] for v in results.values())
        print(f"SIGN-OFF: {'PASSED' if overall_pass else 'FAILED'}")
        for metric, data in results.items():
            status = 'PASS' if data['pass'] else 'FAIL'
            print(f"{metric}: {data['value']:.2f} (Threshold: {data['threshold']}) [{status}]")
            
        if args.output:
            generate_report(results, args.csv, args.template, args.output)
    else:
        sys.exit(1)

if __name__ == "__main__":
    main()
