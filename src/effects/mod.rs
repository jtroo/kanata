mod dj;
mod sticky;

pub mod perform;
pub use dj::Dj;
pub use dj::KSnd;
pub use perform::perform_effect;
pub use sticky::StickyState;

use crate::actions::Action;
use crate::keys::KeyCode;
use crate::keys::KeyEvent;
use crate::keys::KeyValue;
use crate::layers::LayerIndex;
use crate::layers::LayersManager;
use inner::inner;
use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub enum Effect {
    NoOp,

    Key(KeyCode),
    KeySticky(KeyCode),
    KeySeq(Vec<KeyCode>),

    Meh,   // Ctrl+Alt+Shift
    Hyper, // Ctrl+Alt+Shift+Win

    ToggleLayerAlias(String),
    ToggleLayer(LayerIndex),
    MomentaryLayer(LayerIndex),

    Sound(KSnd),
    SoundEx(String),

    Multi(Vec<Effect>),
    // Not Implemented Yet
    // ---------------------
    // OneShotLayer(LayerIndex),
    // OneShotModifier(KeyCode)
    // ToggleModifier(KeyCode)
}

pub fn key_event_to_fx_val(l_mgr: &LayersManager, event: &KeyEvent) -> EffectValue {
    let merged = l_mgr.get(event.code);
    let effect = inner!(&merged.action, if Action::Tap).clone();

    EffectValue {
        fx: effect,
        val: event.value.into(),
    }
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
        Self { fx, val }
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
            effects: Some(vec![EffectValue::new(effect, value)]),
        }
    }

    pub fn new_multiple(stop_processing: bool, effects: Vec<EffectValue>) -> Self {
        OutEffects {
            stop_processing,
            effects: Some(effects),
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
