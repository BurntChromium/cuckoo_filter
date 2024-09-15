//! Implementations of hash functions

/// DBJ2 hash function
///
/// Source: <http://www.cse.yorku.ca/~oz/hash.html>
pub fn hash_djb2(input: &[u8]) -> u32 {
    let mut hash: u32 = 5381;
    for &byte in input {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u32);
    }
    hash
}
/// Compute a 1 byte fingerprint from a hash digest but emit as 32 bits for XORing
///
/// As in the C++ reference implementation, the fingerprint cannot be zero
pub fn byte_fingerprint_long(hash_value: u32) -> u32 {
    let fingerprint = hash_value % (1 << (8 - 1));
    // Prevent a fingerprint of 0 (because 0 implies empty bucket)
    fingerprint + (fingerprint == 0) as u32
}

/// Compute a 1 byte fingerprint and truncate the empty bits
pub fn byte_fingerprint_short(hash_value: u32) -> u8 {
    byte_fingerprint_long(hash_value) as u8
}

/* -------------------- Unit Tests -------------------- */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_hash_test_djb2() {
        let a = hash_djb2("cat".as_bytes());
        let b = hash_djb2("dog".as_bytes());
        assert_ne!(a, b);
    }
}
