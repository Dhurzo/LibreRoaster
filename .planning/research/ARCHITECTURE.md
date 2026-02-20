# Architecture Research: Safety Fix Integration

**Domain:** Embedded firmware reliability and safety-critical control for LibreRoaster  
**Researched:** 2026-02-18  
**Focus:** How v3.0 Critical Safety Fixes integrate with existing architecture  
**Confidence:** HIGH

---

## Executive Summary

This architecture research maps how safety fixes for the v3.0 milestone integrate with the existing LibreRoaster ESP32-C3 firmware. The system implements a layered async architecture using embassy-rs with distributed safety mechanisms rather than a centralized safety component. Safety fixes must work within this existing handler chain pattern, using the ServiceContainer's critical_section for atomic state updates, and extending the dual-verification pattern already present at control-hardware boundaries.

---

## System Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Application Layer                            │
│  ┌─────────────────────┐  ┌──────────────────────────────────────┐ │
│  │   control_loop_task │  │         dual_output_task             │ │
│  │   - Command dispatch│  │   - USB CDC / UART output            │ │
│  │   - Control update  │  │   - Channel multiplexing             │ │
│  └─────────┬───────────┘  └──────────────────┬───────────────────┘ │
│            │                                   │                     │
│  ┌─────────▼──────────────────────────────────▼───────────────────┐ │
│  │              ServiceContainer (critical_section)                │ │
│  │  - RoasterControl access                                        │ │
│  │  - ArtisanInput access                                          │ │
│  │  - CommandMultiplexer access                                    │ │
│  └─────────────────────────┬─────────────────────────────────────┘ │
└────────────────────────────┼────────────────────────────────────────┘
                             │
┌────────────────────────────▼────────────────────────────────────────┐
│                        Control Layer                                │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    RoasterControl                              │  │
│  │  ┌──────────────┐ ┌──────────────┐ ┌───────────────────────┐ │  │
│  │  │ Safety       │ │ Temperature  │ │ ArtisanCommandHandler │ │  │
│  │  │ Command      │ │ Command      │ │                       │ │  │
│  │  │ Handler      │ │ Handler      │ │ - Manual heater/fan  │ │  │
│  │  │              │ │ - PID ctrl   │ │ - UP/DOWN commands   │ │  │
│  │  │ - Emergency  │ │ - Output     │ │                       │ │  │
│  │  │   Stop       │ │   Manager    │ │                       │ │  │
│  │  └──────┬───────┘ └──────────────┘ └───────────────────────┘ │  │
│  │         │                                                       │  │
│  │  ┌──────▼───────────────────────────────────────────────────┐  │  │
│  │  │           SsrCycleGuard                                  │  │  │
│  │  │  - Enforces SSR datasheet minimum interval (1000ms)     │  │  │
│  │  │  - Prevents command flooding                             │  │  │
│  │  └──────────────────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
                             │
┌────────────────────────────▼────────────────────────────────────────┐
│                     Hardware Abstraction Layer                      │
│  ┌────────────────┐  ┌────────────────┐  ┌────────────────────────┐  │
│  │ SSR (LEDC)     │  │ Fan (LEDC)     │  │ MAX31856               │  │
│  │ - PWM output   │  │ - PWM output   │  │ - SPI thermocouple    │  │
│  │ - Duty verify │  │ - Speed read   │  │ - BT/ET sensors       │  │
│  │ - Heat detect │  │                │  │                        │  │
│  └───────┬────────┘  └───────┬────────┘  └───────────┬────────────┘  │
│          │                   │                        │              │
│  ┌───────▼───────────────────▼────────────────────────▼────────────┐  │
│  │                    esp_hal Drivers                              │  │
│  │  LEDC │ SPI2 │ UART0 │ USB_DEVICE │ GPIO                      │  │
│  └────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Component Boundaries

### Safety-Critical Components

| Component | Responsibility | Safety Role | Communicates With |
|-----------|---------------|-------------|-------------------|
| `RoasterControl` | Main control orchestrator | **Central safety coordinator** — validates all commands, enforces limits, triggers emergency shutdown | All handlers, hardware drivers |
| `SafetyCommandHandler` | Emergency stop processing | **First in command chain** — intercepts emergency commands before other handlers | `RoasterControl` via handler chain |
| `SsrCycleGuard` | SSR timing enforcement | **Hardware protection** — enforces 1000ms minimum cycle time per SSR datasheet | `RoasterControl::apply_guarded_heater()` |
| `RoasterControl::emergency_shutdown()` | Full system shutdown | **Final safety measure** — zero-heating, full-fan, error state | All hardware drivers |
| `ServiceContainer` | Shared state management | **Concurrency safety** — critical_section mutexes protect shared state | All tasks via channels |

### Input/Output Components

| Component | Responsibility | Safety Relevance | Communicates With |
|-----------|---------------|------------------|-------------------|
| `ArtisanInput` (input/) | Command parsing | **Input validation** — validates ranges before passing to control | Parser → Multiplexer → RoasterControl |
| `CommandMultiplexer` (input/) | Channel switching | **Protocol isolation** — routes commands to correct handler | ArtisanInput ↔ RoasterControl |
| `ArtisanFormatter` (output/) | Response formatting | **Output validation** — validates response format before sending | RoasterControl → dual_output_task |
| `dual_output_task` | USB CDC / UART dispatch | **Transport safety** — handles write failures gracefully | ArtisanFormatter → USB/UART drivers |

---

## Data Flow: Safety-Critical Paths

### Path 1: Emergency Stop Command

```
Artisan Command (EmergencyStop)
         │
         ▼
┌─────────────────────────┐
│ ArtisanInput::parse()   │── Validates command syntax
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│ CommandMultiplexer      │── Routes to correct channel
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│ ServiceContainer        │── critical_section protect
│ ::with_roaster()       │
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│ RoasterControl          │
│ ::process_artisan_      │── Safety check FIRST in handler chain
│    command(EmergencyStop│
└───────────┬─────────────┘
            │
     ┌──────┴──────┐
     │             │
     ▼             ▼
┌─────────┐  ┌──────────────────┐
│Safety   │  │RoasterControl   │
│Command  │  │::stop_streaming()│
│Handler  │  │  - SSR → 0%     │
│         │  │  - Fan → 100%   │
│flag=true│  │  - PID disabled │
└────┬────┘  └────────┬─────────┘
     │                │
     └───────┬────────┘
             │
             ▼
    ┌────────────────┐
    │ Heater::       │
    │ set_power(0)  │── Hardware write
    └────────────────┘
```

### Path 2: Temperature Safety Check (Every Control Loop)

```
control_loop_task (every 100ms)
         │
         ▼
┌─────────────────────────┐
│ RoasterControl         │
│ ::read_sensors()        │
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│ MAX31856 sensors        │── SPI read BT, ET
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│ RoasterControl         │
│ ::update_temperatures() │
│                         │
│ if bean_temp >= 260°C  │── OVERTEMP_THRESHOLD check
│     emergency_shutdown()│
└─────────────────────────┘
```

### Path 3: SSR Output with Cycle Guard

```
RoasterControl::update_control()
         │
         ▼
┌─────────────────────────┐
│ SsrCycleGuard           │
│ ::next_cycle_allowed()  │
│                         │
│ if now < busy_until     │── 1000ms window check
│     reject/retry        │
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│ SsrControlSimple        │
│ ::set_percentage()      │
│                         │
│ 1. Set LEDC duty        │
│ 2. Read back duty      │
│ 3. Verify within        │── SSR_DUTY_TOLERANCE_TICKS (±2)
│    tolerance           │
│ 4. Retry if needed     │
└─────────────────────────┘
```

---

## Architectural Patterns

### Pattern 1: Embassy-driven control loops

**What:** Each subsystem (SSR, fan, heater) runs as an async loop spawned by Embassy, scheduling hardware updates while yielding to others through `embassy::time::Timer`.

**When to use:** when the system needs soft-real-time coordination across peripherals without blocking the executor.

**Trade-offs:** non-blocking but still monotonic, requires careful state synchronization via `Arc<Mutex<_>>`.

**Example:**
```rust
embassy::executor::Spawner::new().spawn(async move {
    let mut interval = Timer::interval(Duration::from_millis(50));
    loop {
        interval.next().await;
        let duty = ssr_scheduler.next_duty();
        pwm.set_duty(duty);
        // snapshot is landed for telemetry
    }
})?;
```

### Pattern 2: Command multiplexer + formatter trait

**What:** Encapsulate USB and UART endpoints behind a trait that enforces identical output format (CRLF) and buffers to avoid blocking.

**When to use:** when output channels share logical commands but have different physical constraints.

**Trade-offs:** adds indirection but isolates DMA setup from business logic and allows injecting non-blocking wrappers for reliability fixes.

**Example:**
```rust
trait ArtisanFormatter {
    fn write(&mut self, cmd: &str);
}

struct UsbFormatter { dma: UsbDma, buffer: String }
// ensures CLI command + telemetry happen via DMA transfers when idle
```

### Pattern 3: Snapshot-based shared state

**What:** Controllers expose `Arc<Mutex<ControllerState>>`, tasks clone handles for reads/writes, so output or telemetry tasks operate on consistent views without long-held locks.

**When to use:** when multiple async tasks need to read sensor values or actuator states simultaneously.

**Trade-offs:** adds cloning overhead and stale reads if not refreshed, but avoids deadlocks.

### Pattern 4: Handler Chain with Safety First

The architecture processes commands through a handler chain. Safety handlers must be positioned first in this chain to intercept dangerous commands before they reach other handlers.

```rust
// In RoasterControl::process_command()
let mut handlers: [&mut dyn RoasterCommandHandler; 4] = [
    &mut self.safety_handler,      // ← SAFETY FIRST
    &mut self.temp_handler,
    &mut self.artisan_handler,
    &mut self.system_handler,
];

for handler in &mut handlers {
    if handler.can_handle(command) {
        // Safety handler processes EmergencyStop
        // before other handlers see it
        return handler.handle_command(command, current_time, &mut self.status);
    }
}
```

**When to use:** Any new command type that affects heating, cooling, or system state must have a corresponding handler in this chain, with safety-relevant handlers first.

### Pattern 5: Dual Verification at Control Boundaries

Safety-critical hardware operations require dual verification: the command-level check AND the hardware-level check.

```rust
// Command-level (in handler)
if value > 100 {
    return Err(RoasterError::InvalidState);  // First check
}

// Hardware-level (in driver)
fn percentage_to_ledc_duty(percentage: f32) -> u8 {
    let clamped = percentage.clamp(0.0, 100.0);  // Second check
    // ...
}
```

**When to use:** Any fix that involves range validation, bounds checking, or hardware limits should implement dual verification at both the control layer and the hardware abstraction layer.

### Pattern 6: Fail-Safe Defaults with Explicit State

SystemStatus initializes with fail-safe defaults. Any new safety-related field must follow this pattern:

```rust
// From SystemStatus::default()
impl Default for SystemStatus {
    fn default() -> Self {
        Self {
            state: RoasterState::Idle,         // Safe state
            ssr_output: 0.0,                   // Heating OFF
            fan_output: 0.0,                   // Fan OFF
            pid_enabled: false,                 // PID disabled
            fault_condition: false,            // No fault
            ssr_hardware_status: SsrHardwareStatus::NotDetected,
            // ...
        }
    }
}
```

**When to use:** Adding new state fields for safety monitoring must initialize to fail-safe values (0%, OFF, disabled, error state).

### Pattern 7: Graceful Degradation with Hardware Status

The SSR driver monitors hardware status and reports availability. Control logic must check this status before enabling heating:

```rust
// In RoasterControl::update_control()
let desired_output = if self.safety_handler.is_emergency_active() {
    0.0  // Safety override
} else if self.status.pid_enabled {
    if self.status.ssr_hardware_status == SsrHardwareStatus::Available {
        self.update_pid_control(current_time)
    } else {
        warn!("PID enabled but SSR not available");
        0.0  // Graceful degradation
    }
} else {
    0.0
};
```

**When to use:** Any new safety feature that depends on hardware availability must implement graceful degradation rather than forcing operation or panicking.

### Pattern 8: Atomic Safety State Updates

Safety state changes must use critical_section to ensure atomic updates across the entire state:

```rust
// Using ServiceContainer for atomic access
ServiceContainer::with_roaster(|roaster| {
    // This closure runs atomically within critical_section
    roaster.status.fault_condition = true;
    roaster.status.ssr_output = 0.0;
    // Both updates happen together without interruption
    Ok(())
});
```

**When to use:** Any fix that modifies multiple safety-related status fields must use the ServiceContainer's critical_section to ensure atomicity.

---

## Anti-Patterns

### Anti-Pattern 1: Blocking write in CONTROL task

**What:** Directly drive `usb.write()` from CONTROL task, waiting for completion before proceeding.

**Why it's wrong:** A stalled USB endpoint blocks the entire executor, sacrificing SSR duty accuracy and fan updates.

**Do this instead:** Push formatted strings into the command multiplexer and use DMA-backed output futures; only yield to CONTROL after non-blocking confirmation.

### Anti-Pattern 2: Updating hardware and state in separate locks

**What:** Write to controller state snapshot and hardware driver under different mutexes without order.

**Why it's wrong:** Can lead to telemetry reporting stale SSR duty or LEDC brightness, undermining reliability.

**Do this instead:** Bundle hardware write + snapshot update inside the CONTROL task's tick loop, holding a single mutex briefly before releasing.

### Anti-Pattern 3: Bypassing the Handler Chain

**What:** Directly calling hardware methods without going through RoasterControl and its handler chain.

**Why bad:** Bypasses safety checks, validation, cycle guards, and logging. Could cause hardware damage or safety violations.

**Instead:** All heating/fan commands must route through `RoasterControl::process_command()` or `RoasterControl::process_artisan_command()`.

```rust
// BAD: Direct hardware call
heater.set_power(100.0);  // Bypasses ALL safety checks

// GOOD: Through handler chain
roaster.process_command(RoasterCommand::SetHeaterManual(100), now);
```

### Anti-Pattern 4: Non-Atomic Safety State Updates

**What:** Updating safety-critical state fields individually without protection.

**Why bad:** Control loop could read partial state between updates, causing incorrect safety decisions.

```rust
// BAD: Non-atomic update
status.fault_condition = true;
// Control loop could run here!
status.ssr_output = 0.0;

// GOOD: Atomic update via ServiceContainer
ServiceContainer::with_roaster(|roaster| {
    roaster.status.fault_condition = true;
    roaster.status.ssr_output = 0.0;
    Ok(())
});
```

### Anti-Pattern 5: Ignoring Hardware Status

**What:** Setting heating output without checking if SSR hardware is available.

**Why bad:** Could command heating when heat source is not detected, leading to confusion about actual system state.

```rust
// BAD: Ignores hardware status
self.status.ssr_output = desired;
heater.set_power(desired);

// GOOD: Checks and handles unavailability
if self.status.ssr_hardware_status == SsrHardwareStatus::Available {
    heater.set_power(desired)?;
    self.status.ssr_output = desired;
} else {
    warn!("SSR not available, output suppressed");
    self.status.ssr_output = 0.0;
}
```

### Anti-Pattern 6: Swallowing Safety Errors

**What:** Catching safety-related errors without appropriate action.

**Why bad:** Safety errors indicate dangerous conditions that require immediate response.

```rust
// BAD: Swallows error
if let Err(e) = self.read_sensors() {
    debug!("Sensor error: {:?}", e);  // No action!
}

// GOOD: Triggers safety response
if let Err(e) = self.read_sensors() {
    warn!("Sensor error: {:?}", e);
    self.emergency_shutdown("Temperature sensor failure")?;
}
```

---

## Integration Points for v3.0 Safety Fixes

Based on the architecture analysis, safety fixes for v3.0 should integrate at these specific points:

### 1. Handler Chain Extension

- **Location:** `RoasterControl::process_command()` in `src/control/roaster_refactored.rs`
- **Pattern:** Add new handlers to the chain, ensuring safety handlers remain first
- **Considerations:** New safety features may require new `RoasterCommand` variants

### 2. SystemStatus Extension

- **Location:** `SystemStatus` struct in `src/config/constants.rs`
- **Pattern:** Add new safety-related fields with fail-safe defaults
- **Considerations:** Any new field affects serialization for Artisan responses

### 3. Emergency Shutdown Enhancement

- **Location:** `RoasterControl::emergency_shutdown()` in `src/control/roaster_refactored.rs`
- **Pattern:** Extend shutdown sequence to cover additional hardware/sensors
- **Considerations:** Must maintain existing behavior for backward compatibility

### 4. Temperature Validation Enhancement

- **Location:** `RoasterControl::update_temperatures()` in `src/control/roaster_refactored.rs`
- **Pattern:** Add additional temperature safety checks (rate-of-change, differential)
- **Considerations:** Balance safety responsiveness with false-positive avoidance

### 5. SSR Monitor Enhancement

- **Location:** `SsrControlSimple` in `src/hardware/ssr.rs`
- **Pattern:** Extend duty verification, add retry logic, enhance status reporting
- **Considerations:** Must work within existing cycle guard timing

---

## Scaling Considerations

For an ESP32-C3 embedded system, scalability is limited by hardware constraints:

| Scale | Architecture Adjustments |
|-------|--------------------------|
| 0-1 controllers | Current monolithic async executor with shared state is adequate; focus on reliability fixes rather than partitioning. |
| 2-4 controllers | Ensure additional controllers (fans, heaters) reuse the shared snapshot pattern and keep new tasks low priority to avoid starving telemetry. |
| 5+ controllers | Introduce executor priorities/affinity so reliability-critical loops (SSR duty, I/O) outrank new features; consider offloading telemetry formatting to dedicated low-priority tasks. |

### Scaling Priorities

1. **First bottleneck:** Blocking USB/UART writes — mitigate by confirming DMA-based non-blocking wrappers before adding more telemetry.
2. **Second bottleneck:** SSR duty accuracy under load — guard with watchdog verifying duty vs hardware register after each update.

| Concern | At Current Scale | Mitigation for Growth |
|---------|------------------|---------------------|
| **Memory** | 320KB SRAM | Use static allocation, avoid heap in safety paths |
| **Stack depth** | Embassy tasks limited | Keep handler chains shallow, inline small functions |
| **Channel depth** | 8-16 messages | Monitor overflow in testing, increase if needed |
| **Control loop jitter** | Target 100ms | Keep async operations short, use spawn for long work |

---

## Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| Control tasks ↔ Shared snapshots | `Arc<Mutex<ControllerState>>` clones | New reliability fixes keep SSR duty + LEDC settings synchronized before releasing lock. |
| Command multiplexer ↔ Formatter trait | Async queue + DMA futures | Non-blocking I/O changes require the formatter to acknowledge completion before CONTROL tasks assume the message cleared. |
| FanController ↔ LEDC hardware module | Interface exposing `set_brightness(duty)` | LEDC update now includes notification back to the controller state to confirm hardware acknowledgement. |

---

## External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| USB CDC, UART | DMA-backed non-blocking writer | Formatter enforces CRLF and exposes ready state so CONTROL tasks do not block while command multiplexer drains queues. |
| LEDC/SSR hardware | `esp-hal` PWM via embassy timers | Controllers reconfigure timers through `FanController` and new SSR duty scheduler ensuring accurate duty cycle transitions. |

---

## Part 2: Async Mutex Integration (v3.x Milestone)

**Focus:** Replace take/replace pattern with embassy_sync::Mutex for async-safe RoasterControl access  
**Researched:** 2026-02-19  
**Confidence:** HIGH

---

### Current Architecture: The Take/Replace Problem

The ServiceContainer holds `RoasterControl` behind a `critical_section::Mutex<RefCell<Option<RoasterControl>>>`:

```rust
// Current ServiceContainer structure
pub struct ServiceContainer {
    pub roaster: Mutex<RefCell<Option<RoasterControl>>>,  // critical_section::Mutex
    pub artisan_input: Mutex<RefCell<Option<ArtisanInput>>>,
    pub multiplexer: Mutex<RefCell<Option<CommandMultiplexer>>>,
}
```

**The problematic roaster_async_sensor_read() pattern:**

```rust
pub async fn roaster_async_sensor_read() -> Result<(), ContainerError> {
    // Take ownership - blocks ALL other access
    let mut roaster: RoasterControl = critical_section::with(|cs| {
        container.roaster.borrow(cs).borrow_mut().take()  // EXCLUSIVE
    });
    
    // Await while NOT holding the lock - but ownership is moved!
    roaster.read_sensors().await?;
    roaster.update_control(embassy_time::Instant::now())?;
    
    // Replace - gives access back
    critical_section::with(|cs| {
        container.roaster.borrow(cs).borrow_mut().replace(roaster);
    });
    
    Ok(())
}
```

**Why This Is Problematic:**

| Issue | Consequence |
|-------|-------------|
| Exclusive ownership model | No concurrent access during async operation |
| Two-step take/replace | If task preempts between take and replace, container is `None` |
| Not truly async-safe | Lock is not held during await, but ownership model is racy |
| Complex error handling | Must ensure replace happens even on errors |

---

### Recommended Architecture: embassy_sync::Mutex

```rust
use embassy_sync::mutex::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

pub struct ServiceContainer {
    // Replace critical_section::Mutex<RefCell<Option<T>>> with:
    pub roaster: Mutex<CriticalSectionRawMutex, Option<RoasterControl>>,
    // No RefCell needed - Mutex provides interior mutability
}
```

**Why This Works Better:**

1. **Async-aware locking:** The lock is acquired with `.lock().await`:
   - Suspends the current task if lock is held
   - Allows other tasks to run while waiting
   - Automatically releases when the guard drops

2. **No ownership transfer:** The guard provides `&mut T` access:
   ```rust
   pub async fn roaster_async_sensor_read() -> Result<(), ContainerError> {
       let mut guard = Self::get_instance().roaster.lock().await;
       if let Some(roaster) = guard.as_mut() {
           roaster.read_sensors().await?;
           roaster.update_control(embassy_time::Instant::now())?;
       }
       Ok(()) // Guard automatically released here
   }
   ```

3. **Simpler error handling:** No manual take/replace - guard lifetime ensures cleanup

---

### Integration Points

#### 1. ServiceContainer Structure

**File:** `src/application/service_container.rs`

```rust
// Before (current)
use critical_section::Mutex;
use core::cell::RefCell;

pub struct ServiceContainer {
    pub roaster: Mutex<RefCell<Option<RoasterControl>>>,
}

// After (recommended)
use embassy_sync::mutex::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

pub struct ServiceContainer {
    pub roaster: Mutex<CriticalSectionRawMutex, Option<RoasterControl>>,
}
```

#### 2. Initialization

```rust
// Before
impl ServiceContainer {
    pub fn get_instance() -> &'static ServiceContainer {
        static SERVICE: ServiceContainer = ServiceContainer {
            roaster: Mutex::new(RefCell::new(None)),
        };
        &SERVICE
    }
}

// After
impl ServiceContainer {
    pub fn get_instance() -> &'static ServiceContainer {
        static SERVICE: ServiceContainer = ServiceContainer {
            roaster: Mutex::new(None),  // Option<T> directly
        };
        &SERVICE
    }
}
```

#### 3. Migrating with_roaster() Patterns

**Before (sync closure pattern):**
```rust
pub fn with_roaster<R, F>(f: F) -> Result<R, ContainerError>
where
    F: FnOnce(&mut RoasterControl) -> R,
{
    critical_section::with(|cs| {
        match container.roaster.borrow(cs).borrow_mut().as_mut() {
            Some(roaster) => Ok(f(roaster)),
            None => Err(ContainerError::NotInitialized),
        }
    })
}
```

**After - Option 1: Async-first (recommended)**
```rust
pub async fn with_roaster<R, F>(f: F) -> Result<R, ContainerError>
where
    F: FnOnce(&mut RoasterControl) -> R,
{
    let mut guard = Self::get_instance().roaster.lock().await;
    match guard.as_mut() {
        Some(roaster) => Ok(f(roaster)),
        None => Err(ContainerError::NotInitialized),
    }
}
```

**After - Option 2: Keep closure pattern for interrupt contexts**

If some callers run in interrupt context (not async), keep a separate sync method using `critical_section::Mutex<RefCell<...>>` or use `embassy_sync::blocking_mutex` for those paths.

#### 4. Migrating roaster_async_sensor_read()

**Before:**
```rust
pub async fn roaster_async_sensor_read() -> Result<(), ContainerError> {
    let mut roaster = critical_section::with(|cs| {
        container.roaster.borrow(cs).borrow_mut().take()
    });
    
    roaster.read_sensors().await?;
    roaster.update_control(embassy_time::Instant::now())?;
    
    critical_section::with(|cs| {
        container.roaster.borrow(cs).borrow_mut().replace(roaster);
    });
    Ok(())
}
```

**After:**
```rust
pub async fn roaster_async_sensor_read() -> Result<(), ContainerError> {
    let mut guard = Self::get_instance().roaster.lock().await;
    
    if let Some(roaster) = guard.as_mut() {
        roaster.read_sensors().await?;
        roaster.update_control(embassy_time::Instant::now())?;
    }
    
    Ok(())  // Guard dropped, lock released
}
```

---

### Migration Path

#### Phase 1: Add embassy_sync::Mutex alongside existing structure

- Add the new mutex field next to existing one
- Test that both can coexist
- No behavioral change yet

#### Phase 2: Migrate roaster_async_sensor_read()

- Replace take/replace with `.lock().await`
- Verify async behavior works correctly
- This is the primary motivation for the change

#### Phase 3: Migrate with_roaster() callers

- Identify all sync callers in interrupt context
- Either:
  - Convert to async (preferred)
  - Keep using `critical_section` for those specific paths

#### Phase 4: Remove old structure

- Remove `critical_section::Mutex<RefCell<Option<T>>>`
- Remove `RefCell` imports if no longer needed
- Clean up initialization code

---

### Build Order Considerations

| Dependency | Status |
|------------|--------|
| embassy_sync 0.6.1 | Already in Cargo.toml |
| embassy_sync::mutex::Mutex | From same crate |
| CriticalSectionRawMutex | Re-exported from `embassy_sync::blocking_mutex::raw` |

**No changes needed to:**
- RoasterControl struct
- Sensor drivers (max31856)
- Hardware initialization

**Only files needing modification:**
- `src/application/service_container.rs` (primary)
- Callers of `with_roaster()` and `roaster_async_sensor_read()` (secondary)

---

### Risk Assessment

| Risk | Severity | Mitigation |
|------|----------|------------|
| Breaking sync access from interrupts | HIGH | Test all interrupt handlers, convert to async or keep separate path |
| Performance regression | LOW | embassy_sync is well-optimized for embedded |
| Deadlocks | MEDIUM | Ensure all lock acquisitions are async, no nested sync calls |

---

### Alternative Approaches Considered

#### 1. Keep critical_section for sync, embassy_sync for async

```rust
pub struct ServiceContainer {
    // For sync access (interrupts)
    pub roaster_sync: critical_section::Mutex<RefCell<Option<RoasterControl>>>,
    // For async access  
    pub roaster_async: Mutex<CriticalSectionRawMutex, Option<RoasterControl>>,
}
```

**Verdict:** Not recommended - dual state increases complexity.

#### 2. Use embassy_sync::blocking_mutex::Mutex (not async)

For blocking locks in async context that don't hold across await. Not suitable since we need to hold during `.await`.

#### 3. Refactor to avoid shared state entirely

Pass RoasterControl through channels. **Verdict:** Too invasive for this milestone.

---

### References

- [embassy_sync::mutex::Mutex](https://docs.embassy.dev/embassy-sync/git/default/mutex/struct.Mutex.html)
- [CriticalSectionRawMutex](https://docs.embassy.dev/embassy-sync/git/default/blocking_mutex/raw/struct.CriticalSectionRawMutex.html)
- [Sharing Data Among Tasks](https://blog.theembeddedrustacean.com/sharing-data-among-tasks-in-rust-embassy-synchronization-primitives)

---

### Confidence Assessment (Async Mutex)

| Area | Confidence | Reason |
|------|------------|--------|
| embassy_sync API | HIGH | Context7 docs, version 0.6.1 stable |
| Migration pattern | HIGH | Standard embassy pattern |
| RoasterControl compatibility | HIGH | No changes needed to controlled type |
| Interrupt compatibility | MEDIUM | Need to verify sync access still works from handlers |

---

*Architecture research for: LibreRoaster v3.x Async Mutex Integration*  
*Researched: 2026-02-19*
