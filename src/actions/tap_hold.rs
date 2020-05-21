use evdev_rs::enums::EventCode;
use evdev_rs::InputEvent;
use std::collections::HashSet;
use inner::*;

//
// TODO:
// 1. Refactor this file. Tons of boilerplate
// 2. Refactor the inner!(inner!(...)) is there a better way?
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
    effect: Option<TapHoldEffect>,
}

impl TapHoldOut {
    fn new(stop_processing: bool, effect: Effect, value: KeyValue) -> Self {
        TapHoldOut {
            stop_processing,
            effect: Some(TapHoldEffect{
                fx: effect,
                val: value
            })
        }
    }

    fn empty(stop_processing: bool) -> Self {
        TapHoldOut {
            stop_processing,
            effect: None,
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

    fn is_waiting_over(&self, ktrl: &Ktrl, waiting: KeyCode, event: &InputEvent) -> bool {
        let merged_key: &MergedKey = ktrl.l_mgr.get(waiting);
        let new_timestamp = event.time;
        let wait_start_timestamp = inner!(inner!(merged_key.state, if KeyState::KsTapHold), if TapHoldState::ThWaiting).timestamp;

        let secs_diff = new_timestamp.tv_sec - wait_start_timestamp.tv_sec;
        let usecs_diff  = new_timestamp.tv_usec - wait_start_timestamp.tv_usec;

        if secs_diff > 0 {
            true
        } else if usecs_diff > 200000 {
            true
        } else {
            false
        }
    }

    fn process_non_tap_hold_key(&mut self,
                                ktrl: &mut Ktrl,
                                event: &InputEvent) -> TapHoldOut {
        for waiting in &self.waiting_keys {
            if self.is_waiting_over(ktrl, waiting, event) {
                // TODO: return hold_fx and change to ThHolding
            } else {
                // TODO: flush waiting and reset to ThIdle
            }
        }

        assert!(false);
        TapHoldOut::empty(STOP)
    }

    // --------------- High-Level Functions ----------------------

    // Returns true if processed, false if skipped
    pub fn process_tap_hold(&mut self, ktrl: &mut Ktrl, event: &InputEvent) -> TapHoldOut {
        let code = get_keycode_from_event(event)
            .expect(&format!("Invalid code in event {}", event.event_code));
        let merged_key: &mut MergedKey = ktrl.l_mgr.get_mut(code);
        if let Action::TapHold(tap_fx, hold_fx) = merged_key.action.clone() {
            self.process_tap_hold_key(event, &mut merged_key.state, &tap_fx, &hold_fx)
        } else {
            self.process_non_tap_hold_key(ktrl, event)
        }
    }
}
