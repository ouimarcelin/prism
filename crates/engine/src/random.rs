//! Small pseudo-random generator for random playback.

/// xorshift64* generator: fast, reproducible from its seed, not for security.
#[derive(Clone, Debug)]
pub(crate) struct Random(u64);

impl Random {
    /// Creates a generator. A zero seed is replaced, since it would stay zero.
    pub(crate) fn new(seed: u64) -> Self {
        Self(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }

    /// Next value in `0.0..1.0`.
    pub(crate) fn next_f64(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        let value = x.wrapping_mul(0x2545_F491_4F6C_DD1D);
        (value >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_stay_in_range_and_vary() {
        let mut random = Random::new(0);
        let values: Vec<f64> = (0..1000).map(|_| random.next_f64()).collect();
        assert!(values.iter().all(|v| (0.0..1.0).contains(v)));
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        assert!((mean - 0.5).abs() < 0.05, "mean {mean}");
    }

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Random::new(42);
        let mut b = Random::new(42);
        for _ in 0..10 {
            assert_eq!(a.next_f64(), b.next_f64());
        }
    }
}
