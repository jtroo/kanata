use crate::layers::Layers;

#[cfg(test)]
use crate::keys::KeyCode;
#[cfg(test)]
use crate::layers::Layer;
#[cfg(test)]
use crate::actions::Action;

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
    #[cfg(test)]
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

pub fn parse(cfg: &String) -> CfgLayers {
    de::from_str(cfg)
        .expect("Failed to parse the config file")
}
