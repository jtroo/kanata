use crate::keys::KeyCode::*;
use std::vec::Vec;
use std::collections::HashMap;
use std::convert::TryFrom;
use log::{warn, debug};

use crate::keys::KeyCode;
pub use crate::actions::Action;
pub use crate::actions::tap_hold::TapHoldState;
pub use crate::effects::Effect;
use crate::cfg::CfgLayers;

// -------------- Constants -------------

const MAX_KEY: usize = KEY_MAX as usize;

// ---------------- Types ---------------

pub type LayerIndex = usize;
pub type Layer = HashMap<KeyCode, Action>;

#[derive(Clone, Debug)]
pub struct MergedKey {
    pub code: KeyCode,
    pub action: Action,
    pub layer_index: LayerIndex,
}

// MergedKey is wrapped in an Option because
// not all integer in the KEY_MAX range
// have a matching `KeyCode`
pub type Merged = Vec<Option<MergedKey>>;

pub type Layers = Vec<Layer>;
type LayersStates = Vec<bool>;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LockOwner {
    LkTapHold,
    LkSticky,
}

pub struct LayersManager {

    // Serves as a cache of the result
    // of stacking all the layers on top of each other.
    pub merged: Merged,

    // This is a read-only representation of the user's layer configuration.
    // The 0th layer is the base and will always be active
    pub layers: Layers,

    // Holds the on/off state for each layer
    pub layers_states: LayersStates,

    // Allows stateful modules (like TapHold)
    // to lock certain keys. This'll prevent them
    // from being changed by layer changes
    pub locks: HashMap<KeyCode, LockOwner>
}

// -------------- Implementation -------------

fn init_merged() -> Merged {
    let mut merged: Merged = Vec::with_capacity(MAX_KEY);

    for i in 0..MAX_KEY {
        if let Ok(code) = KeyCode::try_from(i) {
            let effect = Effect::Key(code);
            let action = Action::Tap(effect);
            let layer_index = 0;
            merged.push(Some(MergedKey{code, action, layer_index}));
        } else {
            merged.push(None);
        }
    }

    assert!(merged.len() == MAX_KEY);
    merged
}

impl LayersManager {
    pub fn new(cfg: CfgLayers) -> Self {
        let merged = init_merged();
        let layers = cfg.layers;
        let layers_count = layers.len();
        let locks = HashMap::new();

        let mut layers_states = Vec::new();
        layers_states.resize_with(layers_count, Default::default);

        LayersManager{merged, layers, layers_states, locks}
    }

    pub fn lock_key(&mut self, key: KeyCode, owner: LockOwner) {
        assert!(!self.locks.contains_key(&key));
        self.locks.insert(key, owner);
    }

    pub fn unlock_key(&mut self, key: KeyCode, owner: LockOwner) {
        assert!(self.locks[&key] == owner);
        self.locks.remove(&key);
    }

    pub fn init(&mut self) {
        self.turn_layer_on(0);
    }

    fn is_overriding_key(&self, candidate_code: KeyCode, candidate_layer_index: LayerIndex) -> bool {
        let current = self.get(candidate_code);
        return candidate_layer_index >= current.layer_index
    }

    fn get_replacement_merged_key(&self, layers: &Layers, removed_code: KeyCode) -> MergedKey {
        let current = self.get(removed_code);
        let lower_layer_idx = current.layer_index-1;

        for i in lower_layer_idx..0 {
            let lower_action = &layers[i][&removed_code];
            let replacement = MergedKey{
                code: removed_code,
                action: lower_action.clone(),
                layer_index: i
            };

            return replacement;
        }

        MergedKey{
            code: removed_code,
            action: Action::Tap(Effect::Key(removed_code)),
            layer_index: 0
        }
    }



    pub fn get(&self, key: KeyCode) -> &MergedKey {
        match &self.merged[usize::from(key)] {
            Some(merged_key) => merged_key,
            _ => panic!("Invalid KeyCode")
        }
    }

    pub fn get_mut(&mut self, key: KeyCode) -> &mut MergedKey {
        match &mut self.merged[usize::from(key)] {
            Some(merged_key) => merged_key,
            _ => panic!("Invalid KeyCode")
        }
    }

    pub fn turn_layer_on(&mut self, index: LayerIndex) {
        std::assert!(!self.layers_states[index]);

        if self.locks.len() > 0 {
            warn!("Can't turn layer {} on. {:?} are in use", index, &self.locks);
            return;
        }

        let layer = &self.layers[index];
        for (code, action) in layer {
            let is_overriding = self.is_overriding_key(*code, index);

            if is_overriding {
                let new_entry = MergedKey{
                    code: *code,
                    action: action.clone(),
                    layer_index: index
                };

                self.merged[usize::from(*code)] = Some(new_entry);
            }
        }

        self.layers_states[index] = true;
        debug!("Turned layer {} on", index);
    }

    pub fn turn_layer_off(&mut self, index: LayerIndex) {
        std::assert!(index > 0); // Can't turn off the base layer
        std::assert!(self.layers_states[index]);

        if self.locks.len() > 0 {
            warn!("Can't turn layer {} off. {:?} are in use", index, &self.locks);
            return;
        }

        let layer = &self.layers[index];
        for (code, _action) in layer {
            let replacement_entry = self.get_replacement_merged_key(&self.layers, *code);
            self.merged[usize::from(*code)] = Some(replacement_entry);
        }

        self.layers_states[index] = false;
        debug!("Turned layer {} off", index);
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
use crate::effects::Effect::*;

#[cfg(test)]
use crate::actions::Action::*;

#[test]
fn test_mgr() {
    let layers = CfgLayers::new(vec![
        // 0: base layer
        vec![
            // Ex: switch CTRL <--> Capslock
            (KEY_LEFTCTRL, Tap(Key(KEY_CAPSLOCK))),
            (KEY_CAPSLOCK, Tap(Key(KEY_LEFTCTRL))),
        ],

        // 1: arrows layer
        vec![
            // Ex: switch CTRL <--> Capslock
            (KEY_H, Tap(Key(KEY_LEFT))),
            (KEY_J, Tap(Key(KEY_DOWN))),
            (KEY_K, Tap(Key(KEY_UP))),
            (KEY_L, Tap(Key(KEY_RIGHT))),
        ],

        // 2: asdf modifiers
        vec![
            // Ex: switch CTRL <--> Capslock
            (KEY_A, TapHold(Key(KEY_A), Key(KEY_LEFTCTRL))),
            (KEY_S, TapHold(Key(KEY_S), Key(KEY_LEFTSHIFT))),
            (KEY_D, TapHold(Key(KEY_D), Key(KEY_LEFTALT))),
        ],
    ]);

    let mut mgr = LayersManager::new(layers);
    mgr.init();
    assert_eq!(mgr.layers_states.len(), 3);
    assert_eq!(mgr.layers_states[0], true);

    mgr.turn_layer_on(2);
    assert_eq!(mgr.get(KEY_H.into()).action, Tap(Key(KEY_H)));
    assert_eq!(mgr.get(KEY_J.into()).action, Tap(Key(KEY_J)));
    assert_eq!(mgr.get(KEY_K.into()).action, Tap(Key(KEY_K)));
    assert_eq!(mgr.get(KEY_L.into()).action, Tap(Key(KEY_L)));

    assert_eq!(mgr.get(KEY_A.into()).action, TapHold(Key(KEY_A), Key(KEY_LEFTCTRL)));
    assert_eq!(mgr.get(KEY_S.into()).action, TapHold(Key(KEY_S), Key(KEY_LEFTSHIFT)));
    assert_eq!(mgr.get(KEY_D.into()).action, TapHold(Key(KEY_D), Key(KEY_LEFTALT)));

    mgr.turn_layer_on(1);
    assert_eq!(mgr.get(KEY_H.into()).action, Tap(Key(KEY_LEFT)));
    assert_eq!(mgr.get(KEY_J.into()).action, Tap(Key(KEY_DOWN)));
    assert_eq!(mgr.get(KEY_K.into()).action, Tap(Key(KEY_UP)));
    assert_eq!(mgr.get(KEY_L.into()).action, Tap(Key(KEY_RIGHT)));

    assert_eq!(mgr.get(KEY_A.into()).action, TapHold(Key(KEY_A), Key(KEY_LEFTCTRL)));
    assert_eq!(mgr.get(KEY_S.into()).action, TapHold(Key(KEY_S), Key(KEY_LEFTSHIFT)));
    assert_eq!(mgr.get(KEY_D.into()).action, TapHold(Key(KEY_D), Key(KEY_LEFTALT)));

    mgr.turn_layer_off(2);
    assert_eq!(mgr.get(KEY_H.into()).action, Tap(Key(KEY_LEFT)));
    assert_eq!(mgr.get(KEY_J.into()).action, Tap(Key(KEY_DOWN)));
    assert_eq!(mgr.get(KEY_K.into()).action, Tap(Key(KEY_UP)));
    assert_eq!(mgr.get(KEY_L.into()).action, Tap(Key(KEY_RIGHT)));

    assert_eq!(mgr.get(KEY_A.into()).action, Tap(Key(KEY_A)));
    assert_eq!(mgr.get(KEY_S.into()).action, Tap(Key(KEY_S)));
    assert_eq!(mgr.get(KEY_D.into()).action, Tap(Key(KEY_D)));

    mgr.toggle_layer(1);
    assert_eq!(mgr.get(KEY_H.into()).action, Tap(Key(KEY_H)));
    assert_eq!(mgr.get(KEY_J.into()).action, Tap(Key(KEY_J)));
    assert_eq!(mgr.get(KEY_K.into()).action, Tap(Key(KEY_K)));
    assert_eq!(mgr.get(KEY_L.into()).action, Tap(Key(KEY_L)));

    assert_eq!(mgr.get(KEY_A.into()).action, Tap(Key(KEY_A)));
    assert_eq!(mgr.get(KEY_S.into()).action, Tap(Key(KEY_S)));
    assert_eq!(mgr.get(KEY_D.into()).action, Tap(Key(KEY_D)));

    mgr.toggle_layer(1);
    assert_eq!(mgr.get(KEY_H.into()).action, Tap(Key(KEY_LEFT)));
    assert_eq!(mgr.get(KEY_J.into()).action, Tap(Key(KEY_DOWN)));
    assert_eq!(mgr.get(KEY_K.into()).action, Tap(Key(KEY_UP)));
    assert_eq!(mgr.get(KEY_L.into()).action, Tap(Key(KEY_RIGHT)));

    assert_eq!(mgr.get(KEY_A.into()).action, Tap(Key(KEY_A)));
    assert_eq!(mgr.get(KEY_S.into()).action, Tap(Key(KEY_S)));
    assert_eq!(mgr.get(KEY_D.into()).action, Tap(Key(KEY_D)));
}
