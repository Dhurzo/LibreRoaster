/// First-crack acoustic detector for coffee roasting.
/// Uses RMS energy analysis on ADC samples to detect cracking sounds.
/// Reports crack events via the Artisan output channel.
use embassy_time::Instant;
use heapless::Deque;

const ENERGY_WINDOW: usize = 50;
const DEBOUNCE_MS: u64 = 3000;
const CRACK_THRESHOLD_SIGMA: f32 = 3.0;

#[derive(Debug, Clone)]
pub struct CrackEvent {
    pub timestamp: Instant,
    pub energy: f32,
}

pub struct AcousticDetector {
    energy_history: Deque<f32, ENERGY_WINDOW>,
    mean_energy: f32,
    variance: f32,
    sample_count: u32,
    last_crack_time: Option<Instant>,
    crack_count: u32,
    pub first_crack_detected: bool,
}

impl AcousticDetector {
    pub const fn new() -> Self {
        Self {
            energy_history: Deque::new(),
            mean_energy: 0.0,
            variance: 0.0,
            sample_count: 0,
            last_crack_time: None,
            crack_count: 0,
            first_crack_detected: false,
        }
    }

    pub fn reset(&mut self) {
        self.energy_history.clear();
        self.mean_energy = 0.0;
        self.variance = 0.0;
        self.sample_count = 0;
        self.last_crack_time = None;
        self.crack_count = 0;
        self.first_crack_detected = false;
    }

    /// Feed an ADC sample value (0-4095). Returns Some(CrackEvent) if a crack is detected.
    /// This is O(1) — uses Welford's online algorithm for mean/variance.
    pub fn feed_sample(&mut self, adc_value: u16, now: Instant) -> Option<CrackEvent> {
        let energy = (adc_value as f32 / 4095.0) * (adc_value as f32 / 4095.0);

        self.sample_count = self.sample_count.saturating_add(1);
        let n = self.sample_count.min(4096) as f32;
        let delta = energy - self.mean_energy;
        self.mean_energy += delta / n;
        let delta2 = energy - self.mean_energy;
        self.variance = self.variance + delta * delta2;

        if self.energy_history.len() >= ENERGY_WINDOW {
            let _ = self.energy_history.pop_front();
        }
        let _ = self.energy_history.push_back(energy);

        if self.energy_history.len() < 10 {
            return None;
        }

        let std_dev = if self.variance > 0.0 && self.sample_count > 1 {
            libm::sqrtf(self.variance / (self.sample_count - 1) as f32)
        } else {
            return None;
        };

        if energy <= self.mean_energy + CRACK_THRESHOLD_SIGMA * std_dev {
            return None;
        }

        if let Some(last) = self.last_crack_time {
            let elapsed = now.duration_since(last).as_millis() as u64;
            if elapsed < DEBOUNCE_MS && !self.first_crack_detected {
                self.first_crack_detected = true;
            }
        }

        self.last_crack_time = Some(now);
        self.crack_count = self.crack_count.saturating_add(1);

        Some(CrackEvent {
            timestamp: now,
            energy,
        })
    }
}

impl Default for AcousticDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_returns_none() {
        let mut det = AcousticDetector::new();
        let now = Instant::now();
        for _ in 0..5 {
            assert!(det.feed_sample(2000, now).is_none());
        }
    }

    #[test]
    fn spike_after_warmup_detects_crack() {
        let mut det = AcousticDetector::new();
        let now = Instant::now();
        for _ in 0..20 {
            det.feed_sample(2000, now);
        }
        let event = det.feed_sample(4000, now);
        assert!(event.is_some());
    }

    #[test]
    fn reset_clears_state() {
        let mut det = AcousticDetector::new();
        let now = Instant::now();
        for _ in 0..20 {
            det.feed_sample(2000, now);
        }
        let _ = det.feed_sample(4000, now);
        det.reset();
        assert!(!det.first_crack_detected);
        assert_eq!(det.crack_count, 0);
    }

    /// No microphone connected → ADC reads 0 (silence). Should not detect cracks.
    #[test]
    fn no_microphone_silence_returns_none() {
        let mut det = AcousticDetector::new();
        let now = Instant::now();
        // Warm up with silence
        for _ in 0..50 {
            assert!(det.feed_sample(0, now).is_none());
        }
        assert_eq!(det.crack_count, 0);
        assert!(!det.first_crack_detected);
    }

    /// Constant ADC value → no variance → no cracks detected.
    #[test]
    fn constant_signal_returns_none() {
        let mut det = AcousticDetector::new();
        let now = Instant::now();
        for _ in 0..50 {
            assert!(det.feed_sample(2048, now).is_none());
        }
        assert!(!det.first_crack_detected);
    }

    /// Random noise without sharp spikes → no false positives.
    #[test]
    fn noise_without_spikes_returns_none() {
        let mut det = AcousticDetector::new();
        let now = Instant::now();
        // Simulate background noise: small random variations around 2000
        for i in 0..50u16 {
            let noise = 2000u16.wrapping_add(i.wrapping_mul(7) % 100).min(3000);
            assert!(det.feed_sample(noise, now).is_none());
        }
        assert!(!det.first_crack_detected);
    }

    /// Single spike → one crack, but first_crack not confirmed (needs second within 3s).
    #[test]
    fn single_spike_not_first_crack() {
        let mut det = AcousticDetector::new();
        let now = Instant::now();
        for _ in 0..30 {
            det.feed_sample(2000, now);
        }
        let event = det.feed_sample(4000, now);
        assert!(event.is_some());
        assert!(!det.first_crack_detected); // Needs second crack to confirm
        assert_eq!(det.crack_count, 1);
    }
}
