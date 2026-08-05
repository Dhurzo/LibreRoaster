use crate::control::traits::{Fan, Heater, Thermometer};
use crate::control::RoasterError;
use crate::hardware::{fan::FanError, max31856::Max31856Error, ssr::SsrError};
use alloc::sync::Arc;
use core::cell::RefCell;
use critical_section::Mutex as CsMutex;

/// Mock thermometer that can return a configurable temperature or inject errors.
#[derive(Debug, Clone)]
pub struct MockThermometer {
    inject_error: Option<Max31856Error>,
    default_temp: f32,
    /// Shared state so a clone held by the test can inject/read mid-run.
    shared: Arc<CsMutex<RefCell<ThermoShared>>>,
}

#[derive(Debug, Default)]
struct ThermoShared {
    fail_next_reads: u32,
    read_calls: u32,
}

impl MockThermometer {
    pub fn new() -> Self {
        Self {
            inject_error: None,
            default_temp: 25.0,
            shared: Arc::new(CsMutex::new(RefCell::new(ThermoShared::default()))),
        }
    }

    pub fn inject_error(&mut self, error: Max31856Error) {
        self.inject_error = Some(error);
    }

    pub fn clear_error(&mut self) {
        self.inject_error = None;
    }

    pub fn set_default_temp(&mut self, temp: f32) {
        self.default_temp = temp;
    }

    /// Fail the next `n` `read_temperature` calls. Applied to the shared
    /// state, so a clone configured BEFORE `RoasterControl::new` can also be
    /// re-armed from the test after the mock was moved in.
    pub fn fail_next_reads(&mut self, n: u32) {
        critical_section::with(|cs| self.shared.borrow(cs).borrow_mut().fail_next_reads = n);
    }

    /// Number of `read_temperature` calls so far (shared across clones).
    pub fn read_calls(&self) -> u32 {
        critical_section::with(|cs| self.shared.borrow(cs).borrow().read_calls)
    }
}

impl Thermometer for MockThermometer {
    fn read_temperature(&mut self) -> Result<f32, RoasterError> {
        let fail = critical_section::with(|cs| {
            let mut s = self.shared.borrow(cs).borrow_mut();
            s.read_calls = s.read_calls.saturating_add(1);
            if s.fail_next_reads > 0 {
                s.fail_next_reads -= 1;
                true
            } else {
                false
            }
        });
        if fail {
            return Err(RoasterError::from(Max31856Error::CommunicationError {
                source: "injected_read_failure",
            }));
        }
        if let Some(error) = self.inject_error {
            return Err(RoasterError::from(error));
        }
        Ok(self.default_temp)
    }
}

impl Default for MockThermometer {
    fn default() -> Self {
        Self::new()
    }
}

/// Mock SSR/heater that exposes error injection for safe shutdown validation.
#[derive(Debug, Clone)]
pub struct MockSsr {
    inject_error: Option<SsrError>,
    /// Shared state so a clone held by the test can inject/read mid-run.
    shared: Arc<CsMutex<RefCell<SsrShared>>>,
}

#[derive(Debug)]
struct SsrShared {
    current_power: f32,
    status: crate::config::constants::SsrHardwareStatus,
    fail_next_writes: u32,
    write_calls: u32,
}

impl Default for SsrShared {
    fn default() -> Self {
        Self {
            current_power: 0.0,
            status: crate::config::constants::SsrHardwareStatus::Available,
            fail_next_writes: 0,
            write_calls: 0,
        }
    }
}

impl MockSsr {
    pub fn new() -> Self {
        Self {
            inject_error: None,
            shared: Arc::new(CsMutex::new(RefCell::new(SsrShared::default()))),
        }
    }

    pub fn inject_error(&mut self, error: SsrError) {
        self.inject_error = Some(error);
    }

    pub fn clear_error(&mut self) {
        self.inject_error = None;
    }

    /// Fail the next `n` `set_power` calls (shared state — re-armable from a
    /// clone after the mock was moved into `RoasterControl`).
    pub fn fail_next_writes(&mut self, n: u32) {
        critical_section::with(|cs| self.shared.borrow(cs).borrow_mut().fail_next_writes = n);
    }

    /// Override the hardware status reported by `get_status` (simulates a
    /// stuck-on/unknown SSR).
    pub fn set_status(&mut self, status: crate::config::constants::SsrHardwareStatus) {
        critical_section::with(|cs| self.shared.borrow(cs).borrow_mut().status = status);
    }

    /// Last duty successfully written (shared across clones).
    pub fn current_power(&self) -> f32 {
        critical_section::with(|cs| self.shared.borrow(cs).borrow().current_power)
    }

    /// Number of `set_power` calls so far (shared across clones).
    pub fn write_calls(&self) -> u32 {
        critical_section::with(|cs| self.shared.borrow(cs).borrow().write_calls)
    }
}

impl Heater for MockSsr {
    fn set_power(&mut self, duty: f32) -> Result<(), RoasterError> {
        let fail = critical_section::with(|cs| {
            let mut s = self.shared.borrow(cs).borrow_mut();
            s.write_calls = s.write_calls.saturating_add(1);
            if s.fail_next_writes > 0 {
                s.fail_next_writes -= 1;
                true
            } else {
                s.current_power = duty;
                false
            }
        });
        if fail {
            return Err(RoasterError::from(SsrError::PwmError {
                source: "injected_write_failure",
            }));
        }
        if let Some(error) = self.inject_error {
            return Err(RoasterError::from(error));
        }
        Ok(())
    }

    fn get_status(&self) -> crate::config::constants::SsrHardwareStatus {
        critical_section::with(|cs| self.shared.borrow(cs).borrow().status)
    }

    fn last_duty_delta_ticks(&self) -> i16 {
        0
    }

    fn last_retry_count(&self) -> u8 {
        0
    }
}

impl Default for MockSsr {
    fn default() -> Self {
        Self::new()
    }
}

/// Mock fan controller that either reports a speed or injects hardware errors.
#[derive(Debug, Clone)]
pub struct MockFan {
    inject_error: Option<FanError>,
    /// Shared state so a clone held by the test can inject/read mid-run.
    shared: Arc<CsMutex<RefCell<FanShared>>>,
}

#[derive(Debug, Default)]
struct FanShared {
    current_speed: f32,
    fail_next_speed_writes: u32,
    fail_next_emergency_writes: u32,
    speed_calls: u32,
    emergency_calls: u32,
}

impl MockFan {
    pub fn new() -> Self {
        Self {
            inject_error: None,
            shared: Arc::new(CsMutex::new(RefCell::new(FanShared::default()))),
        }
    }

    pub fn inject_error(&mut self, error: FanError) {
        self.inject_error = Some(error);
    }

    pub fn clear_error(&mut self) {
        self.inject_error = None;
    }

    /// Fail the next `n` `set_speed` calls (shared state).
    pub fn fail_next_speed_writes(&mut self, n: u32) {
        critical_section::with(|cs| self.shared.borrow(cs).borrow_mut().fail_next_speed_writes = n);
    }

    /// Fail the next `n` `emergency_set_speed` calls (shared state). This is
    /// the seam the safety paths need: `emergency_set_speed` used to be
    /// infallible, so a fan that cannot reach 100 % on the emergency path
    /// could never be simulated.
    pub fn fail_next_emergency_writes(&mut self, n: u32) {
        critical_section::with(|cs| {
            self.shared
                .borrow(cs)
                .borrow_mut()
                .fail_next_emergency_writes = n
        });
    }

    /// Last speed successfully written (shared across clones).
    pub fn current_speed(&self) -> f32 {
        critical_section::with(|cs| self.shared.borrow(cs).borrow().current_speed)
    }

    /// Number of `set_speed` calls so far.
    pub fn speed_calls(&self) -> u32 {
        critical_section::with(|cs| self.shared.borrow(cs).borrow().speed_calls)
    }

    /// Number of `emergency_set_speed` calls so far.
    pub fn emergency_calls(&self) -> u32 {
        critical_section::with(|cs| self.shared.borrow(cs).borrow().emergency_calls)
    }
}

impl Fan for MockFan {
    fn set_speed(&mut self, duty: f32) -> Result<(), RoasterError> {
        let fail = critical_section::with(|cs| {
            let mut s = self.shared.borrow(cs).borrow_mut();
            s.speed_calls = s.speed_calls.saturating_add(1);
            if s.fail_next_speed_writes > 0 {
                s.fail_next_speed_writes -= 1;
                true
            } else {
                s.current_speed = duty;
                false
            }
        });
        if fail {
            return Err(RoasterError::from(FanError::PwmError {
                source: "injected_speed_failure",
            }));
        }
        if let Some(error) = self.inject_error.clone() {
            return Err(RoasterError::from(error));
        }
        Ok(())
    }

    fn emergency_set_speed(&mut self, percentage: f32) -> Result<(), RoasterError> {
        let fail = critical_section::with(|cs| {
            let mut s = self.shared.borrow(cs).borrow_mut();
            s.emergency_calls = s.emergency_calls.saturating_add(1);
            if s.fail_next_emergency_writes > 0 {
                s.fail_next_emergency_writes -= 1;
                true
            } else {
                s.current_speed = percentage;
                false
            }
        });
        if fail {
            return Err(RoasterError::from(FanError::PwmError {
                source: "injected_emergency_failure",
            }));
        }
        Ok(())
    }

    fn get_speed(&self) -> f32 {
        critical_section::with(|cs| self.shared.borrow(cs).borrow().current_speed)
    }
}

impl Default for MockFan {
    fn default() -> Self {
        Self::new()
    }
}
