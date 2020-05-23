use std::collections::HashSet;
use crate::keys::KeyCode;
use log::debug;

pub struct StickyState {
    pressed: HashSet<KeyCode>
}

impl StickyState {
    pub fn new() -> Self {
        Self{pressed: HashSet::new()}
    }

    pub fn update_pressed(&mut self, key: KeyCode) {
        debug!("Activating sticky {:?}", key);
        self.pressed.insert(key);
    }

    pub fn update_released(&mut self, key: KeyCode) {
        debug!("Deactivating sticky {:?}", key);
        self.pressed.remove(&key);
    }

    pub fn is_pressed(&mut self, key: KeyCode) -> bool {
        self.pressed.contains(&key)
    }
}
