use crate::input::COMMAND_QUEUE_SIZE;
use core::cell::Cell;
use critical_section::Mutex;

pub const QUEUE_DEPTH_BACKLOG_THRESHOLD: usize = COMMAND_QUEUE_SIZE * 3 / 4;

pub struct QueueProcessorMetrics {
    queue_depth: Mutex<Cell<usize>>,
    max_depth: Mutex<Cell<usize>>,
    backlog_events: Mutex<Cell<usize>>,
}

impl QueueProcessorMetrics {
    pub const fn new() -> Self {
        Self {
            queue_depth: Mutex::new(Cell::new(0)),
            max_depth: Mutex::new(Cell::new(0)),
            backlog_events: Mutex::new(Cell::new(0)),
        }
    }

    fn record_depth(&self, depth: usize) {
        critical_section::with(|cs| {
            self.queue_depth.borrow(cs).set(depth);
            let current_max = self.max_depth.borrow(cs).get();
            if depth > current_max {
                self.max_depth.borrow(cs).set(depth);
            }
            if depth >= QUEUE_DEPTH_BACKLOG_THRESHOLD {
                let current_backlog = self.backlog_events.borrow(cs).get();
                self.backlog_events.borrow(cs).set(current_backlog + 1);
            }
        });
    }

    pub fn reset(&self) {
        critical_section::with(|cs| {
            self.queue_depth.borrow(cs).set(0);
            self.max_depth.borrow(cs).set(0);
            self.backlog_events.borrow(cs).set(0);
        });
    }

    fn snapshot(&self) -> QueueProcessorMetricsSnapshot {
        critical_section::with(|cs| QueueProcessorMetricsSnapshot {
            queue_depth: self.queue_depth.borrow(cs).get(),
            max_depth: self.max_depth.borrow(cs).get(),
            backlog_events: self.backlog_events.borrow(cs).get(),
            threshold: QUEUE_DEPTH_BACKLOG_THRESHOLD,
        })
    }
}

impl Default for QueueProcessorMetrics {
    fn default() -> Self {
        Self::new()
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
