use crate::keys::KeyCode;
use crate::keys::KeyCode::*;

use crate::layers::Layer;
use crate::layers::Layers;
use crate::actions::Action;
use crate::actions::Action::*;
use crate::effects::Effect::*;

use ron::de;
use serde::Deserialize;

// ------------------- CfgLayers ---------------------

/// This is a thin-wrapper around `layers::Layers`.
/// It's used only for easy constructions of configuration layers.
/// It encapsulates away the conversion of the input vectors to maps.
#[derive(Debug, Deserialize)]
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

pub fn my_layers() -> CfgLayers {
    let str_cfg = "(
    layers: [
        {
            KEY_F6:  Tap(KeySticky(KEY_LEFTSHIFT)),
            KEY_F7:  Tap(MomentaryLayer(1)),
            KEY_F8:  Tap(Key(KEY_A)),
            KEY_F9:  Tap(Meh),
            KEY_F10: Tap(Hyper),
            KEY_F11: Tap(KeySeq([KEY_LEFTCTRL, KEY_LEFTALT, KEY_LEFTSHIFT])),
            KEY_F12: Tap(ToggleLayer(1)),
        },
        {
            KEY_A: TapHold(Key(KEY_A), Key(KEY_LEFTSHIFT)),
            KEY_S: TapHold(Key(KEY_S), Key(KEY_LEFTALT)),
        },
    ],
)
";

    dbg!(de::from_str(str_cfg).unwrap())
}
