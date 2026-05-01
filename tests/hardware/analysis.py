import json
import csv
import argparse
import sys
import os
from datetime import datetime

def load_thresholds(path):
    with open(path, 'r') as f:
        return json.load(f)


def load_manifest(path):
    with open(path, 'r') as f:
        return json.load(f)


def normalize_suffix(suffix):
    if not suffix:
        return '.json'
    return suffix if suffix.startswith('.') else f'.{suffix}'


def load_metadata(path):
    if not os.path.exists(path):
        return {}
    with open(path, 'r') as f:
        return json.load(f)


def discover_runs(runs_dir, metadata_suffix, scenario_filter=None):
    scenario_filter = None if not scenario_filter else scenario_filter.upper()
    if not os.path.isdir(runs_dir):
        return
    for scenario in sorted(os.listdir(runs_dir)):
        scenario_path = os.path.join(runs_dir, scenario)
        if not os.path.isdir(scenario_path):
            continue
        if scenario_filter and scenario.upper() != scenario_filter:
            continue
        for timestamp in sorted(os.listdir(scenario_path)):
            run_path = os.path.join(scenario_path, timestamp)
            if not os.path.isdir(run_path):
                continue
            csv_path = os.path.join(run_path, 'telemetry.csv')
            metadata_path = f"{csv_path}{metadata_suffix}"
            if os.path.isfile(csv_path):
                yield scenario.upper(), timestamp, csv_path, metadata_path

def analyze_csv(csv_path, thresholds, scenario_entry=None):
    results = {
        'max_command_latency_ms': {'value': 0.0, 'pass': True, 'threshold': thresholds['max_command_latency_ms']},
        'avg_command_latency_ms': {'value': 0.0, 'pass': True, 'threshold': thresholds['avg_command_latency_ms']},
        'max_watchdog_consecutive_fails': {'value': 0, 'pass': True, 'threshold': thresholds['max_watchdog_consecutive_fails']},
        'max_ledc_guard_timeouts': {'value': 0, 'pass': True, 'threshold': thresholds['max_ledc_guard_timeouts']}
    }
    
    latencies = []
    watchdog_fails = []
    guard_timeouts = 0
    et_values = []
    bt_values = []
    heater_values = []
    fan_values = []
    has_read_columns = False
    has_status_columns = False
    gpio_status_values = []
    
    try:
        with open(csv_path, 'r') as csvfile:
            reader = csv.DictReader(csvfile)
            fieldnames = reader.fieldnames or []
            has_read_columns = 'et' in fieldnames and 'bt' in fieldnames and 'valid' in fieldnames
            has_status_columns = 'max_command_latency_us' in fieldnames

            for row in reader:
                if has_status_columns:
                    lat = float(row['max_command_latency_us']) / 1000.0
                    latencies.append(lat)
                    
                    wd_fails = int(row['failure_count'])
                    watchdog_fails.append(wd_fails)
                    
                    guard_timeouts = int(row['guard_timeouts'])

                    try:
                        et_val = float(row.get('et', 0))
                        bt_val = float(row.get('bt', 0))
                        et_values.append(et_val)
                        bt_values.append(bt_val)
                    except (ValueError, TypeError):
                        pass

                    try:
                        heater_val = float(row.get('heater', 0))
                        heater_values.append(heater_val)
                    except (ValueError, TypeError):
                        pass

                    try:
                        fan_val = float(row.get('fan', 0))
                        fan_values.append(fan_val)
                    except (ValueError, TypeError):
                        pass

                elif has_read_columns:
                    try:
                        et_val = float(row['et'])
                        bt_val = float(row['bt'])
                        et_values.append(et_val)
                        bt_values.append(bt_val)
                    except (ValueError, TypeError):
                        pass

    except Exception as e:
        print(f"Error reading CSV {csv_path}: {e}")
        return None

    if not latencies and not et_values:
        print("No data found in CSV.")
        return None

    if latencies:
        max_lat = max(latencies)
        avg_lat = sum(latencies) / len(latencies)
        
        results['max_command_latency_ms']['value'] = max_lat
        results['max_command_latency_ms']['pass'] = max_lat <= thresholds['max_command_latency_ms']
        
        results['avg_command_latency_ms']['value'] = avg_lat
        results['avg_command_latency_ms']['pass'] = avg_lat <= thresholds['avg_command_latency_ms']
        
        max_wd = max(watchdog_fails) if watchdog_fails else 0
        results['max_watchdog_consecutive_fails']['value'] = max_wd
        results['max_watchdog_consecutive_fails']['pass'] = max_wd <= thresholds['max_watchdog_consecutive_fails']
        
        results['max_ledc_guard_timeouts']['value'] = guard_timeouts
        results['max_ledc_guard_timeouts']['pass'] = guard_timeouts <= thresholds['max_ledc_guard_timeouts']

    scenario_id = ''
    scenario_metadata = {}
    if scenario_entry:
        scenario_id = scenario_entry.get('id', '').upper()
        scenario_metadata = scenario_entry.get('metadata', {}) or {}

    is_tc = scenario_id.startswith('TC-')
    is_ssr = scenario_id.startswith('SSR-')
    is_fan = scenario_id.startswith('FAN-')
    is_gpio = scenario_id.startswith('GPIO-')

    has_temps = len(et_values) > 0 and len(bt_values) > 0

    if is_tc and has_temps:
        ambient_min = scenario_metadata.get('ambient_temp_min', thresholds.get('ambient_temp_min', 0.0))
        ambient_max = scenario_metadata.get('ambient_temp_max', thresholds.get('ambient_temp_max', 50.0))
        all_in_range = all(ambient_min <= v <= ambient_max for v in et_values) and \
                       all(ambient_min <= v <= ambient_max for v in bt_values)
        results['ambient_temp_check'] = {
            'value': 1.0 if all_in_range else 0.0,
            'pass': all_in_range,
            'threshold': f'{ambient_min}-{ambient_max}'
        }

    if is_tc and scenario_id == 'TC-02' and has_temps:
        max_delta = scenario_metadata.get('dual_channel_max_delta', thresholds.get('dual_channel_max_delta', 15.0))
        deltas = [abs(e - b) for e, b in zip(et_values, bt_values)]
        max_actual_delta = max(deltas) if deltas else 0.0
        results['dual_channel_delta'] = {
            'value': max_actual_delta,
            'pass': max_actual_delta < max_delta,
            'threshold': max_delta
        }

    if is_tc and scenario_id == 'TC-04' and has_temps:
        stability_delta = scenario_metadata.get('temp_stability_max_delta', thresholds.get('temp_stability_delta', 5.0))
        et_range = max(et_values) - min(et_values) if len(et_values) >= 2 else 0.0
        bt_range = max(bt_values) - min(bt_values) if len(bt_values) >= 2 else 0.0
        max_variance = max(et_range, bt_range)
        results['temp_stability'] = {
            'value': max_variance,
            'pass': max_variance < stability_delta,
            'threshold': stability_delta
        }

    if is_ssr and len(heater_values) > 0:
        expected_heater = scenario_metadata.get('expected_heater', 0.0)
        tolerance = thresholds.get('ssr_duty_tolerance_pct', 3.0)
        max_deviation = max(abs(h - expected_heater) for h in heater_values)
        results['ssr_duty_accuracy'] = {
            'value': max_deviation,
            'pass': max_deviation <= tolerance,
            'threshold': tolerance
        }

    if is_fan and len(fan_values) > 0:
        expected_fan = scenario_metadata.get('expected_fan')
        tolerance = scenario_metadata.get('fan_duty_tolerance_pct', thresholds.get('fan_duty_tolerance_pct', 5.0))
        if expected_fan is not None:
            max_deviation = max(abs(f - expected_fan) for f in fan_values)
            results['fan_duty_accuracy'] = {
                'value': max_deviation,
                'pass': max_deviation <= tolerance,
                'threshold': tolerance
            }

    if is_gpio and len(heater_values) > 0:
        all_consistent = len(set(heater_values)) == 1
        results['gpio_consistency'] = {
            'value': len(set(heater_values)),
            'pass': all_consistent,
            'threshold': 1
        }
    
    return results

def generate_report(results, csv_path, template_path, output_path, scenario_entry, metadata_entry, thresholds):
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

    manifest_entry = {}
    if scenario_entry:
        manifest_entry = scenario_entry
    elif metadata_entry.get('manifest_entry'):
        manifest_entry = metadata_entry['manifest_entry']

    scenario_id = (manifest_entry.get('id') or metadata_entry.get('scenario_id') or 'unknown').upper()
    category = manifest_entry.get('category', 'Unknown')
    description = manifest_entry.get('description', 'Not provided')
    command_sequence = manifest_entry.get('command_sequence', 'Not provided')
    golden_output = manifest_entry.get('golden_output') or metadata_entry.get('golden_output')
    scenario_metadata = manifest_entry.get('metadata', {}) or metadata_entry.get('metadata', {}) or {}
    retention_days = scenario_metadata.get('retention_days', 'unspecified')
    evidence_owner = scenario_metadata.get('evidence_owner', 'unspecified')

    golden_artifact_value = (
        f"[{golden_output}]({golden_output}) (retain {retention_days} days, owner: {evidence_owner})"
        if golden_output
        else f"Not provided (retain {retention_days} days, owner: {evidence_owner})"
    )

    scenario_rows = [
        ("ID", scenario_id),
        ("Category", category),
        ("Description", description),
        ("Command Sequence", command_sequence),
        ("Golden artifact", golden_artifact_value)
    ]
    scenario_table_lines = ["| Field | Value |", "|-------|-------|"]
    for field, value in scenario_rows:
        scenario_table_lines.append(f"| {field} | {value} |")
    scenario_metadata_table = "\n".join(scenario_table_lines)

    metric_labels = {
        'max_command_latency_ms': 'Max command latency (ms)',
        'avg_command_latency_ms': 'Average command latency (ms)',
        'max_watchdog_consecutive_fails': 'Max watchdog consecutive fails',
        'max_ledc_guard_timeouts': 'Max LEDC guard timeouts',
        'ambient_temp_check': 'Ambient temperature in range',
        'ssr_duty_accuracy': 'SSR duty cycle accuracy (max deviation %)',
        'fan_duty_accuracy': 'Fan duty cycle accuracy (max deviation %)',
        'temp_stability': 'Temperature stability (max variance)',
        'dual_channel_delta': 'Dual channel delta (ET-BT)',
        'gpio_consistency': 'GPIO consistency (unique values)'
    }
    threshold_lines = ["| Threshold | Result |", "|-----------|--------|"]
    for metric, data in results.items():
        label = metric_labels.get(metric, metric)
        value_str = f"{data['value']:.2f}"
        threshold_value = thresholds.get(metric, 'N/A')
        badge = "✅ PASS" if data['pass'] else "❌ FAIL"
        threshold_lines.append(f"| {label}: {value_str} / {threshold_value} | {badge} |")
    threshold_verdict_table = "\n".join(threshold_lines)

    report = report.replace("{{SCENARIO_METADATA_TABLE}}", scenario_metadata_table)
    report = report.replace("{{THRESHOLD_VERDICTS_TABLE}}", threshold_verdict_table)

    metadata_section = "\n## Run Metadata\n"
    metadata_section += f"- **Run ID:** {metadata_entry.get('run_id', os.path.basename(csv_path))}\n"
    metadata_section += f"- **Scenario ID:** {metadata_entry.get('scenario_id', scenario_id)}\n"
    metadata_section += f"- **Max Command Latency (us):** {metadata_entry.get('max_command_latency_us', 'N/A')}\n"
    metadata_section += f"- **Manifest Source:** {metadata_entry.get('manifest_path', 'N/A')}\n"

    report = report + metadata_section
    
    os.makedirs(os.path.dirname(output_path), exist_ok=True)
    with open(output_path, 'w') as f:
        f.write(report)
    print(f"Report generated: {output_path}")

def main():
    parser = argparse.ArgumentParser(description='LibreRoaster Threshold Analysis')
    parser.add_argument('--thresholds', type=str, default='tests/hardware/thresholds.json', help='Path to thresholds.json')
    parser.add_argument('--template', type=str, default='tests/hardware/report_template.md', help='Path to report template')
    parser.add_argument('--manifest', type=str, default='tests/hardware/scenario_manifest.json', help='Path to the scenario manifest')
    parser.add_argument('--runs-dir', type=str, default='tests/hardware/runs', help='Directory containing telemetry runs')
    parser.add_argument('--reports-dir', type=str, default='tests/hardware/reports', help='Directory where analysis reports are written')
    parser.add_argument('--metadata-suffix', type=str, default='.json', help='Suffix appended to telemetry CSV for metadata files')
    parser.add_argument('--scenario', type=str, default='all', help='Scenario identifier to analyze (default: all)')

    args = parser.parse_args()

    if not os.path.exists(args.thresholds):
        print(f"Thresholds file not found: {args.thresholds}")
        sys.exit(1)

    if not os.path.exists(args.manifest):
        print(f"Manifest file not found: {args.manifest}")
        sys.exit(1)

    thresholds = load_thresholds(args.thresholds)
    manifest_entries = load_manifest(args.manifest)
    manifest_map = {entry['id'].upper(): entry for entry in manifest_entries if 'id' in entry}
    metadata_suffix = normalize_suffix(args.metadata_suffix)

    scenario_filter = None
    if args.scenario and args.scenario.lower() != 'all':
        scenario_filter = args.scenario.upper()

    runs = list(discover_runs(args.runs_dir, metadata_suffix, scenario_filter))
    if not runs:
        print(f"No runs found under {args.runs_dir} matching scenario filter '{args.scenario}'.")
        sys.exit(0)

    processed = 0
    for scenario, timestamp, csv_path, metadata_path in runs:
        metadata_entry = load_metadata(metadata_path)
        scenario_entry = manifest_map.get(scenario)

        if not scenario_entry and metadata_entry.get('manifest_entry'):
            scenario_entry = metadata_entry['manifest_entry']

        results = analyze_csv(csv_path, thresholds, scenario_entry)
        if not results:
            continue

        report_path = os.path.join(args.reports_dir, scenario, f"{timestamp}.md")
        os.makedirs(os.path.dirname(report_path), exist_ok=True)
        generate_report(results, csv_path, args.template, report_path, scenario_entry, metadata_entry, thresholds)

        overall_pass = all(v['pass'] for v in results.values())
        print(f"Processed run {scenario}@{timestamp}: {'PASSED' if overall_pass else 'FAILED'}")
        processed += 1

    if processed == 0:
        sys.exit(1)

if __name__ == "__main__":
    main()
