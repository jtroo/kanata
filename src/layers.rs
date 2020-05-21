use evdev_rs::enums::EV_KEY;
use std::vec::Vec;
use std::collections::{HashMap, HashSet};
use std::convert::TryInto;
use std::fmt;

// -------------- Constants -------------

const KEY_MAX: usize = EV_KEY::KEY_MAX as usize;
lazy_static::lazy_static! {
    static ref MISSING_KEYCODES: HashSet<u32> = {
        let mut m = HashSet::new();
        let ranges = vec![
            84..85,
            195..200,
            249..352,
            443..448,
            452..464,
            485..497,
            507..512,
            543..560,
            562..576,
            585..592,
            594..608,
            633..767
        ];

        for range in ranges {
            for i in range {
                m.insert(i);
            }
        }

        m
    };
}

// -------------- Config Types -------------

#[derive(Copy, Clone, Default, PartialEq, Eq, Hash)]
pub struct KeyCode {
    c: u32,
}

impl From<usize> for KeyCode {
    fn from(item: usize) -> Self {
        Self{c: item as u32}
    }
}

impl From<EV_KEY> for KeyCode {
    fn from(item: EV_KEY) -> Self {
        Self{c: item as u32}
    }
}

impl From<KeyCode> for usize {
    fn from(item: KeyCode) -> Self {
        item.c as usize
    }
}

impl From<KeyCode> for EV_KEY {
    fn from(item: KeyCode) -> Self {
        evdev_rs::enums::int_to_ev_key(item.c)
            .expect(&format!("Invalid KeyCode: {}", item.c))
    }
}

fn idx_to_ev_key(i: usize) -> EV_KEY {
    let narrow: u32 = i.try_into().expect(&format!("Invalid KeyCode: {}", i));
    evdev_rs::enums::int_to_ev_key(narrow).expect(&format!("Invalid KeyCode: {}", narrow))
}

impl fmt::Debug for KeyCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let evkey: EV_KEY = evdev_rs::enums::int_to_ev_key(self.c)
            .expect(&format!("Invalid KeyCode: {}", self.c));
        evkey.fmt(f)
    }
}

type DanceCount = usize;
type LayerIndex = usize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Default(KeyCode),

    // Not Implemented Yet
    Sticky(KeyCode),
    ToggleLayer(LayerIndex),
    MomentaryLayer(LayerIndex),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Transparent,
    Tap(Effect),
    TapHold(Effect, Effect),

    // Not Implemented Yet
    TapDance(DanceCount, Effect, Effect),
    Sequence(Vec<KeyCode>, Effect),
    Combo(Vec<KeyCode>, Effect),
}

// -------------- Runtime Types -------------

#[derive(Clone, Debug)]
struct KeyState {}

// TODO: check that max size is KEY_MAX
type Layer = HashMap<KeyCode, Action>;

// TODO: check that max size is KEY_MAX
#[derive(Clone, Debug)]
struct MergedKey {
    code: KeyCode,
    action: Action,
    state: KeyState,
    layer_index: LayerIndex,
}

type Merged = Vec<MergedKey>;
type Layers = Vec<Layer>;
type LayersStates = Vec<bool>;

pub struct LayersManager {

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

fn is_overriding_key(merged: &Merged, candidate_code: KeyCode, candidate_layer_index: LayerIndex) -> bool {
    let current = &merged[usize::from(candidate_code)];
    return candidate_layer_index >= current.layer_index
}

fn get_replacement_merged_key(merged: &mut Merged, layers: &Layers, removed_code: KeyCode) -> MergedKey {
    let current: &MergedKey = &merged[usize::from(removed_code)];
    let lower_layer_idx = current.layer_index-1;

    for i in lower_layer_idx..0 {
        let lower_action = &layers[i][&removed_code];
        if *lower_action != Action::Transparent {
            let replacement = MergedKey{
                code: removed_code,
                action: lower_action.clone(),
                state: KeyState{},
                layer_index: i
            };

            return replacement;
        }
    }

    // This should never happen
    assert!(false);
    MergedKey{
        code: Default::default(),
        action: Action::Transparent,
        state: KeyState{},
        layer_index: 0
    }
}


fn init_merged(layers: &Layers) -> Merged {
    let mut merged: Merged = Vec::with_capacity(KEY_MAX);

    for i in 0..KEY_MAX {
        let code: KeyCode = i.into();
        let effect = Effect::Default(code);
        let action = Action::Tap(effect);
        let state = KeyState{};
        let layer_index = 0;
        merged.push(MergedKey{code, action, state, layer_index});
    }

    merged
}

fn get_layers_from_cfg(cfg: Layers) -> Layers {
    let mut out: Layers = Vec::new();
    out.resize_with(cfg.len(), Default::default);

    for (i, layer_cfg) in cfg.iter().enumerate() {
        for (code, action) in layer_cfg {
            out[i].insert(*code, action.clone());
        }
    }

    out
}

impl LayersManager {
    pub fn new(cfg: Layers) -> Self {
        let merged = init_merged(&cfg);
        let layers_count = cfg.len();
        let layers = get_layers_from_cfg(cfg);

        let mut layers_states = Vec::new();
        layers_states.resize_with(layers_count, Default::default);

        LayersManager{merged, layers, layers_states}
    }

    pub fn init(&mut self) {
        self.turn_layer_on(0);
    }

    pub fn turn_layer_on(&mut self, index: LayerIndex) {
        std::assert!(!self.layers_states[index]);

        let layer = &self.layers[index];
        for (code, action) in layer {
            let is_overriding = is_overriding_key(&self.merged, *code, index);

            if is_overriding {
                let new_entry = MergedKey{
                    code: *code,
                    action: action.clone(),
                    state: KeyState{},
                    layer_index: index
                };

                // TODO: handle dropping the existing KeyState gracefully (ex: if currently held...)
                self.merged[usize::from(*code)] = new_entry;
            }
        }

        self.layers_states[index] = true;
    }

    pub fn turn_layer_off(&mut self, index: LayerIndex) {
        std::assert!(index > 0); // Can't turn off the base layer
        std::assert!(self.layers_states[index]);

        let layer = &self.layers[index];
        for (code, _action) in layer {
            let replacement_entry = get_replacement_merged_key(&mut self.merged, &self.layers, *code);
            // TODO: handle dropping the existing KeyState gracefully (ex: if currently held...)
            self.merged[usize::from(*code)] = replacement_entry;
        }

        self.layers_states[index] = false;
    }

    pub fn toggle_layer(&mut self, index: LayerIndex) {
        let is_layer_on = self.layers_states[index];

        if is_layer_on {
            self.turn_layer_off(index);
        } else {
            self.turn_layer_on(index);
        }
    }
}

#[cfg(test)]
fn make_default_layer_entry(src: EV_KEY, dst: EV_KEY) -> (KeyCode, Action) {
    let src_code: KeyCode = src.into();
    let dst_code: KeyCode = dst.into();
    let effect = Effect::Default(dst_code);
    let action = Action::Tap(effect);
    return (src_code, action)
}

#[test]
fn test_mgr_init() {

    let swap: HashMap<EV_KEY, EV_KEY> = [(EV_KEY::KEY_LEFTCTRL, EV_KEY::KEY_CAPSLOCK),
                                         (EV_KEY::KEY_CAPSLOCK, EV_KEY::KEY_LEFTCTRL)].iter().cloned().collect();
    let layers: Layers = vec![
        // 0: base layer
        [
            // Ex: switch CTRL <--> Capslock
            make_default_layer_entry(EV_KEY::KEY_LEFTCTRL, EV_KEY::KEY_CAPSLOCK),
            make_default_layer_entry(EV_KEY::KEY_CAPSLOCK, EV_KEY::KEY_LEFTCTRL),
        ].iter().cloned().collect()
    ];

    let mut mgr = LayersManager::new(layers);
    mgr.init();
    assert_eq!(mgr.layers_states.len(), 1);
    assert_eq!(mgr.layers_states[0], true);

    // TODO: This should fail due to the capslock reassignment above
    for (i, merged_key) in mgr.merged.iter().enumerate() {
        if MISSING_KEYCODES.contains(&(i as u32)) { // missing keycode
            continue;
        }

        let i_evkey: EV_KEY = idx_to_ev_key(i);
        let expected_code = swap.get(&i_evkey).unwrap_or(&i_evkey).clone();

        assert_eq!(merged_key.code, i_evkey.into());
        assert_eq!(merged_key.layer_index, 0);
        // assert_eq!(merged_key.action, Action::Tap(Effect::Default(expected_code.into())));
    }
}
