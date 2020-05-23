use evdev_rs::enums::EV_KEY::*;
use std::vec::Vec;
use std::collections::HashMap;
use log::info;

use crate::keys::KeyCode;
pub use crate::actions::Action;
pub use crate::actions::tap_hold::TapHoldState;
pub use crate::effects::Effect;
use crate::cfg::CfgLayers;

// -------------- Constants -------------

const MAX_KEY: usize = KEY_MAX as usize;

// ---------------- Types ---------------

pub type LayerIndex = usize;

#[derive(Clone, Debug)]
pub enum KeyState {
    KsTap,
    KsTapHold(TapHoldState),
}

impl KeyState {
    fn from_action(action: &Action) -> Self {
        match action {
            Action::Tap(_) => Self::KsTap,
            Action::TapHold(..) => Self::KsTapHold(TapHoldState::ThIdle),
        }
    }
}

pub type Layer = HashMap<KeyCode, Action>;

#[derive(Clone, Debug)]
pub struct MergedKey {
    pub code: KeyCode,
    pub action: Action,
    pub state: KeyState,
    pub layer_index: LayerIndex,
}

pub type Merged = Vec<MergedKey>;
pub type Layers = Vec<Layer>;
type LayersStates = Vec<bool>;

pub struct LayersManager {

    // Serves as a cache of the result
    // of stacking all the layers on top of each other.
    pub merged: Merged,

    // This is a read-only representation of the user's layer configuration.
    // The 0th layer is the base and will always be active
    pub layers: Layers,

    // Holds the on/off state for each layer
    pub layers_states: LayersStates,
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
        let replacement = MergedKey{
            code: removed_code,
            action: lower_action.clone(),
            state: KeyState::from_action(&lower_action),
            layer_index: i
        };

        return replacement;
    }

    MergedKey{
        code: removed_code,
        action: Action::Tap(Effect::Key(removed_code)),
        state: KeyState::KsTap,
        layer_index: 0
    }
}


fn init_merged() -> Merged {
    let mut merged: Merged = Vec::with_capacity(MAX_KEY);

    for i in 0..MAX_KEY {
        let code: KeyCode = i.into();
        let effect = Effect::Key(code);
        let action = Action::Tap(effect);
        let state = KeyState::KsTap;
        let layer_index = 0;
        merged.push(MergedKey{code, action, state, layer_index});
    }

    assert!(merged.len() == MAX_KEY);
    merged
}

impl LayersManager {
    pub fn new(cfg: CfgLayers) -> Self {
        let merged = init_merged();
        let layers = cfg.layers;
        let layers_count = layers.len();

        let mut layers_states = Vec::new();
        layers_states.resize_with(layers_count, Default::default);

        LayersManager{merged, layers, layers_states}
    }

    pub fn init(&mut self) {
        self.turn_layer_on(0);
    }

    pub fn get(&self, key: KeyCode) -> &MergedKey {
        &self.merged[usize::from(key)]
    }

    pub fn get_mut(&mut self, key: KeyCode) -> &mut MergedKey {
        &mut self.merged[usize::from(key)]
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
                    state: KeyState::from_action(&action),
                    layer_index: index
                };

                self.merged[usize::from(*code)] = new_entry;
            }
        }

        self.layers_states[index] = true;
        info!("Turned layer {} on", index);
    }

    pub fn turn_layer_off(&mut self, index: LayerIndex) {
        std::assert!(index > 0); // Can't turn off the base layer
        std::assert!(self.layers_states[index]);

        let layer = &self.layers[index];
        for (code, _action) in layer {
            let replacement_entry = get_replacement_merged_key(&mut self.merged, &self.layers, *code);
            self.merged[usize::from(*code)] = replacement_entry;
        }

        self.layers_states[index] = false;
        info!("Turned layer {} off", index);
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

// ----------------------------------------------------------
// ----------------------- Tests ----------------------------
// ----------------------------------------------------------

#[cfg(test)]
use std::collections::HashSet;

#[cfg(test)]
use evdev_rs::enums::EV_KEY;

#[cfg(test)]
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

#[cfg(test)]
use std::convert::TryInto;

#[cfg(test)]
use crate::cfg::*;

#[cfg(test)]
fn idx_to_ev_key(i: usize) -> EV_KEY {
    let narrow: u32 = i.try_into().expect(&format!("Invalid KeyCode: {}", i));
    evdev_rs::enums::int_to_ev_key(narrow).expect(&format!("Invalid KeyCode: {}", narrow))
}

#[test]
fn test_mgr() {

    let swap: HashMap<EV_KEY, EV_KEY> = [(KEY_LEFTCTRL, KEY_CAPSLOCK),
                                         (KEY_CAPSLOCK, KEY_LEFTCTRL)].iter().cloned().collect();
    let layers = CfgLayers::new(vec![
        // 0: base layer
        vec![
            // Ex: switch CTRL <--> Capslock
            make_key_layer_entry(KEY_LEFTCTRL, KEY_CAPSLOCK),
            make_key_layer_entry(KEY_CAPSLOCK, KEY_LEFTCTRL),
        ],

        // 1: arrows layer
        vec![
            // Ex: switch CTRL <--> Capslock
            make_key_layer_entry(KEY_H, KEY_LEFT),
            make_key_layer_entry(KEY_J, KEY_DOWN),
            make_key_layer_entry(KEY_K, KEY_UP),
            make_key_layer_entry(KEY_L, KEY_RIGHT),
        ],

        // 2: asdf modifiers
        vec![
            // Ex: switch CTRL <--> Capslock
            make_taphold_layer_entry(KEY_A, KEY_A, KEY_LEFTCTRL),
            make_taphold_layer_entry(KEY_S, KEY_S, KEY_LEFTSHIFT),
            make_taphold_layer_entry(KEY_D, KEY_D, KEY_LEFTALT),
        ],
    ]);

    let mut mgr = LayersManager::new(layers);
    mgr.init();
    assert_eq!(mgr.layers_states.len(), 3);
    assert_eq!(mgr.layers_states[0], true);

    for (i, merged_key) in mgr.merged.iter().enumerate() {
        if MISSING_KEYCODES.contains(&(i as u32)) { // missing keycode
            continue;
        }

        let i_evkey: EV_KEY = idx_to_ev_key(i);
        let expected_code = swap.get(&i_evkey).unwrap_or(&i_evkey).clone();

        assert_eq!(merged_key.code, i_evkey.into());
        assert_eq!(merged_key.layer_index, 0);
        assert_eq!(merged_key.action, make_key_action(expected_code));
    }

    mgr.turn_layer_on(2);
    assert_eq!(mgr.get(KEY_H.into()).action, make_key_action(KEY_H));
    assert_eq!(mgr.get(KEY_J.into()).action, make_key_action(KEY_J));
    assert_eq!(mgr.get(KEY_K.into()).action, make_key_action(KEY_K));
    assert_eq!(mgr.get(KEY_L.into()).action, make_key_action(KEY_L));

    assert_eq!(mgr.get(KEY_A.into()).action, make_taphold_action(KEY_A, KEY_LEFTCTRL));
    assert_eq!(mgr.get(KEY_S.into()).action, make_taphold_action(KEY_S, KEY_LEFTSHIFT));
    assert_eq!(mgr.get(KEY_D.into()).action, make_taphold_action(KEY_D, KEY_LEFTALT));

    mgr.turn_layer_on(1);
    assert_eq!(mgr.get(KEY_H.into()).action, make_key_action(KEY_LEFT));
    assert_eq!(mgr.get(KEY_J.into()).action, make_key_action(KEY_DOWN));
    assert_eq!(mgr.get(KEY_K.into()).action, make_key_action(KEY_UP));
    assert_eq!(mgr.get(KEY_L.into()).action, make_key_action(KEY_RIGHT));

    assert_eq!(mgr.get(KEY_A.into()).action, make_taphold_action(KEY_A, KEY_LEFTCTRL));
    assert_eq!(mgr.get(KEY_S.into()).action, make_taphold_action(KEY_S, KEY_LEFTSHIFT));
    assert_eq!(mgr.get(KEY_D.into()).action, make_taphold_action(KEY_D, KEY_LEFTALT));

    mgr.turn_layer_off(2);
    assert_eq!(mgr.get(KEY_H.into()).action, make_key_action(KEY_LEFT));
    assert_eq!(mgr.get(KEY_J.into()).action, make_key_action(KEY_DOWN));
    assert_eq!(mgr.get(KEY_K.into()).action, make_key_action(KEY_UP));
    assert_eq!(mgr.get(KEY_L.into()).action, make_key_action(KEY_RIGHT));

    assert_eq!(mgr.get(KEY_A.into()).action, make_key_action(KEY_A));
    assert_eq!(mgr.get(KEY_S.into()).action, make_key_action(KEY_S));
    assert_eq!(mgr.get(KEY_D.into()).action, make_key_action(KEY_D));

    mgr.toggle_layer(1);
    assert_eq!(mgr.get(KEY_H.into()).action, make_key_action(KEY_H));
    assert_eq!(mgr.get(KEY_J.into()).action, make_key_action(KEY_J));
    assert_eq!(mgr.get(KEY_K.into()).action, make_key_action(KEY_K));
    assert_eq!(mgr.get(KEY_L.into()).action, make_key_action(KEY_L));

    assert_eq!(mgr.get(KEY_A.into()).action, make_key_action(KEY_A));
    assert_eq!(mgr.get(KEY_S.into()).action, make_key_action(KEY_S));
    assert_eq!(mgr.get(KEY_D.into()).action, make_key_action(KEY_D));

    mgr.toggle_layer(1);
    assert_eq!(mgr.get(KEY_H.into()).action, make_key_action(KEY_LEFT));
    assert_eq!(mgr.get(KEY_J.into()).action, make_key_action(KEY_DOWN));
    assert_eq!(mgr.get(KEY_K.into()).action, make_key_action(KEY_UP));
    assert_eq!(mgr.get(KEY_L.into()).action, make_key_action(KEY_RIGHT));

    assert_eq!(mgr.get(KEY_A.into()).action, make_key_action(KEY_A));
    assert_eq!(mgr.get(KEY_S.into()).action, make_key_action(KEY_S));
    assert_eq!(mgr.get(KEY_D.into()).action, make_key_action(KEY_D));
}
