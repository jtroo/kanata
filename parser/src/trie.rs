//! Wrapper around a trie type for (hopefully) easier swapping of libraries if desired.

use bytemuck::cast_slice;
use patricia_tree::map::PatriciaMap;

pub type TrieKeyElement = u16;
pub type TrieKey = Vec<TrieKeyElement>;

#[derive(Debug, Clone)]
pub struct Trie<T> {
    inner: patricia_tree::map::PatriciaMap<T>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum GetOrDescendentExistsResult<T> {
    NotInTrie,
    InTrie,
    HasValue(T),
}

use GetOrDescendentExistsResult::*;

impl<T> Default for Trie<T> {
    fn default() -> Self {
        Self::new()
    }
}

fn key_len(k: &TrieKey) -> usize {
    debug_assert!(std::mem::size_of::<TrieKeyElement>() == 2 * std::mem::size_of::<u8>());
    k.len() * 2
}

impl<T> Trie<T> {
    pub fn new() -> Self {
        Self {
            inner: PatriciaMap::new(),
        }
    }

    pub fn ancestor_exists(&self, key: &TrieKey) -> bool {
        self.inner
            .get_longest_common_prefix(cast_slice(key))
            .is_some()
    }

    pub fn descendant_exists(&self, key: &TrieKey) -> bool {
        // Length of the [u8] interpretation of the [u16] key is doubled.
        self.inner.longest_common_prefix_len(cast_slice(key)) == key_len(key)
    }

    pub fn insert(&mut self, key: TrieKey, val: T) {
        self.inner.insert(cast_slice(&key), val);
    }

    pub fn get_or_descendant_exists(&self, key: &TrieKey) -> GetOrDescendentExistsResult<T>
    where
        T: Copy,
    {
        let mut descendants = self.inner.iter_prefix(cast_slice(key));
        match descendants.next() {
            None => NotInTrie,
            Some(descendant) => {
                if descendant.0.len() == key_len(key) {
                    HasValue(*descendant.1)
                } else {
                    InTrie
                }
            }
        }
    }
}
