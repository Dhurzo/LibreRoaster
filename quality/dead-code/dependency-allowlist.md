# Dependency Allowlist

This guide keeps the DC-03 dependency-audit workflow repeatable. The allowlist lives in `.planning/quality/dependency-allowlist.toml` and the audit runner in `scripts/dependency-audit.sh` makes every `machete`/`udeps` pass reference it.

## Updating the Toml allowlist

1. Add a new `[[allow]]` table in `.planning/quality/dependency-allowlist.toml` for each dependency that `cargo machete` or `cargo +nightly udeps` flags but should stay.
2. Provide `package`, `reason`, and `expires` so reviewers know why the dependency is intentional and when it should be reconsidered.
3. Save the file and re-run `ALLOWLIST_FILE=.planning/quality/dependency-allowlist.toml scripts/dependency-audit.sh` before committing the change.

## Interpreting audit logs

After the runner finishes, inspect `quality/dead-code/dependency/audit-<timestamp>-machete.log` for the machete summary and `quality/dead-code/dependency/audit-<timestamp>-udeps.log` for the nightly comparison:

- The `udeps` log starts with an **Unused dependency review** section that lists every crate `udeps` detected and annotates allowlisted entries (reason/expires).
- The **Allowlist reference** section repeats the allowlist contents so auditors can cross-check the justification used in this run.
- The **Raw cargo +nightly udeps output** section preserves the original command output for reproducibility.

The script exits non-zero if it sees unused crates outside the allowlist, so failed runs highlight new candidates that need either removal or a new allowlist entry.

## Reviewer sign-off process

1. Confirm the log timestamps match the current run and that both logs exist in `quality/dead-code/dependency/`.
2. For each allowlisted dependency in the log, verify the `reason` explains why the crate cannot be dropped and that the `expires` timestamp keeps reviewers honest about revisiting the decision.
3. When new unused dependencies appear, request either removal or a documented exception before approving the batch.
4. Document your sign-off in the DC-03 review artifact; link to the `audit-<timestamp>-udeps.log` so future reviewers know which allowlist state produced the justification.

Keeping this file up to date ensures every `machete`/`udeps` audit run traces to a documented exception and that reviewers can quickly confirm the rationale for each intentional dependency.
