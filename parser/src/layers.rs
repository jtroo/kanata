use kanata_keyberon::key_code::KeyCode;
use kanata_keyberon::layout::*;

use crate::cfg::KanataAction;
use crate::cfg::alloc::*;
use crate::custom_action::*;
use crate::keys::OsCode;

use std::sync::Arc;

// OsCode::KEY_MAX is the biggest OsCode
pub const KEYS_IN_ROW: usize = OsCode::KEY_MAX as usize;
pub const LAYER_ROWS: usize = 2;
pub const DEFAULT_ACTION: KanataAction = KanataAction::KeyCode(KeyCode::ErrorUndefined);

pub type IntermediateLayers = Box<[[Row; LAYER_ROWS]]>;

pub type KLayers =
    Layers<'static, KEYS_IN_ROW, LAYER_ROWS, &'static &'static [&'static CustomAction]>;

pub struct KanataLayers {
    pub(crate) layers:
        Layers<'static, KEYS_IN_ROW, LAYER_ROWS, &'static &'static [&'static CustomAction]>,
    _allocations: Arc<Allocations>,
}

impl std::fmt::Debug for KanataLayers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KanataLayers").finish()
    }
}

pub type Row = [kanata_keyberon::action::Action<'static, &'static &'static [&'static CustomAction]>;
    KEYS_IN_ROW];

pub fn new_layers(layers: usize) -> IntermediateLayers {
    let actual_num_layers = layers;
    // Note: why construct it like this?
    // Because don't want to construct KanataLayers on the stack.
    // The stack will overflow because of lack of placement new.
    let mut layers = Vec::with_capacity(actual_num_layers);
    for _ in 0..actual_num_layers {
        layers.push([[DEFAULT_ACTION; KEYS_IN_ROW], [DEFAULT_ACTION; KEYS_IN_ROW]]);
    }
    layers.into_boxed_slice()
}

impl KanataLayers {
    /// # Safety
    ///
    /// The allocations must hold all of the &'static pointers found in layers.
    pub(crate) unsafe fn new(layers: KLayers, allocations: Arc<Allocations>) -> Self {
        Self {
            layers,
            _allocations: allocations,
        }
    }

    pub(crate) fn get(&self) -> (KLayers, Arc<Allocations>) {
        (self.layers, self._allocations.clone())
    }
}
