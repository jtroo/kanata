use crate::keys::KeyCode;
use crate::layers::LayersManager;
use crate::layers::LockOwner::LkSticky;
use log::debug;
use std::collections::HashSet;

pub struct StickyState {
    pressed: HashSet<KeyCode>,
}

impl StickyState {
    pub fn new() -> Self {
        Self {
            pressed: HashSet::new(),
        }
    }

    pub fn update_pressed(&mut self, l_mgr: &mut LayersManager, key: KeyCode) {
        debug!("Activating sticky {:?}", key);
        self.pressed.insert(key);
        l_mgr.lock_all(LkSticky);
    }

    pub fn update_released(&mut self, l_mgr: &mut LayersManager, key: KeyCode) {
        debug!("Deactivating sticky {:?}", key);
        self.pressed.remove(&key);
        l_mgr.unlock_all(LkSticky);
    }

    pub fn is_pressed(&mut self, key: KeyCode) -> bool {
        self.pressed.contains(&key)
    }
}
