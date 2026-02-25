# Phase 68: README Command Table for Automation Hooks - Context

**Gathered:** 2026-02-23
**Status:** Ready for planning

<domain>
## Phase Boundary

Update README.md so the Supported Artisan Commands table lists REG and STATUS/STAT, and the document points automation engineers to `internalDoc/INSTRUMENTATION_README.MD` for automation-grade expectations. This phase focuses on discoverability of the automation hooks; new command features stay out of scope.

</domain>

<decisions>
## Implementation Decisions

### REG vs STATUS/STAT documentation
- Document REG and STATUS/STAT as separate rows in the Supported Artisan Commands table so each command’s purpose is clear and they are easy to locate.
- Describe REG as the regression trigger that cuts outputs, emits `SAFETY OT-REGRESSION`, and exists for automation to drive regression sequences; mention the instrumentation guide for the full behavior/response expectations.
- Describe STATUS/STAT as the telemetry command automation should poll (STAT is an accepted alias) and emphasize it returns a deterministic CSV of watchdog feed success, guard timeout counts, regression state, and the standard ET/BT/SSR/FAN values so automations know what fields to parse.
- Add a short sentence near the commands table that directs readers to `internalDoc/INSTRUMENTATION_README.MD` for column definitions, STATUS payload expectations, and any REG regression notes automation needs.

### Claude's Discretion
- Exact wording of the command descriptions, the sentence that references the instrumentation guide, and whether the referral sits in the table row or immediately adjacent.
</decisions>

<specifics>
## Specific Ideas

- No additional product references were raised beyond the instrumentation guide; keep the focus on automation discovery of REG and STATUS/STAT.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 68-readme-command-table-for-automation-hooks*
*Context gathered: 2026-02-23*
