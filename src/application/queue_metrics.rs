use crate::input::COMMAND_QUEUE_SIZE;
use core::sync::atomic::{AtomicUsize, Ordering};

pub const QUEUE_DEPTH_BACKLOG_THRESHOLD: usize = COMMAND_QUEUE_SIZE * 3 / 4;

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
        self.queue_depth.store(depth, Ordering::SeqCst);
        self.update_max(depth);
        if depth >= QUEUE_DEPTH_BACKLOG_THRESHOLD && depth > 0 {
            self.backlog_events.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn reset(&self) {
        self.queue_depth.store(0, Ordering::SeqCst);
        self.max_depth.store(0, Ordering::SeqCst);
        self.backlog_events.store(0, Ordering::SeqCst);
    }

    fn snapshot(&self) -> QueueProcessorMetricsSnapshot {
        QueueProcessorMetricsSnapshot {
            queue_depth: self.queue_depth.load(Ordering::SeqCst),
            max_depth: self.max_depth.load(Ordering::SeqCst),
            backlog_events: self.backlog_events.load(Ordering::SeqCst),
            threshold: QUEUE_DEPTH_BACKLOG_THRESHOLD,
        }
    }

    fn update_max(&self, depth: usize) {
        let mut current = self.max_depth.load(Ordering::SeqCst);
        while depth > current {
            match self.max_depth.compare_exchange(
                current,
                depth,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => break,
                Err(prev) => current = prev,
            }
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct QueueProcessorMetricsSnapshot {
    pub queue_depth: usize,
    pub max_depth: usize,
    pub backlog_events: usize,
    pub threshold: usize,
}

pub static QUEUE_PROCESSOR_METRICS: QueueProcessorMetrics = QueueProcessorMetrics::new();

pub fn queue_processor_metrics_snapshot() -> QueueProcessorMetricsSnapshot {
    QUEUE_PROCESSOR_METRICS.snapshot()
}

pub fn reset_queue_processor_metrics() {
    QUEUE_PROCESSOR_METRICS.reset();
}

pub fn record_queue_depth(depth: usize) {
    QUEUE_PROCESSOR_METRICS.record_depth(depth);
}

pub fn queue_processor_backlog_threshold() -> usize {
    QUEUE_DEPTH_BACKLOG_THRESHOLD
}
