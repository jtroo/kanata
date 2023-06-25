//! Wrapper around a trie type for (hopefully) easier swapping of libraries if desired.

use radix_trie::TrieCommon;

pub type TrieKey = Vec<u16>;
pub type TrieVal = (u8, u16);

pub struct Trie {
    inner: radix_trie::Trie<TrieKey, TrieVal>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum GetOrDescendentExistsResult {
    NotInTrie,
    InTrie,
    HasValue(TrieVal),
}

use GetOrDescendentExistsResult::*;

impl Trie {
    pub fn new() -> Self {
        Self {
            inner: radix_trie::Trie::new(),
        }
    }

    pub fn ancestor_exists(&self, key: &TrieKey) -> bool {
        self.inner.get_ancestor(key).is_some()
    }

    pub fn descendant_exists(&self, key: &TrieKey) -> bool {
        self.inner.get_raw_descendant(key).is_some()
    }

    pub fn insert(&mut self, key: TrieKey, val: TrieVal) {
        self.inner.insert(key, val);
    }

    pub fn get_or_descendant_exists(&self, key: &TrieKey) -> GetOrDescendentExistsResult {
        let descendant = self.inner.get_raw_descendant(key);
        match descendant {
            None => NotInTrie,
            Some(subtrie) => {
                // If the key exists in this subtrie, returns the value. Otherwise returns
                // KeyInTrie.
                match subtrie.key() {
                    Some(stkey) => {
                        if key == stkey {
                            HasValue(*subtrie.value().expect("node has value"))
                        } else {
                            InTrie
                        }
                    }
                    None => {
                        // Note: None happens if there are multiple children. The sequence is still
                        // in the trie.
                        InTrie
                    }
                }
            }
        }
    }
}
