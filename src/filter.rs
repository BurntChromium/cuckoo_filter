//! # Cuckoo Filter
//!
//! This implementation is based on the paper _Cuckoo Filter: Practically Better Than Bloom_, by Fan et. al.
//!
//! The paper recommends a (2, 4) CF (2 possible buckets for each item, and 4 fingerprints in each bucket) because it's space optimal for practical false positive rates. Assuming our CF will hold up to a few billion items, 6 bits per fingerprint is sufficient (24 bits per bucket), but we round up to one byte per fingerprint for the sake of practicality.

/// Each fingerprint is 1 byte
const BITS_PER_ITEM: usize = 8;
// Each bucket holds 4 fingerprints
const BUCKET_SIZE: usize = 4;
// We support up to u32 buckets, which means we can hold `u32::MAX * 4` items
const ITEM_LIMIT: usize = u32::MAX as usize * BUCKET_SIZE as usize;

/// We need to cache items that have been evicted so we can relocate them, this is the shape of that cache
struct EvictionVictim {
    index: u32,
    tag: u8,
    used: bool,
}

impl EvictionVictim {
    pub fn new() -> EvictionVictim {
        EvictionVictim {
            index: 0,
            tag: 0,
            used: false,
        }
    }
}

/// A Cuckoo Filter that holds up to 17 billion items
pub struct CuckooFilter {
    eviction_cache: EvictionVictim,
    data: Vec<[u8; BUCKET_SIZE]>,
}

impl CuckooFilter {
    pub fn new(max_items: usize) -> Result<CuckooFilter, String> {
        if max_items > ITEM_LIMIT {
            return Err(format!("requested cuckoo filter exceeds practical constraints, more than {ITEM_LIMIT} items is not supported"));
        }
        let number_of_buckets: usize = max_items / BUCKET_SIZE;
        Ok(CuckooFilter {
            eviction_cache: EvictionVictim::new(),
            data: Vec::with_capacity(number_of_buckets),
        })
    }

    /// Compute a 1 byte fingerprint from the hash value
    pub fn fingerprint(hash_value: u32) -> u8 {
        (hash_value % (1 << BITS_PER_ITEM - 1) + 1) as u8
    }

    /// Approximately how many bytes is this CF using?
    pub fn size_estimate(&self) -> usize {
        self.data.len() * BUCKET_SIZE
    }
}
