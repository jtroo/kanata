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

pub fn make_key_action(code: EV_KEY) -> Action {
    let effect = Effect::Key(code.into());
    Action::Tap(effect)
}

pub fn make_key_layer_entry(src: EV_KEY, dst: EV_KEY) -> (KeyCode, Action) {
    let src_code: KeyCode = src.into();
    let action = make_key_action(dst);
    return (src_code, action)
}

pub fn make_meh_layer_entry(src: EV_KEY) -> (KeyCode, Action) {
    let src_code: KeyCode = src.into();
    let action = Action::Tap(Effect::Meh);
    return (src_code, action)
}

pub fn make_hyper_layer_entry(src: EV_KEY) -> (KeyCode, Action) {
    let src_code: KeyCode = src.into();
    let action = Action::Tap(Effect::Hyper);
    return (src_code, action)
}

pub fn make_keyseq_action(seq: Vec<EV_KEY>) -> Action {
    let kc_vec = seq.iter()
        .map(|evkey| KeyCode::from(evkey.clone()))
        .collect();

    let effect = Effect::KeySeq(kc_vec);
    Action::Tap(effect)
}

pub fn make_keyseq_layer_entry(src: EV_KEY, seq: Vec<EV_KEY>) -> (KeyCode, Action) {
    let src_code: KeyCode = src.into();
    let action = make_keyseq_action(seq);
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
            make_momentary_layer_entry(KEY_F7, 1),
            make_key_layer_entry(KEY_F8, KEY_A),
            make_meh_layer_entry(KEY_F9),
            make_hyper_layer_entry(KEY_F10),
            make_keyseq_layer_entry(KEY_F11, vec![KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFTSHIFT]),
            make_toggle_layer_entry(KEY_F12, 1),
        ],
        vec![
            make_taphold_layer_entry(KEY_A, KEY_A, KEY_LEFTSHIFT),
            make_taphold_layer_entry(KEY_S, KEY_S, KEY_LEFTALT),
        ],
    ])
}
