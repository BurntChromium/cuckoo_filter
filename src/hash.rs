//! Implementations of hash functions

/// DBJ2 hash function
///
/// Source: http://www.cse.yorku.ca/~oz/hash.html
pub fn hash_djb2(input: &[u8]) -> u32 {
    let mut hash: u32 = 5381;
    for &byte in input {
        hash = ((hash << 5) + hash) + byte as u32;
    }
    hash
}

/// SBDM hash function
///
/// Source: http://www.cse.yorku.ca/~oz/hash.html
pub fn hash_sbdm(input: &[u8]) -> u32 {
    let mut hash: u32 = 0;
    for &byte in input {
        hash = byte as u32 + (hash << 6) + (hash << 16) - hash;
    }
    hash
}
