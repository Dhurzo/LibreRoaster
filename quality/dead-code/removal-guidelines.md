# Dead-code removal guidelines (DC-02)

These instructions capture the gated workflow for removing dead modules in small batches so every reviewer can trace the inventory entry -> evidence -> verification chain required by DC-02.

## Forming a removal batch

1. Start at the dead-code inventory described in [quality/dead-code/README.md](quality/dead-code/README.md). Each module listed there includes an ID, risk tier, and the evidence you used to label it as unused. Pick no more than one tier of risk per batch so reviewers can validate the claim without massive diffs.
2. Capture the inventory snapshot (ID + path + risk letter) in your task/issue description before touching code. For example, copy the entry straight from the inventory and paste it into the batch note so future reviewers can see exactly which candidate you removed.
3. Record the modules in a whitespace-separated list that matches what the inventory calls them. The `MODULES` list that you pass to `scripts/dead-code-removal.sh` must be the same names and paths shown in the inventory entry.

## Running a removal batch

1. Set the batch name to something meaningful (module-path-based, ticket number, etc.) so `quality/dead-code/batches/<name>.md` can be tied back to a hashtag/issue.
2. Run the script from the repo root: `BATCH_NAME=my-batch MODULES="src/control src/output" scripts/dead-code-removal.sh`. The runner copies the module list into the batch file before the change, runs `cargo test --locked --lib --tests --no-fail-fast`, and then calls `scripts/quality-baseline.sh` so the full baseline gates (fmt/clippy/tests) are enforced.
3. After the run finishes, the same script rewrites the module list as a "post-removal" snapshot inside the batch file and appends a gate summary (status, test log path, baseline exit code). The log files (`*-cargo-test.log`, `*-quality-baseline.log`) live alongside the summary so you can attach them to the PR or archival evidence bundle.

## Reviewing and linking the evidence

1. Before merging, open `quality/dead-code/batches/<name>.md`. Confirm the "Gate summary" section shows `Status: PASS` and that the test/baseline log paths exist.
2. Link to the gated baseline report from your PR description (e.g., mention the `quality-baseline.sh` log located at `quality/dead-code/batches/<name>-quality-baseline.log`) so auditors have an explicit pointer to the DC-02 verification.
3. If the status is `FAIL`, leave the batch file (and associated logs) in place. Do not merge until the failure is addressed. The failure artifact proves the gating sequence ran and gives triage data.

## Handling failed batches

1. Capture the log files recorded in the batch directory; they contain the `cargo test` output and the quality baseline trace needed to triage the regression.
2. Revert the code changes that were part of the batch or reassign the candidate to a higher risk tier if the failure cannot be resolved this cycle.
3. Update the inventory entry to note the failure and the new risk classification (for example, annotate why `src/control` remains untouched and reference the log paths). This keeps the DC-01 artifact accurate for future batch attempts.
4. Once the issue is resolved, rerun the same `BATCH_NAME` so the batch file shows a `PASS` record for DC-02 reviewers.

Following this sequence keeps dead-code removals small, gated, and fully documented as required by the DC-02 requirement.
