//! Store the following data for presenting in the GUI:
//! - characters used in the output, for a mapping of u16->zippy config char

use super::*;

use std::sync::LazyLock;

use itertools::Itertools;
use parking_lot::Mutex;
use rustc_hash::FxHashMap;

pub fn names(code: u16) -> String {
    // Invariant: keys that exist must have a non-empty vec as a value
    COMMITTED_MAPPING
        .lock()
        .get(&code)
        .map(|v| v.iter().join("|"))
        .unwrap_or_else(|| format!("unknown{code}"))
}

static COMMITTED_MAPPING: LazyLock<Mutex<InputDisplayMapping>> = LazyLock::new(Default::default);

static UNCOMMITTED_MAPPING: LazyLock<Mutex<InputDisplayMapping>> = LazyLock::new(Default::default);

type InputDisplayMapping = FxHashMap<u16, Vec<char>>;

pub(crate) fn commit_mappings() {
    let mut cm = COMMITTED_MAPPING.lock();
    let mut um = UNCOMMITTED_MAPPING.lock();
    cm.clear();
    cm.extend(um.drain());
}

pub(crate) fn reset_uncommitted_mappings() {
    UNCOMMITTED_MAPPING.lock().clear();
    add_uncommitted_mapping(OsCode::KEY_SPACE.into(), '␣');
}

pub(crate) fn add_uncommitted_mapping(code: u16, mapped_to: char) {
    UNCOMMITTED_MAPPING
        .lock()
        .entry(code)
        .and_modify(|v| {
            if !v.contains(&mapped_to) {
                v.push(mapped_to);
            }
        })
        .or_insert_with(|| vec![mapped_to]);
}
