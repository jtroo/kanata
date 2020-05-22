use evdev_rs::enums::EventCode;
use evdev_rs::InputEvent;
use std::collections::HashSet;
use std::vec::Vec;
use crate::layers::LayersManager;
use inner::*;

//
// TODO:
// 1. Refactor this file. Tons of boilerplate
// 2. Refactor the inner!(inner!(...)) is there a better way?
//    E.g nested match? https://aminb.gitbooks.io/rust-for-c/content/destructuring/index.html
// 3. Refactor taking in both `&mut self` and `&mut Ktrl`
//

use crate::layers::{
    Effect,
    Action,
    TapHoldWaiting,
    TapHoldState,
    KeyState,
    MergedKey,
};

use crate::Ktrl;
use crate::keycode::KeyCode;

const STOP: bool = true;
const CONTINUE: bool = false;
const TAP_HOLD_WAIT_PERIOD: i64 = 200000;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum KeyValue {
    Release = 0,
    Press = 1,
    Repeat = 2,
}

impl From<i32> for KeyValue {
    fn from(item: i32) -> Self {
        match item {
            0 => Self::Release,
            1 => Self::Press,
            2 => Self::Repeat,
            _ => {
                assert!(false);
                Self::Release
            }
        }
    }
}

#[derive(Clone, Debug)]
struct TapHoldEffect {
    fx: Effect,
    val: KeyValue,
}

#[derive(Clone, Debug)]
struct TapHoldOut {
    stop_processing: bool,
    effects: Option<Vec<TapHoldEffect>>,
}

impl TapHoldOut {
    fn new(stop_processing: bool, effect: Effect, value: KeyValue) -> Self {
        TapHoldOut {
            stop_processing,
            effects: Some(vec![TapHoldEffect{
                fx: effect,
                val: value
            }])
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
            effects.push(TapHoldEffect{fx: effect, val: value});
        } else {
            self.effects = Some(vec![TapHoldEffect{
                fx: effect,
                val: value
            }]);
        }
    }
}

pub struct TapHoldMgr {
    waiting_keys: HashSet<KeyCode>,
}

impl TapHoldMgr {
    pub fn new() -> Self {
        Self{waiting_keys: HashSet::new()}
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
    fn handle_th_holding(&self,
                         event: &InputEvent,
                         state: &mut TapHoldState,
                         _tap_fx: &Effect,
                         hold_fx: &Effect) -> TapHoldOut {
        assert!(*state == TapHoldState::ThIdle);
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
                // TODO: release all the other waiting taphold
                *state = TapHoldState::ThIdle;
                TapHoldOut::new(STOP, *hold_fx, KeyValue::Release) // forward the release
            },

            KeyValue::Repeat => {
                // Drop repeats. These aren't supported for TapHolds
                TapHoldOut::empty(STOP)
            }
        }
    }

    fn handle_th_waiting(&self,
                         event: &InputEvent,
                         state: &mut TapHoldState,
                         tap_fx: &Effect,
                         _hold_fx: &Effect) -> TapHoldOut {
        assert!(*state == TapHoldState::ThIdle);
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
                // TODO: release all the other waiting taphold
                *state = TapHoldState::ThIdle;
                TapHoldOut::new(STOP, *tap_fx, KeyValue::Release)
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
                      _tap_fx: &Effect,
                      _hold_fx: &Effect) -> TapHoldOut {
        assert!(*state == TapHoldState::ThIdle);
        let keycode: KeyCode = event.event_code.clone().into();
        let value = KeyValue::from(event.value);

        match value {
            KeyValue::Press => {
                // Transition to the waiting state.
                // I.E waiting for either an interruptions => Press+Release the Tap effect
                // or for the TapHold wait period => Send a Hold effect press
                self.waiting_keys.insert(keycode.clone());
                *state = TapHoldState::ThWaiting(
                    TapHoldWaiting{timestamp: event.time.clone()}
                );
                TapHoldOut::empty(STOP)
            },

            KeyValue::Release => {
                // This should never happen.
                // Should only get this event in the waiting state
                assert!(false);
                TapHoldOut::empty(STOP)
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
                ThIdle => self.handle_th_idle(event, th_state, tap_fx, hold_fx),
                ThWaiting => self.handle_th_waiting(event, th_state, tap_fx, hold_fx),
                ThHolding => self.handle_th_holding(event, th_state, tap_fx, hold_fx),
            }
        } else {
            assert!(false);
            TapHoldOut::empty(STOP)
        }
    }

    // --------------- Non-TapHold Functions ----------------------

    fn is_waiting_over(&self, merged_key: &MergedKey, waiting: KeyCode, event: &InputEvent) -> bool {
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

        for waiting in &self.waiting_keys {
            let merged_key: &mut MergedKey = l_mgr.get_mut(waiting.clone());

            if self.is_waiting_over(merged_key, *waiting, event) {
                // Append the press hold_fx to the output
                let hold_fx = match merged_key.action {
                    Action::TapHold(_tap_fx, hold_fx) => hold_fx,
                    _ => {assert!(false); Effect::Default(0.into())},
                };
                out.insert(hold_fx, KeyValue::Press);

                // Change to the holding state
                merged_key.state = KeyState::KsTapHold(TapHoldState::ThHolding);

            } else {
                // Flush the press and release tap_fx
                let tap_fx = match merged_key.action {
                    Action::TapHold(tap_fx, _hold_fx) => tap_fx,
                    _ => {assert!(false); Effect::Default(0.into())},
                };
                out.insert(tap_fx, KeyValue::Press);
                out.insert(tap_fx, KeyValue::Release);

                // Revert to the idle state
                merged_key.state = KeyState::KsTapHold(TapHoldState::ThIdle);
            }
        }

        out
    }

    // --------------- High-Level Functions ----------------------

    // Returns true if processed, false if skipped
    pub fn process_tap_hold(&mut self, l_mgr: &mut LayersManager, event: &InputEvent) -> TapHoldOut {
        let code = get_keycode_from_event(event)
            .expect(&format!("Invalid code in event {}", event.event_code));
        let merged_key: &mut MergedKey = l_mgr.get_mut(code);
        if let Action::TapHold(tap_fx, hold_fx) = merged_key.action.clone() {
            self.process_tap_hold_key(event, &mut merged_key.state, &tap_fx, &hold_fx)
        } else {
            self.process_non_tap_hold_key(l_mgr, event)
        }
    }
}
