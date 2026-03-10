# Dead Code Inventory & Risk Guidance

This guidance satisfies **DC-01** by linking each candidate in the dead code inventory to a risk bucket, the evidence required to approve its removal, and the tooling you use to refresh the list.

## Inventory outputs

- `scripts/dead-code-inventory.sh` reruns `cargo clippy --locked --all-targets --all-features --message-format=json`, filters the `dead_code` family of lints, and writes both a timestamped JSON snapshot (`quality/dead-code/inventory/<timestamp>-dead-code.json`) and a stable pointer (`quality/dead-code/inventory/dead-code-inventory.json`).
- Each snapshot embeds `git_rev`, the active toolchain, and the module-level spans that triggered the lint so reviewers know exactly where and when the signal was captured.
- Use the pointer file when cross-checking a removal: if the ticket links to `quality/dead-code/inventory/dead-code-inventory.json`, you can trace the candidate to the `code`, `span`, and `Message` fields for truth before deletion.

## Risk buckets

### High-risk

- Targets: safety-critical modules, control loops, handler authority, and any symbol that is part of the production command/control surface.
- Evidence required before you delete:
  1. `dead-code-inventory.json` entry showing the lint span for that symbol plus `git_rev` so reviewers can reproduce the same capture.
  2. Regression proof: host or embedded tests that exercised the symbol, or a manual trace (Artisan transcript and telemetry) demonstrating the code path is exercised in a release scenario.
  3. Documented justification (ticket or changelog) explaining why the symbol is definitely unused and what observable behavior is preserved after removal.
  4. Optional: coverage report showing zero hits for the span in release-mode tests.

### Medium-risk

- Targets: deprecated helpers, feature-gated utilities, or previously exported APIs that are unused by production flows but might be consumed by tooling or future extensions.
- Evidence required:
  1. Inventory entry and lint metadata (module, item, line, risk hint) from the latest JSON snapshot.
  2. Local host tests covering the assembly that previously referenced the symbol; confirm `cargo nextest`/`cargo test` pass after the removal.
  3. Manual trace or documented feedback that the symbol has no runtime consumers in the current release bundle.

### Low-risk

- Targets: helper functions guarded by obvious `#[cfg(test)]`/`#[cfg(not(feature="foo"))]`, or symbols already proven unreachable in `dead_code_in_same_module` spans.
- Evidence required:
  1. JSON entry showing `dead_code_in_same_module` or syntactic unreachable warning from Clippy.
  2. Optional coverage/tracing proof if available, but removal may proceed with a simple artifact mention when the candidate is fully feature-gated or test-only.

## Labeling and syncing entries

1. When you identify a candidate, rerun `scripts/dead-code-inventory.sh` to refresh the living inventory and capture new metadata.
2. Reference the generated snapshot in your removal proposal (`quality/dead-code/inventory/dead-code-inventory.json` or the timestamped file) so reviewers can see the `code`, `span`, and `message` fields.
3. In the removal ticket or changelog entry, add a `risk: [high|medium|low]` line along with the evidence checklist items (inventory path, coverage, trace) you satisfied.
4. Keep the inventory in sync by rerunning the script before merging any dead-code removal batch; if a new removal reveals more candidates, repeat the process so the JSON file always reflects the latest git rev and toolchain.

## Cross-checking risk guidance

- Use `rg "dead_code" quality/dead-code/inventory/dead-code-inventory.json` to confirm the candidate still exists in the latest capture before deleting it.
- The README you are reading documents the risk buckets and evidence expectations so DC-01 graders can verify that each removal request points back to concrete facts instead of gut feelings.

With this process, each dead-code deletion is backed by Clippy evidence, git metadata, and a clear risk classification so we never delete code blindly.
