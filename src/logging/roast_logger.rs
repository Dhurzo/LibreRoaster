/// Ring-buffer roast data logger — backup recorder for Artisan disconnects.
/// Stores up to LOG_CAPACITY CSV samples in a heapless ring buffer.
/// On reconnect, Artisan can send `#DUMP` to retrieve buffered data.
use core::cell::RefCell;
use critical_section::Mutex;
use embassy_time::Instant;
use heapless::{Deque, String as HeaplessString};

const LOG_CAPACITY: usize = 256;
const SAMPLE_CAPACITY: usize = 128;
/// Header written at the start of a dump.
const CSV_HEADER: &str = "time_s,bt,et,heater,fan,target,ror";

/// Data for a single log sample.
#[derive(Debug, Clone, Copy)]
pub struct LogSampleData {
    /// Elapsed seconds since roast start.
    pub elapsed_secs: u32,
    /// Bean temperature (°C).
    pub bt: f32,
    /// Environment temperature (°C).
    pub et: f32,
    /// Heater output (0-100%).
    pub heater: f32,
    /// Fan output (0-100%).
    pub fan: f32,
    /// Target temperature (°C).
    pub target: f32,
    /// Rate of rise (°C/min).
    pub ror: f32,
}

static ROAST_LOGGER: Mutex<RefCell<RoastLogger>> =
    Mutex::new(RefCell::new(RoastLogger::new_empty()));

/// Start logging a new roast.
pub fn start_roast(now: Instant) {
    critical_section::with(|cs| ROAST_LOGGER.borrow(cs).borrow_mut().start_roast(now));
}

/// Stop logging.
pub fn stop_roast() {
    critical_section::with(|cs| ROAST_LOGGER.borrow(cs).borrow_mut().stop_roast());
}

/// Log a sample to the ring buffer.
pub fn log_sample(data: LogSampleData) {
    critical_section::with(|cs| {
        ROAST_LOGGER.borrow(cs).borrow_mut().log_sample(data);
    });
}

/// Dump the buffered roast data.
pub fn dump() -> HeaplessString<4096> {
    critical_section::with(|cs| ROAST_LOGGER.borrow(cs).borrow().dump())
}

/// Check if the logger is active.
pub fn is_logging_active() -> bool {
    critical_section::with(|cs| ROAST_LOGGER.borrow(cs).borrow().is_active())
}

pub struct RoastLogger {
    buffer: Deque<HeaplessString<SAMPLE_CAPACITY>, LOG_CAPACITY>,
    active: bool,
}

impl RoastLogger {
    pub fn new() -> Self {
        Self {
            buffer: Deque::new(),
            active: false,
        }
    }

    /// Const-compatible constructor for static initialization.
    pub const fn new_empty() -> Self {
        Self {
            buffer: Deque::new(),
            active: false,
        }
    }

    pub fn start_roast(&mut self, _now: Instant) {
        self.active = true;
        self.buffer.clear();
    }

    pub fn stop_roast(&mut self) {
        self.active = false;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Append a CSV-formatted sample. Oldest sample is evicted if buffer is full.
    pub fn log_sample(&mut self, data: LogSampleData) {
        if !self.active {
            return;
        }
        let mut entry = HeaplessString::<SAMPLE_CAPACITY>::new();
        let _ = core::fmt::Write::write_fmt(
            &mut entry,
            core::format_args!(
                "{},{:.1},{:.1},{:.1},{:.1},{:.1},{:.1}",
                data.elapsed_secs,
                data.bt,
                data.et,
                data.heater,
                data.fan,
                data.target,
                data.ror
            ),
        );
        if self.buffer.len() >= LOG_CAPACITY {
            let _ = self.buffer.pop_front();
        }
        let _ = self.buffer.push_back(entry);
    }

    /// Dump all buffered samples as a CSV string with header row.
    /// Returns a string prefixed with `#DUMP` followed by header and data rows.
    pub fn dump(&self) -> HeaplessString<4096> {
        let mut out = HeaplessString::<4096>::new();
        let _ = out.push_str("#DUMP ");
        let _ = out.push_str(CSV_HEADER);
        let _ = out.push('\n');
        // Collect from front (oldest) to back (newest)
        let (front, back) = self.buffer.as_slices();
        for entry in front.iter().chain(back.iter()) {
            if out.len() + entry.len() + 1 > out.capacity() {
                break;
            }
            let _ = out.push_str(entry.as_str());
            let _ = out.push('\n');
        }
        out
    }

    /// Number of samples currently buffered.
    pub fn sample_count(&self) -> usize {
        self.buffer.len()
    }
}

impl Default for RoastLogger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_and_dump_returns_csv() {
        let mut logger = RoastLogger::new();
        logger.start_roast(Instant::now());
        logger.log_sample(LogSampleData {
            elapsed_secs: 0,
            bt: 50.0,
            et: 40.0,
            heater: 0.0,
            fan: 20.0,
            target: 200.0,
            ror: 0.0,
        });
        logger.log_sample(LogSampleData {
            elapsed_secs: 1,
            bt: 52.0,
            et: 42.0,
            heater: 30.0,
            fan: 25.0,
            target: 200.0,
            ror: 2.0,
        });

        let dump = logger.dump();
        assert!(dump.starts_with("#DUMP time_s,bt,et"));
        assert!(dump.contains("0,50.0,40.0,0.0,20.0,200.0,0.0"));
        assert!(dump.contains("1,52.0,42.0,30.0,25.0,200.0,2.0"));
        assert_eq!(logger.sample_count(), 2);
    }

    #[test]
    fn buffer_wraps_when_full() {
        let mut logger = RoastLogger::new();
        logger.start_roast(Instant::now());
        for i in 0..300u32 {
            logger.log_sample(LogSampleData {
                elapsed_secs: i,
                bt: 100.0,
                et: 90.0,
                heater: 50.0,
                fan: 30.0,
                target: 200.0,
                ror: 0.0,
            });
        }
        assert_eq!(logger.sample_count(), LOG_CAPACITY);
    }

    #[test]
    fn inactive_logger_ignores_samples() {
        let mut logger = RoastLogger::new();
        logger.log_sample(LogSampleData {
            elapsed_secs: 0,
            bt: 50.0,
            et: 40.0,
            heater: 0.0,
            fan: 20.0,
            target: 200.0,
            ror: 0.0,
        });
        assert_eq!(logger.sample_count(), 0);
    }

    #[test]
    fn stop_roast_deactivates() {
        let mut logger = RoastLogger::new();
        logger.start_roast(Instant::now());
        assert!(logger.is_active());
        logger.stop_roast();
        assert!(!logger.is_active());
    }
}
