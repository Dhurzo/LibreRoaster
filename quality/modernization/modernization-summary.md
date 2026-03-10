# Modernization Summary

This document aggregates the latest modernization runs so auditors can see which unsafe surfaces changed and where the logs live.

## Recent runs
| run_id | trigger | register delta | log link |
|--------|---------|----------------|----------|
| TBD | Initial automation setup | n/a | `logs/modernization/TBD` |

## How to read this summary
- Each row references `logs/modernization/<run_id>/` and points to the generated `summary.txt` produced by `scripts/run-modernization.sh`.
- `register delta` entries link (or describe) the sections in `quality/modernization/unsafe-register.md` that changed.
- Before milestone review, update this table with the current run IDs and a one-liner explanation of why the run occurred (e.g., "clippy fix for Artisian command handler").

## Periodic updates
We refresh this summary before each milestone sign-off. Use it to explain modernization activity during verification and point reviewers to the relevant unsafe-register entries.
