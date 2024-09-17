//! Grouped tests for hash function implementations
//!
//! (imports of `rand` and `HashSet` must be within a `test` CFG to keep things `no_std`)

/* -------------------- Unit Tests -------------------- */

#[cfg(test)]
mod tests {
    use crate::{murmur3, Murmur3Hasher};
    use core::hash::Hasher;
    use rand::prelude::*;
    use rand_chacha::ChaCha8Rng;
    use std::collections::HashSet;

    // Constants across all tests
    const NUM_SAMPLES: usize = 10000;
    const ACCEPTABLE_COLLISION_RATE: f32 = 0.01;

    // Utility fns
    fn get_random_string(rng: &mut ChaCha8Rng, len: usize) -> String {
        rng.sample_iter::<char, _>(&rand::distributions::Standard)
            .take(len)
            .map(char::from)
            .collect()
    }

    fn test_hash_collisions_with_random_strings<H: Hasher>(hasher: &mut H) {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let mut input_set: HashSet<String> = HashSet::with_capacity(NUM_SAMPLES);
        let mut output_set: HashSet<u64> = HashSet::with_capacity(NUM_SAMPLES);
        for i in 0..NUM_SAMPLES {
            let random_string = get_random_string(&mut rng, i % 12);
            hasher.write(random_string.as_bytes());
            _ = input_set.insert(random_string.clone());
            _ = output_set.insert(hasher.finish());
        }
        println!("inputs {}, outputs {}", input_set.len(), output_set.len());
        assert!(
            input_set.len() - output_set.len()
                < (ACCEPTABLE_COLLISION_RATE * NUM_SAMPLES as f32) as usize
        );
    }

    #[test]
    fn basic_hash_test_murmur3() {
        let a = murmur3::murmur3_x86_128("cat".as_bytes(), 0);
        let b = murmur3::murmur3_x86_128("dog".as_bytes(), 0);
        assert_ne!(a, b);
    }

    // Check implementation of hash function by counting the number of hash collisions for some random data
    #[test]
    fn collision_rate_murmur3() {
        let mut h = Murmur3Hasher::new();
        test_hash_collisions_with_random_strings::<Murmur3Hasher>(&mut h);
    }
}
