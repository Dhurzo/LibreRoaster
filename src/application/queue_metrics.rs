use crate::application::service_container::ARTISAN_CMD_CHANNEL_SIZE;
use portable_atomic::{AtomicUsize, Ordering};

// Bug B27: the previous threshold was derived from `COMMAND_QUEUE_SIZE`
// (the F5.3-deleted legacy queue), so `backlog_events` would fire at
// 24 deeply-queued commands even though the channel was deleting plain
// commands at cap 8. Recompute against the `ARTISAN_CMD_CHANNEL_SIZE = 8`
// channel that is the actual measurement point so the metric fires under
// real saturation, giving B26's "command silently dropped" path the
// telemetry it should have had all along.
pub const QUEUE_DEPTH_BACKLOG_THRESHOLD: usize = ARTISAN_CMD_CHANNEL_SIZE * 3 / 4;

pub struct QueueProcessorMetrics {
    queue_depth: AtomicUsize,
    max_depth: AtomicUsize,
    backlog_events: AtomicUsize,
}

impl QueueProcessorMetrics {
    pub const fn new() -> Self {
        Self {
            queue_depth: AtomicUsize::new(0),
            max_depth: AtomicUsize::new(0),
            backlog_events: AtomicUsize::new(0),
        }
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

pub static QUEUE_PROCESSOR_METRICS: QueueProcessorMetrics = QueueProcessorMetrics::new();

pub fn record_queue_depth(depth: usize) {
    QUEUE_PROCESSOR_METRICS.record_depth(depth);
}
