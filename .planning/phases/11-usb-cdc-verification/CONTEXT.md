# Phase 11: USB CDC Driver Verification

## Phase Boundary
This phase focuses on **verifying that the USB CDC peripheral works correctly** with the Artisan+ serial protocol. The implementation (USB CDC driver, command multiplexer, dual output task) is already complete. This phase is about confirming it functions as expected.

## Key Decisions

### Automated Tests
- **Type**: Unit tests only (no integration tests)
- **Focus areas**:
  - Multiplexer logic (channel activation, timeout)
  - READ response routing to correct channel
  - Command prioritization (first command wins)
- **Structure**: Hardware mocks for `usb_serial_jtag` peripheral
- **Key test case**: First command received activates channel within 60-second timeout

### Channel Switching
- **Trigger**: First byte received (not complete command)
- **Reason**: Immediate response to Artisan's communication
- **Logging**: Every switch logged at INFO level with timestamp
- **Pending responses**: Dropped (not queued for new channel)
  - Reason: Avoid stale data confusion

### USB Logging
- **Level**: INFO for all USB events
- **Logged events**:
  - Connection/disconnection
  - Commands received
  - Errors (timeout, buffer overflow)
  - State changes (channel switch, timeout expiry)

### Failure Handling
- **Behavior**: Emergency stop when USB fails during active roast
- **Meaning**: Immediately stop heater and fan, enter error state
- **User feedback**: Visual/audio indication of USB failure

## Implementation Reference
- Driver: `src/hardware/usb_cdc/driver.rs`
- Tasks: `src/hardware/usb_cdc/tasks.rs`
- Multiplexer: `src/input/multiplexer.rs`
- Dual Output: `src/application/tasks.rs`

## Verification Requirements
1. USB CDC peripheral initializes correctly
2. Commands received from Artisan via USB
3. Channel switching works on first byte
4. Multiplexer timeout expires correctly
5. READ responses routed to correct channel
6. Error handling on USB disconnection
7. Emergency stop triggered on USB failure during roast

## Notes
- Physical hardware (ESP32-C3) required for full verification
- Unit tests can verify logic without hardware
- Artisan application needed for integration testing
