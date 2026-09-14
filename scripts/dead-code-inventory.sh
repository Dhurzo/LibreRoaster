#!/usr/bin/env bash
set -euo pipefail

inventory_dir="quality/dead-code/inventory"
timestamp=$(date -u +"%Y%m%dT%H%M%SZ")
inventory_file="$inventory_dir/${timestamp}-dead-code.json"
latest_file="$inventory_dir/dead-code-inventory.json"

mkdir -p "$inventory_dir"

git_rev=$(git rev-parse HEAD)
git_branch=$(git symbolic-ref --short HEAD 2>/dev/null || git branch --show-current)

toolchain=$(rustup show active-toolchain 2>/dev/null | awk 'NR==1 {print $1}')
if [[ -z "$toolchain" ]]; then
  toolchain=$(rustc --version)
fi

captured_at=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

allow_lints="-A clippy::expect_used"
if [[ -n "${RUSTFLAGS:-}" ]]; then
  export RUSTFLAGS="${RUSTFLAGS} ${allow_lints}"
else
  export RUSTFLAGS="${allow_lints}"
fi

clippy_output=$(mktemp)
trap 'rm -f "$clippy_output"' EXIT

cargo clippy --locked --lib --tests --benches --examples --all-features --message-format=json > "$clippy_output"

export CLIPPY_OUTPUT="$clippy_output"
export INVENTORY_FILE="$inventory_file"
export LATEST_FILE="$latest_file"
export GIT_REV="$git_rev"
export GIT_BRANCH="$git_branch"
export TOOLCHAIN="$toolchain"
export CAPTURED_AT="$captured_at"
export TIMESTAMP="$timestamp"

python3 <<'PY'
import json
import os
from pathlib import Path

clippy_path = Path(os.environ["CLIPPY_OUTPUT"])
inventory_path = Path(os.environ["INVENTORY_FILE"])
latest_path = Path(os.environ["LATEST_FILE"])

entries = []
with clippy_path.open() as fh:
    for line in fh:
        try:
            record = json.loads(line)
        except json.JSONDecodeError:
            continue
        message = record.get("message")
        if not message:
            continue
        code_obj = message.get("code")
        code_str = code_obj.get("code") if code_obj else ""
        if "dead_code" not in code_str:
            continue
        spans = message.get("spans") or []
        span = spans[0] if spans else {}
        entries.append({
            "code": code_str,
            "level": message.get("level"),
            "message": message.get("message"),
            "rendered": message.get("rendered"),
            "target": record.get("target", {}).get("name"),
            "package": record.get("package_id"),
            "span": {
                "file": span.get("file_name", ""),
                "line": span.get("line_start"),
                "column": span.get("column_start"),
                "label": span.get("label", ""),
            },
        })

inventory = {
    "git_rev": os.environ["GIT_REV"],
    "git_branch": os.environ["GIT_BRANCH"],
    "toolchain": os.environ["TOOLCHAIN"],
    "captured_at": os.environ["CAPTURED_AT"],
    "inventory_timestamp": os.environ["TIMESTAMP"],
    "entries": entries,
}

with inventory_path.open("w") as outf:
    json.dump(inventory, outf, indent=2)

print(f"Dead code inventory generated: {inventory_path}")
print(f"Latest pointer: {latest_path}")
if not entries:
    print("No dead_code lint candidates were captured. Run this script again after new candidates appear.")
else:
    print("Captured dead code candidates (module/item/line/risk hint):")
    for entry in entries:
        span = entry["span"]
        file = span.get("file") or "<unknown>"
        line = span.get("line") or 0
        column = span.get("column") or 0
        label = span.get("label") or entry.get("message") or "<unnamed>"
        code = entry.get("code", "")
        if "dead_code_in_same_module" in code:
            risk = "High risk (unreachable helper inside module)."
        elif "dead_code" in code:
            risk = "Medium risk (unused declaration)."
        else:
            risk = "Low risk (dead code variant)."
        print(f"  • {file}:{line}:{column} – {label} – {risk}")
PY

cp "$inventory_file" "$latest_file"
