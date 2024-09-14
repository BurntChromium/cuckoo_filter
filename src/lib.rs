//! # Cuckoo Filter implementation
//!
//! A Cuckoo Filter is an efficient data structure for determining "set membership" (i.e. 'have I seen this thing before?'). It is similar to a Bloom Filter, but unlike a Bloom Filter, Cuckoo Filters support item deletion. Cuckoo Filters also form the backbone of certain cryptographic protocols.
//!
//! This crate implements a Cuckoo Filter with reasonable parameters for balancing overall capacity and achieving near optimal space savings. This filter can hold up to 17 billion items. At maximum size, this CF should consume about 4 GiB of RAM.

mod filter;
mod hash;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

pub use filter::CuckooFilter;
pub use filter::CuckooFilterOpError;
pub use hash::hash_djb2;
pub use hash::hash_sbdm;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
