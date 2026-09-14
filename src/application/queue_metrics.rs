//! Command-channel saturation metrics for the Artisan command path.
//!
//! Tracks the live and peak command-channel depth and counts backlog events
//! past `QUEUE_DEPTH_BACKLOG_THRESHOLD`, exposing a lock-free snapshot used by
//! STATUS/telemetry so dropped-command backpressure is observable.

use crate::application::service_container::ARTISAN_CMD_CHANNEL_SIZE;
use portable_atomic::{AtomicUsize, Ordering};

// Computed against the `ARTISAN_CMD_CHANNEL_SIZE` channel that is the
// actual measurement point, so the metric fires under real saturation.
/// Depth at/above which a queued command counts as a backlog event (3/4 of channel cap).
pub const QUEUE_DEPTH_BACKLOG_THRESHOLD: usize = ARTISAN_CMD_CHANNEL_SIZE * 3 / 4;

/// Lock-free counters for command-channel depth, peak depth, and backlog events.
pub struct QueueProcessorMetrics {
    queue_depth: AtomicUsize,
    max_depth: AtomicUsize,
    backlog_events: AtomicUsize,
}

impl QueueProcessorMetrics {
    /// Returns a zeroed metrics struct (usable in a `static`).
    pub const fn new() -> Self {
        Self {
            queue_depth: AtomicUsize::new(0),
            max_depth: AtomicUsize::new(0),
            backlog_events: AtomicUsize::new(0),
        }
    }

    /// Snapshot the three counters so STATUS/telemetry consumers (or future
    /// instrumentation) can read them without a wire-format change.
    pub fn snapshot(&self) -> (usize, usize, usize) {
        (
            self.queue_depth.load(Ordering::Relaxed),
            self.max_depth.load(Ordering::Relaxed),
            self.backlog_events.load(Ordering::Relaxed),
        )
    }

    fn record_depth(&self, depth: usize) {
        self.queue_depth.store(depth, Ordering::Relaxed);
        let current_max = self.max_depth.load(Ordering::Relaxed);
        if depth > current_max {
            // Best-effort: if another writer updated max_depth concurrently,
            // we keep the larger value. Relaxed is fine since we only need
            // eventual consistency for a telemetry metric.
            let _ = self.max_depth.compare_exchange(
                current_max,
                depth,
                Ordering::Relaxed,
                Ordering::Relaxed,
            );
        }
        if depth >= QUEUE_DEPTH_BACKLOG_THRESHOLD {
            self.backlog_events.fetch_add(1, Ordering::Relaxed);
        }
    }
}

impl Default for QueueProcessorMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Process-wide instance updated by `record_queue_depth` each control tick.
pub static QUEUE_PROCESSOR_METRICS: QueueProcessorMetrics = QueueProcessorMetrics::new();

/// Records the current command-channel depth into the global metrics instance.
pub fn record_queue_depth(depth: usize) {
    QUEUE_PROCESSOR_METRICS.record_depth(depth);
}

/// Observable snapshot of the queue metrics
/// `(queue_depth, max_depth, backlog_events)`.
pub fn queue_metrics_snapshot() -> (usize, usize, usize) {
    QUEUE_PROCESSOR_METRICS.snapshot()
}
