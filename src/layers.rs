use kanata_keyberon::layout::*;

use crate::cfg::KanataAction;
use crate::custom_action::*;
use crate::keys::OsCode;

// OsCode::KEY_MAX is the biggest OsCode
pub const KEYS_IN_ROW: usize = OsCode::KEY_MAX as usize;
pub const LAYER_COLUMNS: usize = 2;
pub const MAX_LAYERS: usize = 25;
pub const ACTUAL_NUM_LAYERS: usize = MAX_LAYERS * 2;

pub type KanataLayers = Layers<
    'static,
    KEYS_IN_ROW,
    LAYER_COLUMNS,
    ACTUAL_NUM_LAYERS,
    &'static &'static [&'static CustomAction],
>;

type Row = [kanata_keyberon::action::Action<'static, &'static &'static [&'static CustomAction]>;
    KEYS_IN_ROW];

pub fn new_layers() -> Box<KanataLayers> {
    let boxed_slice: Box<[[Row; LAYER_COLUMNS]]> = {
        let mut layers = Vec::with_capacity(ACTUAL_NUM_LAYERS);
        for _ in 0..ACTUAL_NUM_LAYERS {
            layers.push([
                [KanataAction::Trans; KEYS_IN_ROW],
                [KanataAction::Trans; KEYS_IN_ROW],
            ]);
        }
        layers
    }
    .into_boxed_slice();
    let ptr = Box::into_raw(boxed_slice) as *mut KanataLayers;
    unsafe { Box::from_raw(ptr) }
}
