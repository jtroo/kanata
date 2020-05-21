use evdev_rs::enums::EV_KEY;
use evdev_rs::enums::EventCode;
use evdev_rs::InputEvent;

use crate::layers::Effect;
use crate::layers::Action;
use crate::layers::TapHoldWaiting;
use crate::layers::TapHoldState;
use crate::layers::KeyState;
use crate::layers::MergedKey;

use crate::Ktrl;
use crate::keycode::KeyCode;

const RELEASE: i32 = 0;
const PRESS: i32 = 1;
const REPEAT: i32 = 2;

//
// TODO:
// 1. Hold a set of WAITING TapHoldState
//    that'll be resetted upon interruptions
//
// 2. Don't use ktrl.kbd_out directly!
//    Make these functions as pure as possible for unit-testing!!!
//

fn get_keycode_from_event(event: &InputEvent) -> Option<KeyCode> {
    if let EventCode::EV_KEY(ev_key) = &event.event_code {
        let code: KeyCode = KeyCode::from(ev_key.clone());
        Some(code)
    } else {
        None
    }
}

fn handle_th_idle(ktrl: &Ktrl,
                  event: &InputEvent,
                  state: &mut TapHoldState,
                  tap_fx: &Effect,
                  hold_fx: &Effect) {
    let value = event.value;

    if value == RELEASE {
        tap_fx.release(ktrl);
    } else if value == PRESS {
        *state = TapHoldState::TH_WAITING(TapHoldWaiting{timestamp: event.time})
    } else if value == REPEAT {
        // drop repeats
    } else {
        assert!(false);
    }
}

// Assumes this is an event tied to a TapHold assigned MergedKey
fn process_tap_hold_impl(ktrl: &Ktrl,
                         event: &InputEvent,
                         state: &mut KeyState,
                         tap_fx: &Effect,
                         hold_fx: &Effect) {
    if let KeyState::KsTapHold(th_state) = state {
        match th_state {
            TH_IDLE => handle_th_idle(ktrl, event, state, tap_fx, hold_fx),
            // TH_WAITING => handle_th_waiting(event, state, tap_fx, hold_fx),
            // TH_HOLDING => handle_th_holding(event, state, tap_fx, hold_fx),
        }
    } else {
        assert!(false);
    }
}

// Returns true if processed, false if skipped
pub fn process_tap_hold(ktrl: &mut Ktrl, event: &InputEvent) -> bool {
    let code = get_keycode_from_event(event)
        .expect(&format!("Invalid code in event {}", event.event_code));
    let merged_key: &mut MergedKey = ktrl.lmgr.get_mut(code);
    if let Action::TapHold(tap_fx, hold_fx) = merged_key.action.clone() {
        process_tap_hold_impl(ktrl, event, &mut merged_key.state, &tap_fx, &hold_fx);
        true
    } else {
        false
    }
}

