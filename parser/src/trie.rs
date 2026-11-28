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

    /// Returns every key that starts with `prefix`, including `prefix` itself if it is a key.
    pub fn keys_with_prefix<'a>(
        &'a self,
        prefix: &'a [u16],
    ) -> impl Iterator<Item = Vec<TrieKeyElement>> + 'a {
        self.inner.iter_prefix(cast_slice(prefix)).map(|(key, _)| {
            let (elements, _) = key.as_chunks::<2>();
            elements
                .iter()
                .map(|&bytes| TrieKeyElement::from_ne_bytes(bytes))
                .collect()
        })
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_with_prefix_lists_every_key_below_the_prefix() {
        let mut trie = Trie::new();
        trie.insert([1u16, 2], ());
        trie.insert([1u16, 3, 4], ());
        trie.insert([2u16, 2], ());
        // Elements sharing a byte with the prefix element must not match.
        trie.insert([0x0101u16], ());
        trie.insert([0x0102u16], ());

        let mut keys: Vec<_> = trie.keys_with_prefix(&[1]).collect();
        keys.sort();
        assert_eq!(keys, [vec![1, 2], vec![1, 3, 4]]);
        assert_eq!(trie.keys_with_prefix(&[2]).count(), 1);
        assert_eq!(trie.keys_with_prefix(&[9]).count(), 0);
        // The empty prefix lists the whole trie.
        assert_eq!(trie.keys_with_prefix(&[]).count(), 5);
    }
}
