//! Exact re-implementation of `java.util.Random` (the 48-bit LCG defined by the JDK spec).
//!
//! World generation and player spawn placement must be bit-identical to the Java game for
//! a given seed, so every random number in the port flows through this type. See PORTING.md.

const MULTIPLIER: i64 = 0x5DEECE66D;
const ADDEND: i64 = 0xB;
const MASK: i64 = (1 << 48) - 1;

#[derive(Debug, Clone)]
pub struct JavaRandom {
    seed: i64,
    next_next_gaussian: f64,
    have_next_next_gaussian: bool,
}

impl JavaRandom {
    pub fn new(seed: i64) -> Self {
        let mut r = JavaRandom { seed: 0, next_next_gaussian: 0.0, have_next_next_gaussian: false };
        r.set_seed(seed);
        r
    }

    /// Equivalent of `new Random()` — seeded from the clock. Java uses a seed uniquifier
    /// plus nanoTime; distribution-wise a nanosecond clock seed is equivalent.
    pub fn from_time() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as i64)
            .unwrap_or(0);
        Self::new(nanos)
    }

    pub fn set_seed(&mut self, seed: i64) {
        self.seed = (seed ^ MULTIPLIER) & MASK;
        self.have_next_next_gaussian = false;
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.seed = (self.seed.wrapping_mul(MULTIPLIER).wrapping_add(ADDEND)) & MASK;
        ((self.seed as u64) >> (48 - bits)) as i32
    }

    /// `nextInt()`
    pub fn next_int(&mut self) -> i32 {
        self.next(32)
    }

    /// `nextInt(bound)` — panics on bound <= 0, exactly like Java throws.
    pub fn next_int_bound(&mut self, bound: i32) -> i32 {
        assert!(bound > 0, "bound must be positive");
        if (bound & -bound) == bound {
            // power of two
            return ((bound as i64).wrapping_mul(self.next(31) as i64) >> 31) as i32;
        }
        loop {
            let bits = self.next(31);
            let val = bits % bound;
            if bits - val + (bound - 1) >= 0 {
                return val;
            }
        }
    }

    /// `nextLong()`
    pub fn next_long(&mut self) -> i64 {
        ((self.next(32) as i64) << 32).wrapping_add(self.next(32) as i64)
    }

    /// `nextBoolean()`
    pub fn next_boolean(&mut self) -> bool {
        self.next(1) != 0
    }

    /// `nextFloat()`
    pub fn next_float(&mut self) -> f32 {
        self.next(24) as f32 / (1 << 24) as f32
    }

    /// `nextDouble()`
    pub fn next_double(&mut self) -> f64 {
        (((self.next(26) as i64) << 27).wrapping_add(self.next(27) as i64)) as f64
            * (1.0 / (1i64 << 53) as f64)
    }

    /// `nextGaussian()` — Marsaglia polar method, matching the JDK implementation
    /// (including `StrictMath` log/sqrt, which agree with Rust's f64 ops on these inputs).
    pub fn next_gaussian(&mut self) -> f64 {
        if self.have_next_next_gaussian {
            self.have_next_next_gaussian = false;
            return self.next_next_gaussian;
        }
        loop {
            let v1 = 2.0 * self.next_double() - 1.0;
            let v2 = 2.0 * self.next_double() - 1.0;
            let s = v1 * v1 + v2 * v2;
            if s < 1.0 && s != 0.0 {
                let multiplier = (-2.0 * s.ln() / s).sqrt();
                self.next_next_gaussian = v2 * multiplier;
                self.have_next_next_gaussian = true;
                return v1 * multiplier;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Expected values captured from OpenJDK: `new Random(42)` / `new Random(12345)`.
    #[test]
    fn next_int_matches_jvm() {
        let mut r = JavaRandom::new(42);
        let vals: Vec<i32> = (0..5).map(|_| r.next_int()).collect();
        assert_eq!(vals, vec![-1170105035, 234785527, -1360544799, 205897768, 1325939940]);
    }

    #[test]
    fn next_int_bound_matches_jvm() {
        let mut r = JavaRandom::new(42);
        let vals: Vec<i32> = (0..8).map(|_| r.next_int_bound(100)).collect();
        assert_eq!(vals, vec![0, 63, 11, 15, 94, 50, 18, 71]);
    }

    #[test]
    fn next_double_matches_jvm() {
        let mut r = JavaRandom::new(12345);
        assert!((r.next_double() - 0.3618031071604718).abs() < 1e-15);
        assert!((r.next_double() - 0.932993485288541).abs() < 1e-15);
    }

    #[test]
    fn next_boolean_and_float_match_jvm() {
        let mut r = JavaRandom::new(12345);
        let bools: Vec<bool> = (0..6).map(|_| r.next_boolean()).collect();
        assert_eq!(bools, vec![false, true, false, false, true, true]);
        let mut r = JavaRandom::new(12345);
        assert!((r.next_float() - 0.36180305).abs() < 1e-7);
    }

    #[test]
    fn next_gaussian_matches_jvm() {
        let mut r = JavaRandom::new(42);
        assert!((r.next_gaussian() - 1.1419053154730547).abs() < 1e-12);
        assert!((r.next_gaussian() - 0.9194079489827879).abs() < 1e-12);
    }
}
