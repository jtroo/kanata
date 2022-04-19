//! TBD: Configuration parser.
//!
//! How the configuration maps to keyberon:
//!
//! If the mapped keys are defined as:
//!
//!     (defsrc
//!         esc 1 2 3 4
//!     )
//!
//! and the layers are:
//!
//!     (deflayer one
//!         esc a s d f
//!     )
//!
//!     (deflayer two
//!         esc a o e u
//!     )
//!
//! Then the keyberon layers will be as follows:
//!
//!     xx means unimportant. See `keys.rs` for reference
//!
//!     layers[0] = { xx, 1, 30, 31, 32, 33, xx... }
//!     layers[1] = { xx, 1, 30, 24, 18, 22, xx... }
//!
//!  Note that this example isn't practical, but `(defsrc esc 1 2 3 4)` is used because these keys
//!  are at the beginning of the array. The column index for layers is the numerical value of
//!  the key from `keys::OsCode`. So if you want to change how the physical key `A` works, when on
//!  a layer, you would change index `30` (see `keys::OsCode::KEY_A`) of the desired layer to the
//!  desired `keyberon::action::Action`. `DEFAULT_LAYERS` has some examples.


#![allow(dead_code)]

use crate::keys::*;
use crate::default_layers::*;

use std::collections::HashSet;

use keyberon::action::*;
use keyberon::layout::*;

pub struct Cfg {
    /// Mapped keys are the result of the kmonad `defsrc` declaration. Events for keys that are not
    /// mapped by by ktrl will send directly to the OS and won't be processed internally.
    ///
    /// TODO: currently not used, `create_mapped_keys` is used instead (hardcoded).
    pub mapped_keys: HashSet<OsCode>,
}

impl Cfg {
    pub fn new() -> Self {
        let mut mapped_keys = HashSet::new();
        mapped_keys.insert(OsCode::KEY_A); // FIXME: parse from cfg
        Self {
            mapped_keys,
        }
    }
}

/// TODO: replace this with cfg fns
pub fn create_layout() -> Layout<256, 1, 25> {
    Layout::new(&DEFAULT_LAYERS)
}

pub const MAPPED_KEYS_LEN: usize = 256;

/// TODO: replace this with cfg fns
pub fn create_mapped_keys() -> [bool; MAPPED_KEYS_LEN] {
    let mut map = [false; MAPPED_KEYS_LEN];
    map[OsCode::KEY_ESC as usize] = true;
    map[OsCode::KEY_1 as usize] = true;
    map[OsCode::KEY_2 as usize] = true;
    map[OsCode::KEY_3 as usize] = true;
    map[OsCode::KEY_4 as usize] = true;
    map
}

pub type KeyOutputs = [Option<Vec<OsCode>>; MAPPED_KEYS_LEN];

fn add_kc_output(i: usize, kc: OsCode, outs: &mut KeyOutputs) {
    log::info!("Adding {:?} to idx {}", kc, i);
    match outs[i].as_mut() {
        None => {
            outs[i] = Some(vec![kc]);
        }
        Some(v) => {
            v.push(kc);
        }
    }
}

/// TODO: replace this with cfg fns
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
    for layer in DEFAULT_LAYERS.iter() {
        for (i, action) in layer[0].iter().enumerate() {
            match action {
                Action::KeyCode(kc) => {
                    add_kc_output(i, kc.into(), &mut outs);
                }
                Action::HoldTap {
                    tap,
                    hold,
                    timeout: _,
                    config: _,
                    tap_hold_interval: _,
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
