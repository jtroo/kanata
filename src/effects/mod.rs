use crate::keys::KeyValue;
use crate::keys::KeyCode;
use crate::layers::LayerIndex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    // Used externally for Press+Release
    Default(KeyCode),

    // Not Implemented Yet
    // ---------------------
    // ToggleLayer(LayerIndex),
    // OneShotLayer(LayerIndex),

    // ToggleModifier(KeyCode)
    // OneShotModifier(KeyCode)

    // TODO: Consider how to implement KeyChords.
    // e.g pressing shift-keys ('!', '@', '#').
    // or ctrl-keys ('ctrl-j', 'ctrl-k')
}

// ------------------- Output Effects -----------------

// These are returned by action handlers.
// E.g TapHoldMgr::process

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectValue {
    pub fx: Effect,
    pub val: KeyValue,
}

impl EffectValue {
    pub fn new(fx: Effect, val: KeyValue) -> Self {
        Self{fx, val}
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutEffects {
    pub stop_processing: bool,
    pub effects: Option<Vec<EffectValue>>,
}

impl OutEffects {
    pub fn new(stop_processing: bool, effect: Effect, value: KeyValue) -> Self {
        OutEffects {
            stop_processing,
            effects: Some(vec![EffectValue::new(effect, value)])
        }
    }

    #[cfg(test)]
    pub fn new_multiple(stop_processing: bool, effects: Vec<EffectValue>) -> Self {
        OutEffects {
            stop_processing,
            effects: Some(effects)
        }
    }

    pub fn empty(stop_processing: bool) -> Self {
        OutEffects {
            stop_processing,
            effects: None,
        }
    }

    pub fn insert(&mut self, effect: Effect, value: KeyValue) {
        if let Some(effects) = &mut self.effects {
            effects.push(EffectValue::new(effect, value));
        } else {
            self.effects = Some(vec![EffectValue::new(effect, value)]);
        }
    }
}
