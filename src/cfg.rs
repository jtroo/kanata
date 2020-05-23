use evdev_rs::enums::EV_KEY;
use evdev_rs::enums::EV_KEY::*;

use crate::keys::KeyCode;
use crate::layers::Layer;
use crate::layers::LayerIndex;
use crate::layers::Layers;
use crate::actions::Action;
use crate::effects::Effect;

// ------------------- CfgLayers ---------------------

/// This is a thin-wrapper around `layers::Layers`.
/// It's used only for easy constructions of configuration layers.
/// It encapsulates away the conversion of the input vectors to maps.
pub struct CfgLayers {
    pub layers: Layers,
}

impl CfgLayers {
    pub fn new(layers: Vec<Vec<(KeyCode, Action)>>) -> Self {
        let mut converted: Vec<Layer> = vec![];
        for layer_vec in layers {
            converted.push(layer_vec.into_iter().collect::<Layer>());
        }

        Self{layers: converted}
    }

    #[cfg(test)]
    pub fn empty() -> Self {
        Self{layers: Vec::new()}
    }
}


// ------------------- Util Functions ---------------------

pub fn make_taphold_action(tap: EV_KEY, hold: EV_KEY) -> Action {
    let tap_fx = Effect::Key(tap.into());
    let hold_fx = Effect::Key(hold.into());
    Action::TapHold(tap_fx, hold_fx)
}

pub fn make_taphold_layer_entry(src: EV_KEY, tap: EV_KEY, hold: EV_KEY) -> (KeyCode, Action) {
    let src_code: KeyCode = src.into();
    let action = make_taphold_action(tap, hold);
    return (src_code, action)
}

#[cfg(test)]
pub fn make_default_action(code: EV_KEY) -> Action {
    let effect = Effect::Key(code.into());
    Action::Tap(effect)
}

#[cfg(test)]
pub fn make_default_layer_entry(src: EV_KEY, dst: EV_KEY) -> (KeyCode, Action) {
    let src_code: KeyCode = src.into();
    let action = make_default_action(dst);
    return (src_code, action)
}

pub fn make_toggle_layer_action(idx: LayerIndex) -> Action {
    let effect = Effect::ToggleLayer(idx);
    Action::Tap(effect)
}

pub fn make_toggle_layer_entry(key: EV_KEY, idx: LayerIndex) -> (KeyCode, Action) {
    let code: KeyCode = key.into();
    let action = make_toggle_layer_action(idx);
    return (code, action)
}

pub fn make_momentary_layer_action(idx: LayerIndex) -> Action {
    let effect = Effect::MomentaryLayer(idx);
    Action::Tap(effect)
}

pub fn make_momentary_layer_entry(key: EV_KEY, idx: LayerIndex) -> (KeyCode, Action) {
    let code: KeyCode = key.into();
    let action = make_momentary_layer_action(idx);
    return (code, action)
}

pub fn my_layers() -> CfgLayers {
    CfgLayers::new(vec![
        // 0: base layer
        vec![
            make_momentary_layer_entry(KEY_Q, 1),
            make_toggle_layer_entry(KEY_F12, 1),
        ],
        vec![
            make_taphold_layer_entry(KEY_A, KEY_A, KEY_LEFTSHIFT),
            make_taphold_layer_entry(KEY_S, KEY_S, KEY_LEFTALT),
        ],
    ])
}
