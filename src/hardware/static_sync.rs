use core::cell::UnsafeCell;

/// Shared SyncCell wrapper for static mutable data
///
/// This provides a consolidated module for the SyncCell pattern used across
/// UART and USB CDC tasks. Uses UnsafeCell for interior mutability,
/// maintaining API compatibility with existing code using `*cell.get() = value` pattern.
pub struct SyncCell<T>(UnsafeCell<T>);

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
