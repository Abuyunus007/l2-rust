//! What the bots count and time. Shared by all bot tasks and the sampler: this is tool state, not
//! game state, so atomics and one mutex are fine here (D-10 is about the game server's objects).

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

/// Longest error text kept as a key of the error table.
const MAX_ERROR_CHARS: usize = 120;

/// The durations of one kind of step, in microseconds.
#[derive(Debug, Default)]
struct Timings {
    connect: Vec<u32>,
    login: Vec<u32>,
    game: Vec<u32>,
    enter: Vec<u32>,
}

/// Counters and timings of a run.
#[derive(Debug, Default)]
pub struct Stats {
    /// Bots that were started.
    pub started: AtomicU32,
    /// Bots that got `PlayOk` (the whole login server part).
    pub logged_in: AtomicU32,
    /// Bots in the world right now.
    pub in_world: AtomicU32,
    /// The most bots that were in the world at once.
    pub peak_in_world: AtomicU32,
    /// Times a bot entered the world (restarts enter again).
    pub entered: AtomicU32,
    /// Restarts to the character screen that completed.
    pub restarts: AtomicU32,
    /// Bots that ended with an error or a panic.
    pub errors: AtomicU32,
    /// Bots the server disconnected while they played.
    pub disconnects: AtomicU32,
    /// Game packets received while in the world / sent by the bots in the world.
    pub packets_rx: AtomicU64,
    pub packets_tx: AtomicU64,
    timings: Mutex<Timings>,
    error_texts: Mutex<BTreeMap<String, u32>>,
}

/// min / mean / median / 99th percentile / max of a list, in microseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub count: usize,
    pub min: u32,
    pub avg: u32,
    pub p50: u32,
    pub p99: u32,
    pub max: u32,
}

impl Summary {
    /// `None` for an empty list. Percentiles use the nearest rank.
    pub fn of(values: &[u32]) -> Option<Self> {
        if values.is_empty() {
            return None;
        }
        let mut sorted = values.to_vec();
        sorted.sort_unstable();
        let rank = |percent: usize| sorted[(sorted.len() * percent).div_ceil(100).max(1) - 1];
        let sum: u64 = sorted.iter().map(|&v| u64::from(v)).sum();
        Some(Self {
            count: sorted.len(),
            min: sorted[0],
            avg: u32::try_from(sum / sorted.len() as u64).unwrap_or(u32::MAX),
            p50: rank(50),
            p99: rank(99),
            max: sorted[sorted.len() - 1],
        })
    }
}

/// The timings of a finished run.
#[derive(Debug, Clone, Default)]
pub struct TimingSummaries {
    /// TCP connect to the login server until `Init` arrived.
    pub connect: Option<Summary>,
    /// Connect until `PlayOk`: connect, GameGuard, login, server list, server login.
    pub login: Option<Summary>,
    /// Game server connect until the character list arrived (handshake, `AuthLogin`).
    pub game: Option<Summary>,
    /// `CharacterSelect` until the last `EnterWorld` packet.
    pub enter: Option<Summary>,
}

fn micros(d: Duration) -> u32 {
    u32::try_from(d.as_micros()).unwrap_or(u32::MAX)
}

impl Stats {
    pub fn record_connect(&self, took: Duration) {
        if let Ok(mut t) = self.timings.lock() {
            t.connect.push(micros(took));
        }
    }

    pub fn record_login(&self, took: Duration) {
        self.logged_in.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut t) = self.timings.lock() {
            t.login.push(micros(took));
        }
    }

    pub fn record_game(&self, took: Duration) {
        if let Ok(mut t) = self.timings.lock() {
            t.game.push(micros(took));
        }
    }

    pub fn record_enter(&self, took: Duration) {
        self.entered.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut t) = self.timings.lock() {
            t.enter.push(micros(took));
        }
    }

    /// A bot is in the world now.
    pub fn world_entered(&self) {
        let now = self.in_world.fetch_add(1, Ordering::Relaxed) + 1;
        self.peak_in_world.fetch_max(now, Ordering::Relaxed);
    }

    /// A bot left the world (restart, end of the run or a disconnect).
    pub fn world_left(&self) {
        // Never below zero, whatever the order of calls.
        let mut current = self.in_world.load(Ordering::Relaxed);
        while current > 0 {
            match self.in_world.compare_exchange_weak(
                current,
                current - 1,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current = actual,
            }
        }
    }

    /// Counts one failed bot and remembers why (the text is cut, equal texts are counted).
    pub fn record_error(&self, text: &str) {
        self.errors.fetch_add(1, Ordering::Relaxed);
        let key: String = text
            .lines()
            .next()
            .unwrap_or("")
            .chars()
            .take(MAX_ERROR_CHARS)
            .collect();
        if let Ok(mut errors) = self.error_texts.lock() {
            *errors.entry(key).or_insert(0) += 1;
        }
    }

    /// The error texts with their counts, most frequent first.
    pub fn error_table(&self) -> Vec<(String, u32)> {
        let mut table: Vec<_> = self
            .error_texts
            .lock()
            .map(|errors| errors.iter().map(|(k, v)| (k.clone(), *v)).collect())
            .unwrap_or_default();
        table.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        table
    }

    pub fn timing_summaries(&self) -> TimingSummaries {
        let Ok(t) = self.timings.lock() else {
            return TimingSummaries::default();
        };
        TimingSummaries {
            connect: Summary::of(&t.connect),
            login: Summary::of(&t.login),
            game: Summary::of(&t.game),
            enter: Summary::of(&t.enter),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_of_one_to_hundred() {
        let values: Vec<u32> = (1..=100).collect();
        let s = Summary::of(&values).unwrap();
        assert_eq!(
            (s.count, s.min, s.avg, s.p50, s.p99, s.max),
            (100, 1, 50, 50, 99, 100)
        );
        assert_eq!(Summary::of(&[]), None);
        let one = Summary::of(&[7]).unwrap();
        assert_eq!((one.min, one.p50, one.p99, one.max), (7, 7, 7, 7));
    }

    #[test]
    fn world_gauge_tracks_the_peak_and_never_goes_negative() {
        let stats = Stats::default();
        stats.world_entered();
        stats.world_entered();
        stats.world_left();
        stats.world_entered();
        assert_eq!(stats.in_world.load(Ordering::Relaxed), 2);
        assert_eq!(stats.peak_in_world.load(Ordering::Relaxed), 2);
        stats.world_left();
        stats.world_left();
        stats.world_left();
        assert_eq!(stats.in_world.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn errors_are_grouped_by_their_first_line() {
        let stats = Stats::default();
        stats.record_error("login failed\nbacktrace...");
        stats.record_error("login failed");
        stats.record_error("timed out");
        assert_eq!(stats.errors.load(Ordering::Relaxed), 3);
        assert_eq!(
            stats.error_table(),
            [("login failed".to_owned(), 2), ("timed out".to_owned(), 1)]
        );
    }

    #[test]
    fn timings_are_summarised() {
        let stats = Stats::default();
        stats.record_connect(Duration::from_millis(2));
        stats.record_login(Duration::from_millis(10));
        stats.record_login(Duration::from_millis(30));
        let t = stats.timing_summaries();
        assert_eq!(t.connect.unwrap().avg, 2_000);
        assert_eq!(t.login.unwrap().max, 30_000);
        assert_eq!(stats.logged_in.load(Ordering::Relaxed), 2);
        assert!(t.game.is_none() && t.enter.is_none());
    }
}
