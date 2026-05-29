use core::cell::UnsafeCell;

/// Shared SyncCell wrapper for static mutable data
///
/// This provides a consolidated module for the SyncCell pattern used across
/// UART and USB CDC tasks. Uses UnsafeCell for interior mutability,
/// maintaining API compatibility with existing code using `*cell.get() = value` pattern.
pub struct SyncCell<T>(UnsafeCell<T>);

// SAFETY: SyncCell is only used for static singletons accessed from a single
// Embassy executor on the single-core ESP32-C3. All writes go through raw
// pointer operations (`*cell.get() = value`) performed inside critical
// sections or during one-time init before tasks start.
// NOTE: A `T: Send` bound would be more principled but is incompatible with
// the current usage pattern (raw pointers stored inside SyncCell for driver
// singletons). If porting to a multi-core ESP32 variant, this impl MUST be
// revisited — consider wrapping inner types in Mutex or using channel-based
// access instead.
unsafe impl<T> Sync for SyncCell<T> {}

impl<T> SyncCell<T> {
    /// Creates a new SyncCell with the given initial value
    pub const fn new(val: T) -> Self {
        Self(UnsafeCell::new(val))
    }

    /// Returns raw pointer to inner data
    ///
    /// Callers use this pattern:
    /// ```ignore
    /// *CELL.get() = Some(value);
    /// ```
    pub fn get(&self) -> *mut T {
        self.0.get()
    }
}
