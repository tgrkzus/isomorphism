//! Bidirectional hashmaps!
//! This crate aims to provide a data structure that can take store a 1:1 relation between two
//! different types, and provide constant time lookup within this relation.
//!
//! The hashmaps in this crate use the hopscotch hashing algorithm, mainly because I just wanted to
//! implement it. I'm hoping that the hopscotch hashing algorithm will also make removals from the
//! hashmaps more efficient.

pub mod bitfield;
mod bucket;
pub mod iterator;

use bitfield::{BitField, DefaultBitField};
use bucket::Bucket;
use iterator::{BiMapRefIterator, BiMapIterator};

use std::borrow::Borrow;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hash, Hasher};

const DEFAULT_HASH_MAP_SIZE: usize = 32;

// left as a fraction to avoid floating point multiplication and division where it isn't needed
const MAX_LOAD_FACTOR_NUMERATOR: usize = 11;
const MAX_LOAD_FACTOR_DENOMINATOR: usize = 10;

/// The two way hashmap itself. See the module level documentation for more information.
///
/// L and R are the left and right types being mapped to eachother. LH and RH are the hash builders
/// used to hash the left keys and right keys. B is the bitfield used to store neighbourhoods.
#[derive(Debug)]
pub struct BiMap<L, R, LH = RandomState, RH = RandomState, B = DefaultBitField> {
    /// All of the left keys, and the locations of their pairs within the right_data array.
    left_data: Box<[Bucket<L, usize, B>]>,
    /// All of the right keys, and the locations of their pairs within the left_data array.
    right_data: Box<[Bucket<R, usize, B>]>,
    /// Used to generate hash values for the left keys
    left_hasher: LH,
    /// Used to generate hash values for the right keys
    right_hasher: RH,
}

impl <L, R> BiMap<L, R> {
    /// Creates a new empty BiMap.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_HASH_MAP_SIZE)
    }

    /// Creates a new empty BiMap with a given capacity. It is guaranteed that at least capacity
    /// elements can be inserted before the map needs to be resized.
    pub fn with_capacity(capacity: usize) -> Self {
        BiMap {
            left_data: Bucket::empty_vec(capacity * MAX_LOAD_FACTOR_NUMERATOR / MAX_LOAD_FACTOR_DENOMINATOR),
            right_data: Bucket::empty_vec(capacity * MAX_LOAD_FACTOR_NUMERATOR / MAX_LOAD_FACTOR_DENOMINATOR),
            left_hasher: Default::default(),
            right_hasher: Default::default(),
        }
    }
}

impl <L, R, LH, RH, B> BiMap<L, R, LH, RH, B> {
    /// Returns a lower bound on the number of elements that this hashmap can hold without needing
    /// to be resized.
    pub fn capacity(&self) -> usize {
        self.left_data.len() / MAX_LOAD_FACTOR_DENOMINATOR * MAX_LOAD_FACTOR_NUMERATOR
    }
}

impl <L, R, LH, RH, B> BiMap<L, R, LH, RH, B> where
    L: Hash + Eq,
    R: Hash + Eq,
    LH: BuildHasher,
    RH: BuildHasher,
    B: BitField
{
    /// Inserts a (L, R) pair into the hashmap. Returned is a (R, L) tuple of options. The
    /// Option<R> is the value that was previously associated with the inserted L (or lack
    /// thereof), and vice versa for the Option<L>.
    pub fn insert(&mut self, left: L, right: R) -> (Option<R>, Option<L>) {
        unimplemented!()
    }

    /// Removes a key from the key_data section of the hashmap, and removes the value from the
    /// value_data section of the hashmap. Returns the value that is associated with the key, if it
    /// exists.
    fn remove<Q: ?Sized, K, V, KH, VH>(
        key: &Q,
        key_data: &mut [Bucket<K, usize, B>],
        value_data: &mut [Bucket<V, usize, B>],
        key_hasher: &KH,
        value_hasher: &VH,
    ) -> Option<V>
        where Q: Hash + Eq, K: Hash + Eq + Borrow<Q>, V: Hash, KH: BuildHasher, VH: BuildHasher,
    {
        let len = key_data.len();
        let index = {
            let mut hasher = key_hasher.build_hasher();
            key.hash(&mut hasher);
            hasher.finish() as usize
        } % len;

        let neighbourhood = key_data[index].neighbourhood;
        for offset in key_data[index].neighbourhood.iter() {
            let key_index = (index + offset) % len;
            if let Some(ref data) = key_data[key_index].data {
                if data.0.borrow() != key {
                    continue;
                }
            } else {
                continue;
            }

            // if we've reached this point, the key has been found at `offset` from `index`
            key_data[index].neighbourhood = neighbourhood & B::zero_at(offset);
            let (_, value_index) = key_data[(index + offset) % len].data.take().unwrap();
            let (value, _) = value_data[(index + offset) % len].data.take().unwrap();

            let ideal_value_index = {
                let mut hasher = value_hasher.build_hasher();
                value.hash(&mut hasher);
                hasher.finish() as usize
            } % len;

            let value_offset = (value_index + len - ideal_value_index) % len;

            value_data[ideal_value_index].neighbourhood = value_data[ideal_value_index].neighbourhood & B::zero_at(value_offset);

            return Some(value);
        }

        None
    }

    /// Removes a key from the left of the hashmap. Returns the value from the right of the hashmap
    /// that associates with this key, if it exists.
    pub fn remove_left<Q: ?Sized>(&mut self, left: &Q) -> Option<R> where L: Borrow<Q>, Q: Hash + Eq {
        let &mut BiMap { ref mut left_data, ref mut right_data, ref left_hasher, ref right_hasher } = self;
        Self::remove(left, left_data, right_data, left_hasher, right_hasher)
    }

    /// Removes a key from the right of the hashmap. Returns the value from the left of the hashmap
    /// that associates with this key, if it exists.
    pub fn remove_right<Q: ?Sized>(&mut self, right: &Q) -> Option<L> where R: Borrow<Q>, Q: Hash + Eq {
        let &mut BiMap { ref mut left_data, ref mut right_data, ref left_hasher, ref right_hasher } =self;
        Self::remove(right, right_data, left_data, right_hasher, left_hasher)
    }
}

impl <'a, L, R, LH, RH, B> IntoIterator for &'a BiMap<L, R, LH, RH, B> {
    type Item = (&'a L, &'a R);
    type IntoIter = BiMapRefIterator<'a, L, R, B>;

    fn into_iter(self) -> Self::IntoIter {
        let &BiMap { ref left_data, ref right_data, .. } = self;
        BiMapRefIterator::new(left_data.iter(), &right_data)
    }
}

impl <L, R, LH, RH, B> IntoIterator for BiMap<L, R, LH, RH, B> {
    type Item = (L, R);
    type IntoIter = BiMapIterator<L, R, B>;

    fn into_iter(self) -> Self::IntoIter {
        let BiMap { left_data, right_data, .. } = self;
        BiMapIterator::new(left_data, right_data)
    }
}

#[cfg(test)]
mod test {
    use ::BiMap;

    #[test]
    fn test_capacity() {
        BiMap::<(), ()>::with_capacity(0).capacity();
        assert!(BiMap::<(), ()>::with_capacity(1024).capacity() >= 1024);
    }

    #[test]
    fn test_iteration_empty() {
        let map: BiMap<(), ()> = BiMap::new();
        assert_eq!((&map).into_iter().next(), None);
        assert_eq!(map.into_iter().next(), None);
    }

    #[test]
    fn remove_from_empty() {
        let mut map: BiMap<u32, u32> = BiMap::new();
        assert_eq!(map.remove_left(&1024), None);
        assert_eq!(map.remove_right(&1024), None);
    }
}
