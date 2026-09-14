#!/bin/bash
set -euxo pipefail

echo "Running cargo fmt --all -- --check..."
cargo fmt --all -- --check

# Production code: enforce deny-level policy (matches [lints.clippy] in Cargo.toml)
# Test code is exempt via clippy.toml allow-*-in-tests = true
echo "Running cargo clippy --locked --all-targets -- -W clippy::unwrap_used -W clippy::expect_used -W clippy::panic..."
cargo clippy --locked --all-targets -- -W clippy::unwrap_used -W clippy::expect_used -W clippy::panic

# Host tests need the `test` feature (and the host target) so the Embassy time
# driver is provided; without it the link fails with `undefined symbol:
# _embassy_time_now`.
echo "Running cargo test --locked --target x86_64-unknown-linux-gnu --features test --lib --tests --no-fail-fast..."
cargo test --locked --target x86_64-unknown-linux-gnu --features test --lib --tests --no-fail-fast

echo "All quality checks passed."
