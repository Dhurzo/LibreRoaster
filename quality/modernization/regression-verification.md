# Regression Verification Guide

## Command flows to rerun
Run the following representative tests after modernization to cover every major module:
1. `cargo test --test command_idempotence`
2. `cargo test --test command_multiplexer_concurrency`
3. `cargo test --test artisan_integration_test --features regression`
4. `cargo test --test mock_uart_integration`

Each test targets a distinct command or path through the command multiplexer so behavior-equivalence remains tractable.

## Acceptable drift
Telemetry outputs may differ slightly (timestamps, execution order) but must remain within documented thresholds; semantic invariants such as command ordering, response markers (READ/START/STOP), and packet structure must stay identical.

## Detection approach
1. Run the automated suite above (`cargo test` commands) via `scripts/run-regression-checks.sh`.
2. For hardware-facing scenarios touched by modernization, perform manual spot-checks (e.g., replaying critical UART sequences) and note the steps in the regression summary.
3. Log artifacts live under `logs/regression/<run_id>/` so reviewers can tie drift to specific runs.
