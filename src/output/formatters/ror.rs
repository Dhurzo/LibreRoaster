// ROR (Rate of Rise) calculator for Artisan protocol
//
// This module handles ROR calculation using BT temperature history.
// All operations use heapless types to ensure predictable memory usage.
//
// # Memory Strategy
//
// - `BT_HISTORY_SIZE`: Fixed history for BT temperature tracking (5 samples)
// - `ROR_FILTER_ALPHA`: Filter coefficient for smoothing (0.3)
// - `ROR_MIN_SAMPLES`: Minimum samples required for ROR calculation (2)
//
// ## Memory Usage
//
// - BT history: BT_HISTORY_SIZE × sizeof(f32) bytes
// - No dynamic memory allocation during calculation

use crate::memory::{BT_HISTORY_SIZE, ROR_MIN_SAMPLES};
use heapless::Deque;

/// ROR calculator with history tracking
#[derive(Debug, Clone)]
pub struct RorCalculator {
    history: Deque<f32, BT_HISTORY_SIZE>,
}

impl RorCalculator {
    /// Create a new ROR calculator with empty history
    pub fn new() -> Self {
        Self {
            history: Deque::new(),
        }
    }

    /// Update BT history and calculate current ROR
    ///
    /// This method:
    /// 1. Adds the new BT temperature to history
    /// 2. Maintains a fixed-size history (removes oldest if full)
    /// 3. Calculates ROR as (current - oldest) / (samples - 1)
    ///
    /// # Arguments
    ///
    /// * `current_bt` - Latest BT temperature reading
    ///
    /// # Returns
    ///
    /// Current ROR value in °C/s, or 0.0 if insufficient history
    pub fn calculate_ror(&mut self, current_bt: f32) -> f32 {
        Self::update_bt_history(&mut self.history, current_bt);
        self.compute_queued_ror()
    }

    /// Update BT history with new temperature sample
    ///
    /// Removes oldest sample if history is full, then adds new sample
    ///
    /// # Arguments
    ///
    /// * `history` - Mutable reference to BT history deque
    /// * `current_bt` - New BT temperature to add
    fn update_bt_history(history: &mut Deque<f32, BT_HISTORY_SIZE>, current_bt: f32) {
        if history.len() >= BT_HISTORY_SIZE {
            let _ = history.pop_front();
        }
        let _ = history.push_back(current_bt);
    }

    /// Calculate ROR from BT history
    ///
    /// ROR = (BT_current - BT_oldest) / (time_elapsed)
    ///
    /// Assuming 1-second intervals between samples:
    /// ROR = (last_bt - first_bt) / (samples - 1)
    ///
    /// # Arguments
    ///
    /// * `history` - Reference to BT history
    ///
    /// # Returns
    ///
    /// ROR value in °C/s, or 0.0 if insufficient samples
    #[allow(dead_code)]
    fn compute_ror_from_history(history: &[f32]) -> f32 {
        if history.len() < ROR_MIN_SAMPLES {
            return 0.0;
        }

        let samples = history.len();
        let first_bt = history[0];
        let last_bt = history[samples - 1];

        (last_bt - first_bt) / (samples as f32 - 1.0)
    }

    fn compute_queued_ror(&self) -> f32 {
        let (front, back) = self.history.as_slices();
        let combined_len = front.len() + back.len();
        if combined_len < ROR_MIN_SAMPLES {
            return 0.0;
        }

        let mut combined = [0.0f32; BT_HISTORY_SIZE];
        for (i, &v) in front.iter().enumerate() {
            combined[i] = v;
        }
        for (i, &v) in back.iter().enumerate() {
            combined[front.len() + i] = v;
        }

        Self::compute_ror_from_history(&combined[..combined_len])
    }

    /// Get current history length
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Get current history as a slice
    pub fn get_history(&self) -> &[f32] {
        self.history.as_slices().0
    }

    /// Reset ROR calculator (clear history)
    pub fn reset(&mut self) {
        self.history.clear();
    }
}

impl Default for RorCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_ror_insufficient_history() {
        let mut calc = RorCalculator::new();
        let ror = calc.calculate_ror(100.0);
        assert_eq!(ror, 0.0);
    }

    #[test]
    fn test_calculate_ror_two_samples() {
        let mut calc = RorCalculator::new();
        calc.calculate_ror(100.0);
        let ror = calc.calculate_ror(110.0);
        assert_eq!(ror, 10.0);
    }

    #[test]
    fn test_calculate_ror_multiple_samples() {
        let mut calc = RorCalculator::new();
        calc.calculate_ror(100.0);
        calc.calculate_ror(105.0);
        calc.calculate_ror(110.0);
        calc.calculate_ror(115.0);
        let ror = calc.calculate_ror(120.0);
        // (120 - 100) / 4 = 5.0
        assert_eq!(ror, 5.0);
    }

    #[test]
    fn test_calculate_ror_full_history() {
        let mut calc = RorCalculator::new();
        // Fill history to capacity
        for i in 0..BT_HISTORY_SIZE {
            let _ = calc.calculate_ror(100.0 + (i as f32) * 5.0);
        }
        // Add one more (should push out oldest)
        let _ = calc.calculate_ror(100.0 + (BT_HISTORY_SIZE as f32) * 5.0 + 10.0);
        // Check history length
        assert_eq!(calc.history_len(), BT_HISTORY_SIZE);
    }

    #[test]
    fn test_reset() {
        let mut calc = RorCalculator::new();
        calc.calculate_ror(100.0);
        calc.calculate_ror(105.0);
        calc.calculate_ror(110.0);
        calc.reset();
        assert_eq!(calc.history_len(), 0);
        assert_eq!(calc.calculate_ror(100.0), 0.0);
    }
}
