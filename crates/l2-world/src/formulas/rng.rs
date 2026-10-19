//! The random draws the formulas make, as a trait that mirrors Java `Rnd`.
//!
//! Java: `commons/util/Rnd.java`. Formulas take `&mut impl Rnd` (or `&mut dyn Rnd`) and call the
//! same method Java calls, with the same bounds, in the same order. The world's seedable RNG
//! implements it by forwarding to the matching [`WorldRng`] method; tests implement it with the
//! draws recorded in the Java vectors, which also lets them check the order and the bounds.
//!
//! Range rules (same as Java): `get_int(bound)` is `[0, bound)` and `0` when `bound <= 0`;
//! `get_int_range(origin, bound)` is **inclusive** at both ends and `origin` when `origin >= bound`.
//! A call that draws nothing (those edge cases) is not a draw: the vector recorder skips it too.
//! Java's `float` overloads (`get(float)`, `get(float, float)`) are not mirrored: no formula uses
//! them and `WorldRng` has no such range method.

use l2_util::WorldRng;

/// Java `Rnd`, one method per overload the game logic uses.
pub trait Rnd {
    /// `Rnd.nextBoolean()`.
    fn next_boolean(&mut self) -> bool;
    /// `Rnd.nextBytes(bytes)`.
    fn next_bytes(&mut self, bytes: &mut [u8]);
    /// `Rnd.get(int bound)`.
    fn get_int(&mut self, bound: i32) -> i32;
    /// `Rnd.get(int origin, int bound)`, both ends inclusive.
    fn get_int_range(&mut self, origin: i32, bound: i32) -> i32;
    /// `Rnd.nextInt()`.
    fn next_int(&mut self) -> i32;
    /// `Rnd.get(long bound)`.
    fn get_long(&mut self, bound: i64) -> i64;
    /// `Rnd.get(long origin, long bound)`, both ends inclusive.
    fn get_long_range(&mut self, origin: i64, bound: i64) -> i64;
    /// `Rnd.nextLong()`.
    fn next_long(&mut self) -> i64;
    /// `Rnd.nextFloat()`: `[0, 1)`.
    fn next_float(&mut self) -> f32;
    /// `Rnd.get(double bound)`.
    fn get_double(&mut self, bound: f64) -> f64;
    /// `Rnd.get(double origin, double bound)`.
    fn get_double_range(&mut self, origin: f64, bound: f64) -> f64;
    /// `Rnd.nextDouble()`: `[0, 1)`.
    fn next_double(&mut self) -> f64;
    /// `Rnd.nextGaussian()`.
    fn next_gaussian(&mut self) -> f64;
}

impl Rnd for WorldRng {
    fn next_boolean(&mut self) -> bool {
        self.next_bool()
    }

    fn next_bytes(&mut self, bytes: &mut [u8]) {
        WorldRng::next_bytes(self, bytes);
    }

    fn get_int(&mut self, bound: i32) -> i32 {
        self.below(bound)
    }

    fn get_int_range(&mut self, origin: i32, bound: i32) -> i32 {
        self.get(origin, bound)
    }

    fn next_int(&mut self) -> i32 {
        self.next_i32()
    }

    fn get_long(&mut self, bound: i64) -> i64 {
        self.below_i64(bound)
    }

    fn get_long_range(&mut self, origin: i64, bound: i64) -> i64 {
        self.get_i64(origin, bound)
    }

    fn next_long(&mut self) -> i64 {
        self.next_i64()
    }

    fn next_float(&mut self) -> f32 {
        self.next_f32()
    }

    fn get_double(&mut self, bound: f64) -> f64 {
        self.below_f64(bound)
    }

    fn get_double_range(&mut self, origin: f64, bound: f64) -> f64 {
        self.get_f64(origin, bound)
    }

    fn next_double(&mut self) -> f64 {
        self.next_f64()
    }

    fn next_gaussian(&mut self) -> f64 {
        WorldRng::next_gaussian(self)
    }
}

/// A scripted [`Rnd`] for unit tests: `get_int` and `get_int_range` return the given values in
/// order (panicking when they run out), except in Java's no-draw cases (`bound <= 0`,
/// `origin >= bound`) which consume nothing; every other draw is a test bug.
#[cfg(test)]
pub(crate) mod testing {
    use super::Rnd;

    pub(crate) struct Fixed {
        draws: Vec<i32>,
        next: usize,
    }

    impl Fixed {
        pub(crate) fn new(draws: &[i32]) -> Self {
            Self {
                draws: draws.to_vec(),
                next: 0,
            }
        }

        fn take(&mut self) -> i32 {
            let v = self.draws[self.next];
            self.next += 1;
            v
        }

        /// True when every scripted value was drawn.
        pub(crate) fn exhausted(&self) -> bool {
            self.next == self.draws.len()
        }
    }

    impl Rnd for Fixed {
        fn next_boolean(&mut self) -> bool {
            unreachable!("next_boolean")
        }
        fn next_bytes(&mut self, _bytes: &mut [u8]) {
            unreachable!("next_bytes")
        }
        fn get_int(&mut self, bound: i32) -> i32 {
            if bound <= 0 { 0 } else { self.take() }
        }
        fn get_int_range(&mut self, origin: i32, bound: i32) -> i32 {
            if origin >= bound { origin } else { self.take() }
        }
        fn next_int(&mut self) -> i32 {
            unreachable!("next_int")
        }
        fn get_long(&mut self, _bound: i64) -> i64 {
            unreachable!("get_long")
        }
        fn get_long_range(&mut self, _origin: i64, _bound: i64) -> i64 {
            unreachable!("get_long_range")
        }
        fn next_long(&mut self) -> i64 {
            unreachable!("next_long")
        }
        fn next_float(&mut self) -> f32 {
            unreachable!("next_float")
        }
        fn get_double(&mut self, _bound: f64) -> f64 {
            unreachable!("get_double")
        }
        fn get_double_range(&mut self, _origin: f64, _bound: f64) -> f64 {
            unreachable!("get_double_range")
        }
        fn next_double(&mut self) -> f64 {
            unreachable!("next_double")
        }
        fn next_gaussian(&mut self) -> f64 {
            unreachable!("next_gaussian")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The trait forwards to the same-named `WorldRng` method: same seed, same values.
    #[test]
    fn world_rng_forwards_with_the_same_ranges() {
        let mut a = WorldRng::from_seed(42);
        let mut b = WorldRng::from_seed(42);
        for _ in 0..50 {
            assert_eq!(Rnd::get_int(&mut a, 1000), b.below(1000));
            assert_eq!(Rnd::get_int_range(&mut a, -7, 7), b.get(-7, 7));
        }
    }

    /// Java's edge cases: no draw for a bound <= 0 or origin >= bound, inclusive upper end.
    #[test]
    fn edge_cases_match_java() {
        let mut rng = WorldRng::from_seed(1);
        assert_eq!(Rnd::get_int(&mut rng, 0), 0);
        assert_eq!(Rnd::get_int(&mut rng, -5), 0);
        assert_eq!(Rnd::get_int_range(&mut rng, 5, 5), 5);
        assert_eq!(Rnd::get_int_range(&mut rng, 6, 5), 6);
        let mut seen = [false; 3];
        for _ in 0..200 {
            let v = Rnd::get_int_range(&mut rng, -1, 1);
            assert!((-1..=1).contains(&v));
            seen[(v + 1) as usize] = true;
        }
        assert!(seen.iter().all(|s| *s), "both ends of the range are reachable");
    }

    #[test]
    fn same_seed_same_sequence() {
        let mut a = WorldRng::from_seed(9);
        let mut b = WorldRng::from_seed(9);
        let xs: Vec<i32> = (0..20).map(|_| Rnd::get_int(&mut a, 100)).collect();
        let ys: Vec<i32> = (0..20).map(|_| Rnd::get_int(&mut b, 100)).collect();
        assert_eq!(xs, ys);
    }
}
