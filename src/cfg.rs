use crate::layers::Layers;

#[cfg(test)]
use crate::actions::Action;
#[cfg(test)]
use crate::keys::KeyCode;
#[cfg(test)]
use crate::layers::Layer;

use ron::de;
use serde::Deserialize;

// ------------------- Cfg ---------------------

/// This is a thin-wrapper around `layers::Layers`.
/// It's used only for easy constructions of configuration layers.
/// It encapsulates away the conversion of the input vectors to maps.
#[derive(Debug, Deserialize)]
pub struct Cfg {
    pub layers: Layers,
    pub tap_hold_wait_time: u64,
    pub tap_dance_wait_time: u64,
}

impl Cfg {
    #[cfg(test)]
    pub fn new(layers: Vec<Vec<(KeyCode, Action)>>) -> Self {
        let mut converted: Vec<Layer> = vec![];
        for layer_vec in layers {
            converted.push(layer_vec.into_iter().collect::<Layer>());
        }

        Self {
            layers: converted,
            tap_hold_wait_time: 0,
            tap_dance_wait_time: 0,
        }
    }
}

// ------------------- Util Functions ---------------------

pub fn parse(cfg: &String) -> Cfg {
    de::from_str(cfg).expect("Failed to parse the config file")
}
