//! Deterministic shuffle using a simple PRNG for reproducibility.

use std::collections::VecDeque;

/// A simple xorshift64 PRNG for deterministic shuffling.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// Create a new RNG with the given seed.
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    /// Generate the next pseudo-random u64.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// Generate a random usize in [0, bound).
    pub fn next_usize(&mut self, bound: usize) -> usize {
        if bound == 0 {
            return 0;
        }
        (self.next_u64() % bound as u64) as usize
    }
}

/// Fisher-Yates shuffle with deterministic RNG.
pub fn shuffle<T>(items: &mut VecDeque<T>, seed: u64) {
    let mut rng = Rng::new(seed);
    let len = items.len();
    for i in (1..len).rev() {
        let j = rng.next_usize(i + 1);
        items.swap(i, j);
    }
}

#[cfg(test)]
mod tests {
    use super::{shuffle, Rng};
    use std::collections::VecDeque;

    #[test]
    fn rng_is_deterministic() {
        let mut rng1 = Rng::new(42);
        let mut rng2 = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(rng1.next_u64(), rng2.next_u64());
        }
    }

    #[test]
    fn shuffle_produces_permutation() {
        let mut items: VecDeque<u8> = (0..10).collect();
        let original = items.clone();
        shuffle(&mut items, 42);
        assert_eq!(items.len(), original.len());
        let mut sorted = items.clone();
        sorted.make_contiguous().sort();
        assert_eq!(sorted, original);
    }

    #[test]
    fn shuffle_with_different_seeds_produces_different_results() {
        let mut items1: VecDeque<u8> = (0..10).collect();
        let mut items2 = items1.clone();
        shuffle(&mut items1, 42);
        shuffle(&mut items2, 43);
        assert_ne!(items1, items2);
    }

    #[test]
    fn shuffle_handles_empty_and_single() {
        let mut empty: VecDeque<u8> = VecDeque::new();
        shuffle(&mut empty, 42);
        assert!(empty.is_empty());

        let mut single = VecDeque::from(vec![1]);
        shuffle(&mut single, 42);
        assert_eq!(single, VecDeque::from(vec![1]));
    }
}
"}}] }