# LibreRoaster Compatibility Report — Official Artisan Application

**Last updated:** 2026-05-02
**Comparison target:** official Artisan application and public project/docs (`artisan-scope.org`, `artisan-roaster-scope/artisan`)

This report evaluates LibreRoaster against the official Artisan application from an engineering perspective. The goal is not to ask whether LibreRoaster is “good” or “bad,” but to state clearly where it is compatible, where it is only partially compatible, and where it does not currently overlap with Artisan’s broader ecosystem.

## 1. Executive summary

LibreRoaster is **compatible with the official Artisan application as a serial roasting controller**.

LibreRoaster is **not broadly compatible with Artisan as a full device and data ecosystem**.

That distinction is the key result.

## 2. What “compatible” means here

Artisan supports many device families, transports, protocols, file formats, and cloud/data workflows. LibreRoaster intentionally implements a much narrower surface:

- serial command exchange,
- TC4-style polling and control,
- temperature-scale synchronization,
- live roast control with PID and profiles,
- deep runtime telemetry through `STATUS`.

If the intended use case is “Artisan drives the roaster live over serial,” LibreRoaster is a good fit.

If the intended use case is “LibreRoaster should participate in every Artisan import/export/device feature,” it is not.

## 3. Compatibility matrix

| Area | Status | Notes |
|---|---|---|
| Serial transport | **Compatible** | USB CDC and UART cover the expected serial-controller workflow |
| Basic handshake (`CHAN`, `UNITS`, `FILT`) | **Compatible** | accepted and acknowledged by firmware |
| `READ` polling workflow | **Compatible** | TC4-style temperature polling is implemented |
| Manual heater/fan control | **Compatible** | `OT1`, `OT2`, `IO3`, `UP`, `DOWN`, `STOP` supported |
| PID control commands | **Compatible** | semicolon PID commands plus limited comma aliases |
| Runtime profile-following commands | **Partially compatible** | command-driven profile loading supported, file-level profile interchange is not |
| Temperature scale sync | **Compatible** | `UNITS` behavior implemented, visible in `STATUS` field 19 |
| Deep telemetry for automation | **Compatible, firmware-specific** | `STATUS` line is richer than bare TC4 polling |
| Artisan `.alog` file compatibility | **Not compatible** | LibreRoaster does not read/write Artisan profile files directly |
| WebSocket device mode | **Not compatible** | no WebSocket protocol implementation in firmware |
| Modbus device mode | **Not compatible** | no Modbus transport layer in firmware |
| artisan.plus cloud sync | **Not compatible** | no cloud sync or metadata synchronization layer |
| Broad vendor-device ecosystem | **Not compatible** | LibreRoaster is one device firmware, not a driver framework |

## 4. Strong compatibility areas

### 4.1 Serial session model

Artisan expects that many roasting devices behave like line-oriented serial controllers. LibreRoaster fits that model directly.

It supports:

- line-based command exchange,
- standard startup-oriented commands,
- repeated `READ` polling,
- runtime actuation commands,
- PID control paths.

This is the main reason the integration works well.

### 4.2 TC4-style polling shape

LibreRoaster emits the expected temperature-polling shape through `READ`, including the common 5-field TC4-style response and the extended form used while PID is enabled.

That means an Artisan session aimed at “serial roaster telemetry + control” can map onto LibreRoaster without needing a custom desktop-side protocol.

### 4.3 Scale negotiation

The firmware respects `UNITS;C` / `UNITS;F` and exposes the active scale in `STATUS`. That is a useful compatibility improvement because it makes scale state externally observable instead of hidden.

## 5. Partial compatibility areas

### 5.1 Profiles

LibreRoaster supports runtime profile commands such as `PROFILE` and `FANPROFILE`. That is valuable, but it should not be confused with full profile ecosystem compatibility.

Artisan’s broader data world includes profile files, metadata, historical replay, and richer desktop constructs. LibreRoaster only covers the live command-injection part.

### 5.2 Telemetry semantics

LibreRoaster’s `STATUS` line is useful and detailed, but it is firmware-specific in intent. It is best viewed as a LibreRoaster operational interface that Artisan or surrounding tooling can consume, not as a guarantee that every Artisan plugin or script will already understand all 19 fields semantically.

## 6. Non-compatible areas

### 6.1 Artisan file formats

The official Artisan project uses richer application-side data models and file formats such as `.alog`. LibreRoaster does not implement that persistence model.

That means:

- no native `.alog` import/export,
- no direct profile-file parity,
- no drop-in replacement for Artisan’s archival/reporting layer.

### 6.2 Alternative protocol families

The official Artisan project supports protocol families well beyond a TC4-like serial controller, including WebSockets and Modbus-oriented device integrations.

LibreRoaster currently does not implement those transports.

### 6.3 Cloud and inventory workflows

Artisan’s wider ecosystem includes artisan.plus-oriented workflows. LibreRoaster has no equivalent sync, metadata, scheduling, or inventory layer.

## 7. Important implementation-level caveats

### 7.1 Command range vs safe range

LibreRoaster currently accepts target-temperature commands up to 300 °C while its own documented safe-temperature envelope is lower. This does not break the serial integration, but it does create behavioral ambiguity for an Artisan operator who assumes “accepted target” means “aligned with safe configured behavior.”

### 7.2 SSR timing drift

The firmware currently has an internal mismatch between advertised SSR PWM frequency and the timer configuration performed during hardware init. Again, this does not break serial handshaking, but it affects how faithfully an Artisan-side operator can reason about heater behavior.

### 7.3 `FILT` is compatibility-oriented, not semantics-rich

LibreRoaster acknowledges `FILT`, but the implementation is intentionally shallow. It should be viewed as handshake compatibility rather than full feature parity.

## 8. Bottom-line assessment by use case

### Use case: live roast control from the official Artisan app

**Assessment:** strong fit

Why:

- serial transport supported,
- command workflow supported,
- polling supported,
- PID and profile commands supported,
- diagnostics richer than minimal TC4.

### Use case: interchange with Artisan’s historical/profile data ecosystem

**Assessment:** weak fit without extra tooling

Why:

- no `.alog` compatibility,
- no metadata persistence model,
- no direct archive/report interoperability.

### Use case: replacement for Artisan’s broader device ecosystem

**Assessment:** not a fit

Why:

- no Modbus/WebSocket/device-driver breadth,
- no cloud/inventory workflows,
- firmware scope is intentionally narrower.

## 9. Recommendation

The repository should keep describing LibreRoaster as:

**“compatible with the official Artisan app as a serial roaster controller”**

and avoid implying:

**“fully compatible with the entire Artisan platform and ecosystem.”**

That wording is technically honest and matches the current implementation.

## 10. Follow-up opportunities

If broader Artisan compatibility ever becomes a roadmap goal, the biggest gaps to close would be:

1. a defined profile/data interchange format,
2. stronger semantics around startup/configuration commands,
3. a decision on whether to support any non-serial protocol family,
4. explicit tooling for export/import rather than relying only on live-session control.
