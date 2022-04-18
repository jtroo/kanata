//! TBD: Configuration parser.
//! The [kmonad configuration syntax](https://github.com/kmonad/kmonad/blob/master/keymap/tutorial.kbd)
//! is clean and works great. Might steal it eventually.

#![allow(dead_code)]

use std::collections::HashSet;

pub struct Cfg {
    pub mapped_keys: HashSet<u32>
}

impl Cfg {
    #[cfg(test)]
    pub fn new() -> Self {
        Self { mapped_keys: HashSet::new() }
    }
}
