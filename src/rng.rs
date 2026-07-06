//! The game's random number generator: xoshiro256++ seeded via SplitMix64.
//!
//! Replaces the port-era `java.util.Random` re-implementation (v0.1.0 and earlier);
//! Java-seed/save compatibility is no longer a goal. World generation stays fully
//! deterministic for a given seed — just with this generator instead of the JVM's LCG.

#[derive(Debug, Clone)]
pub struct Rng {
    state: [u64; 4],
    next_gaussian: Option<f64>,
}

fn split_mix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

impl Rng {
    pub fn new(seed: i64) -> Rng {
        let mut r = Rng { state: [0; 4], next_gaussian: None };
        r.set_seed(seed);
        r
    }

    /// Seeded from the clock.
    pub fn from_time() -> Rng {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as i64)
            .unwrap_or(0);
        Self::new(nanos)
    }

    pub fn set_seed(&mut self, seed: i64) {
        let mut sm = seed as u64;
        for s in &mut self.state {
            *s = split_mix64(&mut sm);
        }
        self.next_gaussian = None;
    }

    fn next_u64(&mut self) -> u64 {
        let result = self.state[0]
            .wrapping_add(self.state[3])
            .rotate_left(23)
            .wrapping_add(self.state[0]);
        let t = self.state[1] << 17;
        self.state[2] ^= self.state[0];
        self.state[3] ^= self.state[1];
        self.state[1] ^= self.state[2];
        self.state[0] ^= self.state[3];
        self.state[2] ^= t;
        self.state[3] = self.state[3].rotate_left(45);
        result
    }

    /// A uniformly random i32.
    pub fn next_int(&mut self) -> i32 {
        self.next_u64() as i32
    }

    /// A uniformly random value in `0..bound`. Panics on `bound <= 0`.
    pub fn next_int_bound(&mut self, bound: i32) -> i32 {
        assert!(bound > 0, "bound must be positive");
        // Lemire's multiply-shift bounded generation (bias negligible for game use).
        let x = self.next_u64() as u32 as u64;
        ((x * bound as u64) >> 32) as i32
    }

    pub fn next_long(&mut self) -> i64 {
        self.next_u64() as i64
    }

    pub fn next_boolean(&mut self) -> bool {
        self.next_u64() >> 63 != 0
    }

    /// Uniform in [0, 1).
    pub fn next_float(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 * (1.0 / (1u32 << 24) as f32)
    }

    /// Uniform in [0, 1).
    pub fn next_double(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Standard normal (mean 0, std-dev 1), Marsaglia polar method.
    pub fn next_gaussian(&mut self) -> f64 {
        if let Some(g) = self.next_gaussian.take() {
            return g;
        }
        loop {
            let v1 = 2.0 * self.next_double() - 1.0;
            let v2 = 2.0 * self.next_double() - 1.0;
            let s = v1 * v1 + v2 * v2;
            if s < 1.0 && s != 0.0 {
                let multiplier = (-2.0 * s.ln() / s).sqrt();
                self.next_gaussian = Some(v2 * multiplier);
                return v1 * multiplier;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_for_seed() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        let mut c = Rng::new(43);
        assert_ne!(Rng::new(42).next_u64(), c.next_u64());
    }

    #[test]
    fn bounds_respected() {
        let mut r = Rng::new(7);
        for _ in 0..10_000 {
            let v = r.next_int_bound(37);
            assert!((0..37).contains(&v));
            let f = r.next_float();
            assert!((0.0..1.0).contains(&f));
            let d = r.next_double();
            assert!((0.0..1.0).contains(&d));
        }
    }

    #[test]
    fn gaussian_sane() {
        let mut r = Rng::new(1);
        let n = 20_000;
        let mean: f64 = (0..n).map(|_| r.next_gaussian()).sum::<f64>() / n as f64;
        assert!(mean.abs() < 0.05, "gaussian mean {mean} too far from 0");
    }
}
