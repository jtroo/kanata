// std
// ktrl
use crate::effects::OutEffects;
use crate::keys::KeyCode;
use crate::keys::KeyEvent;
use crate::keys::KeyValue;
use crate::layers::LayersManager;
use crate::layers::LockOwner;

// inner
use inner::inner;

use crate::layers::{Action, Action::TapDance, Effect};

const STOP: bool = true;
const CONTINUE: bool = false;

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
            TapDance(len, tap_fx, dance_fx) => Self {
                len: *len,
                tap_fx: tap_fx.clone(),
                dance_fx: dance_fx.clone(),
            },
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
    TdDancing(TapDanceWaiting),
}

pub struct TapDanceMgr {
    dancing: Option<KeyCode>,
    state: TapDanceState,
    wait_time: u64,
}

// --------------- TapDance-specific Functions ----------------------

impl TapDanceMgr {
    pub fn new(wait_time: u64) -> Self {
        assert!(wait_time < (i64::MAX as u64));
        Self {
            state: TapDanceState::TdIdle,
            dancing: None,
            wait_time,
        }
    }

    fn lock_key(l_mgr: &mut LayersManager, key: KeyCode) {
        l_mgr.lock_key(key, LockOwner::LkTapDance);
    }

    fn unlock_key(l_mgr: &mut LayersManager, key: KeyCode) {
        l_mgr.unlock_key(key, LockOwner::LkTapDance);
    }

    fn set_dancing(&mut self, l_mgr: &mut LayersManager, key: KeyCode) {
        self.dancing = Some(key);
        Self::lock_key(l_mgr, key);
    }

    fn clear_dancing(&mut self, l_mgr: &mut LayersManager) {
        if let Some(dancing) = self.dancing {
            Self::unlock_key(l_mgr, dancing);
        } else {
            unreachable!();
        }

        self.dancing = None;
    }

    fn did_dance_timeout(&self, event: &KeyEvent) -> bool {
        let new_timestamp = event.time.clone();
        let wait_start_timestamp = inner!(&self.state, if TapDanceState::TdDancing)
            .timestamp
            .clone();
        let secs_diff = new_timestamp.tv_sec - wait_start_timestamp.tv_sec;
        let usecs_diff = new_timestamp.tv_usec - wait_start_timestamp.tv_usec;
        let diff = (secs_diff * 1_000_000) + usecs_diff;
        diff >= (self.wait_time as i64) * 1000
    }

    fn handle_th_dancing(
        &mut self,
        l_mgr: &mut LayersManager,
        event: &KeyEvent,
        td_cfg: &TapDanceCfg,
    ) -> OutEffects {
        let did_timeout = self.did_dance_timeout(event);
        let did_key_change = event.code != self.dancing.unwrap().clone();

        if did_timeout || did_key_change {
            let mut fx_vals = self.get_buffered_key_events(td_cfg).effects.unwrap();
            self.state = TapDanceState::TdIdle;
            self.clear_dancing(l_mgr);
            let idle_out = self.handle_th_idle(l_mgr, event, td_cfg);
            if let Some(mut new_fx_vals) = idle_out.effects {
                fx_vals.append(&mut new_fx_vals);
            }
            return OutEffects::new_multiple(STOP, fx_vals);
        }

        let value = KeyValue::from(event.value);
        let wait_state = inner!(&self.state, if TapDanceState::TdDancing);
        match value {
            KeyValue::Press => {
                let mut new_state = wait_state.clone();
                new_state.presses_so_far += 1;

                if new_state.presses_so_far >= td_cfg.len {
                    self.state = TapDanceState::TdDancing(new_state);
                    OutEffects::new(STOP, td_cfg.dance_fx.clone(), KeyValue::Press)
                } else {
                    self.state = TapDanceState::TdDancing(new_state);
                    OutEffects::empty(STOP)
                }
            }
            KeyValue::Release => {
                let mut new_state = wait_state.clone();
                new_state.releases_so_far += 1;

                if new_state.releases_so_far >= td_cfg.len {
                    self.state = TapDanceState::TdIdle;
                    self.clear_dancing(l_mgr);
                    OutEffects::new(STOP, td_cfg.dance_fx.clone(), KeyValue::Release)
                } else {
                    self.state = TapDanceState::TdDancing(new_state);
                    OutEffects::empty(STOP)
                }
            }

            KeyValue::Repeat => {
                // Drop repeats. These aren't supported for TapDances
                OutEffects::empty(STOP)
            }
        }
    }

    fn handle_th_idle(
        &mut self,
        l_mgr: &mut LayersManager,
        event: &KeyEvent,
        td_cfg: &TapDanceCfg,
    ) -> OutEffects {
        assert!(self.state == TapDanceState::TdIdle);

        let keycode: KeyCode = event.code;
        let value = KeyValue::from(event.value);

        match value {
            KeyValue::Press => {
                self.state = TapDanceState::TdDancing(TapDanceWaiting {
                    timestamp: event.time.clone(),
                    presses_so_far: 1,
                    releases_so_far: 0,
                });
                self.set_dancing(l_mgr, keycode);
                OutEffects::empty(STOP)
            }

            KeyValue::Release => {
                // Forward the release
                OutEffects::new(STOP, td_cfg.tap_fx.clone(), KeyValue::Release)
            }

            KeyValue::Repeat => {
                // Drop repeats. These aren't supported for TapDances
                OutEffects::empty(STOP)
            }
        }
    }

    // Assumes this is an event tied to a TapDance assigned MergedKey
    fn process_tap_dance_key(
        &mut self,
        l_mgr: &mut LayersManager,
        event: &KeyEvent,
        td_cfg: &TapDanceCfg,
    ) -> OutEffects {
        let state = &self.state;
        match state {
            TapDanceState::TdIdle => self.handle_th_idle(l_mgr, event, td_cfg),
            TapDanceState::TdDancing(_) => self.handle_th_dancing(l_mgr, event, td_cfg),
        }
    }

    // --------------- Non-TapDance Functions ----------------------

    fn get_buffered_key_events(&self, td_cfg: &TapDanceCfg) -> OutEffects {
        let mut out = OutEffects::empty(STOP);
        let wait_state = inner!(&self.state, if TapDanceState::TdDancing);

        for _i in 0..wait_state.presses_so_far {
            out.insert(td_cfg.tap_fx.clone(), KeyValue::Press);
        }
        for _i in 0..wait_state.releases_so_far {
            out.insert(td_cfg.tap_fx.clone(), KeyValue::Release);
        }

        out
    }

    fn process_non_tap_dance_key(
        &mut self,
        l_mgr: &mut LayersManager,
        _event: &KeyEvent,
    ) -> OutEffects {
        match self.dancing {
            None => OutEffects::empty(CONTINUE),

            Some(dancing) => {
                let action = &l_mgr.get(dancing).action;
                let td_cfg = TapDanceCfg::from_action(action);

                let mut out = self.get_buffered_key_events(&td_cfg);
                out.stop_processing = CONTINUE;

                self.state = TapDanceState::TdIdle;
                self.clear_dancing(l_mgr);

                out
            }
        }
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
        self.dancing.is_none()
    }
}

#[cfg(test)]
use crate::cfg::*;
#[cfg(test)]
use crate::effects::Effect::*;
#[cfg(test)]
use crate::keys::KeyCode::*;

#[test]
fn test_tap_dance() {
    use std::collections::HashMap;
    let mut h = HashMap::new();
    h.insert("base".to_string(), 0);
    let cfg = Cfg::new(
        h,
        vec![
            vec![
                (KEY_A, TapDance(3, Key(KEY_A), Key(KEY_LEFTCTRL))),
            ],
    ]);

    let mut l_mgr = LayersManager::new(&cfg.layers, &cfg.layer_aliases);
    let mut th_mgr = TapDanceMgr::new(500);

    l_mgr.init();

    let ev_th_press = KeyEvent::new_press(KEY_A);
    let ev_th_release = KeyEvent::new_release(KEY_A);

    // 1st
    assert_eq!(
        th_mgr.process(&mut l_mgr, &ev_th_press),
        OutEffects::empty(STOP)
    );
    assert_eq!(th_mgr.is_idle(), false);
    assert_eq!(l_mgr.is_key_locked(KEY_A), true);
    assert_eq!(
        th_mgr.process(&mut l_mgr, &ev_th_release),
        OutEffects::empty(STOP)
    );

    assert_eq!(
        th_mgr.process(&mut l_mgr, &ev_th_press),
        OutEffects::empty(STOP)
    );
    assert_eq!(th_mgr.is_idle(), false);
    assert_eq!(l_mgr.is_key_locked(KEY_A), true);
    assert_eq!(
        th_mgr.process(&mut l_mgr, &ev_th_release),
        OutEffects::empty(STOP)
    );

    assert_eq!(
        th_mgr.process(&mut l_mgr, &ev_th_press),
        OutEffects::new(STOP, Key(KEY_LEFTCTRL), KeyValue::Press)
    );
    assert_eq!(th_mgr.is_idle(), false);
    assert_eq!(l_mgr.is_key_locked(KEY_A), true);
    assert_eq!(
        th_mgr.process(&mut l_mgr, &ev_th_release),
        OutEffects::new(STOP, Key(KEY_LEFTCTRL), KeyValue::Release)
    );

    assert_eq!(th_mgr.is_idle(), true);
    assert_eq!(l_mgr.is_key_locked(KEY_A), false);
}

// TODO
// 1. Test Action::Tap interruption
// 2. Test Action::TapHold interruption
// 3. Test other Action::TapDance interruption
// 4. Test dance timeout
