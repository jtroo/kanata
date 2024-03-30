use kanata_keyberon::layout::*;

use crate::cfg::KanataAction;
use crate::custom_action::*;
use crate::keys::OsCode;

// OsCode::KEY_MAX is the biggest OsCode
pub const KEYS_IN_ROW: usize = OsCode::KEY_MAX as usize;
pub const LAYER_ROWS: usize = 2;

pub type IntermediateLayers = Box<[[Row; LAYER_ROWS]]>;

pub type KanataLayers =
    Layers<'static, KEYS_IN_ROW, LAYER_ROWS, &'static &'static [&'static CustomAction]>;

pub type Row = [kanata_keyberon::action::Action<'static, &'static &'static [&'static CustomAction]>;
    KEYS_IN_ROW];

pub fn new_layers(layers: usize) -> IntermediateLayers {
    let actual_num_layers = layers * 2;
    // Note: why construct it like this?
    // Because don't want to construct KanataLayers on the stack.
    // The stack will overflow because of lack of placement new.
    let mut layers = Vec::with_capacity(actual_num_layers);
    for _ in 0..actual_num_layers {
        layers.push([
            [KanataAction::Trans; KEYS_IN_ROW],
            [KanataAction::Trans; KEYS_IN_ROW],
        ]);
    }
    layers.into_boxed_slice()
}
