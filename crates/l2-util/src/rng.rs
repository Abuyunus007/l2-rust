//! The world's random number generator (D-13).
//!
//! Java: `commons/util/Rnd.java` (static `ThreadLocalRandom` wrappers).
//!
//! Game logic must draw every random value from one [`WorldRng`] owned by the world, never from
//! `rand::rng()` or other thread-local sources. Production seeds it from the OS; tests use
//! [`WorldRng::from_seed`] so replays are reproducible. The *sequence* of values differs from
//! Java (different algorithm); only the ranges and edge cases match.
//!
//! | Java `Rnd`               | `WorldRng`                     |
//! |--------------------------|--------------------------------|
//! | `get(int bound)`         | [`WorldRng::below`]            |
//! | `get(int origin, bound)` | [`WorldRng::get`] (inclusive)  |
//! | `get(long ...)`          | [`WorldRng::below_i64`], [`WorldRng::get_i64`] |
//! | `get(double ...)`        | [`WorldRng::below_f64`], [`WorldRng::get_f64`] |
//! | `nextInt/Long/Double/Float/Boolean/Bytes/Gaussian` | `next_*` |

use rand::{Rng, RngExt, SeedableRng};
use rand_chacha::ChaCha12Rng;

/// Seedable RNG with the same range semantics as Java `Rnd`.
#[derive(Debug, Clone)]
pub struct WorldRng {
    inner: ChaCha12Rng,
    /// Second value of the last polar-method pair, like Java's cached `nextGaussian`.
    spare_gaussian: Option<f64>,
}

impl WorldRng {
    /// A reproducible generator. Use in tests and replays.
    pub fn from_seed(seed: u64) -> Self {
        Self {
            inner: ChaCha12Rng::seed_from_u64(seed),
            spare_gaussian: None,
        }
    }

    /// A generator seeded from the operating system. Use in production.
    pub fn from_entropy() -> Self {
        Self {
            inner: ChaCha12Rng::from_rng(&mut rand::rng()),
            spare_gaussian: None,
        }
    }

    pub fn next_bool(&mut self) -> bool {
        self.inner.random()
    }

    pub fn next_bytes(&mut self, dest: &mut [u8]) {
        self.inner.fill_bytes(dest);
    }

    /// Any `i32`, like Java `nextInt()`.
    pub fn next_i32(&mut self) -> i32 {
        self.inner.random()
    }

    /// Any `i64`, like Java `nextLong()`.
    pub fn next_i64(&mut self) -> i64 {
        self.inner.random()
    }

    /// `[0, 1)`, like Java `nextFloat()`.
    pub fn next_f32(&mut self) -> f32 {
        self.inner.random()
    }

    /// `[0, 1)`, like Java `nextDouble()`.
    pub fn next_f64(&mut self) -> f64 {
        self.inner.random()
    }

    /// Java `Rnd.get(int bound)`: `[0, bound)`, and `0` when `bound <= 0`.
    pub fn below(&mut self, bound: i32) -> i32 {
        if bound <= 0 {
            0
        } else {
            self.inner.random_range(0..bound)
        }
    }

    /// Java `Rnd.get(int origin, int bound)`: **inclusive** `[origin, bound]`.
    ///
    /// Returns `origin` when `origin >= bound`. When `bound == i32::MAX` the range is
    /// `[origin, bound)`, as in Java (it can't add 1 without overflowing).
    pub fn get(&mut self, origin: i32, bound: i32) -> i32 {
        if origin >= bound {
            origin
        } else if bound == i32::MAX {
            self.inner.random_range(origin..bound)
        } else {
            self.inner.random_range(origin..=bound)
        }
    }

    /// Java `Rnd.get(long bound)`: `[0, bound)`, and `0` when `bound <= 0`.
    pub fn below_i64(&mut self, bound: i64) -> i64 {
        if bound <= 0 {
            0
        } else {
            self.inner.random_range(0..bound)
        }
    }

    /// Java `Rnd.get(long origin, long bound)`: inclusive, same edge cases as [`WorldRng::get`].
    pub fn get_i64(&mut self, origin: i64, bound: i64) -> i64 {
        if origin >= bound {
            origin
        } else if bound == i64::MAX {
            self.inner.random_range(origin..bound)
        } else {
            self.inner.random_range(origin..=bound)
        }
    }

    /// Java `Rnd.get(double bound)`: `[0, bound)`, and `0.0` when `bound <= 0`.
    pub fn below_f64(&mut self, bound: f64) -> f64 {
        if bound <= 0.0 {
            0.0
        } else {
            self.inner.random_range(0.0..bound)
        }
    }

    /// Java `Rnd.get(double origin, double bound)`: `[origin, bound]`, and `origin` when
    /// `origin >= bound`. (Java widens the bound by the smallest double; inclusive is the same.)
    pub fn get_f64(&mut self, origin: f64, bound: f64) -> f64 {
        if origin >= bound {
            origin
        } else {
            self.inner.random_range(origin..=bound)
        }
    }

    /// Standard normal distribution (mean 0, deviation 1), like Java `nextGaussian()`.
    /// Uses the Marsaglia polar method.
    pub fn next_gaussian(&mut self) -> f64 {
        if let Some(spare) = self.spare_gaussian.take() {
            return spare;
        }
        loop {
            let u = self.inner.random_range(-1.0..1.0_f64);
            let v = self.inner.random_range(-1.0..1.0_f64);
            let s = u * u + v * v;
            if s > 0.0 && s < 1.0 {
                let factor = (-2.0 * s.ln() / s).sqrt();
                self.spare_gaussian = Some(v * factor);
                return u * factor;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn rng_is_deterministic_with_seed() {
        let mut a = WorldRng::from_seed(42);
        let mut b = WorldRng::from_seed(42);
        let xs: Vec<i32> = (0..100).map(|_| a.next_i32()).collect();
        let ys: Vec<i32> = (0..100).map(|_| b.next_i32()).collect();
        assert_eq!(xs, ys);
    }

    #[test]
    fn different_seeds_differ() {
        let mut a = WorldRng::from_seed(1);
        let mut b = WorldRng::from_seed(2);
        let xs: Vec<i32> = (0..10).map(|_| a.next_i32()).collect();
        let ys: Vec<i32> = (0..10).map(|_| b.next_i32()).collect();
        assert_ne!(xs, ys);
    }

    #[test]
    fn rng_get_is_inclusive() {
        let mut rng = WorldRng::from_seed(7);
        let mut seen = [false; 3];
        for _ in 0..1_000 {
            let v = rng.get(1, 3);
            assert!((1..=3).contains(&v));
            seen[(v - 1) as usize] = true;
        }
        assert_eq!(seen, [true, true, true], "both ends must be reachable");
    }

    #[test]
    fn java_edge_cases() {
        let mut rng = WorldRng::from_seed(7);
        assert_eq!(rng.below(0), 0);
        assert_eq!(rng.below(-5), 0);
        assert_eq!(rng.get(5, 5), 5);
        assert_eq!(rng.get(9, 3), 9);
        assert_eq!(rng.below_i64(0), 0);
        assert_eq!(rng.get_i64(4, 4), 4);
        assert_eq!(rng.below_f64(0.0), 0.0);
        assert_eq!(rng.get_f64(2.5, 1.0), 2.5);
        // Must not overflow at the top of the range.
        let v = rng.get(i32::MAX - 1, i32::MAX);
        assert_eq!(v, i32::MAX - 1);
    }

    #[test]
    fn gaussian_is_roughly_standard() {
        let mut rng = WorldRng::from_seed(3);
        let n = 20_000;
        let samples: Vec<f64> = (0..n).map(|_| rng.next_gaussian()).collect();
        let mean = samples.iter().sum::<f64>() / f64::from(n);
        let var = samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / f64::from(n);
        assert!(mean.abs() < 0.05, "mean {mean}");
        assert!((var - 1.0).abs() < 0.05, "variance {var}");
    }

    proptest! {
        #[test]
        fn below_stays_in_range(seed: u64, bound in 1..i32::MAX) {
            let mut rng = WorldRng::from_seed(seed);
            let v = rng.below(bound);
            prop_assert!((0..bound).contains(&v));
        }

        #[test]
        fn get_stays_in_range(seed: u64, a: i32, b: i32) {
            let mut rng = WorldRng::from_seed(seed);
            let v = rng.get(a, b);
            if a >= b { prop_assert_eq!(v, a); } else { prop_assert!(v >= a && v <= b); }
        }
    }
}
