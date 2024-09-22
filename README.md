# A Cuckoo Filter implementation

A Cuckoo Filter is an efficient data structure for determining set membership. Set membership answers the question "have I seen this thing before?". A Cuckoo Filter (CF) is similar to a Bloom Filter, but unlike a Bloom Filter, Cuckoo Filters support item deletion. Cuckoo Filters also form the backbone of certain cryptographic protocols.

Cuckoo Filters are a probabilistic data structure. This means that when the CF says "yes, I have seen this", it may be incorrect with a small probability (there is a risk of false positives in the event of a hash collision). However, if the CF answers "no, I haven't seen this", then this response is always correct. (The correctness of the statement "I _haven't_ seen this" depends on a particular implementation detail that not all CFs handle (it requires an eviction cache). This CF does use an eviction cache.)  

This crate is a library that implements Cuckoo Filter with reasonable parameters for balancing overall capacity and achieving near optimal space savings. This filter can hold up to ~8.5 billion items. At maximum size, this CF should consume about 8.5 GiB of RAM (each item consumes 1 byte). This implementation is based off of [this paper (PDF link)](https://www.cs.cmu.edu/~binfan/papers/conext14_cuckoofilter.pdf).

This implementation
- does not require the standard library (it enforces `![no_std]`), but it does require `alloc` (to use a Vector)
- does not support dynamic resizing (resizing would be very expensive: you'd have to build a new filter, then re-insert each item, potentially with a long series of evictions if you are trying to shrink the filter)

### Why not use a normal Hash Table?

Compared to a normal hash table, a Cuckoo Filter is much more efficient. This filter stores only 8 bits per item, and searching for an item (probing) operates in constant time because each item can only be in one of 9 fixed locations. (By contrast, the standard library `HashMap` may, theoretically, need to perform arbitrarily many collision resolution steps using [quadratic probing](https://en.wikipedia.org/wiki/Quadratic_probing), although this is unlikely.)

### The standard API for this Cuckoo Filter

There are three primary APIs for the filter: `insert`, `lookup`, and `delete` (this follows the paper's naming convention). 

- `insert` places an item into the filter (well, it places the item's "fingerprint" into the filter)
- `lookup` checks if the item is in the filter, and returns `true` if found, or `false` if not found
- `delete` removes an item from the filter

The Filter accepts any hash function which implements `Hasher + Default`. (Perf FYI: it calls `Default` on each operation to ensure idempotence, lacking a better supported way to reset a `Hasher`. This is normally not expensive, but if you're using a strange hash function, be aware).

There is a default hashing function provided (Murmur3) that is faster than Rust's default (SipHash).

```rust
// Try to make a filter supporting 128 items (creating a filter can fail if you try to request more than item limit of ~8 billion)
let try_filter = CuckooFilter::<Murmur3Hasher>::new(128, false);;
let mut filter = try_filter.unwrap();
// Something to insert
let item = "the cat says meow";
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

The Cuckoo Filter may report that it is full, despite there being empty slots left. This occurs when there are too many hash collisions on the data. You may want to create the filter with a bit of headroom to mitigate the risk of this. Unit testing indicates that this _usually_ doesn't happen until the filter is well over 95% full, but your luck may vary. (There is no way around this without removing data from the filter, which breaks semantic guarantees.)

Additional APIs are available, check the documentation for details.

### To Do List

- ~~Unit tests~~ Basic unit tests are covered, now need to cover the edge cases
- ~~Switch to a proper (64 bit) hash function instead of DBJ2~~
- Property tests
- Benchmarking
- Multi threading (first pass is probably an RwLock on the whole filter, more granular locks are possible but may not be worth the cost)
- ~~Support `Hashable` objects so users don't need to get the bytes themselves?~~