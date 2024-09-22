//! # Cuckoo Filter implementation
//!
//! A Cuckoo Filter is an efficient data structure for determining set membership. Set membership answers the question "have I seen this thing before?". A Cuckoo Filter (CF) is similar to a Bloom Filter, but unlike a Bloom Filter, Cuckoo Filters support item deletion. Cuckoo Filters also form the backbone of certain cryptographic protocols.
//!
//! This crate implements a Cuckoo Filter with reasonable parameters for balancing overall capacity and achieving near optimal space savings. This filter can hold up to 8.5 billion items. At maximum size, this CF should consume about 8.5 GiB of RAM.
//!
//! This implementation supports `![no_std]`, but it does require `alloc` (to use a Vector).
//!
//! ### Using this Cuckoo Filter
//! There are three primary APIs for the filter: `insert`, `lookup`, and `delete` (this follows the paper's naming convention).
//!
//! - `insert` places an item into the filter (well, it places the item's "fingerprint" into the filter)
//! - `lookup` checks if the item is in the filter, and returns `true` if found, or `false` if not found
//! - `delete` removes an item from the filter
//!
//! ```rust
//! use cuckoo_filter::CuckooFilter;
//! use cuckoo_filter::Murmur3Hasher;
//!
//! // Try to make a filter supporting 128 items (can fail if you try to request more than item limit)
//! let try_filter = CuckooFilter::<Murmur3Hasher>::new(128, false);
//! let mut filter = try_filter.unwrap();
//!
//! // Something to insert
//! let item = "the cat says meow";
//! // Insertions can fail if the filter is out of space
//! let insertion = filter.insert(&item);
//! assert!(insertion.is_ok());
//!
//! // Lookups cannot fail - returns True or False
//! let is_found = filter.lookup(&item);
//! assert!(is_found);
//!
//! // Deletion can fail if you try to delete something not in the filter
//! let deletion = filter.delete(&item);
//! assert!(deletion.is_ok());
//! // Check that the item is no longer present
//! assert!(!filter.lookup(&item));
//! ```
//!
//! The Cuckoo Filter may report that it is full, despite there being empty slots left. This occurs when there are too many hash collisions on the data. You may want to create the filter with a bit of headroom to mitigate the risk of this. Unit testing indicates that this _usually_ doesn't happen until the filter is well over 95% full, but your luck may vary.

// We use the standard library in tests only, not for
#![cfg_attr(not(test), no_std)]
extern crate alloc;

mod filter;
mod murmur3;

pub use filter::CuckooFilter;
pub use filter::CuckooFilterError;
pub use murmur3::murmur3_x86_64bit;
pub use murmur3::Murmur3Hasher;
