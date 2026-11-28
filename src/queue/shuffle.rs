//! Deterministic shuffle using a small PRNG so behavior is reproducible in tests.

use std::collections::VecDeque;

/// A xorshift64 PRNG with a non-zero state.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// Create a generator from a seed; zero is mapped to one.
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    /// Generate the next pseudo-random value.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// Generate a value in `[0, bound)`; zero for an empty bound.
    pub fn next_usize(&mut self, bound: usize) -> usize {
        if bound == 0 {
            return 0;
        }
        (self.next_u64() % bound as u64) as usize
    }
}

/// Fisher-Yates shuffle of a queue with a caller-supplied seed.
pub fn shuffle<T>(items: &mut VecDeque<T>, seed: u64) {
    let mut rng = Rng::new(seed);
    let len = items.len();
    for i in (1..len).rev() {
        let j = rng.next_usize(i + 1);
        items.swap(i, j);
    }
}

/// Derive a seed from wall-clock time without external randomness crates.
pub fn time_seed() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    (nanos as u64) ^ (std::process::id() as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

#[cfg(test)]
mod tests {
    use super::{shuffle, time_seed, Rng};
    use std::collections::VecDeque;

    #[test]
    fn rng_is_deterministic() {
        let mut first = Rng::new(42);
        let mut second = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(first.next_u64(), second.next_u64());
        }
        assert_eq!(Rng::new(0).state, 1);
        assert_eq!(Rng::new(0).next_usize(0), 0);
    }

    #[test]
    fn shuffle_produces_a_permutation() {
        let mut items: VecDeque<u8> = (0..10).collect();
        let original = items.clone();
        shuffle(&mut items, 42);
        assert_eq!(items.len(), original.len());
        let mut sorted = items.clone();
        sorted.make_contiguous().sort();
        assert_eq!(sorted, original);
    }

    #[test]
    fn seeds_change_the_order() {
        let mut first: VecDeque<u8> = (0..32).collect();
        let mut second = first.clone();
        shuffle(&mut first, 42);
        shuffle(&mut second, 43);
        assert_ne!(first, second);
    }

    #[test]
    fn shuffle_handles_empty_and_single_queues() {
        let mut empty: VecDeque<u8> = VecDeque::new();
        shuffle(&mut empty, 42);
        assert!(empty.is_empty());

        let mut single = VecDeque::from(vec![1]);
        shuffle(&mut single, 42);
        assert_eq!(single, VecDeque::from(vec![1]));
    }

    #[test]
    fn time_seeds_are_usable() {
        assert_ne!(time_seed(), 0);
    }
}
