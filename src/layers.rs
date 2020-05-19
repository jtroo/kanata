use evdev_rs::enums::EV_KEY;
use std::vec::Vec;
use std::collections::HashMap;
use std::convert::TryInto;

// -------------- Constants -------------

const KEY_MAX: usize = EV_KEY::KEY_MAX as usize;

// -------------- Config Types -------------

type DanceCount = usize;
type KeyCode = u32;
type LayerIndex = usize;
type ActionRef = Box<Action>;

#[derive(Clone)]
enum Action {
    Regular(KeyCode),
    TapHold(ActionRef, ActionRef),
    TapDance(DanceCount, ActionRef, ActionRef),
    ToggleLayer(LayerIndex),
    MomentaryLayer(LayerIndex),
}

type LayerCfg = HashMap<KeyCode, Action>;

// -------------- Runtime Types -------------

struct KeyStateImpl {}

struct KeyState {
    state: KeyStateImpl,
    action: Action,
}

impl KeyState {
    pub fn new(action: Action) -> Self {
        KeyState{state: KeyStateImpl{}, action}
    }
}

struct Layer {
    keys: HashMap<KeyCode, KeyState>,
}

// -------------- Implementation -------------

struct Layers {

    // Serves as a cache of the result
    // of stacking all the layers on top of each other.
    merged: [KeyCode; KEY_MAX],

    // This is a read-only representation of the user's layer configuration.
    // The 0th layer is the base and will always be active
    layers: Vec<Layer>,
}

impl Layers {
    pub fn new(layers: Vec<LayerCfg>) -> Self {
        let mut merged = [0 as KeyCode; KEY_MAX];
        for i in 0..KEY_MAX {
            merged[i] = i.try_into().unwrap();
        }

        let mut _layers: Vec<Layer> = vec![];
        _layers.reserve(layers.len());

        for (i, layer_cfg) in layers.iter().enumerate() {
            for (code, action) in layer_cfg {
                _layers[i].keys.insert(*code, KeyState::new(action.clone()));
            }
        }

        Layers{merged, layers:_layers}
    }

    // pub fn turn_layer_on(layer_num: usize) {}
    // pub fn turn_layer_off(layer_num: usize) {}
    // pub fn toggle_layer(layer_num: usize) {}
}
