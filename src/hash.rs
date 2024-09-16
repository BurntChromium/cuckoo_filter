//! Implementations of hash functions

use crate::filter::BucketIndex;
use crate::filter::Fingerprint;

/// DBJ2 hash function, with XOR instead of add
///
/// Source: <http://www.cse.yorku.ca/~oz/hash.html>
pub fn hash_djb2(input: &[u8]) -> u32 {
    let mut hash: u32 = 5381;
    for &byte in input {
        hash = hash.wrapping_mul(33) ^ (byte as u32);
    }
    hash
}

/// Compute a 1 byte fingerprint from a hash digest but emit as 32 bits for XORing
///
/// As in the C++ reference implementation, the fingerprint cannot be zero
pub fn byte_fingerprint_long(hash_value: u32) -> BucketIndex {
    let fingerprint = hash_value % (1 << (8 - 1));
    // Prevent a fingerprint of 0 (because 0 implies empty bucket)
    fingerprint + (fingerprint == 0) as u32
}

/// Compute a 1 byte fingerprint and truncate the empty bits
pub fn byte_fingerprint_short(hash_value: u32) -> Fingerprint {
    byte_fingerprint_long(hash_value) as u8
}

/* -------------------- Unit Tests -------------------- */

#[cfg(test)]
mod tests {
    use super::*;
    use rand::prelude::*;
    use rand_chacha::ChaCha8Rng;
    use std::collections::HashSet;
    use std::hash::Hash;

    // Constants across all tests
    const NUM_SAMPLES: usize = 10000;
    const ACCEPTABLE_COLLISION_RATE: f32 = 0.01;

    // This trait allows us to test multiple hash function implementations and insert them into a HashSet to check their collision rates
    pub trait HashOutput: Copy + Eq + Hash {}
    impl HashOutput for u32 {}
    impl HashOutput for u64 {}

    // Utility fns
    fn get_random_string(rng: &mut ChaCha8Rng, len: usize) -> String {
        rng.sample_iter::<char, _>(&rand::distributions::Standard)
            .take(len)
            .map(char::from)
            .collect()
    }

    fn test_hash_collisions_with_random_strings<T: HashOutput>(hash_fn: fn(&[u8]) -> T) {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let mut input_set: HashSet<String> = HashSet::with_capacity(NUM_SAMPLES);
        let mut output_set: HashSet<T> = HashSet::with_capacity(NUM_SAMPLES);
        for i in 0..NUM_SAMPLES {
            let random_string = get_random_string(&mut rng, i % 12);
            _ = input_set.insert(random_string.clone());
            _ = output_set.insert(hash_fn(random_string.as_bytes()));
        }
        println!("inputs {}, outputs {}", input_set.len(), output_set.len());
        assert!(
            input_set.len() - output_set.len()
                < (ACCEPTABLE_COLLISION_RATE * NUM_SAMPLES as f32) as usize
        );
    }

    #[test]
    fn basic_hash_test_djb2() {
        let a = hash_djb2("cat".as_bytes());
        let b = hash_djb2("dog".as_bytes());
        assert_ne!(a, b);
    }

    // Check implementation of hash function by counting the number of hash collisions for some random data
    #[test]
    fn collision_rate_dbj2() {
        test_hash_collisions_with_random_strings::<u32>(hash_djb2);
    }
}
