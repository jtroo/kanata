//! Collection where keys are slices of a type, and supports a get operation to check if the key
//! exists, is a subset of any existing key, or is neither of the aforementioned cases.
//!
//! In the underlying structure the value is cloned for each participating member of slice key, so
//! you should ensure values are cheaply clonable. If the value is not, consider putting it inside
//! `Rc` or `Arc`.

use rustc_hash::FxHashMap;
use std::hash::Hash;

// # Design considerations:
//
// It was considered whether `key` should be in an `Arc` instead of a `Box` or whether
// `SsmKeyValue` should be wrapped in `Arc`.
//
// ## No usage of `Arc`
//
// With no reference counting, the key slice `&[K]` will be allocated in a separate box for every
// instance of `SsmKeyValue`. The number of instances of the key is equal to the length of the key.
// This is obviously the best choice if key lengths are mostly expected to be 1. For key lengths
// larger than 1, the point at which an `Arc` would be better would need to be measured.
//
// ## `key: Arc<[K]>`
//
// The benefit of using an `Arc` for the key instead of `Box` is that clones don't create a new
// allocation. The downside is that the allocations use more space, namely there is an extra
// `2 * usize` in the allocation for the strong and weak pointers, so 16 extra bytes.
//
// Kanata uses `K=u16` only today (August 2025). This means perfectly sized allocations, it would
// take a 3 length key for `Box` to begin to reach `Arc`'s size:
//   - Arc: 16 + (3*2) = 22 bytes
//   - Box:  3 x (3*2) = 18 bytes
//
// A 4-length key is much worse:
//   - Arc: 16 + (4*2) = 22 bytes
//   - Box:  4 x (4*2) = 32 bytes
//
// In practice, allocators have allocation space overhead and/or minimum allocation sizes. With the
// effects of these overheads and CPU caching, the estimate of when `Arc` outperforms `Box` for
// read-only usage is likely a key length of 3 or even 2. Read-only is notable because Kanata
// doesn't care about write performance; write only happens at parse time and only reads are done
// for standard runtime.
//
// ## Vec<Arc<SsmKeyValue<...>>
//
// This has the downside of needing to follow two pointers to dereference `key`. For `Box`-only,
// or `key: Arc<[K]>`, this is not the case. Having two indirections is not desirable.

#[derive(Debug, Clone)]
pub struct SubsetMap<K, V> {
    map: FxHashMap<K, Vec<SsmKeyValue<K, V>>>,
}

#[derive(Debug, Clone)]
struct SsmKeyValue<K, V> {
    key: Box<[K]>,
    value: V,
}

impl<K, V> SsmKeyValue<K, V>
where
    K: Clone,
{
    fn ssmkv_new(key: impl AsRef<[K]>, value: V) -> Self {
        Self {
            key: key.as_ref().to_vec().into_boxed_slice(),
            value,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum GetOrIsSubsetOfKnownKey<T> {
    HasValue(T),
    IsSubset,
    Neither,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SsmKeyExistedBeforeInsert {
    Existed,
    NotThere,
}

use GetOrIsSubsetOfKnownKey::*;

impl<K, V> Default for SubsetMap<K, V>
where
    K: Clone + PartialEq + Ord + Hash,
    V: Clone,
{
    fn default() -> Self {
        Self::ssm_new()
    }
}

impl<K, V> SubsetMap<K, V>
where
    K: Clone + PartialEq + Ord + Hash,
    V: Clone,
{
    pub fn ssm_new() -> Self {
        Self {
            map: FxHashMap::default(),
        }
    }

    /// Inserts a potentially unsorted key. Sorts the key and then calls ssm_insert_ksorted.
    pub fn ssm_insert(&mut self, mut key: impl AsMut<[K]>, val: V) -> SsmKeyExistedBeforeInsert {
        key.as_mut().sort();
        self.ssm_insert_ksorted(key.as_mut(), val)
    }

    /// Inserts a sorted key. Failure to enforce that the key is sorted results in defined but
    /// unspecified behaviour.
    pub fn ssm_insert_ksorted(
        &mut self,
        key: impl AsRef<[K]>,
        val: V,
    ) -> SsmKeyExistedBeforeInsert {
        let mut key_existed = SsmKeyExistedBeforeInsert::NotThere;
        for k in key.as_ref().iter().cloned() {
            let keyvals_for_key_item = self.map.entry(k).or_default();
            match keyvals_for_key_item
                .binary_search_by(|probe| probe.key.as_ref().cmp(key.as_ref()))
            {
                Ok(pos) => {
                    key_existed = SsmKeyExistedBeforeInsert::Existed;
                    keyvals_for_key_item[pos] = SsmKeyValue::ssmkv_new(key.as_ref(), val.clone());
                }
                Err(pos) => {
                    keyvals_for_key_item
                        .insert(pos, SsmKeyValue::ssmkv_new(key.as_ref(), val.clone()));
                }
            }
        }
        key_existed
    }

    /// Gets using a potentially unsorted key. Sorts the key then calls
    /// ssm_get_or_is_subset_ksorted.
    pub fn ssm_get_or_is_subset(&self, mut key: impl AsMut<[K]>) -> GetOrIsSubsetOfKnownKey<V> {
        key.as_mut().sort();
        self.ssm_get_or_is_subset_ksorted(key.as_mut())
    }

    /// Gets using a sorted key. Failure to enforce a sorted key results in defined but unspecified
    /// behaviour.
    pub fn ssm_get_or_is_subset_ksorted(
        &self,
        get_key: impl AsRef<[K]>,
    ) -> GetOrIsSubsetOfKnownKey<V> {
        let get_key = get_key.as_ref();
        if get_key.is_empty() {
            return match self.is_empty() {
                true => Neither,
                false => IsSubset,
            };
        }
        match self.map.get(&get_key[0]) {
            None => Neither,
            Some(keyvals_for_key_item) => {
                match keyvals_for_key_item
                    .binary_search_by(|probe| probe.key.as_ref().cmp(get_key.as_ref()))
                {
                    Ok(pos) => HasValue(keyvals_for_key_item[pos].value.clone()),
                    Err(_) => {
                        for kv in keyvals_for_key_item.iter() {
                            if get_key.iter().all(|kitem| kv.key.contains(kitem)) {
                                return IsSubset;
                            }
                        }
                        Neither
                    }
                }
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}
