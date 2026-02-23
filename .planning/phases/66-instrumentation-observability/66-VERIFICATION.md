---
phase: 66-instrumentation-observability
verified: 2026-02-23T17:42:24Z
status: passed
score: 3/3 must-haves verified
---

# Phase 66: Instrumentation Observability Verification Report

**Phase Goal:** Make safety instrumentation metrics observable to automation by adding a dedicated Artisan command that publishes watchdog, guard, and regression telemetry in a deterministic CSV payload.
**Verified:** 2026-02-23T17:42:24Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
| --- | ----- | ------ | -------- |
| 1 | Automation can poll `STATUS` and parse a deterministic CSV that lists ET, BT, heater, fan, watchdog health, consecutive failures, LEDC guard timeouts, and regression activity without altering `READ`. | ✓ VERIFIED | `src/input/parser.rs` adds `StatusReport` and tests (`test_parse_status_command`, `test_parse_stat_command`) while `src/application/tasks.rs` routes this variant through `ArtisanFormatter::format_status_response` and the shared output channel. |
| 2 | Watchdog feed state, failure reason, guard counts, and regression flags appear in the payload so monitoring can correlate events without touching `READ`. | ✓ VERIFIED | `src/output/artisan.rs::format_status_response` emits `{ET, BT, Heater, Fan, WatchdogOK, WatchdogFailures, LastWatchdogReason, LEDCGuardTimeouts, RegressionActive}` while `src/application/tasks.rs` keeps `SystemStatus` watcher/guard fields fresh (watchdog feed updates, guard timeout counters, regression flag flows). |
| 3 | Documentation explains the `STATUS` payload and parsing expectations for auditors/automation. | ✓ VERIFIED | `internalDoc/INSTRUMENTATION_README.MD` spells out the `STATUS` CSV headers, sample payload, and column meanings for automation vs `READ`. |

**Score:** 3/3 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
| -------- | -------- | ------ | ------- |
| `src/input/parser.rs` | Artisan parser recognizes `STATUS`/`STAT`, returns `ArtisanCommand::StatusReport`, and keeps `READ` stable with regression tests. | ✓ VERIFIED | Branch at `parse_artisan_command` line 67-107 handles the new command and tests at lines 151-195/397-495 guard parsing behavior. |
| `src/output/artisan.rs` | `format_status_response` and tests ensure the CSV has watchdog/guard/regression fields in fixed order. | ✓ VERIFIED | Function at lines 138-165 emits nine columns, and regression test `test_format_status_response_columns_order` (lines 304-318) locks the order. |
| `src/application/tasks.rs` | Control loop detects `StatusReport`, formats it, and enqueues the line through the shared Artisan output channel. | ✓ VERIFIED | Loop at lines 33-62 handles `StatusReport`, calls `ArtisanFormatter::format_status_response`, and sends the string via `ServiceContainer::get_output_channel()`. |
| `internalDoc/INSTRUMENTATION_README.MD` | Documentation describes the `STATUS` payload, sample response, and column guidance for automation/auditors. | ✓ VERIFIED | Section at lines 15-37 defines the CSV header, example, and per-column explanations. |

### Key Link Verification

| From | To | Via | Status | Details |
| ---- | -- | --- | ------ | ------- |
| `src/input/parser.rs` | `src/control/roaster_refactored.rs` | `ArtisanCommand::StatusReport` handling in `process_artisan_command` | ✓ WIRED | Parser produces `StatusReport`, and `RoasterControl::process_artisan_command` (lines 559-569) updates SSR status then invokes `ArtisanFormatter::format_status_response`. |
| `src/control/roaster_refactored.rs` | `src/output/artisan.rs` | `ArtisanFormatter::format_status_response` call | ✓ WIRED | `process_artisan_command` uses the formatter to generate the CSV string from the latest `SystemStatus`. |
| `src/application/tasks.rs` | `src/output/artisan.rs` | Control loop routes formatted CSV to the shared output channel | ✓ WIRED | The task obtains `StatusReport` commands, formats them, and sends the resulting `String<128>` through `output_channel` (lines 42-63). |

### Requirements Coverage

| Requirement | Status | Blocking Issue |
| ----------- | ------ | -------------- |
| OBS-01 (instrumentation snapshot with watchdog/guard/regression fields) | ✓ SATISFIED | None — `format_status_response` exposes the required columns and control loop keeps `SystemStatus` values fresh. |
| OBS-02 (Expose snapshot via new Artisan command) | ✓ SATISFIED | None — parser, roaster, and application tasks now handle `STATUS` without impacting `READ`. |
| OBS-03 (Document `STATUS` payload/parsing expectations) | ✓ SATISFIED | None — `internalDoc/INSTRUMENTATION_README.MD` describes the CSV, sample output, and how automation should interpret it. |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
| ---- | ---- | ------- | -------- | ------ |
| None detected | - | - | - | - |

### Human Verification Required

None — automated checks cover parsing, formatting, and docs.

### Gaps Summary

None. The dedicated `STATUS` command, deterministic CSV payload, and documentation exist with the wiring and tests needed for automation/audit observability.

_Verified: 2026-02-23T17:42:24Z_
_Verifier: Claude (gsd-verifier)_
