//! TBD: Configuration parser.
//! The [kmonad configuration syntax](https://github.com/kmonad/kmonad/blob/master/keymap/tutorial.kbd)
//! is clean and works great. Might steal it eventually.

#![allow(dead_code)]

use crate::keys::*;
use std::collections::HashSet;

pub struct Cfg {
    /// Mapped keys are the result of the kmonad `defsrc` declaration. Events for keys that are not
    /// mapped by by ktrl will send directly to the OS and won't be processed internally.
    pub mapped_keys: HashSet<KeyCode>,

    /// Contains the keys of the `defsrc` declaration in the order they appear.
    pub src_keymap: Vec<KeyCode>,
}

impl Cfg {
    pub fn new() -> Self {
        let mut mapped_keys = HashSet::new();
        mapped_keys.insert(KeyCode::KEY_A); // FIXME: parse from cfg
        let src_keymap = Vec::new();
        Self {
            mapped_keys,
            src_keymap,
        }
    }
}

// If defsrc is:
//
//     (defsrc
//         esc 1 2 3 4
//     )
//
// and layers are:
//
//     (deflayer one
//         esc a s d f
//     )
//
//     (deflayer two
//         esc a o e u
//     )
//
// Then the keyberon layout will be as follows:
//
//     xx means unimportant. See `keys.rs` for reference
//
//     layout[0] = { xx, 1, 30, 31, 32, 33, xx... }
//     layout[1] = { xx, 1, 30, 24, 18, 22, xx... }
//
//  Note that this example isn't practical, but `(defsrc esc 1 2 3 4)` is used because these keys
//  are at the beginning of the array.

use keyberon::action::*;
use keyberon::key_code;
use keyberon::layout::*;

static LAYERS: Layers = &[
    &[&[
        // layout 0
        Action::NoOp,
        k(key_code::KeyCode::Escape),
        k(key_code::KeyCode::A),
        k(key_code::KeyCode::S),
        k(key_code::KeyCode::D),
        k(key_code::KeyCode::F),
    ]],
    &[&[
        // layout 1
        Action::NoOp,
        k(key_code::KeyCode::Escape),
        k(key_code::KeyCode::A),
        k(key_code::KeyCode::O),
        k(key_code::KeyCode::E),
        k(key_code::KeyCode::U),
    ]],
];

pub fn create_layout() -> Layout {
    Layout::new(LAYERS)
}

pub const MAPPED_KEYS_LEN: usize = 256;

pub fn create_mapped_keys() -> [bool; MAPPED_KEYS_LEN] {
    let mut map = [false; MAPPED_KEYS_LEN];
    map[KeyCode::KEY_ESC as usize] = true;
    map[KeyCode::KEY_1 as usize] = true;
    map[KeyCode::KEY_2 as usize] = true;
    map[KeyCode::KEY_3 as usize] = true;
    map[KeyCode::KEY_4 as usize] = true;
    map
}

pub type KeyOutputs = [Option<Vec<KeyCode>>; MAPPED_KEYS_LEN];

fn add_kc_output(i: usize, kc: KeyCode, outs: &mut KeyOutputs) {
    match outs[i].as_mut() {
        None => {
            outs[i] = Some(vec![kc]);
        }
        Some(v) => {
            v.push(kc);
        }
    }
}

pub fn create_key_outputs() -> KeyOutputs {
    // Option<Vec<..>> is not Copy, so need to manually write out all of the None values :(
    let mut outs = [
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None,
    ];
    for layer in LAYERS.iter() {
        for (i, action) in layer[0].iter().enumerate() {
            match action {
                Action::KeyCode(kc) => {
                    add_kc_output(i, kc.into(), &mut outs);
                }
                Action::HoldTap {
                    tap,
                    hold,
                    timeout: _,
                } => {
                    if let Action::KeyCode(kc) = tap {
                        add_kc_output(i, kc.into(), &mut outs);
                    }
                    if let Action::KeyCode(kc) = hold {
                        add_kc_output(i, kc.into(), &mut outs);
                    }
                }
                _ => {} // do nothing for other types
            };
        }
    }
    outs
}
