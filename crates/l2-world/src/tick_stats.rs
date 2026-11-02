//! How long the world's ticks really take (M9-H02): a ring of the last samples, read by the
//! admin endpoint. The nominal tick is 50 ms (`world::TICK`); this is the measured time of
//! `World::tick`, so a load test can see when the world falls behind.

use std::time::Duration;

/// Samples kept: the last minute at 20 ticks per second.
pub const TICK_SAMPLES: usize = 1200;

/// A ring of tick durations in microseconds.
#[derive(Debug, Clone, Default)]
pub struct TickStats {
    micros: Vec<u32>,
    next: usize,
}

impl TickStats {
    /// Adds one sample; the oldest one goes once [`TICK_SAMPLES`] are stored.
    pub fn record(&mut self, took: Duration) {
        let micros = u32::try_from(took.as_micros()).unwrap_or(u32::MAX);
        if self.micros.len() < TICK_SAMPLES {
            self.micros.push(micros);
        } else {
            self.micros[self.next] = micros;
            self.next = (self.next + 1) % TICK_SAMPLES;
        }
    }

    /// How many samples are stored.
    pub fn samples(&self) -> u32 {
        u32::try_from(self.micros.len()).unwrap_or(u32::MAX)
    }

    /// The mean of the stored samples in microseconds (0 without samples).
    pub fn average_us(&self) -> u32 {
        if self.micros.is_empty() {
            return 0;
        }
        let sum: u64 = self.micros.iter().map(|&m| u64::from(m)).sum();
        u32::try_from(sum / self.micros.len() as u64).unwrap_or(u32::MAX)
    }

    /// The 99th percentile (nearest rank) in microseconds (0 without samples).
    pub fn p99_us(&self) -> u32 {
        if self.micros.is_empty() {
            return 0;
        }
        let mut sorted = self.micros.clone();
        sorted.sort_unstable();
        let rank = (sorted.len() * 99).div_ceil(100).max(1);
        sorted[rank - 1]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_zero() {
        let stats = TickStats::default();
        assert_eq!((stats.samples(), stats.average_us(), stats.p99_us()), (0, 0, 0));
    }

    #[test]
    fn average_and_p99_of_one_to_hundred_ms() {
        let mut stats = TickStats::default();
        for ms in 1..=100u64 {
            stats.record(Duration::from_millis(ms));
        }
        assert_eq!(stats.samples(), 100);
        assert_eq!(stats.average_us(), 50_500);
        // Nearest rank: ceil(0.99 * 100) = the 99th sample.
        assert_eq!(stats.p99_us(), 99_000);
    }

    #[test]
    fn keeps_only_the_last_samples() {
        let mut stats = TickStats::default();
        for _ in 0..TICK_SAMPLES {
            stats.record(Duration::from_millis(500));
        }
        for _ in 0..TICK_SAMPLES {
            stats.record(Duration::from_millis(10));
        }
        assert_eq!(stats.samples() as usize, TICK_SAMPLES);
        assert_eq!(stats.p99_us(), 10_000);
    }
}
