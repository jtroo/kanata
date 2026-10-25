//! Wrapper around a trie type for (hopefully) easier swapping of libraries if desired.

use bytemuck::cast_slice;
use patricia_tree::map::PatriciaMap;

pub type TrieKeyElement = u16;

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

fn key_len(k: impl AsRef<[u16]>) -> usize {
    debug_assert!(std::mem::size_of::<TrieKeyElement>() == 2 * std::mem::size_of::<u8>());
    k.as_ref().len() * 2
}

impl<T> Trie<T> {
    pub fn new() -> Self {
        Self {
            inner: PatriciaMap::new(),
        }
    }

    pub fn ancestor_exists(&self, key: impl AsRef<[u16]>) -> bool {
        self.inner
            .get_longest_common_prefix(cast_slice(key.as_ref()))
            .is_some()
    }

    pub fn descendant_exists(&self, key: impl AsRef<[u16]>) -> bool {
        // Length of the [u8] interpretation of the [u16] key is doubled.
        self.inner
            .longest_common_prefix_len(cast_slice(key.as_ref()))
            == key_len(key)
    }

    pub fn insert(&mut self, key: impl AsRef<[u16]>, val: T) {
        self.inner.insert(cast_slice(key.as_ref()), val);
    }

    pub fn get_or_descendant_exists(&self, key: impl AsRef<[u16]>) -> GetOrDescendentExistsResult<T>
    where
        T: Clone,
    {
        let mut descendants = self.inner.iter_prefix(cast_slice(key.as_ref()));
        match descendants.next() {
            None => NotInTrie,
            Some(descendant) => {
                if descendant.0.len() == key_len(key.as_ref()) {
                    HasValue(descendant.1.clone())
                } else {
                    InTrie
                }
            }
        }
    }

    /// Returns every `(key, value)` pair whose key starts with `key`, i.e.
    /// all completions below the given prefix. Key elements are reconstructed
    /// as little-endian `u16`s, the inverse of the `cast_slice` performed
    /// when inserting (kanata targets little-endian platforms exclusively).
    pub fn get_prefix_matches(&self, key: impl AsRef<[u16]>) -> Vec<(Vec<u16>, T)>
    where
        T: Clone,
    {
        self.inner
            .iter_prefix(cast_slice(key.as_ref()))
            .map(|(k, v)| {
                let elems = k
                    .chunks_exact(2)
                    .map(|b| u16::from_le_bytes([b[0], b[1]]))
                    .collect();
                (elems, v.clone())
            })
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_prefix_matches_returns_all_completions() {
        let mut trie = Trie::new();
        trie.insert([1u16, 2, 3], 10u32);
        trie.insert([1u16, 2, 4], 20u32);
        trie.insert([9u16, 9], 30u32);
        trie.insert([1u16, 2], 40u32); // exact match for the prefix itself

        let mut matches = trie.get_prefix_matches([1u16, 2]);
        matches.sort();
        assert_eq!(
            matches,
            vec![(vec![1, 2], 40), (vec![1, 2, 3], 10), (vec![1, 2, 4], 20)]
        );

        // No completions below an unrelated prefix.
        assert!(trie.get_prefix_matches([7u16, 7]).is_empty());
    }
}
