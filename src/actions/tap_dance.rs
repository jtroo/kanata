// std
use std::vec::Vec;
use std::collections::HashSet;

// ktrl
use crate::layers::LockOwner;
use crate::layers::LayersManager;
use crate::keys::KeyCode;
use crate::keys::KeyValue;
use crate::keys::KeyEvent;
use crate::effects::OutEffects;

// inner
use inner::inner;

use crate::layers::{
    Effect,
    Action,
    Action::TapDance,
};

const STOP: bool = true;
const CONTINUE: bool = false;
const TAP_DANCE_WAIT_PERIOD: i64 = 200000;

// This struct isn't used in Action::TapDance
// due to overhead it'll create in the config file.
// Lots of wrappers in the ron text
struct TapDanceCfg {
    len: usize,
    tap_fx: Effect,
    dance_fx: Effect,
}

impl TapDanceCfg {
    fn from_action(action: &Action) -> Self {
        match action {
            TapDance(len, tap_fx, dance_fx) => Self{len: *len,
                                                    tap_fx: tap_fx.clone(),
                                                    dance_fx: dance_fx.clone()},
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TapDanceWaiting {
    pub timestamp: evdev_rs::TimeVal,
    pub presses_so_far: usize,
    pub releases_so_far: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TapDanceState {
    TdIdle,
    TdWaiting(TapDanceWaiting),
}

pub struct TapDanceMgr {
    // KEY_MAX elements
    states: Vec<TapDanceState>,

    // A list of keys that are currently in TdWaiting
    waiting_keys: HashSet<KeyCode>,
}

// --------------- TapDance-specific Functions ----------------------

impl TapDanceMgr {
    pub fn new() -> Self {
        let mut states = Vec::new();
        states.resize_with(KeyCode::KEY_MAX as usize, || TapDanceState::TdIdle);

        Self{states,
             waiting_keys: HashSet::new()}
    }

    fn lock_key(l_mgr: &mut LayersManager, key: KeyCode) {
        l_mgr.lock_key(key, LockOwner::LkTapDance);
    }

    fn unlock_key(l_mgr: &mut LayersManager, key: KeyCode) {
        l_mgr.unlock_key(key, LockOwner::LkTapDance);
    }

    fn insert_waiting(&mut self, l_mgr: &mut LayersManager, key: KeyCode) {
        self.waiting_keys.insert(key);
        Self::lock_key(l_mgr, key);
    }

    fn remove_waiting(&mut self, l_mgr: &mut LayersManager, key: KeyCode) {
        self.waiting_keys.remove(&key);
        Self::unlock_key(l_mgr, key);
    }

    fn clear_waiting(&mut self, l_mgr: &mut LayersManager) {
        for key in &self.waiting_keys {
            Self::unlock_key(l_mgr, *key);
        }

        self.waiting_keys.clear();
    }

    fn handle_th_waiting(&mut self,
                         l_mgr: &mut LayersManager,
                         event: &KeyEvent,
                         td_cfg: &TapDanceCfg) -> OutEffects {
        let state = &mut self.states[event.code as usize];
        let wait_state = inner!(state, if TapDanceState::TdWaiting);
        let value = KeyValue::from(event.value);

        match value {
            KeyValue::Press => {
                let mut new_state = wait_state.clone();
                new_state.presses_so_far += 1;

                if new_state.presses_so_far >= td_cfg.len {
                    OutEffects::new(STOP, td_cfg.dance_fx.clone(), KeyValue::Press)
                } else {
                    *state = TapDanceState::TdWaiting(new_state);
                    OutEffects::empty(STOP)
                }
            },
            KeyValue::Release => {
                let mut new_state = wait_state.clone();
                new_state.releases_so_far += 1;

                if wait_state.releases_so_far >= td_cfg.len {
                    *state = TapDanceState::TdIdle;
                    self.remove_waiting(l_mgr, event.code);
                    OutEffects::new(STOP, td_cfg.dance_fx.clone(), KeyValue::Release)
                } else {
                    *state = TapDanceState::TdWaiting(new_state);
                    OutEffects::empty(STOP)
                }
            },

            KeyValue::Repeat => {
                // Drop repeats. These aren't supported for TapDances
                OutEffects::empty(STOP)
            }
        }
    }

    fn handle_th_idle(&mut self,
                      l_mgr: &mut LayersManager,
                      event: &KeyEvent,
                      td_cfg: &TapDanceCfg) -> OutEffects {
        let state = &mut self.states[event.code as usize];
        assert!(*state == TapDanceState::TdIdle);

        let keycode: KeyCode = event.code;
        let value = KeyValue::from(event.value);

        match value {
            KeyValue::Press => {
                *state = TapDanceState::TdWaiting(
                    TapDanceWaiting{timestamp: event.time.clone(),
                                    presses_so_far: 1,
                                    releases_so_far: 0}
                );
                self.insert_waiting(l_mgr, keycode);
                OutEffects::empty(STOP)
            },

            KeyValue::Release => {
                // Forward the release
                OutEffects::new(STOP, td_cfg.tap_fx.clone(), KeyValue::Release)
            },

            KeyValue::Repeat => {
                // Drop repeats. These aren't supported for TapDances
                OutEffects::empty(STOP)
            }
        }
    }

    // Assumes this is an event tied to a TapDance assigned MergedKey
    fn process_tap_dance_key(&mut self,
                            l_mgr: &mut LayersManager,
                            event: &KeyEvent,
                            td_cfg: &TapDanceCfg) -> OutEffects {
        let state = &self.states[event.code as usize];
        match state{
            TapDanceState::TdIdle => self.handle_th_idle(l_mgr, event, td_cfg),
            TapDanceState::TdWaiting(_) => self.handle_th_waiting(l_mgr, event, td_cfg),
        }
    }

    // --------------- Non-TapDance Functions ----------------------

    // fn is_waiting_over(key_state: &TapDanceState, event: &KeyEvent) -> bool {
    //     let new_timestamp = event.time.clone();
    //     let wait_start_timestamp = inner!(key_state, if TapDanceState::TdWaiting).timestamp.clone();

    //     let secs_diff = new_timestamp.tv_sec - wait_start_timestamp.tv_sec;
    //     let usecs_diff  = new_timestamp.tv_usec - wait_start_timestamp.tv_usec;

    //     if secs_diff > 0 {
    //         true
    //     } else if usecs_diff > TAP_DANCE_WAIT_PERIOD {
    //         true
    //     } else {
    //         false
    //     }
    // }

    fn process_non_tap_dance_key(&mut self,
                                l_mgr: &mut LayersManager,
                                _event: &KeyEvent) -> OutEffects {
        let mut out = OutEffects::empty(CONTINUE);

        for waiting in &self.waiting_keys {
            let action = &l_mgr.get(*waiting).action;
            let td_cfg = TapDanceCfg::from_action(action);
            let state = &mut self.states[*waiting as usize];
            let wait_state = inner!(state, if TapDanceState::TdWaiting);

            for _i in 0..wait_state.presses_so_far {
                out.insert(td_cfg.tap_fx.clone(), KeyValue::Press);
            }
            for _i in 0..wait_state.releases_so_far {
                out.insert(td_cfg.tap_fx.clone(), KeyValue::Release);
            }

            *state = TapDanceState::TdIdle;
        }

        self.clear_waiting(l_mgr);
        out
    }

    // --------------- High-Level Functions ----------------------

    // Returns true if processed, false if skipped
    pub fn process(&mut self, l_mgr: &mut LayersManager, event: &KeyEvent) -> OutEffects {
        let code = event.code;
        let action = &l_mgr.get(code).action;

        if let Action::TapDance(..) = action {
            let td_cfg = TapDanceCfg::from_action(action);
            self.process_tap_dance_key(l_mgr, event, &td_cfg)
        } else {
            self.process_non_tap_dance_key(l_mgr, event)
        }
    }

    #[cfg(test)]
    pub fn is_idle(&self) -> bool {
        self.waiting_keys.len() == 0
    }
}
