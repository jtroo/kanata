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

#[derive(Clone, PartialEq, Eq)]
enum Effect {
    Default(KeyCode),

    // Not Implemented Yet
    Sticky(KeyCode),
    ToggleLayer(LayerIndex),
    MomentaryLayer(LayerIndex),
}

#[derive(Clone, PartialEq, Eq)]
enum Action {
    Transparent,
    Tap(Effect),
    TapHold(Effect, Effect),

    // Not Implemented Yet
    TapDance(DanceCount, Effect, Effect),
    Sequence(Vec<KeyCode>, Effect),
    Combo(Vec<KeyCode>, Effect),
}

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

// TODO: check that max size is KEY_MAX
type Layer = HashMap<KeyCode, Action>;

// TODO: check that max size is KEY_MAX
#[derive(Clone)]
struct MergedKey {
    code: KeyCode,
    state: KeyState,
    layer_index: LayerIndex,
}

type Merged = Vec<MergedKey>;
type Layers = Vec<Layer>;
type LayersStates = Vec<bool>;

struct LayersManager {

    // Serves as a cache of the result
    // of stacking all the layers on top of each other.
    merged: Merged,

    // This is a read-only representation of the user's layer configuration.
    // The 0th layer is the base and will always be active
    layers: Layers,

    // Holds the on/off state for each layer
    layers_states: LayersStates,
}

// -------------- Implementation -------------

fn keycode_to_idx(x: KeyCode) -> usize {
    x.try_into().unwrap()
}

fn idx_to_keycode(x: usize) -> KeyCode {
    x as u32
}

fn is_overriding_key(merged: &Merged, candidate_code: KeyCode, candidate_layer_index: LayerIndex) -> bool {
    let current = &merged[keycode_to_idx(candidate_code)];
    return candidate_layer_index > current.layer_index
}

fn get_replacement_merged_key(merged: &mut Merged, layers: &Layers, removed_code: KeyCode) -> MergedKey {
    let current = &merged[keycode_to_idx(removed_code)];
    for i in current.layer_index-1..0 {
        let lower_action = &layers[i][&removed_code];
        if *lower_action != Action::Transparent {
            let replacement = MergedKey{
                code: removed_code,
                state: KeyState::new(lower_action.clone()),
                layer_index: i
            };

            return replacement;
        }
    }

    // This should never happen
    assert!(false);
    MergedKey{code: 0, state: KeyState::new(Action::Transparent), layer_index: 0}
}


fn init_merged(layers: &Layers) -> Merged {
    let mut merged: Merged = vec![];
    for i in 0..KEY_MAX {
        let code_idx: u32 = idx_to_keycode(i);
        let effect = Effect::Default(code_idx);
        let action = Action::Tap(effect);
        let state = KeyState::new(action);
        merged[i] = MergedKey{code: code_idx, state, layer_index: 0};
    }

    merged
}

fn get_layers_from_cfg(cfg: Layers) -> Layers {
    let mut out: Layers = vec![];
    out.reserve(cfg.len());

    for (i, layer_cfg) in cfg.iter().enumerate() {
        for (code, action) in layer_cfg {
            out[i].insert(*code, action.clone());
        }
    }

    out
}

impl LayersManager {
    pub fn new(cfg: Layers) -> Self {
        let base_layer = &cfg[0];
        let merged = init_merged(&cfg);
        let layers = get_layers_from_cfg(cfg);
        let layers_states = vec![];
        LayersManager{merged, layers, layers_states}
    }

    pub fn init(&mut self) {
        self.turn_layer_on(0);
    }

    pub fn turn_layer_on(&mut self, index: LayerIndex) {
        std::assert!(self.layers_states[index]);

        let layer = &self.layers[index];
        for (code, action) in layer {
            let is_overriding = is_overriding_key(&self.merged, *code, index);

            if is_overriding {
                let new_entry = MergedKey{
                    code: *code,
                    state: KeyState::new(action.clone()),
                    layer_index: index
                };

                // TODO: handle dropping the existing KeyState gracefully (ex: if currently held...)
                self.merged[keycode_to_idx(*code)] = new_entry;
            }
        }

        self.layers_states[index] = true;
    }

    pub fn turn_layer_off(&mut self, index: LayerIndex) {
        std::assert!(index > 0); // Can't turn off the base layer
        std::assert!(!self.layers_states[index]);

        let layer = &self.layers[index];
        for (code, _action) in layer {
            let replacement_entry = get_replacement_merged_key(&mut self.merged, &self.layers, *code);
            // TODO: handle dropping the existing KeyState gracefully (ex: if currently held...)
            self.merged[keycode_to_idx(*code)] = replacement_entry;
        }

        self.layers_states[index] = false;
    }

    pub fn toggle_layer(&mut self, index: LayerIndex) {
        let is_layer_on = self.layers_states[index];

        if (is_layer_on) {
            self.turn_layer_off(index);
        } else {
            self.turn_layer_on(index);
        }
    }
}
