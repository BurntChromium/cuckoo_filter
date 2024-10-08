//! # Cuckoo Filter
//!
//! This implementation is based on the paper _Cuckoo Filter: Practically Better Than Bloom_, by Fan et. al.
//!
//! The paper recommends a (2, 4) CF (2 possible buckets for each item, and 4 fingerprints in each bucket) because it's space optimal for practical false positive rates. Assuming our CF will hold up to a few billion items, 6 bits per fingerprint is sufficient (24 bits per bucket), but we round up to one byte per fingerprint for the sake of practicality.
//!
//! The paper's authors have provided a reference C++ implementation in this repository: <https://github.com/efficient/cuckoofilter>

use alloc::vec;
use alloc::vec::Vec;
use core::default::Default;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;

pub type BucketIndex = u32;
pub type Fingerprint = u8;

const MAX_EVICTIONS: u16 = 500;
/// Each bucket holds 4 fingerprints
const BUCKET_SIZE: usize = 4;
/// With 32 bit hash functions, we can hold (address) up to 32 bits worth of buckets
const MAX_BUCKETS: usize = u32::MAX as usize;
/// The item limit needs to respect the POW(2) rounding we do
const ITEM_LIMIT: usize = (MAX_BUCKETS.next_power_of_two() >> 1) * BUCKET_SIZE;

/// An eviction cache holds an item that we couldn't reinsert
///
/// An item being here means that the filter is "probabilistically full". It may not be technically 100% saturated, but we ran into so many hash collisions that we had to stop. (Using a bad hash function may result in being "full" early)
#[derive(Debug)]
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

/// Possible errors for the Cuckoo Filter
#[derive(Debug, Eq, PartialEq)]
pub enum CuckooFilterError {
    /// Requested capacity at initialization exceeds item limit
    CapacityExceedsItemLimit,
    /// Model had too many collisions and ran out of effective space
    OutOfSpace,
    /// For `insert_unique`, when item already exists
    ItemAlreadyExists,
    /// For `delete`, when item doesn't exist
    ItemDoesNotExist,
}

/// A Cuckoo Filter that holds up to 8.5 billion items
///
/// ### Implementation Notes
///
/// - The eviction cache holds an item that we couldn't reinsert, and represents when the data structure is effectively/probabilistically full (as opposed to mechanically full)
/// - The `length_u32` parameter lets us wrap around (modulo) bucket indices that would be too large
#[derive(Debug)]
pub struct CuckooFilter<H: Hasher + Default> {
    eviction_cache: EvictionVictim,
    eviction_counts: Vec<u16>,
    swap_counts: Vec<u16>,
    data_trace: Vec<(BucketIndex, BucketIndex, Fingerprint)>,
    data: Vec<[Fingerprint; BUCKET_SIZE]>,
    length_u32: u32,
    hasher: H,
    phantom: PhantomData<H>,
}

impl<H: Hasher + Default> CuckooFilter<H> {
    /// Try to create a new Cuckoo Filter
    ///
    /// This can fail if the desired filter would be too large. This evaluation can optionally be performed at compile time. To do that, `max_items` must be a `const` variable!
    ///
    /// ### Caveats
    ///
    /// - We must round the size of our backing vector of data to a power of two. This is because we will modulo the index when our hash function creates a bucket index bigger than the backing vector. If the data was *not* a power of 2, our indices would be subject to "Modulo bias" and cause more hash collisions.
    ///
    /// Add item to filter. Returns Err if filter is full
    ///
    /// ```
    /// use cuckoo_filter::CuckooFilter;
    /// use cuckoo_filter::Murmur3Hasher;
    ///
    /// let try_filter = CuckooFilter::<Murmur3Hasher>::new(128, false);
    /// assert!(try_filter.is_ok())
    /// ```
    ///
    /// # Errors
    ///
    /// - `CuckooFilterError::CapacityExceedsItemLimit` you tried to request a filter with a capacity larger than `ITEM_LIMIT`
    pub fn new(
        max_items: usize,
        compile_time_check: bool,
    ) -> Result<CuckooFilter<H>, CuckooFilterError> {
        // Check item limit
        if compile_time_check {
            assert!(
                max_items < ITEM_LIMIT,
                "cuckoo filter initialized with too many items"
            );
        }
        if max_items > ITEM_LIMIT {
            return Err(CuckooFilterError::CapacityExceedsItemLimit);
        }
        // If we didn't care about modulo bias, we could use this many buckets
        let number_of_buckets_exact: usize = max_items / BUCKET_SIZE;
        // But to avoid hash collisions, we round up
        let number_of_buckets_actual: usize = number_of_buckets_exact.next_power_of_two();
        Ok(CuckooFilter {
            eviction_cache: EvictionVictim::new(),
            eviction_counts: Vec::new(),
            swap_counts: Vec::new(),
            data_trace: Vec::new(),
            data: vec![[0u8; BUCKET_SIZE]; number_of_buckets_actual],
            length_u32: number_of_buckets_actual as u32,
            hasher: H::default(),
            phantom: PhantomData,
        })
    }

    /// Approximately how many bytes is this CF using?
    pub fn estimate_size(&self) -> usize {
        self.data.len() * BUCKET_SIZE
    }

    /// Is the Cuckoo Filter full of items (practically speaking)?
    ///
    /// Criteria is that we have something left over in the Eviction cache after trying to move it for the max number of kicks
    pub fn is_full(&self) -> bool {
        self.eviction_cache.used
    }

    /// Given a hash value (digest), compute the buckets and fingerprint
    ///
    /// We modulo the bucket indices because the hash may output a value larger than the true length of the backing data array. However, because our length is a power of 2, we can use bitwise AND.
    ///
    /// This is (mostly) Equation 1 in section 3.1 of the paper
    ///
    /// However, unlike Equation 1, we follow the reference implementation from the authors and instead compute bucket 2 by XORing with a magic constant
    fn digest_to_buckets(&self, hash_value: u64) -> (BucketIndex, BucketIndex, Fingerprint) {
        let upper_bits: u32 = (hash_value >> 32) as u32;
        let fingerprint_u32: u32 = upper_bits & ((1 << 8) - 1);
        let bucket_1 = hash_value as u32 % self.length_u32; // lower bits
        let bucket_2 = (bucket_1 ^ fingerprint_u32.wrapping_mul(0x5bd1e995)) % self.length_u32;
        (bucket_1, bucket_2, fingerprint_u32 as u8)
    }

    /// Calculate the buckets given a `Hash`able item
    fn buckets_from_item<T: Hash>(&mut self, item: &T) -> (BucketIndex, BucketIndex, Fingerprint) {
        // To preserve idempotence, we need to reset the hasher's state every time
        self.hasher = H::default();
        item.hash(&mut self.hasher);
        let hash_value: u64 = self.hasher.finish();
        self.digest_to_buckets(hash_value)
    }

    ///Compute buckets from a provided hash function without touching the internal state. This doesn't use the `Hash` trait, so it requires having access to the bytes of the item.
    ///
    /// This has a theoretical performance benefit because we don't need to reset the hasher (call `H::default()`). Your mileage may vary.
    fn buckets_from_item_stateless(
        &self,
        item: &[u8],
        hasher: fn(&[u8]) -> u64,
    ) -> (BucketIndex, BucketIndex, Fingerprint) {
        let hash_value: u64 = hasher(item);
        self.digest_to_buckets(hash_value)
    }

    /// We can calculate a new bucket for an evicted item despite only having that item's fingerprint
    ///
    /// This normally would be Equation 2 in Section 3.1 of the paper, but because we use the magic number optimization that no longer applies
    // That code would have been (old_bucket ^ (fingerprint as u32)) & (self.length_u32 - 1)
    fn bucket_from_evicted(
        &self,
        old_bucket: BucketIndex,
        fingerprint: Fingerprint,
    ) -> BucketIndex {
        (old_bucket ^ (fingerprint as u32).wrapping_mul(0x5bd1e995)) % self.length_u32
    }

    /// Internal method to try inserting a fingerprint into a bucket.
    ///
    /// True means success, false means the bucket was full
    fn try_insert_at_bucket(
        &mut self,
        bucket_index: BucketIndex,
        fingerprint: Fingerprint,
    ) -> bool {
        let bucket = &mut self.data[bucket_index as usize];
        for slot in bucket.iter_mut() {
            if *slot == 0 {
                *slot = fingerprint;
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
        let bucket = &mut self.data[bucket_index as usize];
        let evicted_fingerprint = bucket[slot];
        bucket[slot] = fingerprint;
        evicted_fingerprint
    }

    /// Tries to place an item into the filter
    ///
    /// Internal method, public APIs wrap this
    fn internal_insert(
        &mut self,
        candidate_1: u32,
        candidate_2: u32,
        fingerprint: u8,
    ) -> Result<(), CuckooFilterError> {
        // If the cache is filled then we're (effectively) out of space
        if self.eviction_cache.used {
            return Err(CuckooFilterError::OutOfSpace);
        }
        // Try inserting into either bucket
        for &bucket_index in &[candidate_1, candidate_2] {
            if self.try_insert_at_bucket(bucket_index, fingerprint) {
                self.eviction_counts.push(0);
                self.data_trace
                    .push((candidate_1, candidate_2, fingerprint));
                self.swap_counts.push(0);
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

        let mut swaps: u16 = 0;

        for kick in 0..MAX_EVICTIONS {
            // If kick == 0, we already tried inserting into a bucket
            if kick > 0 && self.try_insert_at_bucket(target_bucket_index, evicted_fingerprint) {
                self.eviction_counts.push(kick as u16);
                self.data_trace
                    .push((candidate_1, candidate_2, fingerprint));
                self.swap_counts.push(swaps);
                return Ok(());
            }

            // Randomly choose a slot to evict from and swap
            let slot = (target_bucket_index % BUCKET_SIZE as u32) as usize;
            evicted_fingerprint = self.swap_at_bucket(target_bucket_index, fingerprint, slot);
            swaps += 1;

            // Recalculate the next target bucket based on the evicted fingerprint
            target_bucket_index =
                self.bucket_from_evicted(target_bucket_index, evicted_fingerprint);
        }
        // If MAX_EVICTIONS is reached, store the fingerprint in the eviction cache -- this avoids "missing" the item we couldn't insert so that lookups are still correct even when it's full
        self.eviction_cache.index = target_bucket_index;
        self.eviction_cache.fingerprint = evicted_fingerprint;
        self.eviction_cache.used = true;
        self.eviction_counts.push(MAX_EVICTIONS as u16);
        self.swap_counts.push(swaps);
        Err(CuckooFilterError::OutOfSpace)
    }

    /// Add item to filter. Returns Err if filter is full
    ///
    /// ```
    /// use cuckoo_filter::CuckooFilter;
    /// use cuckoo_filter::Murmur3Hasher;
    ///
    /// let try_filter = CuckooFilter::<Murmur3Hasher>::new(128, false);
    /// let mut filter = try_filter.unwrap();
    /// let ins = filter.insert(&"hello, I am some data");
    /// assert!(ins.is_ok());
    /// ```
    ///
    /// # Errors
    ///
    /// - `CuckooFilterError::OutOfSpace`: the filter is "practically" full and will no longer accept items (the last insert failed because it tried to evict too many items). This can occur _before_ the filter is "theoretically" full due to hash collisions.
    pub fn insert<T: Hash>(&mut self, item: &T) -> Result<(), CuckooFilterError> {
        let (candidate_1, candidate_2, fingerprint) = self.buckets_from_item(item);
        self.internal_insert(candidate_1, candidate_2, fingerprint)
    }

    /// Add item to filter, but use a provided stateless hash function. Requires the item to be passed as bytes (because we're bypassing the `Hash` Trait).
    ///
    /// This allows items to be inserted that don't implement `Hash`, for whatever reason.
    ///
    /// Technically, this should be "faster" because it doesn't require resetting the internal Hasher state, but depending on compiler optimizations it may not pan out. Benchmark on your system first!
    ///
    /// ```
    /// use cuckoo_filter::*;
    ///
    /// let try_filter = CuckooFilter::<Murmur3Hasher>::new(128, false);
    /// let mut filter = try_filter.unwrap();
    /// let ins = filter.insert_stateless(&"hello, I am some data".as_bytes(), murmur3_x86_64bit);
    /// assert!(ins.is_ok());
    /// ```
    ///
    /// # Errors
    ///
    /// - `CuckooFilterError::OutOfSpace`: the filter is "practically" full and will no longer accept items (the last insert failed because it tried to evict too many items). This can occur _before_ the filter is "theoretically" full due to hash collisions.
    pub fn insert_stateless(
        &mut self,
        item: &[u8],
        hash_function: fn(&[u8]) -> u64,
    ) -> Result<(), CuckooFilterError> {
        let (candidate_1, candidate_2, fingerprint) =
            self.buckets_from_item_stateless(item, hash_function);
        self.internal_insert(candidate_1, candidate_2, fingerprint)
    }

    /// Identifies if an item is in the filter
    ///
    /// This is an internal method that public APIs wrap around
    fn internal_lookup(&self, candidate_1: u32, candidate_2: u32, fingerprint: u8) -> bool {
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

    /// Add item to filter. Returns Err if filter is full, or if item already exists.
    // pub fn insert_unique(item: &Input) -> Result<(), CuckooFilterOpError> {
    //     Ok(())
    // }

    /// Check if item is in filter
    ///
    /// ```
    /// use cuckoo_filter::*;
    ///
    /// let try_filter = CuckooFilter::<Murmur3Hasher>::new(128, false);
    /// let mut filter = try_filter.unwrap();
    ///
    /// let item = "hello, I am some data";
    /// let _ = filter.insert(&item);
    /// let was_found = filter.lookup(&item);
    /// assert!(was_found);
    /// ```
    pub fn lookup<T: Hash>(&mut self, item: &T) -> bool {
        let (candidate_1, candidate_2, fingerprint) = self.buckets_from_item(item);
        self.internal_lookup(candidate_1, candidate_2, fingerprint)
    }

    /// Check if item is in filter, but use a provided stateless hash function.
    ///
    /// ```
    /// use cuckoo_filter::*;
    ///
    /// let try_filter = CuckooFilter::<Murmur3Hasher>::new(128, false);
    /// let mut filter = try_filter.unwrap();
    ///
    /// let item = "hello, I am some data";
    /// let _ = filter.insert_stateless(&item.as_bytes(), murmur3_x86_64bit);
    /// let was_found = filter.lookup_stateless(&item.as_bytes(), murmur3_x86_64bit);
    /// assert!(was_found);
    /// ```
    pub fn lookup_stateless(&self, item: &[u8], hash_function: fn(&[u8]) -> u64) -> bool {
        let (candidate_1, candidate_2, fingerprint) =
            self.buckets_from_item_stateless(item, hash_function);
        self.internal_lookup(candidate_1, candidate_2, fingerprint)
    }

    fn internal_delete(
        &mut self,
        candidate_1: u32,
        candidate_2: u32,
        fingerprint: u8,
    ) -> Result<(), CuckooFilterError> {
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
            for entry in &mut self.data[bucket_index as usize] {
                if *entry == fingerprint {
                    *entry = 0;
                    return Ok(());
                }
            }
        }
        Err(CuckooFilterError::ItemDoesNotExist)
    }

    /// Delete an item from the filter
    ///
    /// ```
    /// use cuckoo_filter::*;
    ///
    /// let try_filter = CuckooFilter::<Murmur3Hasher>::new(128, false);
    /// let mut filter = try_filter.unwrap();
    ///
    /// let item = "hello, I am some data";
    /// let _ = filter.insert(&item);
    /// let was_found = filter.lookup(&item);
    /// assert!(was_found);
    ///
    /// let was_deleted = filter.delete(&item);
    /// assert!(was_deleted.is_ok());
    /// ```
    pub fn delete<T: Hash>(&mut self, item: &T) -> Result<(), CuckooFilterError> {
        let (candidate_1, candidate_2, fingerprint) = self.buckets_from_item(item);
        self.internal_delete(candidate_1, candidate_2, fingerprint)
    }

    /// Delete an item from the filter, using a provided stateless hash function
    ///
    /// ```
    /// use cuckoo_filter::*;
    ///
    /// let try_filter = CuckooFilter::<Murmur3Hasher>::new(128, false);
    /// let mut filter = try_filter.unwrap();
    ///
    /// let item = "hello, I am some data";
    /// let _ = filter.insert_stateless(&item.as_bytes(), murmur3_x86_64bit);
    /// let was_found = filter.lookup_stateless(&item.as_bytes(), murmur3_x86_64bit);
    /// assert!(was_found);
    ///
    /// let was_deleted = filter.delete_stateless(&item.as_bytes(), murmur3_x86_64bit);
    /// assert!(was_deleted.is_ok());
    /// ```
    pub fn delete_stateless(
        &mut self,
        item: &[u8],
        hash_function: fn(&[u8]) -> u64,
    ) -> Result<(), CuckooFilterError> {
        let (candidate_1, candidate_2, fingerprint) =
            self.buckets_from_item_stateless(item, hash_function);
        self.internal_delete(candidate_1, candidate_2, fingerprint)
    }
}

/* -------------------- Unit Tests -------------------- */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{murmur3_x86_64bit, Murmur3Hasher};
    use rand::{distributions::Uniform, prelude::*};
    use rand_chacha::ChaCha8Rng;

    // Utility fns
    fn get_random_string(rng: &mut ChaCha8Rng, len: usize) -> String {
        rng.sample_iter::<char, _>(&rand::distributions::Standard)
            .take(len)
            .map(char::from)
            .collect()
    }

    #[test]
    fn make_filter_normal_conditions() {
        let filter = CuckooFilter::<Murmur3Hasher>::new(128, false);
        assert!(filter.is_ok());
        let cf = filter.unwrap();
        assert_eq!(cf.length_u32, 128 / 4);
        assert_eq!(128 / 4, cf.data.len() as u32);
    }

    // The filter should hold exactly the item limit but no more (error is around secondary checks relating to power of 2 rounding)
    #[test]
    fn make_filter_item_limit_boundary() {
        let filter = CuckooFilter::<Murmur3Hasher>::new(ITEM_LIMIT, false);
        assert!(filter.is_ok());
        let filter2 = CuckooFilter::<Murmur3Hasher>::new(ITEM_LIMIT + 1, false);
        assert!(filter2.is_err());
        assert_eq!(
            CuckooFilterError::CapacityExceedsItemLimit,
            filter2.unwrap_err()
        );
    }

    // Check that the comp time check throws
    #[test]
    #[should_panic(expected = "cuckoo filter initialized with too many items")]
    fn make_filter_comp_time_check() {
        const TOO_MANY_ITEMS: usize = ITEM_LIMIT + 1;
        let _filter = CuckooFilter::<Murmur3Hasher>::new(TOO_MANY_ITEMS, true);
    }

    #[test]
    fn check_size() {
        let filter = CuckooFilter::<Murmur3Hasher>::new(128, false);
        let cf = filter.unwrap();
        assert_eq!(cf.estimate_size(), 128);
    }

    #[test]
    fn check_bucket_equivalence() {
        let filter = CuckooFilter::<Murmur3Hasher>::new(1, false);
        let cf = filter.unwrap();
        let (b1, b2, f) = cf.digest_to_buckets(murmur3_x86_64bit(&"test".as_bytes()));
        let b2alt = cf.bucket_from_evicted(b1, f);
        let b1alt = cf.bucket_from_evicted(b2, f);
        assert_eq!(b1, b1alt);
        assert_eq!(b2, b2alt);
    }

    #[test]
    fn insert_bytes() {
        let filter = CuckooFilter::<Murmur3Hasher>::new(128, false);
        let mut cf = filter.unwrap();
        let r = cf.insert(&[1, 2, 3, 4, 5]);
        assert!(r.is_ok());
    }

    #[test]
    fn insert_number() {
        let filter = CuckooFilter::<Murmur3Hasher>::new(128, false);
        let mut cf = filter.unwrap();
        let r = cf.insert(&19384);
        assert!(r.is_ok());
    }

    #[test]
    fn insert_string() {
        let filter = CuckooFilter::<Murmur3Hasher>::new(128, false);
        let mut cf = filter.unwrap();
        let r = cf.insert(&"hello");
        assert!(r.is_ok());
    }

    #[test]
    fn retrieve_item() {
        let filter = CuckooFilter::<Murmur3Hasher>::new(128, false);
        let mut cf = filter.unwrap();
        let item = [1u8, 2, 3, 4, 5];
        let r = cf.insert(&item);
        assert!(r.is_ok());
        let is_found = cf.lookup(&item);
        assert!(is_found);
    }

    #[test]
    fn delete_item() {
        let filter = CuckooFilter::<Murmur3Hasher>::new(128, false);
        let mut cf = filter.unwrap();
        let item = [1u8, 2, 3, 4, 5];
        let r = cf.insert(&item);
        assert!(r.is_ok());
        let is_found = cf.lookup(&item);
        assert!(is_found);
        let d = cf.delete(&item);
        assert!(d.is_ok());
        // Check that the item is no longer present
        assert!(!cf.lookup(&item));
    }

    // LOAD TESTS: realistically, the filter will fail to fill due to hash collisions before it's "theoretically" full - but we should be able to fill most of it! This is disabled by default due to load
    #[test]
    #[ignore]
    fn load_test_10m_ints() {
        const SIZE: usize = 10_000_000;
        let between = Uniform::from(0..u64::MAX);
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let maybe_filter = CuckooFilter::<Murmur3Hasher>::new(SIZE, false);
        let mut filter = maybe_filter.unwrap();
        let mut success_count: usize = 0;
        for _ in 0..SIZE {
            let i = rng.sample(between);
            let r = filter.insert(&i);
            if r.is_ok() {
                success_count += 1;
            }
        }
        println!("successes: {success_count} / trials: {SIZE}");
        assert!((success_count as f32 / SIZE as f32) > 0.95f32);
    }

    #[test]
    fn load_test_ten_thousand_str() {
        // Initialize
        const SIZE: usize = 10_000;
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let maybe_filter = CuckooFilter::<Murmur3Hasher>::new(SIZE, false);
        let mut filter = maybe_filter.unwrap();
        let mut success_count: usize = 0;
        let mut cache: Vec<String> = Vec::with_capacity(SIZE);

        // Insert random strings
        for i in 0..SIZE {
            let random_string = get_random_string(&mut rng, (i % 12) + 1);
            let r = filter.insert(&random_string);
            if r.is_ok() {
                success_count += 1;
                // Check that the random string is present
                assert!(filter.lookup(&random_string));
                cache.push(random_string);
            }
        }

        println!("successes: {success_count} / trials: {SIZE}");
        // Check that at least 95% of writes succeeded (before running out of space)
        assert!((success_count as f32 / SIZE as f32) > 0.95f32);
        // Consistency check
        assert_eq!(cache.len(), success_count);

        // Try to find every item that we inserted
        let mut check_count: usize = 0;
        for i in cache.iter() {
            if filter.lookup(i) {
                check_count += 1;
            }
        }
        println!("checks: {check_count} / trials: {SIZE}");
        assert_eq!(check_count, cache.len());
    }

    #[test]
    fn load_test_ten_thousand_str_stateless() {
        // Initialize
        const SIZE: usize = 10_000;
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let maybe_filter = CuckooFilter::<Murmur3Hasher>::new(SIZE, false);
        let mut filter = maybe_filter.unwrap();
        let mut success_count: usize = 0;
        let mut cache: Vec<String> = Vec::with_capacity(SIZE);

        // Insert random strings
        for i in 0..SIZE {
            let random_string = get_random_string(&mut rng, (i % 12) + 1);
            let r = filter.insert_stateless(&random_string.as_bytes(), murmur3_x86_64bit);
            if r.is_ok() {
                success_count += 1;
                // Check that the random string is present
                assert!(filter.lookup_stateless(&random_string.as_bytes(), murmur3_x86_64bit));
                cache.push(random_string);
            }
        }

        println!("successes: {success_count} / trials: {SIZE}");
        println!(
            "number of items that required swaps {}",
            filter.swap_counts.iter().filter(|x| **x > 0).count()
        );
        println!(
            "total kicks: {}",
            filter.eviction_counts.iter().sum::<u16>()
        );
        // Check that at least 95% of writes succeeded (before running out of space)
        assert!((success_count as f32 / SIZE as f32) > 0.95f32);
        // Consistency check
        assert_eq!(cache.len(), success_count);
        // Compute cumulative evictions
        let mut cumulative_evicts: Vec<usize> = Vec::with_capacity(filter.eviction_counts.len());
        let mut running_total: usize = 0;
        for i in filter.eviction_counts.iter() {
            running_total += *i as usize;
            cumulative_evicts.push(running_total);
        }

        // Try to find every item that we inserted
        let mut check_count: usize = 0;
        for (index, i) in cache.iter().enumerate() {
            if filter.lookup_stateless(i.as_bytes(), murmur3_x86_64bit) {
                check_count += 1;
            } else {
                println!(
                    "{index}th item not found: {} kicks, {} swaps, {} cumulative kicks",
                    filter.eviction_counts[index],
                    filter.swap_counts[index],
                    cumulative_evicts[index]
                );
            }
        }
        println!("checks: {check_count} / trials: {SIZE}");
        assert_eq!(check_count, cache.len());
    }
}
