//! # Cuckoo Filter
//!
//! This implementation is based on the paper _Cuckoo Filter: Practically Better Than Bloom_, by Fan et. al.
//!
//! The paper recommends a (2, 4) CF (2 possible buckets for each item, and 4 fingerprints in each bucket) because it's space optimal for practical false positive rates. Assuming our CF will hold up to a few billion items, 6 bits per fingerprint is sufficient (24 bits per bucket), but we round up to one byte per fingerprint for the sake of practicality.
//!
//! The paper's authors have provided a reference C++ implementation in this repository: https://github.com/efficient/cuckoofilter

use crate::hash;

type Input = [u8];
type BucketIndex = u32;
type Fingerprint = u8;

const MAX_EVICTIONS: u16 = 500;
/// Each bucket holds 4 fingerprints
const BUCKET_SIZE: usize = 4;
/// We support up to u32 buckets, which means we can hold `u32::MAX * 4` items
const ITEM_LIMIT: usize = u32::MAX as usize * BUCKET_SIZE as usize;
/// Easily swap hash functions during development, TODO: pick one
const HASH_FN: fn(&[u8]) -> u32 = hash::hash_djb2;

/// An eviction cache holds an item that we couldn't reinsert
///
/// An item being here means that the filter is "probabilistically full". It may not be technically 100% saturated, but we ran into so many hash collisions that we had to stop. (Using a bad hash function may result in being "full" early)
struct EvictionVictim {
    index: u32,
    fingerprint: Fingerprint,
    used: bool,
}

impl EvictionVictim {
    fn new() -> EvictionVictim {
        EvictionVictim {
            index: 0,
            fingerprint: 0,
            used: false,
        }
    }

    fn reset(&mut self) {
        self.index = 0;
        self.fingerprint = 0;
        self.used = false;
    }
}

/// An error that can result at "runtime" when performing insert/delete operations
pub enum CuckooFilterOpError {
    OutOfSpace,
    ItemAlreadyExists,
    ItemDoesNotExist,
}

/// A Cuckoo Filter that holds up to 17 billion items
///
/// ### Notes
///
/// - The eviction cache holds an item that we couldn't reinsert, and represents when the data structure is effectively/probabilistically full (as opposed to mechanically full)
/// - The `length_u32` parameter lets us wrap around (modulo) bucket indices that would be too large
pub struct CuckooFilter {
    eviction_cache: EvictionVictim,
    data: Vec<[Fingerprint; BUCKET_SIZE]>,
    length_u32: u32,
}

impl CuckooFilter {
    /// Try to create a new Cuckoo Filter
    ///
    /// This can fail if the desired filter would be too large.
    ///
    /// ### Caveats
    ///
    /// - We must round the size of our backing vector of data to a power of two. This is because we will modulo the index when our hash function creates a bucket index bigger than the backing vector. If the data was *not* a power of 2, our indices would be subject to "Modulo bias" and cause more hash collisions.
    pub fn new(max_items: usize) -> Result<CuckooFilter, String> {
        // Check item limit
        if max_items > ITEM_LIMIT {
            return Err(format!("requested cuckoo filter exceeds practical constraints, more than {ITEM_LIMIT} items is not supported"));
        }
        // If we didn't care about modulo bias, we could use this many buckets
        let number_of_buckets_exact: usize = max_items / BUCKET_SIZE;
        // But to avoid hash collisions, we round up
        let number_of_buckets_actual: usize = number_of_buckets_exact.next_power_of_two();
        // Check for overflow after rounding to next power of two
        if number_of_buckets_actual > u32::MAX as usize {
            return Err(format!("requested cuckoo filter would exceed size constraints due to power of two rounding: requested items requires {number_of_buckets_actual} buckets, which is the next highest power of 2."));
        }
        Ok(CuckooFilter {
            eviction_cache: EvictionVictim::new(),
            data: Vec::with_capacity(number_of_buckets_actual),
            length_u32: number_of_buckets_actual as u32,
        })
    }

    /// Approximately how many bytes is this CF using?
    pub fn estimate_size(&self) -> usize {
        self.data.len() * BUCKET_SIZE
    }

    /// Is the Cuckoo Filter full of items?
    ///
    /// Criteria is that we have something left over in the Eviction cache after trying to move it for the max number of kicks
    pub fn is_full(&self) -> bool {
        self.eviction_cache.used
    }

    /// Calculate the buckets given an actual input item
    ///
    /// We modulo the bucket indices because the hash may output a value larger than the true length of the backing data array. However, because our length is a power of 2, we can use bitwise AND.
    ///
    /// This is (mostly) Equation 1 in section 3.1 of the paper
    ///
    /// However, unlike Equation 1, we follow the reference implementation from the authors and instead compute bucket 2 by XORing with a magic constant
    fn buckets_from_item(&self, item: &Input) -> (BucketIndex, BucketIndex, Fingerprint) {
        let bucket_1 = HASH_FN(item) & (self.length_u32 - 1);
        // The magic constant is from MurmurHash2 (as in the reference impl)
        let bucket_2 = bucket_1 ^ (hash::byte_fingerprint_long(bucket_1) * 0x5bd1e995);
        (
            bucket_1,
            bucket_2 & (self.length_u32 - 1),
            hash::byte_fingerprint_short(bucket_1),
        )
    }

    /// We can calculate a new bucket for an evicted item despite only having that item's fingerprint
    ///
    /// This is Equation 2 in Section 3.1 of the paper
    fn bucket_from_evicted(
        &self,
        old_bucket: BucketIndex,
        fingerprint: Fingerprint,
    ) -> BucketIndex {
        (old_bucket ^ (fingerprint as u32)) & (self.length_u32 - 1)
    }

    /// Internal method to try inserting a fingerprint into a bucket.
    ///
    /// True means success, false means the bucket was full
    fn try_insert_at_bucket(
        &mut self,
        bucket_index: BucketIndex,
        fingerprint: Fingerprint,
    ) -> bool {
        let mut bucket = self.data[bucket_index as usize];
        for f_print in bucket.iter_mut() {
            if *f_print == 0 {
                *f_print = fingerprint;
                return true;
            }
        }
        false
    }

    /// Internal method to swap an existing fingerprint for a new one (the Cuckoo mechanism)
    fn swap_at_bucket(
        &mut self,
        bucket_index: BucketIndex,
        fingerprint: Fingerprint,
        slot: usize,
    ) -> Fingerprint {
        let mut bucket = self.data[bucket_index as usize];
        let evicted_fingerprint = bucket[slot];
        bucket[slot] = fingerprint;
        evicted_fingerprint
    }

    /// Add item to filter. Returns Err if filter is full
    pub fn insert(&mut self, item: &Input) -> Result<(), CuckooFilterOpError> {
        // If the cache is filled then we're (effectively) out of space
        if self.eviction_cache.used {
            return Err(CuckooFilterOpError::OutOfSpace);
        }

        let (candidate_1, candidate_2, fingerprint) = self.buckets_from_item(item);

        // Try inserting into either bucket
        for &bucket_index in &[candidate_1, candidate_2] {
            if self.try_insert_at_bucket(bucket_index, fingerprint) {
                return Ok(());
            }
        }

        // If both buckets are full, begin eviction process
        let mut target_bucket_index = if fingerprint % 2 == 0 {
            candidate_1
        } else {
            candidate_2
        };
        let mut evicted_fingerprint: u8 = 0;

        for kick in 0..MAX_EVICTIONS {
            if kick > 0 && self.try_insert_at_bucket(target_bucket_index, fingerprint) {
                return Ok(());
            }

            // Randomly choose a slot to evict from and swap
            let slot = (target_bucket_index % BUCKET_SIZE as u32) as usize;
            evicted_fingerprint = self.swap_at_bucket(target_bucket_index, fingerprint, slot);

            // Recalculate the next target bucket based on the evicted fingerprint
            target_bucket_index =
                self.bucket_from_evicted(target_bucket_index, evicted_fingerprint);
        }
        // If MAX_EVICTIONS is reached, store the fingerprint in the eviction cache -- this avoids "missing" the item we couldn't insert so that lookups are still correct even when it's full
        self.eviction_cache.index = target_bucket_index;
        self.eviction_cache.fingerprint = evicted_fingerprint;
        self.eviction_cache.used = true;
        Err(CuckooFilterOpError::OutOfSpace)
    }

    /// Add item to filter. Returns Err if filter is full, or if item already exists.
    // pub fn insert_unique(item: &Input) -> Result<(), CuckooFilterOpError> {
    //     Ok(())
    // }

    /// Check if item is in filter
    pub fn lookup(&self, item: &Input) -> bool {
        let (candidate_1, candidate_2, fingerprint) = self.buckets_from_item(item);
        // Check cache
        if self.eviction_cache.used
            && fingerprint == self.eviction_cache.fingerprint
            && (self.eviction_cache.index == candidate_1
                || self.eviction_cache.index == candidate_2)
        {
            return true;
        }
        // Check buckets
        for &bucket_index in &[candidate_1, candidate_2] {
            for entry in self.data[bucket_index as usize] {
                if entry == fingerprint {
                    return true;
                }
            }
        }
        false
    }

    /// Delete an item from the filter
    pub fn delete(&mut self, item: &Input) -> Result<(), CuckooFilterOpError> {
        let (candidate_1, candidate_2, fingerprint) = self.buckets_from_item(item);
        // Check cache and clear if found
        if self.eviction_cache.used
            && fingerprint == self.eviction_cache.fingerprint
            && (self.eviction_cache.index == candidate_1
                || self.eviction_cache.index == candidate_2)
        {
            self.eviction_cache.reset();
            return Ok(());
        }
        // Check buckets and clear if found
        for &bucket_index in &[candidate_1, candidate_2] {
            for ref mut entry in self.data[bucket_index as usize] {
                if *entry == fingerprint {
                    *entry = 0;
                    return Ok(());
                }
            }
        }
        Err(CuckooFilterOpError::ItemDoesNotExist)
    }
}

/* -------------------- Unit Tests -------------------- */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_filter_normal_conditions() {
        let filter = CuckooFilter::new(128);
        assert!(filter.is_ok());
        let cf = filter.unwrap();
        assert_eq!(cf.length_u32, 128 / 4);
        assert_eq!(0, cf.data.len() as u32);
    }
}
