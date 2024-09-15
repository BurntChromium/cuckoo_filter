# A Cuckoo Filter implementation

A Cuckoo Filter is an efficient data structure for determining set membership. Set membership answers the question "have I seen this thing before?". A Cuckoo Filter (CF) is similar to a Bloom Filter, but unlike a Bloom Filter, Cuckoo Filters support item deletion. Cuckoo Filters also form the backbone of certain cryptographic protocols.

Cuckoo Filters are a probabilistic data structure. This means that when the CF says "yes, I have seen this", it may be incorrect with a small probability (again, similar to a Bloom filter). However, if the CF answers "no, I haven't seen this", then this response is always correct. However, this correctness for the "_haven't_ seen this" statement depends on a particular implementation detail that not all CFs handle (it requires an eviction cache). This CF does use an eviction cache.  

This crate implements a Cuckoo Filter with reasonable parameters for balancing overall capacity and achieving near optimal space savings. This filter can hold up to 8.5 billion items. At maximum size, this CF should consume about 4 GiB of RAM. This implementation is based off of [this paper (PDF link)](https://www.cs.cmu.edu/~binfan/papers/conext14_cuckoofilter.pdf).

### Using the Filter

There are three primary APIs for the filter: `insert`, `lookup`, and `delete` (this follows the paper's naming convention). 

### To Do List

- Unit tests
- Multi threading (first pass is probably an RwLock on the whole filter, more granular locks are possible but may not be worth the cost)
- Better hash functions