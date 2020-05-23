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
use crate::effects::EffectValue;
use crate::effects::OutEffects;

// inner
use inner::inner;

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
                         hold_fx: &Effect) -> OutEffects {
        assert!(*state == TapHoldState::ThHolding);
        let value = KeyValue::from(event.value);

        match value {
            KeyValue::Press => {
                // Should never happen.
                // Should only see this in the idle state
                assert!(false);
                OutEffects::empty(STOP)
            },

            KeyValue::Release => {
                // Cleanup the hold
                *state = TapHoldState::ThIdle;
                self.waiting_keys.clear();
                let kc = get_keycode_from_event(event).unwrap();
                self.holding_keys.remove(&kc);
                OutEffects::new(STOP, hold_fx.clone(), KeyValue::Release) // forward the release
            },

            KeyValue::Repeat => {
                // Drop repeats. These aren't supported for TapHolds
                OutEffects::empty(STOP)
            }
        }
    }

    fn handle_th_waiting(&mut self,
                         event: &InputEvent,
                         state: &mut TapHoldState,
                         tap_fx: &Effect,
                         _hold_fx: &Effect) -> OutEffects {
        let value = KeyValue::from(event.value);

        match value {
            KeyValue::Press => {
                // Should never happen.
                // Should only see this in the idle state
                assert!(false);
                OutEffects::empty(STOP)
            },

            KeyValue::Release => {
                // Forward the release.
                // We didn't reach the hold state
                *state = TapHoldState::ThIdle;
                self.waiting_keys.clear();

                OutEffects::new_multiple(STOP, vec![
                    EffectValue::new(tap_fx.clone(), KeyValue::Press),
                    EffectValue::new(tap_fx.clone(), KeyValue::Release)
                ])
            },

            KeyValue::Repeat => {
                // Drop repeats. These aren't supported for TapHolds
                OutEffects::empty(STOP)
            }
        }
    }

    fn handle_th_idle(&mut self,
                      event: &InputEvent,
                      state: &mut TapHoldState,
                      tap_fx: &Effect,
                      _hold_fx: &Effect) -> OutEffects {
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
                OutEffects::empty(STOP)
            },

            KeyValue::Release => {
                // Forward the release
                OutEffects::new(STOP, tap_fx.clone(), KeyValue::Release)
            },

            KeyValue::Repeat => {
                // Drop repeats. These aren't supported for TapHolds
                OutEffects::empty(STOP)
            }
        }
    }

    // Assumes this is an event tied to a TapHold assigned MergedKey
    fn process_tap_hold_key(&mut self,
                            event: &InputEvent,
                            state: &mut KeyState,
                            tap_fx: &Effect,
                            hold_fx: &Effect) -> OutEffects {
        if let KeyState::KsTapHold(th_state) = state {
            match &th_state {
                TapHoldState::ThIdle => self.handle_th_idle(event, th_state, tap_fx, hold_fx),
                TapHoldState::ThWaiting(_) => self.handle_th_waiting(event, th_state, tap_fx, hold_fx),
                TapHoldState::ThHolding => self.handle_th_holding(event, th_state, tap_fx, hold_fx),
            }
        } else {
            assert!(false);
            OutEffects::empty(STOP)
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

    fn get_th_effects_from_action(action: &Action) -> Option<(Effect, Effect)> {
        match action {
            Action::TapHold(tap_fx, hold_fx) => Some((tap_fx.clone(),
                                                      hold_fx.clone())),
            _ => None
        }
    }

    fn process_non_tap_hold_key(&mut self,
                                l_mgr: &mut LayersManager,
                                event: &InputEvent) -> OutEffects {
        let mut out = OutEffects::empty(CONTINUE);

        for waiting in self.waiting_keys.drain(..) {
            let merged_key: &mut MergedKey = l_mgr.get_mut(waiting.clone());
            let (tap_fx, hold_fx) = Self::get_th_effects_from_action(&merged_key.action).unwrap();

            if Self::is_waiting_over(merged_key, event) {
                // Append the press hold_fx to the output
                out.insert(hold_fx.clone(), KeyValue::Press);

                // Change to the holding state
                merged_key.state = KeyState::KsTapHold(TapHoldState::ThHolding);
                self.holding_keys.insert(waiting);

            } else {
                // Flush the press and release tap_fx
                out.insert(tap_fx, KeyValue::Press);

                // Revert to the idle state
                merged_key.state = KeyState::KsTapHold(TapHoldState::ThIdle);
            }
        }

        out
    }

    // --------------- High-Level Functions ----------------------

    // Returns true if processed, false if skipped
    pub fn process(&mut self, l_mgr: &mut LayersManager, event: &InputEvent) -> OutEffects {
        let code = match get_keycode_from_event(event) {
            Some(code) => code,
            None => { return OutEffects::empty(CONTINUE) },
        };

        let merged_key: &mut MergedKey = l_mgr.get_mut(code);
        if let Action::TapHold(tap_fx, hold_fx) = merged_key.action.clone() {
            self.process_tap_hold_key(event, &mut merged_key.state, &tap_fx, &hold_fx)
        } else {
            self.process_non_tap_hold_key(l_mgr, event)
        }
    }

    // Used by Ktrl to make sure toggling layers is okay
    pub fn is_idle(&self) -> bool {
        self.waiting_keys.len() == 0 &&
            self.holding_keys.len() == 0
    }
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
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_non_th_press), OutEffects::empty(CONTINUE));
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_non_th_release), OutEffects::empty(CONTINUE));
    assert_eq!(th_mgr.is_idle(), true);
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
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_th_press), OutEffects::empty(STOP));
    assert_eq!(th_mgr.is_idle(), false);
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_th_release), OutEffects::new_multiple(STOP, vec![
        EffectValue::new(Effect::Key(KEY_A.into()), KeyValue::Press),
        EffectValue::new(Effect::Key(KEY_A.into()), KeyValue::Release),
    ]));
    assert_eq!(th_mgr.is_idle(), true);

    // 2nd
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_th_press), OutEffects::empty(STOP));
    assert_eq!(th_mgr.is_idle(), false);
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_th_release), OutEffects::new_multiple(STOP, vec![
        EffectValue::new(Effect::Key(KEY_A.into()), KeyValue::Press),
        EffectValue::new(Effect::Key(KEY_A.into()), KeyValue::Release),
    ]));
    assert_eq!(th_mgr.is_idle(), true);

    // interruptions: 1
    ev_th_release.time.tv_usec = TAP_HOLD_WAIT_PERIOD + 1;
    let ev_non_th_press = KeyEvent::new_press(&EventCode::EV_KEY(KEY_W)).event;
    let ev_non_th_release = KeyEvent::new_release(&EventCode::EV_KEY(KEY_W)).event;
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_th_press), OutEffects::empty(STOP));
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_non_th_press), OutEffects::new(CONTINUE, Effect::Key(KEY_A.into()), KeyValue::Press));
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_th_release), OutEffects::new(STOP, Effect::Key(KEY_A.into()), KeyValue::Release));
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_non_th_release), OutEffects::empty(CONTINUE));
    assert_eq!(th_mgr.is_idle(), true);

    // interruptions: 2
    ev_th_release.time.tv_usec = TAP_HOLD_WAIT_PERIOD + 1;
    let ev_non_th_press = KeyEvent::new_press(&EventCode::EV_KEY(KEY_W)).event;
    let ev_non_th_release = KeyEvent::new_release(&EventCode::EV_KEY(KEY_W)).event;
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_th_press), OutEffects::empty(STOP));
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_non_th_press), OutEffects::new(CONTINUE, Effect::Key(KEY_A.into()), KeyValue::Press));
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_non_th_release), OutEffects::empty(CONTINUE));
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_th_release), OutEffects::new(STOP, Effect::Key(KEY_A.into()), KeyValue::Release));
    assert_eq!(th_mgr.is_idle(), true);
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
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_th_press), OutEffects::empty(STOP));
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_th_release), OutEffects::new_multiple(STOP, vec![
        EffectValue::new(Effect::Key(KEY_A.into()), KeyValue::Press),
        EffectValue::new(Effect::Key(KEY_A.into()), KeyValue::Release),
    ]));
    assert_eq!(th_mgr.is_idle(), true);


    // -------------------------------

    // Hold with other key
    let mut ev_non_th_press = KeyEvent::new_press(&EventCode::EV_KEY(KEY_W)).event;
    let mut ev_non_th_release = KeyEvent::new_release(&EventCode::EV_KEY(KEY_W)).event;
    ev_non_th_press.time.tv_usec = TAP_HOLD_WAIT_PERIOD + 1;
    ev_non_th_release.time.tv_usec = TAP_HOLD_WAIT_PERIOD + 2;
    ev_th_release.time.tv_usec = TAP_HOLD_WAIT_PERIOD + 3;

    assert_eq!(th_mgr.process(&mut l_mgr, &ev_th_press), OutEffects::empty(STOP));
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_non_th_press), OutEffects::new(CONTINUE, Effect::Key(KEY_LEFTCTRL.into()), KeyValue::Press));
    assert_eq!(th_mgr.is_idle(), false);
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_non_th_release), OutEffects::empty(CONTINUE));
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_th_release), OutEffects::new(STOP, Effect::Key(KEY_LEFTCTRL.into()), KeyValue::Release));

    // -------------------------------

    // Hold with other key (different order)
    ev_non_th_press.time.tv_usec = TAP_HOLD_WAIT_PERIOD + 1;
    ev_th_release.time.tv_usec = TAP_HOLD_WAIT_PERIOD + 2;
    ev_non_th_release.time.tv_usec = TAP_HOLD_WAIT_PERIOD + 3;

    assert_eq!(th_mgr.process(&mut l_mgr, &ev_th_press), OutEffects::empty(STOP));
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_non_th_press), OutEffects::new(CONTINUE, Effect::Key(KEY_LEFTCTRL.into()), KeyValue::Press));
    assert_eq!(th_mgr.is_idle(), false);
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_th_release), OutEffects::new(STOP, Effect::Key(KEY_LEFTCTRL.into()), KeyValue::Release));
    assert_eq!(th_mgr.process(&mut l_mgr, &ev_non_th_release), OutEffects::empty(CONTINUE));
}
