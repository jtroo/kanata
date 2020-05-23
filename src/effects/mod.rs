use evdev_rs::enums::EventCode;
use evdev_rs::InputEvent;

pub mod perform;
pub use perform::perform_effect;

use crate::keys::KeyValue;
use crate::keys::KeyCode;
use crate::layers::LayerIndex;
use crate::layers::LayersManager;
use crate::actions::Action;
use inner::inner;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Key(KeyCode),
    // KeqSeq(Vec<KeyCode>),

    ToggleLayer(LayerIndex),
    MomentaryLayer(LayerIndex),

    // Not Implemented Yet
    // ---------------------
    // OneShotLayer(LayerIndex),
    // OneShotModifier(KeyCode)
    // ToggleModifier(KeyCode)

    // TODO: Consider how to implement KeyChords.
    // e.g pressing shift-keys ('!', '@', '#').
    // or ctrl-keys ('ctrl-j', 'ctrl-k')
}

pub fn event_to_fx_val(l_mgr: &LayersManager, event: &InputEvent) -> Option<EffectValue> {
    match &event.event_code {
        EventCode::EV_KEY(evkey) => {
            let kc = KeyCode::from(evkey.clone());
            let merged = l_mgr.get(kc);
            let effect = inner!(&merged.action, if Action::Tap).clone();

            Some(EffectValue{
                fx: effect,
                val: event.value.into(),
            })
        }
        _ => None
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
