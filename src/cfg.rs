use crate::layers::Layers;

#[cfg(test)]
use crate::keys::KeyCode;
#[cfg(test)]
use crate::layers::Layer;
#[cfg(test)]
use crate::actions::Action;

use ron::de;
use serde::Deserialize;
use std::collections::HashMap;

// ------------------- Cfg ---------------------

/// This is a thin-wrapper around `layers::Layers`.
/// It's used only for easy constructions of configuration layers.
/// It encapsulates away the conversion of the input vectors to maps.
#[derive(Debug, Deserialize)]
pub struct Cfg {
    pub layers: Layers,
    pub layer_aliases: HashMap<usize, Option<String>>,
    pub tap_hold_wait_time: u64,
    pub tap_dance_wait_time: u64,
}

#[derive(Debug, Deserialize)]
pub struct PreCfg {
    pub layers: Layers,
    pub layer_aliases: HashMap<usize, String>,
    pub tap_hold_wait_time: u64,
    pub tap_dance_wait_time: u64,
}

impl Cfg {
    #[cfg(test)]
    pub fn new(aliases: HashMap<usize, String>, layers: Vec<Vec<(KeyCode, Action)>>) -> Self {
        let mut converted: Vec<Layer> = vec![];
        let mut layer_aliases: HashMap<usize, Option<String>> =
            HashMap::with_capacity(layers.len());
        for i in 0..layers.len() {
            if let Some(alias) = aliases.get(&i) {
                layer_aliases.insert(i, Some(alias.clone()));
            } else {
                layer_aliases.insert(i, None);
            }
            converted.push(layers[i].clone().into_iter().collect::<Layer>());
        }

        Self {
            layers: converted,
            layer_aliases,
            tap_hold_wait_time: 0,
            tap_dance_wait_time: 0,
        }
    }
}

// ------------------- Util Functions ---------------------

pub fn parse(cfg: &String) -> Cfg {
    let pre: PreCfg = de::from_str(cfg).expect("Failed to parse the config file");
    let mut aliases = HashMap::new();
    for i in 0..pre.layers.len() {
        if let Some(alias) = pre.layer_aliases.get(&i) {
            aliases.insert(i, Some(alias.clone()));
        } else {
            aliases.insert(i, None);
        }
    }
    Cfg {
        layers: pre.layers,
        layer_aliases: aliases,
        tap_hold_wait_time: pre.tap_hold_wait_time,
        tap_dance_wait_time: pre.tap_dance_wait_time,
    }
}
