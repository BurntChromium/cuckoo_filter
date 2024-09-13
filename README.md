# A Cuckoo Filter implementation

A Cuckoo Filter is an efficient data structure for determining "set membership" (i.e. 'have I seen this thing before?'). It is similar to a Bloom Filter, but unlike a Bloom Filter, Cuckoo Filters support item deletion. Cuckoo Filters also form the backbone of certain cryptographic protocols.

This crate implements a Cuckoo Filter with reasonable parameters for balancing overall capacity and achieving near optimal space savings. This filter can hold up to 17 billion items. At maximum size, this CF should consume about 4 GiB of RAM. This implementation is based off of [this paper (PDF link)](https://www.cs.cmu.edu/~binfan/papers/conext14_cuckoofilter.pdf).