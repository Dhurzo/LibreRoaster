# Roadmap: LibreRoaster

## Milestones

- ✅ **v1.5 Serial Protocol Implementation** — Phases 17-18 (shipped 2026-02-04)
- ✅ **v1.6 Documentation** — Phases 19-20 (shipped 2026-02-05)
- ✅ **v1.7 Non-Blocking USB Logging** — Phases 21-25 (completed 2026-02-05)
- 🔜 **v1.8 - TBD** — Next milestone

<details>
<summary>✅ v1.7 Non-Blocking USB Logging — COMPLETE</summary>

### Phase 21: Logging Foundation ✅ COMPLETE
- **Goal**: Secure non-blocking logging infrastructure using `defmt` and `bbqueue`.
- **Requirements**: LOG-06
- **Status**: ✅ Complete (2026-02-05)
  - defmt integrated
  - bbqueue buffer initialized
  - Non-blocking writes verified

### Phase 22: Async Transport & Metadata ✅ COMPLETE
- **Goal**: Deliver logs safely to hardware serial with channel metadata.
- **Requirements**: LOG-03
- **Status**: ✅ Complete (2026-02-05)
  - Channel prefix module created ([USB], [UART], [SYSTEM])
  - UART0 transport layer created (Phase 25)
  - Async drain task - Documented (Phase 25)
  - HW verification pending (no hardware available)

### Phase 23: USB Traffic Sniffing ✅ COMPLETE
- **Goal**: Real-time visibility into Artisan protocol traffic.
- **Requirements**: LOG-01, LOG-02
- **Status**: ✅ Complete (2026-02-05)
  - log_channel! macro created
  - USB reader/writer tasks instrumented with RX/TX logging
  - Unit tests added for log format validation

### Phase 24: Defmt + bbqueue Foundation ✅ COMPLETE
- **Goal**: Non-blocking logging infrastructure using defmt and bbqueue.
- **Requirements**: LOG-06
- **Status**: ✅ Complete (2026-02-05)
  - Closes critical gap: LOG-06 (defmt implemented)
  - defmt = "0.3", defmt-rtt = "0.4" added to Cargo.toml
  - log_channel! macro uses defmt::info! with format_args!
  - bbqueue deferred (embedded feature complexity)
- **Plans:**
  - ✅ 24-01-PLAN.md — Add defmt/bbqueue dependencies and update log_channel! macro

### Phase 25: UART Drain Task ✅ COMPLETE
- **Goal**: Background task that drains logs to UART0.
- **Requirements**: LOG-03
- **Status**: ✅ Complete (2026-02-05)
  - Closes critical gap: drain_task.rs created
  - Closes integration gap: logging module → UART0
  - Uses esp_println::println! for direct UART0 output
- **Plans:**
  - ✅ 25-01-PLAN.md — Create UART drain task

</details>

---

## Progress

| Milestone | Phases | Status | Completion Date |
|-----------|--------|--------|----------------|
| v1.5 Serial Protocol | 17-18 | ✅ Shipped | 2026-02-04 |
| v1.6 Documentation | 19-20 | ✅ Complete | 2026-02-05 |
| v1.7 USB Logging | 21-25 | ✅ Complete | 2026-02-05 |

---

*For current project status, see .planning/STATE.md*
