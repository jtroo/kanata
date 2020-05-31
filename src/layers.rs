use crate::keys::KeyCode::*;
use log::{debug, warn};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::vec::Vec;

pub use crate::actions::tap_hold::TapHoldState;
pub use crate::actions::Action;
pub use crate::effects::Effect;
use crate::keys::KeyCode;

// -------------- Constants -------------

const MAX_KEY: usize = KEY_MAX as usize;

// ---------------- Types ---------------

pub type LayerIndex = usize;
pub type Layer = HashMap<KeyCode, Action>;
pub type LayerAliases = HashMap<String, LayerIndex>;

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
    LkTapDance,
    LkSticky,
}

pub struct LayersManager {
    // Serves as a cache of the result
    // of stacking all the layers on top of each other.
    pub merged: Merged,

    // This is a read-only representation of the user's layer configuration.
    // The 0th layer is the base and will always be active
    pub layers: Layers,

    pub layer_aliases: LayerAliases,

    // Holds the on/off state for each layer
    pub layers_states: LayersStates,

    // Allows stateful modules (like TapHold)
    // to lock certain keys. This'll prevent them
    // from being changed by layer changes
    pub key_locks: HashMap<KeyCode, LockOwner>,

    // Similar to key_locks,
    // but locks prevents layer changes globally.
    // I.E not key-specific like key_locks
    pub global_lock: Option<LockOwner>,
}

// -------------- Implementation -------------

fn init_merged() -> Merged {
    let mut merged: Merged = Vec::with_capacity(MAX_KEY);

    for i in 0..MAX_KEY {
        if let Ok(code) = KeyCode::try_from(i) {
            let effect = Effect::Key(code);
            let action = Action::Tap(effect);
            let layer_index = 0;
            merged.push(Some(MergedKey {
                code,
                action,
                layer_index,
            }));
        } else {
            merged.push(None);
        }
    }

    assert!(merged.len() == MAX_KEY);
    merged
}

impl LayersManager {
    pub fn new(layers: &Layers, layer_aliases: &LayerAliases) -> Self {
        let merged = init_merged();
        let layers = layers.clone();
        let layer_aliases = layer_aliases.clone();
        let layers_count = layers.len();
        let key_locks = HashMap::new();

        let mut layers_states = Vec::new();
        layers_states.resize_with(layers_count, Default::default);

        LayersManager {
            merged,
            layers,
            layer_aliases,
            layers_states,
            key_locks,
            global_lock: None,
        }
    }

    // ---------------- Locks -------------------------

    pub fn lock_key(&mut self, key: KeyCode, owner: LockOwner) {
        assert!(!self.key_locks.contains_key(&key));
        self.key_locks.insert(key, owner);
    }

    pub fn unlock_key(&mut self, key: KeyCode, owner: LockOwner) {
        assert!(self.key_locks[&key] == owner);
        self.key_locks.remove(&key);
    }

    pub fn lock_all(&mut self, owner: LockOwner) {
        assert!(self.global_lock.is_none());
        self.global_lock = Some(owner);
    }

    pub fn unlock_all(&mut self, _owner: LockOwner) {
        assert!(self.global_lock.is_some());
        self.global_lock = None;
    }

    pub fn is_all_locked(&self) -> bool {
        self.global_lock.is_some()
    }

    #[cfg(test)]
    pub fn is_key_locked(&self, key: KeyCode) -> bool {
        self.key_locks.contains_key(&key)
    }

    // ---------------- Layers Changes -------------------------

    pub fn init(&mut self) {
        self.turn_layer_on(0);
    }

    fn is_overriding_key(
        &self,
        candidate_code: KeyCode,
        candidate_layer_index: LayerIndex,
    ) -> bool {
        let current = self.get(candidate_code);
        return candidate_layer_index >= current.layer_index;
    }

    fn get_replacement_merged_key(&self, layers: &Layers, removed_code: KeyCode) -> MergedKey {
        let current = self.get(removed_code);
        let lower_layer_idx = current.layer_index - 1;

        for i in lower_layer_idx..=0 {
            let lower_layer = &layers[i];
            if !lower_layer.contains_key(&removed_code) {
                continue;
            }

            let lower_action = &layers[i][&removed_code];
            let replacement = MergedKey {
                code: removed_code,
                action: lower_action.clone(),
                layer_index: i,
            };

            return replacement;
        }

        MergedKey {
            code: removed_code,
            action: Action::Tap(Effect::Key(removed_code)),
            layer_index: 0,
        }
    }

    pub fn get(&self, key: KeyCode) -> &MergedKey {
        match &self.merged[usize::from(key)] {
            Some(merged_key) => merged_key,
            _ => panic!("Invalid KeyCode"),
        }
    }

    // Returns None if false. Some(KeyCode) with the locked key
    fn will_layer_override_held_lock(&self, layer: &Layer) -> Option<KeyCode> {
        for key in layer.keys() {
            if self.key_locks.contains_key(key) {
                return Some(*key);
            }
        }

        None
    }

    fn is_layer_change_safe(&self, index: LayerIndex, layer: &Layer) -> bool {
        if self.is_all_locked() {
            warn!(
                "Can't turn layer {} on. You're currently using a blocking action/effect",
                index
            );
            return false;
        }

        if let Some(locked) = self.will_layer_override_held_lock(&layer) {
            warn!("Can't turn layer {} on. {:?} is in use", index, locked);
            return false;
        }

        true
    }

    pub fn turn_layer_on(&mut self, index: LayerIndex) {
        std::assert!(!self.layers_states[index]);
        let layer = &self.layers[index];

        if !self.is_layer_change_safe(index, layer) {
            return;
        }

        for (code, action) in layer {
            let is_overriding = self.is_overriding_key(*code, index);

            if is_overriding {
                let new_entry = MergedKey {
                    code: *code,
                    action: action.clone(),
                    layer_index: index,
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

        let layer = &self.layers[index];
        if !self.is_layer_change_safe(index, layer) {
            return;
        }

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

    pub fn toggle_layer_alias(&mut self, name: String) {
        if let Some(index) = self.get_idx_from_alias(name) {
            // clone into idx to avoid mutable borrow reservation conflict
            let idx = index.clone();
            self.toggle_layer(idx);
        }
    }


    fn get_idx_from_alias(&self, name: String) -> Option<&usize> {
        self.layer_aliases.get(&name)
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

#[cfg(test)]
use crate::cfg::Cfg;

#[test]
fn test_mgr() {
    let mut h = HashMap::new();
    h.insert("base".to_string(), 0);
    h.insert("arrows".to_string(), 1);
    h.insert("asdf".to_string(), 2);
    let cfg = Cfg::new(
        h,
        vec![
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

    let mut mgr = LayersManager::new(&cfg.layers, &cfg.layer_aliases);
    mgr.init();
    assert_eq!(mgr.layers_states.len(), 3);
    assert_eq!(mgr.layers_states[0], true);

    mgr.turn_layer_on(2);
    assert_eq!(mgr.get(KEY_H.into()).action, Tap(Key(KEY_H)));
    assert_eq!(mgr.get(KEY_J.into()).action, Tap(Key(KEY_J)));
    assert_eq!(mgr.get(KEY_K.into()).action, Tap(Key(KEY_K)));
    assert_eq!(mgr.get(KEY_L.into()).action, Tap(Key(KEY_L)));

    assert_eq!(
        mgr.get(KEY_A.into()).action,
        TapHold(Key(KEY_A), Key(KEY_LEFTCTRL))
    );
    assert_eq!(
        mgr.get(KEY_S.into()).action,
        TapHold(Key(KEY_S), Key(KEY_LEFTSHIFT))
    );
    assert_eq!(
        mgr.get(KEY_D.into()).action,
        TapHold(Key(KEY_D), Key(KEY_LEFTALT))
    );

    mgr.turn_layer_on(1);
    assert_eq!(mgr.get(KEY_H.into()).action, Tap(Key(KEY_LEFT)));
    assert_eq!(mgr.get(KEY_J.into()).action, Tap(Key(KEY_DOWN)));
    assert_eq!(mgr.get(KEY_K.into()).action, Tap(Key(KEY_UP)));
    assert_eq!(mgr.get(KEY_L.into()).action, Tap(Key(KEY_RIGHT)));

    assert_eq!(
        mgr.get(KEY_A.into()).action,
        TapHold(Key(KEY_A), Key(KEY_LEFTCTRL))
    );
    assert_eq!(
        mgr.get(KEY_S.into()).action,
        TapHold(Key(KEY_S), Key(KEY_LEFTSHIFT))
    );
    assert_eq!(
        mgr.get(KEY_D.into()).action,
        TapHold(Key(KEY_D), Key(KEY_LEFTALT))
    );

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

    mgr.lock_all(LockOwner::LkTapHold);
    mgr.toggle_layer(1);
    assert_eq!(mgr.get(KEY_H.into()).action, Tap(Key(KEY_LEFT)));
    mgr.unlock_all(LockOwner::LkTapHold);
    mgr.toggle_layer(1);
    assert_eq!(mgr.get(KEY_H.into()).action, Tap(Key(KEY_H)));
}

#[test]
fn test_overlapping_keys() {
    let mut h = HashMap::new();
    h.insert("base".to_string(), 0);
    h.insert("arrows".to_string(), 1);
    let cfg = Cfg::new(
        h,
        vec![
            // 0: base layer
            vec![
                (KEY_A, TapHold(Key(KEY_A), Key(KEY_LEFTSHIFT))),
            ],

            // 1: arrows layer
            // Ex: switch CTRL <--> Capslock
            vec![
                (KEY_A, TapHold(Key(KEY_A), Key(KEY_LEFTSHIFT))),
            ]
        ],
    );

    let mut mgr = LayersManager::new(&cfg.layers, &cfg.layer_aliases);
    mgr.init();

    assert_eq!(mgr.layers_states.len(), 2);
    assert_eq!(
        mgr.get(KEY_A.into()).action,
        TapHold(Key(KEY_A), Key(KEY_LEFTSHIFT))
    );
    mgr.turn_layer_on(1);
    assert_eq!(
        mgr.get(KEY_A.into()).action,
        TapHold(Key(KEY_A), Key(KEY_LEFTSHIFT))
    );
    mgr.turn_layer_off(1);
    assert_eq!(
        mgr.get(KEY_A.into()).action,
        TapHold(Key(KEY_A), Key(KEY_LEFTSHIFT))
    );
}
