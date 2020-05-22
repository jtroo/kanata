// evdev-rs
use evdev_rs::enums::EventCode;
use evdev_rs::InputEvent;

// std
use std::vec::Vec;
use std::collections::HashSet;

// ktrl
use crate::layers::LayersManager;
use crate::keys::KeyCode;
use crate::keys::KeyValue;

// inner
use inner::*;

use crate::layers::{
    Effect,
    Action,
    KeyState,
    MergedKey,
};

const STOP: bool = true;
const CONTINUE: bool = false;
const TAP_HOLD_WAIT_PERIOD: i64 = 200000;

#[derive(Clone, Debug, PartialEq)]
pub struct TapHoldWaiting {
    pub timestamp: evdev_rs::TimeVal,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TapHoldState {
    ThIdle,
    ThWaiting(TapHoldWaiting),
    ThHolding,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectValue {
    pub fx: Effect,
    pub val: KeyValue,
}

impl EffectValue {
    pub fn new(fx: Effect, val: KeyValue) -> Self {
        Self{fx, val}
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TapHoldOut {
    pub stop_processing: bool,
    pub effects: Option<Vec<EffectValue>>,
}

impl TapHoldOut {
    fn new(stop_processing: bool, effect: Effect, value: KeyValue) -> Self {
        TapHoldOut {
            stop_processing,
            effects: Some(vec![EffectValue::new(effect, value)])
        }
    }

    #[cfg(test)]
    fn new_multiple(stop_processing: bool, effects: Vec<EffectValue>) -> Self {
        TapHoldOut {
            stop_processing,
            effects: Some(effects)
        }
    }

    fn empty(stop_processing: bool) -> Self {
        TapHoldOut {
            stop_processing,
            effects: None,
        }
    }

    fn insert(&mut self, effect: Effect, value: KeyValue) {
        if let Some(effects) = &mut self.effects {
            effects.push(EffectValue::new(effect, value));
        } else {
            self.effects = Some(vec![EffectValue::new(effect, value)]);
        }
    }
}

pub struct TapHoldMgr {
    // A list of keys that are currently in ThWaiting
    waiting_keys: Vec<KeyCode>,

    // A list of keys that are currently in ThHolding
    holding_keys: HashSet<KeyCode>,
}

impl TapHoldMgr {
    pub fn new() -> Self {
        Self{waiting_keys: Vec::new(),
             holding_keys: HashSet::new()}
    }
}

fn get_keycode_from_event(event: &InputEvent) -> Option<KeyCode> {
    if let EventCode::EV_KEY(ev_key) = &event.event_code {
        let code: KeyCode = KeyCode::from(ev_key.clone());
        Some(code)
    } else {
        None
    }
}

// --------------- TapHold-specific Functions ----------------------

impl TapHoldMgr {
    fn handle_th_holding(&mut self,
                         event: &InputEvent,
                         state: &mut TapHoldState,
                         _tap_fx: &Effect,
                         hold_fx: &Effect) -> TapHoldOut {
        assert!(*state == TapHoldState::ThHolding);
        let value = KeyValue::from(event.value);

        match value {
            KeyValue::Press => {
                // Should never happen.
                // Should only see this in the idle state
                assert!(false);
                TapHoldOut::empty(STOP)
            },

            KeyValue::Release => {
                // Cleanup the hold
                *state = TapHoldState::ThIdle;
                self.waiting_keys.clear();
                let kc = get_keycode_from_event(event).unwrap();
                self.holding_keys.remove(&kc);
                TapHoldOut::new(STOP, *hold_fx, KeyValue::Release) // forward the release
            },

            KeyValue::Repeat => {
                // Drop repeats. These aren't supported for TapHolds
                TapHoldOut::empty(STOP)
            }
        }
    }

    fn handle_th_waiting(&mut self,
                         event: &InputEvent,
                         state: &mut TapHoldState,
                         tap_fx: &Effect,
                         _hold_fx: &Effect) -> TapHoldOut {
        let value = KeyValue::from(event.value);

        match value {
            KeyValue::Press => {
                // Should never happen.
                // Should only see this in the idle state
                assert!(false);
                TapHoldOut::empty(STOP)
            },

            KeyValue::Release => {
                // Forward the release.
                // We didn't reach the hold state
                *state = TapHoldState::ThIdle;
                self.waiting_keys.clear();
                let mut out = TapHoldOut::new(STOP, *tap_fx, KeyValue::Press);
                out.insert(*tap_fx, KeyValue::Release);
                out
            },

            KeyValue::Repeat => {
                // Drop repeats. These aren't supported for TapHolds
                TapHoldOut::empty(STOP)
            }
        }
    }

    fn handle_th_idle(&mut self,
                      event: &InputEvent,
                      state: &mut TapHoldState,
                      tap_fx: &Effect,
                      _hold_fx: &Effect) -> TapHoldOut {
        assert!(*state == TapHoldState::ThIdle);
        let keycode: KeyCode = event.event_code.clone().into();
        let value = KeyValue::from(event.value);

        match value {
            KeyValue::Press => {
                // Transition to the waiting state.
                // I.E waiting for either an interruptions => Press+Release the Tap effect
                // or for the TapHold wait period => Send a Hold effect press
                self.waiting_keys.push(keycode.clone());
                *state = TapHoldState::ThWaiting(
                    TapHoldWaiting{timestamp: event.time.clone()}
                );
                TapHoldOut::empty(STOP)
            },

            KeyValue::Release => {
                // Forward the release
                TapHoldOut::new(STOP, *tap_fx, KeyValue::Release)
            },

            KeyValue::Repeat => {
                // Drop repeats. These aren't supported for TapHolds
                TapHoldOut::empty(STOP)
            }
        }
    }

    // Assumes this is an event tied to a TapHold assigned MergedKey
    fn process_tap_hold_key(&mut self,
                            event: &InputEvent,
                            state: &mut KeyState,
                            tap_fx: &Effect,
                            hold_fx: &Effect) -> TapHoldOut {
        if let KeyState::KsTapHold(th_state) = state {
            match &th_state {
                TapHoldState::ThIdle => self.handle_th_idle(event, th_state, tap_fx, hold_fx),
                TapHoldState::ThWaiting(_) => self.handle_th_waiting(event, th_state, tap_fx, hold_fx),
                TapHoldState::ThHolding => self.handle_th_holding(event, th_state, tap_fx, hold_fx),
            }
        } else {
            assert!(false);
            TapHoldOut::empty(STOP)
        }
    }

    // --------------- Non-TapHold Functions ----------------------

    fn is_waiting_over(merged_key: &MergedKey, event: &InputEvent) -> bool {
        let new_timestamp = event.time.clone();
        let wait_start_timestamp = inner!(inner!(&merged_key.state, if KeyState::KsTapHold), if TapHoldState::ThWaiting).timestamp.clone();

        let secs_diff = new_timestamp.tv_sec - wait_start_timestamp.tv_sec;
        let usecs_diff  = new_timestamp.tv_usec - wait_start_timestamp.tv_usec;

        if secs_diff > 0 {
            true
        } else if usecs_diff > TAP_HOLD_WAIT_PERIOD {
            true
        } else {
            false
        }
    }

    fn process_non_tap_hold_key(&mut self,
                                l_mgr: &mut LayersManager,
                                event: &InputEvent) -> TapHoldOut {
        let mut out = TapHoldOut::empty(CONTINUE);

        for waiting in self.waiting_keys.drain(..) {
            let merged_key: &mut MergedKey = l_mgr.get_mut(waiting.clone());

            if Self::is_waiting_over(merged_key, event) {
                // Append the press hold_fx to the output
                let hold_fx = match merged_key.action {
                    Action::TapHold(_tap_fx, hold_fx) => hold_fx,
                    _ => {assert!(false); Effect::Default(0.into())},
                };
                out.insert(hold_fx, KeyValue::Press);

                // Change to the holding state
                merged_key.state = KeyState::KsTapHold(TapHoldState::ThHolding);
                self.holding_keys.insert(waiting);

            } else {
                // Flush the press and release tap_fx
                let tap_fx = match merged_key.action {
                    Action::TapHold(tap_fx, _hold_fx) => tap_fx,
                    _ => {assert!(false); Effect::Default(0.into())},
                };
                out.insert(tap_fx, KeyValue::Press);

                // Revert to the idle state
                merged_key.state = KeyState::KsTapHold(TapHoldState::ThIdle);
            }
        }

        out
    }

    // --------------- High-Level Functions ----------------------

    // Returns true if processed, false if skipped
    pub fn process(&mut self, l_mgr: &mut LayersManager, event: &InputEvent) -> TapHoldOut {
        let code = match get_keycode_from_event(event) {
            Some(code) => code,
            None => { return TapHoldOut::empty(CONTINUE) },
        };

        let merged_key: &mut MergedKey = l_mgr.get_mut(code);
        if let Action::TapHold(tap_fx, hold_fx) = merged_key.action.clone() {
            self.process_tap_hold_key(event, &mut merged_key.state, &tap_fx, &hold_fx)
        } else {
            self.process_non_tap_hold_key(l_mgr, event)
        }
    }

    // // Used by Ktrl to make sure toggling layers is okay
    // pub fn is_idle(&self) -> bool {
    //     self.waiting_keys.len() == 0 &&
    //         self.holding_keys.len() == 0
    // }
}

#[cfg(test)]
use crate::keys::KeyEvent;
#[cfg(test)]
use crate::cfg::*;

#[cfg(test)]
use evdev_rs::enums::EV_KEY::*;

#[test]
fn test_skipped() {
    let mut th_mgr = TapHoldMgr::new();
    let mut l_mgr = LayersManager::new(CfgLayers::empty());
    let ev_non_th_press = KeyEvent::new_press(&EventCode::EV_KEY(KEY_A)).event;
    let ev_non_th_release = KeyEvent::new_release(&EventCode::EV_KEY(KEY_A)).event;
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_non_th_press), TapHoldOut::empty(CONTINUE));
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_non_th_release), TapHoldOut::empty(CONTINUE));
    assert_eq!(th_mgr.waiting_keys.len(), 0);
}

#[test]
fn test_tap() {
    let layers = CfgLayers::new(vec![
        // 0: base layer
        vec![
            make_taphold_layer_entry(KEY_A, KEY_A, KEY_LEFTCTRL),
            make_taphold_layer_entry(KEY_S, KEY_S, KEY_LEFTALT),
        ],
    ]);

    let mut l_mgr = LayersManager::new(layers);
    let mut th_mgr = TapHoldMgr::new();

    l_mgr.init();

    let ev_th_press = KeyEvent::new_press(&EventCode::EV_KEY(KEY_A)).event;
    let mut ev_th_release = KeyEvent::new_release(&EventCode::EV_KEY(KEY_A)).event;
    ev_th_release.time.tv_usec += 100;

    // 1st
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_th_press), TapHoldOut::empty(STOP));
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_th_release), TapHoldOut::new_multiple(STOP, vec![
        EffectValue::new(Effect::Default(KEY_A.into()), KeyValue::Press),
        EffectValue::new(Effect::Default(KEY_A.into()), KeyValue::Release),
    ]));
    assert_eq!(th_mgr.waiting_keys.len(), 0);

    // 2nd
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_th_press), TapHoldOut::empty(STOP));
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_th_release), TapHoldOut::new_multiple(STOP, vec![
        EffectValue::new(Effect::Default(KEY_A.into()), KeyValue::Press),
        EffectValue::new(Effect::Default(KEY_A.into()), KeyValue::Release),
    ]));
    assert_eq!(th_mgr.waiting_keys.len(), 0);

    // interruptions: 1
    ev_th_release.time.tv_usec = TAP_HOLD_WAIT_PERIOD + 1;
    let ev_non_th_press = KeyEvent::new_press(&EventCode::EV_KEY(KEY_W)).event;
    let ev_non_th_release = KeyEvent::new_release(&EventCode::EV_KEY(KEY_W)).event;
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_th_press), TapHoldOut::empty(STOP));
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_non_th_press), TapHoldOut::new(CONTINUE, Effect::Default(KEY_A.into()), KeyValue::Press));
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_th_release), TapHoldOut::new(STOP, Effect::Default(KEY_A.into()), KeyValue::Release));
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_non_th_release), TapHoldOut::empty(CONTINUE));
    assert_eq!(th_mgr.waiting_keys.len(), 0);

    // interruptions: 2
    ev_th_release.time.tv_usec = TAP_HOLD_WAIT_PERIOD + 1;
    let ev_non_th_press = KeyEvent::new_press(&EventCode::EV_KEY(KEY_W)).event;
    let ev_non_th_release = KeyEvent::new_release(&EventCode::EV_KEY(KEY_W)).event;
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_th_press), TapHoldOut::empty(STOP));
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_non_th_press), TapHoldOut::new(CONTINUE, Effect::Default(KEY_A.into()), KeyValue::Press));
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_non_th_release), TapHoldOut::empty(CONTINUE));
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_th_release), TapHoldOut::new(STOP, Effect::Default(KEY_A.into()), KeyValue::Release));
    assert_eq!(th_mgr.waiting_keys.len(), 0);
}

#[test]
fn test_hold() {
    let layers = CfgLayers::new(vec![
        // 0: base layer
        vec![
            make_taphold_layer_entry(KEY_A, KEY_A, KEY_LEFTCTRL),
            make_taphold_layer_entry(KEY_S, KEY_S, KEY_LEFTALT),
        ],
    ]);

    let mut l_mgr = LayersManager::new(layers);
    let mut th_mgr = TapHoldMgr::new();

    l_mgr.init();

    let ev_th_press = KeyEvent::new_press(&EventCode::EV_KEY(KEY_A)).event;
    let mut ev_th_release = KeyEvent::new_release(&EventCode::EV_KEY(KEY_A)).event;
    ev_th_release.time.tv_usec = TAP_HOLD_WAIT_PERIOD + 1;

    // No hold + other key chord
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_th_press), TapHoldOut::empty(STOP));
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_th_release), TapHoldOut::new_multiple(STOP, vec![
        EffectValue::new(Effect::Default(KEY_A.into()), KeyValue::Press),
        EffectValue::new(Effect::Default(KEY_A.into()), KeyValue::Release),
    ]));
    assert_eq!(th_mgr.waiting_keys.len(), 0);


    // -------------------------------

    // Hold with other key
    let mut ev_non_th_press = KeyEvent::new_press(&EventCode::EV_KEY(KEY_W)).event;
    let mut ev_non_th_release = KeyEvent::new_release(&EventCode::EV_KEY(KEY_W)).event;
    ev_non_th_press.time.tv_usec = TAP_HOLD_WAIT_PERIOD + 1;
    ev_non_th_release.time.tv_usec = TAP_HOLD_WAIT_PERIOD + 2;
    ev_th_release.time.tv_usec = TAP_HOLD_WAIT_PERIOD + 3;

    assert_eq!(th_mgr.process(&mut l_mgr, &ev_th_press), TapHoldOut::empty(STOP));
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_non_th_press), TapHoldOut::new(CONTINUE, Effect::Default(KEY_LEFTCTRL.into()), KeyValue::Press));
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_non_th_release), TapHoldOut::empty(CONTINUE));
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_th_release), TapHoldOut::new(STOP, Effect::Default(KEY_LEFTCTRL.into()), KeyValue::Release));

    // -------------------------------

    // Hold with other key (different order)
    ev_non_th_press.time.tv_usec = TAP_HOLD_WAIT_PERIOD + 1;
    ev_th_release.time.tv_usec = TAP_HOLD_WAIT_PERIOD + 2;
    ev_non_th_release.time.tv_usec = TAP_HOLD_WAIT_PERIOD + 3;

    assert_eq!(th_mgr.process(&mut l_mgr, &ev_th_press), TapHoldOut::empty(STOP));
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_non_th_press), TapHoldOut::new(CONTINUE, Effect::Default(KEY_LEFTCTRL.into()), KeyValue::Press));
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_th_release), TapHoldOut::new(STOP, Effect::Default(KEY_LEFTCTRL.into()), KeyValue::Release));
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_non_th_release), TapHoldOut::empty(CONTINUE));
}
