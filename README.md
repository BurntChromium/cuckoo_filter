# A Cuckoo Filter implementation

A Cuckoo Filter is an efficient data structure for determining set membership. Set membership answers the question "have I seen this thing before?". A Cuckoo Filter (CF) is similar to a Bloom Filter, but unlike a Bloom Filter, Cuckoo Filters support item deletion. Cuckoo Filters also form the backbone of certain cryptographic protocols.

Cuckoo Filters are a probabilistic data structure. This means that when the CF says "yes, I have seen this", it may be incorrect with a small probability (again, similar to a Bloom filter). However, if the CF answers "no, I haven't seen this", then this response is always correct. However, this correctness for the "_haven't_ seen this" statement depends on a particular implementation detail that not all CFs handle (it requires an eviction cache). This CF does use an eviction cache.  

This crate implements an opinionated Cuckoo Filter with reasonable parameters for balancing overall capacity and achieving near optimal space savings. This filter can hold up to 8.5 billion items. At maximum size, this CF should consume about 4 GiB of RAM. This implementation is based off of [this paper (PDF link)](https://www.cs.cmu.edu/~binfan/papers/conext14_cuckoofilter.pdf).

This implementation
- does not require the standard library (it enforces `![no_std]`), but it does require `alloc` (to use a Vector)
- does not support dynamic resizing (resizing would be very expensive: you'd have to build a new filter, then re-insert each item, potentially with a long series of evictions if you are trying to shrink the filter)

### Using this Cuckoo Filter

There are three primary APIs for the filter: `insert`, `lookup`, and `delete` (this follows the paper's naming convention). 

- `insert` places an item into the filter (well, it places the item's "fingerprint" into the filter)
- `lookup` checks if the item is in the filter, and returns `true` if found, or `false` if not found
- `delete` removes an item from the filter

```rust
// Try to make a filter supporting 128 items (creating a filter can fail if you try to request more than item limit of ~8 billion)
let try_filter = CuckooFilter::new(128, false);
let mut filter = try_filter.unwrap();
// Something to insert, as bytes
let item = [1u8, 2, 3, 4, 5];
// Insertions can fail if the filter is out of space
let insertion = cf.insert(&item);
assert!(insertion.is_ok());
// Lookups cannot fail - returns True or False
let is_found = cf.lookup(&item);
assert!(is_found);
// Deletion can fail if you try to delete something not in the filter 
let deletion = cf.delete(&item);
assert!(deletion.is_ok());
// Check that the item is no longer present
assert!(!filter.lookup(&item));
```

### To Do List

- ~~Unit tests~~ Basic unit tests are covered, now need to cover the edge cases
- Switch to a proper (64 bit) hash function instead of DBJ2
- Property tests
- Benchmarking
- Multi threading (first pass is probably an RwLock on the whole filter, more granular locks are possible but may not be worth the cost)
- Support `Hashable` objects so users don't need to get the bytes themselves?