/// Ring-buffer roast data logger — backup recorder for Artisan disconnects.
/// Stores up to LOG_CAPACITY CSV samples in a heapless ring buffer.
/// On reconnect, Artisan can send `#DUMP` to retrieve buffered data.
use core::cell::RefCell;
use critical_section::Mutex;
use embassy_time::Instant;
use heapless::{Deque, String as HeaplessString};

/// Capacity of the ring buffer in samples. Bug V2-7: exposed as `pub` so the
/// `RoasterControl.dump_pending` deque can be sized to the same number of
/// rows (the queue must hold a full-ring dump without losing any row).
pub const LOG_CAPACITY: usize = 256;
const SAMPLE_CAPACITY: usize = 128;
/// Header written at the start of a dump.
const CSV_HEADER: &str = "time_s,bt,et,heater,fan,target,ror";
// F3.6 (Gap #1): aggregate dump buffer. Was 4096 (truncated long roasts);
// bumped to 8192 so a full 256-sample ring (~32 KB worst case) is unlikely to
// fit, but typical 10-15 min roasts at 1 Hz fit comfortably with headroom.
// Per-row truncation still happens in `handle_dump_log` via `TRACE_EVENT_MAX_LEN`.
pub const DUMP_BUFFER_SIZE: usize = 8192;

/// Data for a single log sample.
#[derive(Debug, Clone, Copy)]
pub struct LogSampleData {
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
    /// Rate of rise (display scale: °C/min or °F/min — Bug DRA-1: the caller
    /// converts from internal °C/s using the active display scale).
    pub ror: f32,
}

static ROAST_LOGGER: Mutex<RefCell<RoastLogger>> =
    Mutex::new(RefCell::new(RoastLogger::new_empty()));

/// Start logging a new roast.
///
/// Bug V2-8: the epoch is now owned by the logger. The previous design took
/// `now: Instant` and discarded it, then relied on a per-task `roast_start`
/// captured from the *continuous-telemetry rising edge* (which also fires on
/// manual `OT1`/`OT2`) and never reset it between roasts — a second roast on
/// the same boot inherited the first roast's uptime as its `time_s` base.
/// Storing `start` here means the epoch is fixed exactly when START happens
/// (the only caller is `handle_start_roast`) and is reset on every START.
pub fn start_roast(now: Instant) {
    critical_section::with(|cs| ROAST_LOGGER.borrow(cs).borrow_mut().start_roast(now));
}

/// Stop logging.
pub fn stop_roast() {
    critical_section::with(|cs| ROAST_LOGGER.borrow(cs).borrow_mut().stop_roast());
}

/// Log a sample to the ring buffer. Bug V2-8: the sample's `time_s` column is
/// derived from the logger's own epoch (`start`) and the supplied `now`,
/// NOT from a caller-provided `elapsed_secs`. The caller (the telemetry task)
/// no longer owns the time base.
pub fn log_sample(data: LogSampleData, now: Instant) {
    critical_section::with(|cs| {
        ROAST_LOGGER.borrow(cs).borrow_mut().log_sample(data, now);
    });
}

/// Dump the buffered roast data.
pub fn dump() -> HeaplessString<DUMP_BUFFER_SIZE> {
    critical_section::with(|cs| ROAST_LOGGER.borrow(cs).borrow().dump())
}

/// Check if the logger is active.
pub fn is_logging_active() -> bool {
    critical_section::with(|cs| ROAST_LOGGER.borrow(cs).borrow().is_active())
}

pub struct RoastLogger {
    buffer: Deque<HeaplessString<SAMPLE_CAPACITY>, LOG_CAPACITY>,
    active: bool,
    /// Bug V2-8: epoch fixed by `start_roast(now)`. `None` until the first
    /// START; `log_sample` falls back to `0` for samples logged before a
    /// START (defensive — should not happen in practice, since the task only
    /// logs while `active`, which only the START path sets).
    start: Option<Instant>,
}

impl RoastLogger {
    pub fn new() -> Self {
        Self {
            buffer: Deque::new(),
            active: false,
            start: None,
        }
    }

    /// Const-compatible constructor for static initialization.
    pub const fn new_empty() -> Self {
        Self {
            buffer: Deque::new(),
            active: false,
            start: None,
        }
    }

    pub fn start_roast(&mut self, now: Instant) {
        self.active = true;
        self.buffer.clear();
        // Bug V2-8: own the epoch. Every START resets it, so a second roast
        // on the same boot does not inherit the first roast's uptime.
        self.start = Some(now);
    }

    pub fn stop_roast(&mut self) {
        self.active = false;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Append a CSV-formatted sample. Oldest sample is evicted if buffer is full.
    /// Bug V2-8: derive `time_s` from `self.start` and `now`, not the caller.
    /// Bug A1 (2026-07-25): `Instant::duration_since` panics on `now < self.start`
    /// (embassy-time's `Instant` is a saturating-checking `Instant` that
    /// `unwrap!`-s the underlying subtraction). On the tick where `START` is
    /// processed, the control loop's `tick_start` can be slightly EARLIER
    /// than the `Instant::now()` captured inside `start_roast`, producing
    /// `now < self.start` and a panic that locks the duty holding its last
    /// value until the RTC WDT resets the device. Use saturating arithmetic
    /// so out-of-order `now` reads degrade to 0 elapsed instead of panicking.
    pub fn log_sample(&mut self, data: LogSampleData, now: Instant) {
        if !self.active {
            return;
        }
        let elapsed_secs = self
            .start
            .map(|s| now.saturating_duration_since(s).as_secs() as u32)
            .unwrap_or(0);
        let mut entry = HeaplessString::<SAMPLE_CAPACITY>::new();
        let _ = core::fmt::Write::write_fmt(
            &mut entry,
            core::format_args!(
                "{},{:.1},{:.1},{:.1},{:.1},{:.1},{:.1}",
                elapsed_secs,
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
    ///
    /// Bug V2-7(4) / B17 residual: the previous iteration walked
    /// `front.iter().chain(back.iter())` (oldest-first) and `break`-ed when
    /// the output buffer filled — so a long roast lost the **newest** rows
    /// (the end of the roast, the most valuable part). This version first
    /// plans which rows fit by walking **newest → oldest** and accumulating
    /// their lengths, then emits the selected rows in **chronological order**
    /// (oldest-first). The tail of the roast is always preserved at the cost
    /// of the oldest pre-charge samples.
    pub fn dump(&self) -> HeaplessString<DUMP_BUFFER_SIZE> {
        let (front, back) = self.buffer.as_slices();
        let total = front.len() + back.len();

        // Phase 1 — plan which rows fit, newest-first. `indices[0..count]`
        // holds the selected chronological positions in newest-first order.
        // We use a heapless::Vec so the array lives on the stack with a fixed
        // upper bound = LOG_CAPACITY; no allocation.
        //
        // Bug V2-7(4): reserve room for the `#DUMP `+CSV_HEADER+`\n` line the
        // emitter writes BEFORE the rows. Otherwise the plan would select one
        // row too many and the phase-2 emit would silently truncate the last
        // row (the newest — the most important one).
        const HEADER_LEN: usize = 6 /* "#DUMP " */ + CSV_HEADER.len() + 1 /* '\n' */;
        let mut indices: heapless::Vec<usize, LOG_CAPACITY> = heapless::Vec::new();
        let mut accumulated: usize = HEADER_LEN;
        for chrono_pos in (0..total).rev() {
            let entry_len = entry_len_at(chrono_pos, front, back);
            // +1 for the trailing '\n'.
            if accumulated.saturating_add(entry_len).saturating_add(1) > DUMP_BUFFER_SIZE {
                break;
            }
            accumulated = accumulated.saturating_add(entry_len).saturating_add(1);
            if indices.push(chrono_pos).is_err() {
                break;
            }
        }
        let count = indices.len();

        // Build a fast "is selected?" lookup indexed by chronological position.
        let mut in_selected = [false; LOG_CAPACITY];
        for i in 0..count {
            let pos = indices[i];
            if pos < LOG_CAPACITY {
                in_selected[pos] = true;
            }
        }

        // Phase 2 — emit oldest-first, skipping non-selected rows.
        let mut out = HeaplessString::<DUMP_BUFFER_SIZE>::new();
        let _ = out.push_str("#DUMP ");
        let _ = out.push_str(CSV_HEADER);
        let _ = out.push('\n');
        let mut chrono_pos: usize = 0;
        for entry in front.iter().chain(back.iter()) {
            if chrono_pos < LOG_CAPACITY && in_selected[chrono_pos] {
                let _ = out.push_str(entry.as_str());
                let _ = out.push('\n');
            }
            chrono_pos = chrono_pos.saturating_add(1);
        }
        out
    }

    /// Number of samples currently buffered.
    pub fn sample_count(&self) -> usize {
        self.buffer.len()
    }
}

/// Length (bytes) of the entry at chronological position `pos`
/// (0 = oldest). `front` and `back` are the two ring slices.
fn entry_len_at(
    pos: usize,
    front: &[HeaplessString<SAMPLE_CAPACITY>],
    back: &[HeaplessString<SAMPLE_CAPACITY>],
) -> usize {
    // The Deque stores oldest at `front[..]` then `back[..]` (newest).
    if pos < front.len() {
        front[pos].len()
    } else {
        let back_pos = pos - front.len();
        if back_pos < back.len() {
            back[back_pos].len()
        } else {
            0
        }
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

    /// Helper: a generic sample whose individual fields are irrelevant to
    /// the time-base tests. Bug V2-8 moved `elapsed_secs` out of `LogSampleData`
    /// (the logger derives it from `start` + `now`), so this helper takes no
    /// elapsed argument.
    fn sample() -> LogSampleData {
        LogSampleData {
            bt: 100.0,
            et: 90.0,
            heater: 50.0,
            fan: 30.0,
            target: 200.0,
            ror: 0.0,
        }
    }

    #[test]
    fn log_and_dump_returns_csv() {
        let mut logger = RoastLogger::new();
        let t0 = Instant::from_millis(0);
        logger.start_roast(t0);
        logger.log_sample(
            LogSampleData {
                bt: 50.0,
                et: 40.0,
                heater: 0.0,
                fan: 20.0,
                target: 200.0,
                ror: 0.0,
            },
            Instant::from_millis(0),
        );
        logger.log_sample(
            LogSampleData {
                bt: 52.0,
                et: 42.0,
                heater: 30.0,
                fan: 25.0,
                target: 200.0,
                ror: 2.0,
            },
            Instant::from_millis(1000),
        );

        let dump = logger.dump();
        assert!(dump.starts_with("#DUMP time_s,bt,et"));
        assert!(dump.contains("0,50.0,40.0,0.0,20.0,200.0,0.0"));
        assert!(dump.contains("1,52.0,42.0,30.0,25.0,200.0,2.0"));
        assert_eq!(logger.sample_count(), 2);
    }

    #[test]
    fn buffer_wraps_when_full() {
        let mut logger = RoastLogger::new();
        let t0 = Instant::from_millis(0);
        logger.start_roast(t0);
        for i in 0..300u32 {
            logger.log_sample(sample(), Instant::from_millis((i as u64) * 1000));
        }
        assert_eq!(logger.sample_count(), LOG_CAPACITY);
    }

    #[test]
    fn inactive_logger_ignores_samples() {
        let mut logger = RoastLogger::new();
        logger.log_sample(
            LogSampleData {
                bt: 50.0,
                et: 40.0,
                heater: 0.0,
                fan: 20.0,
                target: 200.0,
                ror: 0.0,
            },
            Instant::from_millis(0),
        );
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

    /// Bug V2-8(a): a second roast on the same boot must reset the time base.
    /// The previous design never reset `TickState.roast_start`, so the second
    /// roast's `#DUMP` started at the accumulated uptime (minutes in).
    #[test]
    fn second_roast_resets_epoch() {
        let mut logger = RoastLogger::new();
        // First roast at t=10s, log a couple samples.
        logger.start_roast(Instant::from_millis(10_000));
        logger.log_sample(sample(), Instant::from_millis(11_000));
        logger.log_sample(sample(), Instant::from_millis(12_000));
        logger.stop_roast();

        // Second roast starts much later. Its epoch must be t=100_000ms,
        // not inherited from the first roast.
        logger.start_roast(Instant::from_millis(100_000));
        logger.log_sample(sample(), Instant::from_millis(100_500));
        logger.log_sample(sample(), Instant::from_millis(101_500));

        let dump = logger.dump();
        // The first sample of the second roast must be elapsed=0 (500ms rounds
        // down to 0s), NOT elapsed=90 (which would be the inherited uptime).
        // Find the first data row after the header.
        let first_data_line = dump
            .lines()
            .find(|l| !l.starts_with("#DUMP") && !l.is_empty());
        assert!(
            first_data_line.is_some(),
            "dump must contain at least one data row: {}",
            dump.as_str()
        );
        let line = first_data_line.unwrap();
        assert!(
            line.starts_with("0,"),
            "second roast first sample must have time_s=0, got: {}",
            line
        );
    }

    /// Bug V2-8: the logger computes `time_s` from its own epoch, ignoring any
    /// caller-provided notion of elapsed. A sample logged at now=start+5s must
    /// be tagged `time_s=5` regardless of how the caller may have computed it.
    #[test]
    fn log_sample_uses_internal_epoch() {
        let mut logger = RoastLogger::new();
        let t0 = Instant::from_millis(1_000_000);
        logger.start_roast(t0);
        // now = t0 + 5s exactly.
        logger.log_sample(
            LogSampleData {
                bt: 100.0,
                et: 90.0,
                heater: 50.0,
                fan: 30.0,
                target: 200.0,
                ror: 0.0,
            },
            Instant::from_millis(1_005_000),
        );
        let dump = logger.dump();
        assert!(
            dump.contains("5,100.0,90.0,50.0,30.0,200.0,0.0"),
            "row must use epoch-derived time_s=5: {}",
            dump.as_str()
        );
    }

    /// Bug V2-7(4) / B17: `dump()` used to lose the NEWEST rows when the
    /// output buffer filled (it walked oldest-first and `break`-ed). A long
    /// roast (more rows than fit in DUMP_BUFFER_SIZE) must keep the TAIL —
    /// the end of the roast — at the cost of the oldest pre-charge samples.
    #[test]
    fn dump_preserves_tail_of_long_roast() {
        let mut logger = RoastLogger::new();
        logger.start_roast(Instant::from_millis(0));
        // Log LOG_CAPACITY samples with distinct, monotonic BT values so we
        // can identify which ones survived. Use BT = 100 + i for i in 0..256.
        // Each row at SAMPLE_CAPACITY=128 bytes is ~30 bytes, so 256 rows
        // ≈ 7.5 KiB > DUMP_BUFFER_SIZE (8 KiB minus the header). The exact
        // cut point doesn't matter for this test; what matters is that the
        // NEWEST row (BT=355) is retained and the OLDEST (BT=100) is dropped.
        for i in 0..LOG_CAPACITY as u32 {
            logger.log_sample(
                LogSampleData {
                    bt: 100.0 + i as f32,
                    et: 90.0,
                    heater: 50.0,
                    fan: 30.0,
                    target: 200.0,
                    ror: 0.0,
                },
                Instant::from_millis(i as u64 * 1000),
            );
        }
        let dump = logger.dump();
        // The newest row must be present.
        assert!(
            dump.contains("355.0,90.0"),
            "tail of long roast (newest BT=355) must be preserved: ...{}",
            &dump.as_str()[dump.as_str().len().saturating_sub(200)..]
        );
        // The very oldest row must be dropped (the buffer cannot hold all
        // 256 rows once the header is accounted for).
        assert!(
            !dump.contains("100.0,90.0,50.0,30.0,200.0"),
            "oldest row (BT=100) must be dropped to preserve the tail"
        );
    }
}
