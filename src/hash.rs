//! Implementations of hash functions

use crate::filter::BucketIndex;
use crate::filter::Fingerprint;

use core::ops::Shl;

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

/// Copies data into a slice, borrowed from the `murmur3` package <https://docs.rs/murmur3/latest/murmur3/>. See `NOTICE` file for copyright information.
fn copy_into_array<A, T>(slice: &[T]) -> A
where
    A: Default + AsMut<[T]>,
    T: Copy,
{
    let mut a = A::default();
    <A as AsMut<[T]>>::as_mut(&mut a).copy_from_slice(slice);
    a
}

/// Internal mixing operation, borrowed from the `murmur3` package <https://docs.rs/murmur3/latest/murmur3/>. See `NOTICE` file for copyright information.
fn fmix32(k: u32) -> u32 {
    const C1: u32 = 0x85eb_ca6b;
    const C2: u32 = 0xc2b2_ae35;
    const R1: u32 = 16;
    const R2: u32 = 13;
    let mut tmp = k;
    tmp ^= tmp >> R1;
    tmp = tmp.wrapping_mul(C1);
    tmp ^= tmp >> R2;
    tmp = tmp.wrapping_mul(C2);
    tmp ^= tmp >> R1;
    tmp
}

/// Murmur3 hash function, borrowed from the `murmur3` package <https://docs.rs/murmur3/latest/murmur3/>. See `NOTICE` file for copyright information.
///
/// This function has been modified to remove its dependency on the standard library.
pub fn murmur3_x86_128(source: &[u8], seed: u32) -> u128 {
    const C1: u32 = 0x239b_961b;
    const C2: u32 = 0xab0e_9789;
    const C3: u32 = 0x38b3_4ae5;
    const C4: u32 = 0xa1e3_8b93;
    const C5: u32 = 0x561c_cd1b;
    const C6: u32 = 0x0bca_a747;
    const C7: u32 = 0x96cd_1c35;
    const C8: u32 = 0x32ac_3b17;
    const M: u32 = 5;

    let mut h1: u32 = seed;
    let mut h2: u32 = seed;
    let mut h3: u32 = seed;
    let mut h4: u32 = seed;

    let mut buf = [0; 16];
    let mut processed: usize = 0;
    while processed < source.len() {
        let remaining = source.len() - processed;
        let read = remaining.min(16);
        buf[..read].copy_from_slice(&source[processed..processed + read]);
        processed += read;

        if read == 16 {
            let k1 = u32::from_le_bytes(copy_into_array(&buf[0..4]));
            let k2 = u32::from_le_bytes(copy_into_array(&buf[4..8]));
            let k3 = u32::from_le_bytes(copy_into_array(&buf[8..12]));
            let k4 = u32::from_le_bytes(copy_into_array(&buf[12..16]));
            h1 ^= k1.wrapping_mul(C1).rotate_left(15).wrapping_mul(C2);
            h1 = h1
                .rotate_left(19)
                .wrapping_add(h2)
                .wrapping_mul(M)
                .wrapping_add(C5);
            h2 ^= k2.wrapping_mul(C2).rotate_left(16).wrapping_mul(C3);
            h2 = h2
                .rotate_left(17)
                .wrapping_add(h3)
                .wrapping_mul(M)
                .wrapping_add(C6);
            h3 ^= k3.wrapping_mul(C3).rotate_left(17).wrapping_mul(C4);
            h3 = h3
                .rotate_left(15)
                .wrapping_add(h4)
                .wrapping_mul(M)
                .wrapping_add(C7);
            h4 ^= k4.wrapping_mul(C4).rotate_left(18).wrapping_mul(C1);
            h4 = h4
                .rotate_left(13)
                .wrapping_add(h1)
                .wrapping_mul(M)
                .wrapping_add(C8);
        } else if processed == source.len() {
            h1 ^= processed as u32;
            h2 ^= processed as u32;
            h3 ^= processed as u32;
            h4 ^= processed as u32;
            h1 = h1.wrapping_add(h2);
            h1 = h1.wrapping_add(h3);
            h1 = h1.wrapping_add(h4);
            h2 = h2.wrapping_add(h1);
            h3 = h3.wrapping_add(h1);
            h4 = h4.wrapping_add(h1);
            h1 = fmix32(h1);
            h2 = fmix32(h2);
            h3 = fmix32(h3);
            h4 = fmix32(h4);
            h1 = h1.wrapping_add(h2);
            h1 = h1.wrapping_add(h3);
            h1 = h1.wrapping_add(h4);
            h2 = h2.wrapping_add(h1);
            h3 = h3.wrapping_add(h1);
            h4 = h4.wrapping_add(h1);
            let x = ((h4 as u128) << 96) | ((h3 as u128) << 64) | ((h2 as u128) << 32) | h1 as u128;
            return x;
        } else {
            let mut k1 = 0;
            let mut k2 = 0;
            let mut k3 = 0;
            let mut k4 = 0;
            if read >= 15 {
                k4 ^= (buf[14] as u32).shl(16);
            }
            if read >= 14 {
                k4 ^= (buf[13] as u32).shl(8);
            }
            if read >= 13 {
                k4 ^= buf[12] as u32;
                k4 = k4.wrapping_mul(C4).rotate_left(18).wrapping_mul(C1);
                h4 ^= k4;
            }
            if read >= 12 {
                k3 ^= (buf[11] as u32).shl(24);
            }
            if read >= 11 {
                k3 ^= (buf[10] as u32).shl(16);
            }
            if read >= 10 {
                k3 ^= (buf[9] as u32).shl(8);
            }
            if read >= 9 {
                k3 ^= buf[8] as u32;
                k3 = k3.wrapping_mul(C3).rotate_left(17).wrapping_mul(C4);
                h3 ^= k3;
            }
            if read >= 8 {
                k2 ^= (buf[7] as u32).shl(24);
            }
            if read >= 7 {
                k2 ^= (buf[6] as u32).shl(16);
            }
            if read >= 6 {
                k2 ^= (buf[5] as u32).shl(8);
            }
            if read >= 5 {
                k2 ^= buf[4] as u32;
                k2 = k2.wrapping_mul(C2).rotate_left(16).wrapping_mul(C3);
                h2 ^= k2;
            }
            if read >= 4 {
                k1 ^= (buf[3] as u32).shl(24);
            }
            if read >= 3 {
                k1 ^= (buf[2] as u32).shl(16);
            }
            if read >= 2 {
                k1 ^= (buf[1] as u32).shl(8);
            }
            if read >= 1 {
                k1 ^= buf[0] as u32;
            }
            k1 = k1.wrapping_mul(C1);
            k1 = k1.rotate_left(15);
            k1 = k1.wrapping_mul(C2);
            h1 ^= k1;
        }
    }
    unreachable!("The loop should always return in the last block")
}

/// Compute a 1 byte fingerprint from a hash digest but emit as 32 bits for XORing
///
/// As in the C++ reference implementation, the fingerprint cannot be zero
pub fn byte_fingerprint_long(hash_value: u32) -> BucketIndex {
    let fingerprint = hash_value & ((1 << 8) - 1);
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
