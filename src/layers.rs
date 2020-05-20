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

#[derive(Clone)]
enum Effect {
    Default(KeyCode),

    // Not Implemented Yet
    Sticky(KeyCode),
    ToggleLayer(LayerIndex),
    MomentaryLayer(LayerIndex),
}

#[derive(Clone)]
enum Action {
    Tap(Effect),
    TapHold(Effect, Effect),

    // Not Implemented Yet
    TapDance(DanceCount, Effect, Effect),
    Sequence(Vec<KeyCode>, Effect),
    Combo(Vec<KeyCode>, Effect),
}

type LayerCfg = HashMap<KeyCode, Action>;

// -------------- Runtime Types -------------

#[derive(Clone)]
struct KeyStateImpl {}

#[derive(Clone)]
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
    keys: HashMap<KeyCode, Action>,
}

// Max size is KEY_MAX
type Merged = Vec<KeyState>;

struct Layers {

    // Serves as a cache of the result
    // of stacking all the layers on top of each other.
    merged: Merged,

    // This is a read-only representation of the user's layer configuration.
    // The 0th layer is the base and will always be active
    layers: Vec<Layer>,
}

// -------------- Implementation -------------

fn init_merged(base_layer: &LayerCfg) -> Merged {
    let mut merged: Merged = vec![];
    for i in 0..KEY_MAX {
        let code: u32 = i.try_into().unwrap();
        let effect = Effect::Default(code);
        let action = Action::Tap(effect);
        merged[i] = KeyState::new(action);
    }

    // TODO: Refactor to `turn_layer_on`
    for (code, action) in base_layer {
        merged[*code as usize].action = action.clone();
    }

    merged
}

fn get_layers_from_cfg(cfg: Vec<LayerCfg>) -> Vec<Layer> {
    let mut out: Vec<Layer> = vec![];
    out.reserve(cfg.len());

    for (i, layer_cfg) in cfg.iter().enumerate() {
        for (code, action) in layer_cfg {
            out[i].keys.insert(*code, action.clone());
        }
    }

    out
}

impl Layers {
    pub fn new(cfg: Vec<LayerCfg>) -> Self {
        let base_layer = &cfg[0];
        let merged = init_merged(base_layer);
        let layers = get_layers_from_cfg(cfg);
        Layers{merged, layers}
    }

    // pub fn turn_layer_on(layer_num: usize) {}
    // pub fn turn_layer_off(layer_num: usize) {}
    // pub fn toggle_layer(layer_num: usize) {}
}
