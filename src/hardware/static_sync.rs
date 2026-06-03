use core::cell::UnsafeCell;

/// Shared SyncCell wrapper for static mutable data
///
/// This provides a consolidated module for the SyncCell pattern used across
/// UART and USB CDC tasks. Uses UnsafeCell for interior mutability,
/// maintaining API compatibility with existing code using `*cell.get() = value` pattern.
pub struct SyncCell<T>(UnsafeCell<T>);

// SAFETY: SyncCell is only used for static singletons on the single-core
// ESP32-C3. The stored types are raw pointers (`*mut T`) to driver
// singletons initialized once before tasks start. Access to the pointed-to
// data is serialized by an async Mutex (UART_MUTEX, USB_CDC_MUTEX) that
// guarantees exclusive &mut access at runtime. On single-core Embassy, only
// one task executes at a time, so no concurrent access occurs.
//
// Why no `T: Send` bound: The stored type is `*mut DriverType`, which is
// `!Send`. We cannot add `T: Send` without replacing the raw-pointer pattern.
// The safety of this impl depends on the INVARIANT that all dereferencing of
// the pointer inside SyncCell happens within an async Mutex guard scope.
//
// PORTING WARNING: If porting to multi-core (e.g., ESP32-S3), this impl is
// UNSOUND. Replace with channel-based driver access or a proper Mutex wrapper.
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
