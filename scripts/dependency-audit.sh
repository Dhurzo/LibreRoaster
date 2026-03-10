#!/usr/bin/env bash
set -euo pipefail

ALLOWLIST_FILE=${ALLOWLIST_FILE:-.planning/quality/dependency-allowlist.toml}
LOG_DIR=quality/dead-code/dependency
TIMESTAMP=$(date -u +"%Y%m%dT%H%M%SZ")
MACHETE_LOG="$LOG_DIR/audit-${TIMESTAMP}-machete.log"
UDEPS_LOG="$LOG_DIR/audit-${TIMESTAMP}-udeps.log"
RAW_UDEPS="$LOG_DIR/audit-${TIMESTAMP}-udeps.raw"

mkdir -p "$LOG_DIR"

echo "Running cargo machete (--with-metadata --skip-target-dir)..."
MACHETE_STATUS=0
if cargo machete --with-metadata --skip-target-dir | tee "$MACHETE_LOG"; then
  echo "cargo machete completed (exit code 0)."
else
  MACHETE_STATUS=$?
  if [[ $MACHETE_STATUS -eq 2 ]]; then
    echo "cargo machete errored (exit code 2); see $MACHETE_LOG" >&2
    exit 1
  fi
  echo "cargo machete detected unused dependencies; see $MACHETE_LOG" >&2
fi

echo "Cleaning cargo artifacts before nightly udeps..."
cargo clean

echo "Running cargo +nightly udeps (quiet)..."
UDEPS_STATUS=0
if cargo +nightly udeps --quiet > "$RAW_UDEPS"; then
  echo "cargo +nightly udeps completed (exit code 0)."
else
  UDEPS_STATUS=$?
  if [[ $UDEPS_STATUS -ge 2 ]]; then
    echo "cargo +nightly udeps errored (exit code $UDEPS_STATUS); see $RAW_UDEPS" >&2
    exit 1
  fi
  echo "cargo +nightly udeps reported unused dependencies; see $RAW_UDEPS" >&2
fi

export ALLOWLIST_FILE
export RAW_UDEPS
export UDEPS_LOG
export TIMESTAMP

mapfile -t new_packages < <(python3 - <<'PY'
import os
import pathlib
import re

try:
    import tomllib as toml
except ModuleNotFoundError:
    try:
        import tomli as toml
    except ModuleNotFoundError:
        toml = None

def simple_toml_parse(text):
    entries = []
    current = {}
    for raw_line in text.splitlines():
        line = raw_line.strip()
        if not line or line.startswith('#'):
            continue
        if line.startswith('[allow'):
            if current:
                entries.append(current)
            current = {}
            continue
        if '=' in line:
            key, value = line.split('=', 1)
            key = key.strip()
            value = value.strip()
            if value.startswith('"') and value.endswith('"'):
                value = value[1:-1]
            current[key] = value.strip()
    if current:
        entries.append(current)
    return entries

allowlist_path = pathlib.Path(os.environ['ALLOWLIST_FILE'])
raw_path = pathlib.Path(os.environ['RAW_UDEPS'])
log_path = pathlib.Path(os.environ['UDEPS_LOG'])
timestamp = os.environ.get('TIMESTAMP', '?')

allowlist_entries = []
if allowlist_path.exists():
    raw_text = allowlist_path.read_text()
    if toml is not None:
        try:
            data = toml.loads(raw_text)
        except Exception:
            data = {}
        entries = data.get('allow')
        if isinstance(entries, dict):
            allowlist_entries = [entries]
        elif isinstance(entries, list):
            allowlist_entries = entries
        else:
            allowlist_entries = []
    else:
        allowlist_entries = simple_toml_parse(raw_text)

allowmap = {}
for entry in allowlist_entries:
    pkg = entry.get('package')
    if not pkg:
        continue
    allowmap[pkg] = {
        'reason': entry.get('reason', '').strip(),
        'expires': entry.get('expires', '').strip(),
    }

raw_lines = raw_path.read_text().splitlines()
pattern = re.compile(r'unused dependency:\s+([A-Za-z0-9_\-.]+)', re.IGNORECASE)
unused = []
for line in raw_lines:
    match = pattern.search(line)
    if match:
        unused.append(match.group(1))

new_pkgs = [pkg for pkg in unused if pkg not in allowmap]

log_lines = []
log_lines.append(f"# cargo +nightly udeps audit ({timestamp})")
log_lines.append(f"# Allowlist source: {allowlist_path}")
log_lines.append('')
log_lines.append('## Unused dependency review')
if unused:
    for pkg in unused:
        if pkg in allowmap:
            info = allowmap[pkg]
            reason = info['reason'] or 'reason not provided'
            expires = info['expires'] or 'no expiry'
            log_lines.append(f"- {pkg} (allowlisted: {reason}; expires {expires})")
        else:
            log_lines.append(f"- {pkg} (flagged for review)")
else:
    log_lines.append('- (none detected)')

log_lines.append('')
log_lines.append('## Allowlist reference')
if allowmap:
    for pkg in sorted(allowmap):
        info = allowmap[pkg]
        reason = info['reason'] or 'reason not provided'
        expires = info['expires'] or 'no expiry'
        log_lines.append(f"- {pkg}: {reason} (expires {expires})")
else:
    log_lines.append('- (none defined)')

log_lines.append('')
log_lines.append('## Raw cargo +nightly udeps output (quiet)')
log_lines.extend(raw_lines)
log_path.write_text('\n'.join(log_lines) + '\n')

if new_pkgs:
    print('\n'.join(new_pkgs))
PY
)

rm -f "$RAW_UDEPS"

echo "Machete results: $MACHETE_LOG"
echo "Udeps audit: $UDEPS_LOG"

if [[ ${#new_packages[@]} -gt 0 ]]; then
  echo "Unused dependencies outside allowlist detected:" >&2
  for pkg in "${new_packages[@]}"; do
    echo "  - $pkg" >&2
  done
  exit 1
fi

echo "Dependency audit completed with all unused crates accounted for (allowlist verified)."
