# Requirements: LibreRoaster v2.4

**Defined:** 2026-02-08
**Core Value:** Artisan can read temperatures and control heater/fan during a roast session via serial connection.

## v2.4 Requirements

### Logging Redirection

- [ ] **LOG-01**: All logging output redirected to UART0
- [ ] **LOG-02**: USB Serial handles Artisan commands only (no log output)
- [ ] **LOG-03**: No log interference on Artisan communication channel
- [ ] **LOG-04**: UART logging at 115200 baud (same as Artisan communication)

### Architecture

- [ ] **LOG-05**: Logging infrastructure uses UART0 peripheral
- [ ] **LOG-06**: USB Serial dedicated to Artisan command/response traffic
- [ ] **LOG-07**: Clean separation between debug output and protocol communication

## v3 Requirements (Future)

### Additional Features

- **LOG-03**: Configurable logging levels (DEBUG, INFO, WARN, ERROR)
- **LOG-04**: Remote logging via WiFi (future hardware)
- **LOG-05**: Log buffering for UART underflow scenarios

## Out of Scope

| Feature | Reason |
|---------|--------|
| USB Serial removal | USB still needed for Artisan commands |
| WiFi logging | Future hardware expansion |
| Log persistence | Not required for basic operation |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| LOG-01 | 43 | Pending |
| LOG-02 | 43 | Pending |
| LOG-03 | 43 | Pending |
| LOG-04 | 43 | Pending |
| LOG-05 | 43 | Pending |
| LOG-06 | 43 | Pending |
| LOG-07 | 43 | Pending |

---

*Requirements defined: 2026-02-08*
*Last updated: 2026-02-08 after v2.4 milestone initialization*
