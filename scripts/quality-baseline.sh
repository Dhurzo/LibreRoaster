#!/bin/bash
set -euxo pipefail

echo "Running cargo fmt -- --check..."
cargo fmt -- --check

# Production code: enforce deny-level policy (matches [lints.clippy] in Cargo.toml)
# Test code is exempt via clippy.toml allow-*-in-tests = true
echo "Running cargo clippy --locked --lib --bins -- -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic..."
cargo clippy --locked --lib --bins -- -D clippy::unwrap_used -D clippy::expect_used -D clippy::panic

echo "Running cargo test --locked --lib --tests --no-fail-fast..."
cargo test --locked --lib --tests --no-fail-fast

echo "All quality checks passed."
