#!/bin/bash
set -euxo pipefail

echo "Running cargo fmt -- --check..."
cargo fmt -- --check

echo "Running cargo clippy --workspace --all-features -- -D warnings..."
cargo clippy --workspace --all-features -- -D warnings

echo "Running cargo test --workspace --all-features..."
cargo test --workspace --all-features

echo "All quality checks passed."
