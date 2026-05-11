# LibreRoaster Control System Bug Audit Report

**Generated**: 2026-04-29 | **Audit Scope**: Control logic, actuator safety, PID integration, state transitions, sensor-to-actuator decision flow
**Methodology**: Static analysis of control/, safety/, hardware/ssr*, hardware/fan*, config/, and main loop orchestration
**Focus**: Concrete bug risks with file paths, line numbers, runtime impact, and roasting scenarios

---

## Severity Legend

| Label | Meaning |
|-------|---------|
| **CRITICAL** | Life/safety risk or system failure during roasting |
| **HIGH** | Observable misbehavior in production (data loss, protocol violation, safety gap) |
| **MEDIUM** | Degraded correctness or testability; may surface under specific conditions |
| **LOW** | Code-quality or documentation defect with no immediate runtime impact |

---

## 1. CRITICAL: SSR Hardware Status Bypass in Control Loop

**Severity**: CRITICAL
**Files**:
- `src/control/roaster_refactored.rs:708-711` (PID update check)
- `src/control/roaster_refactored.rs:34-46` (sensor read check)

**Issue**:
The control loop has inconsistent SSR hardware status checking. In `update_pid_control()`, the SSR availability check only happens when PID is enabled, but in `read_sensors()`, it only checks for over-temperature conditions. This creates a safety gap where the SSR could be in an error state but the PID controller might still attempt to apply power.

**Logic Issue**:
```rust
// In update_pid_control() - only checks when PID enabled
if self.status.ssr_hardware_status != SsrHardwareStatus::Available {
    warn!("PID update requested but SSR not available - skipping");
    return 0.0;  // Safe fallback
}

// In read_sensors() - only checks for overtemp, not SSR status
match self.sensor.read_sensors(&mut self.status).await {
    Err(RoasterError::TemperatureOutOfRange { ... }) => {
        // Emergency shutdown only for overtemp, not SSR errors
    }
}
```

**Runtime Impact**: If SSR hardware fails (e.g., open circuit, short circuit, or detection failure), the PID controller may continue sending PWM signals while the SSR is non-functional, leading to:
- Loss of temperature control during critical roast phases
- Potential overheating if SSR fails in "ON" state
- False confidence in system status

**Roasting Scenario**: During a roast, if the SSR hardware detection fails due to a faulty GPIO connection or noise on the heat detection line, the system might not detect the actual SSR state, continuing to apply PWM while no heating occurs, or failing to detect when SSR is stuck ON.

---

## 2. HIGH: PID Integrator Windup with Guard Interaction

**Severity**: HIGH
**Files**:
- `src/control/pid.rs:200-210` (bound_to_actuator function)
- `src/control/pid.rs:26-29` (is_saturated definition)

**Issue**:
The PID integrator anti-windup logic has a potential race condition between guard timeout detection and integrator clamping. The `bound_to_actuator()` function only clamps the integrator when the desired output exceeds applied output by more than `SATURATION_EPSILON`, but this doesn't account for guard timeouts that prevent actual actuation.

**Logic Issue**:
```rust
fn bound_to_actuator(&mut self, mv: f32) -> f32 {
    if let Some(feedback) = self.last_feedback {
        let applied = feedback.applied_output.clamp(self.output_min, self.output_max);
        // BUG: Only checks if desired > applied, but guard may block even if equal
        if mv > applied + SATURATION_EPSILON {
            self.integrator_clamped = true;
            return applied;  // Returns old value, doesn't reset integrator
        }
    }
    mv
}
```

**Runtime Impact**: During SSR guard timeouts (100ms cycles), the integrator continues to accumulate error even though no actual heating is occurring. When the guard releases, the PID may apply excessive power due to windup, causing temperature overshoot.

**Roasting Scenario**: When the SSR guard is active (e.g., during zero-crossing delays), the PID integrator keeps accumulating temperature error. When the guard releases, the system applies a full "windup" correction, potentially causing temperature spikes during critical roast development phases.

---

## 3. HIGH: Fan Speed Range Validation Gap

**Severity**: HIGH
**Files**:
- `src/control/roaster_refactored.rs:485-504` (SetFanSpeed command handling)
- `src/hardware/fan.rs:64-99` (fan speed setting)

**Issue**:
The fan speed validation only checks if the value is within 0-100% range but doesn't validate against the actual hardware capabilities. The fan controller may accept invalid speeds that don't translate to proper PWM duty cycles.

**Logic Issue**:
```rust
// In roaster_refactored.rs - only clamps to 0-100%
let clamped_speed = speed_percent.clamp(0.0, 100.0);

// In fan.rs - clamps again but may still produce invalid PWM
let clamped = speed_percent.clamp(0.0, 100.0);
let scaled = clamped * 255.0 / 100.0;  // May produce values > 255 due to floating point
```

**Runtime Impact**: Invalid fan speeds could lead to:
- Fan running at unexpected speeds during roast development
- Inadequate cooling during critical phases
- PWM signal corruption if calculations exceed hardware limits

**Roasting Scenario**: If an invalid fan speed is set (e.g., through a malformed Artisan command), the fan may not provide proper cooling during the development phase, leading to uneven roasting or scorching.

---

## 4. MEDIUM: State Transition Race Condition

**Severity**: MEDIUM
**Files**:
- `src/control/roaster_refactored.rs:736-742` (PID state transition logic)
- `src/application/tasks.rs:523-526` (continuous output toggle)

**Issue**:
The state transition from `Heating` to `Stable` state only checks temperature error within the PID update loop, not in the main control loop. This creates a race condition where the system might remain in `Heating` state even when temperature is stable.

**Logic Issue**:
```rust
// Only checked in PID update, not in main control loop
if self.state == crate::config::constants::RoasterState::Heating {
    let temp_error = (self.status.bean_temp - self.status.target_temp).abs();
    if temp_error < 2.0 {
        self.state = crate::config::constants::RoasterState::Stable;  // Late update
    }
}
```

**Runtime Impact**: The system may continue reporting "Heating" state to Artisan even when temperature is stable, leading to:
- Incorrect status reporting
- Potential confusion for roast operators
- Inconsistent behavior with Artisan expectations

**Roasting Scenario**: During the approach to target temperature, the system might switch to "Stable" state too late or not at all, causing Artisan to show incorrect roast progression status.

---

## 5. MEDIUM: Watchdog Feed Race Condition

**Severity**: MEDIUM
**Files**:
- `src/safety/watchdog.rs:55-65` (feed_async implementation)
- `src/application/tasks.rs:380-440` (watchdog feeding in control loop)

**Issue**:
The software watchdog feeding has a potential race condition between the atomic counter update and the hardware watchdog feeding. If the control loop timing is interrupted, the software watchdog might timeout while the hardware watchdog is still being fed.

**Logic Issue**:
```rust
pub fn feed_async(&mut self, _bean_temp: f32) -> Result<(), WatchdogError> {
    let was_zero = WATCHDOG_COUNTER.swap(MAX_MISSED_FEEDS, Ordering::SeqCst);
    if was_zero == 0 {
        self.last_failure = Some("watchdog_timeout");
        return Err(WatchdogError::FeedFailed("watchdog_timeout"));
    }
    self.last_failure = None;
    // Hardware feed happens after software check - potential race
    super::hw_watchdog::feed();
    Ok(())
}
```

**Runtime Impact**: During high system load or timing interrupts, the watchdog might trigger a false timeout, causing:
- Unnecessary system resets
- Disruption of ongoing roasts
- Inconsistent watchdog telemetry

**Roasting Scenario**: During intensive logging or data processing spikes, the watchdog feeding might be delayed, causing a false timeout and system reset in the middle of a critical roast phase.

---

## 6. LOW: LEDC Guard Timeout Handling

**Severity**: LOW
**Files**:
- `src/hardware/ledc_guard.rs:46-67` (try_acquire implementation)
- `src/control/controllers/actuator.rs:48-74` (SSR guard usage)

**Issue**:
The LEDC guard uses a busy-wait loop with spin_loop() which could cause priority inversion on a single-core system. The 40ms timeout provides an escape hatch but masks potential design issues.

**Logic Issue**:
```rust
loop {
    if !self.locked.swap(true, Ordering::Acquire) {
        return Ok(LedcGuardToken { guard: self });
    }
    if Instant::now().duration_since(start) >= timeout {
        record_timeout(channel_name);  // Timeout masks underlying issue
        return Err(LedcGuardError { channel: channel_name });
    }
    spin_loop();  // Busy-wait consumes CPU cycles
}
```

**Runtime Impact**: During high system load, the guard timeout might occur more frequently, leading to:
- Increased SSR cycle delays
- Potential temperature control instability
- Higher CPU utilization

**Roasting Scenario**: During simultaneous sensor reads and actuator updates, the LEDC guard might timeout more frequently, causing slight delays in SSR switching that could affect temperature precision.

---

## 7. LOW: Profile Interpolation Edge Cases

**Severity**: LOW
**Files**:
- `src/config/constants.rs:160-183` (RoastProfile target_at)
- `src/config/constants.rs:125-142` (FanProfile target_at)

**Issue**:
The profile interpolation logic has edge cases around zero division and boundary conditions that could cause unexpected behavior during profile following.

**Logic Issue**:
```rust
// In RoastProfile::target_at - potential division by zero
let range = curr.time_secs - prev.time_secs;
if range == 0 {
    return Some(curr.temperature);  // Should probably use interpolation
}
let frac = (elapsed_secs - prev.time_secs) as f32 / range as f32;
```

**Runtime Impact**: During profile execution, if setpoints have identical timestamps, the interpolation might skip values or use unexpected fallbacks, leading to:
- Temperature target jumps
- Inconsistent profile following
- Unexpected roast development

**Roasting Scenario**: If a roast profile contains duplicate timestamps (e.g., due to malformed Artisan data), the temperature target might jump unexpectedly, affecting roast consistency.

---

## Summary and Recommendations

### Critical Issues (1)
- **SSR Hardware Status Bypass**: Immediate safety concern - needs hardware status checking in all control paths

### High Issues (2)
- **PID Integrator Windup**: Fix guard-integrator interaction to prevent windup during guard timeouts
- **Fan Speed Validation**: Add hardware-specific validation beyond simple range checking

### Medium Issues (2)
- **State Transition Race**: Add state checking in main control loop, not just PID loop
- **Watchdog Race Condition**: Improve atomicity of watchdog feeding logic

### Low Issues (2)
- **LEDC Guard Timeout**: Consider async guard acquisition to avoid busy-waiting
- **Profile Interpolation**: Add validation for profile data integrity

### Verification Priority
1. **Critical**: SSR status checking in all control paths
2. **High**: PID anti-windup and fan validation
3. **Medium**: State machine and watchdog consistency
4. **Low**: Edge case handling and optimization

All findings include exact file paths, line numbers, and specific roasting scenarios where bugs could manifest. The audit focused on concrete logic issues rather than code quality concerns, as requested.