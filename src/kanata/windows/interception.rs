use anyhow::{anyhow, Result};
use kanata_interception as ic;
use parking_lot::Mutex;
use std::sync::mpsc::SyncSender as Sender;
use std::sync::Arc;

use super::PRESSED_KEYS;
use crate::kanata::*;
use crate::oskbd::KeyValue;
use kanata_parser::keys::OsCode;

impl Kanata {
    pub fn event_loop(kanata: Arc<Mutex<Self>>, tx: Sender<KeyEvent>) -> Result<()> {
        let intrcptn = ic::Interception::new().ok_or_else(|| anyhow!("interception driver should init: have you completed the interception driver installation?"))?;
        intrcptn.set_filter(ic::is_keyboard, ic::Filter::KeyFilter(ic::KeyFilter::all()));
        let mut strokes = [ic::Stroke::Keyboard {
            code: ic::ScanCode::Esc,
            state: ic::KeyState::empty(),
            information: 0,
        }; 32];

        let mouse_to_intercept_hwid: Option<[u8; HWID_ARR_SZ]> = kanata.lock().intercept_mouse_hwid;
        if mouse_to_intercept_hwid.is_some() {
            intrcptn.set_filter(
                ic::is_mouse,
                ic::Filter::MouseFilter(ic::MouseState::all() & (!ic::MouseState::MOVE)),
            );
        }
        let mut is_dev_interceptable: HashMap<ic::Device, bool> = HashMap::default();

        loop {
            let dev = intrcptn.wait();
            if dev > 0 {
                let num_strokes = intrcptn.receive(dev, &mut strokes) as usize;
                for i in 0..num_strokes {
                    let mut key_event = match strokes[i] {
                        ic::Stroke::Keyboard { state, .. } => {
                            log::debug!("got stroke {:?}", strokes[i]);
                            let code = match OsCodeWrapper::try_from(strokes[i]) {
                                Ok(c) => c.0,
                                _ => {
                                    log::debug!("could not map code to oscode");
                                    intrcptn.send(dev, &strokes[i..i + 1]);
                                    continue;
                                }
                            };
                            let value = match state.contains(ic::KeyState::UP) {
                                false => KeyValue::Press,
                                true => KeyValue::Release,
                            };
                            KeyEvent { code, value }
                        }
                        ic::Stroke::Mouse { state, rolling, .. } => {
                            if let Some(hwid) = mouse_to_intercept_hwid {
                                log::trace!("checking mouse stroke {:?}", strokes[i]);
                                if let Some(event) = mouse_state_to_event(
                                    dev,
                                    &hwid,
                                    state,
                                    rolling,
                                    &intrcptn,
                                    &mut is_dev_interceptable,
                                ) {
                                    event
                                } else {
                                    intrcptn.send(dev, &strokes[i..i + 1]);
                                    continue;
                                }
                            } else {
                                intrcptn.send(dev, &strokes[i..i + 1]);
                                continue;
                            }
                        }
                    };
                    check_for_exit(&key_event);
                    if !MAPPED_KEYS.lock().contains(&key_event.code) {
                        log::debug!("{key_event:?} is not mapped");
                        intrcptn.send(dev, &strokes[i..i + 1]);
                        continue;
                    }
                    log::debug!("sending {key_event:?} to processing loop");
                    match key_event.value {
                        KeyValue::Release => {
                            PRESSED_KEYS.lock().remove(&key_event.code);
                        }
                        KeyValue::Press => {
                            let mut pressed_keys = PRESSED_KEYS.lock();
                            if pressed_keys.contains(&key_event.code) {
                                key_event.value = KeyValue::Repeat;
                            } else {
                                pressed_keys.insert(key_event.code);
                            }
                        }
                        _ => {}
                    }
                    tx.try_send(key_event)?;
                }
            }
        }
    }
}

fn mouse_state_to_event(
    input_dev: ic::Device,
    allowed_hwid: &[u8; HWID_ARR_SZ],
    state: ic::MouseState,
    rolling: i16,
    intrcptn: &ic::Interception,
    is_dev_interceptable: &mut HashMap<ic::Device, bool>,
) -> Option<KeyEvent> {
    if !match is_dev_interceptable.get(&input_dev) {
        Some(v) => *v,
        None => {
            let mut hwid = [0u8; HWID_ARR_SZ];
            log::trace!("getting hardware id for input dev: {input_dev}");
            let res = intrcptn.get_hardware_id(input_dev, &mut hwid);
            let dev_is_interceptable = hwid == *allowed_hwid;
            log::info!("res {res}; device #{input_dev} hwid {hwid:?} matches allowed mouse input: {dev_is_interceptable}");
            is_dev_interceptable.insert(input_dev, dev_is_interceptable);
            dev_is_interceptable
        }
    } {
        return None;
    }

    if state.contains(ic::MouseState::RIGHT_BUTTON_DOWN) {
        Some(KeyEvent {
            code: OsCode::BTN_RIGHT,
            value: KeyValue::Press,
        })
    } else if state.contains(ic::MouseState::RIGHT_BUTTON_UP) {
        Some(KeyEvent {
            code: OsCode::BTN_RIGHT,
            value: KeyValue::Release,
        })
    } else if state.contains(ic::MouseState::LEFT_BUTTON_DOWN) {
        Some(KeyEvent {
            code: OsCode::BTN_LEFT,
            value: KeyValue::Press,
        })
    } else if state.contains(ic::MouseState::LEFT_BUTTON_UP) {
        Some(KeyEvent {
            code: OsCode::BTN_LEFT,
            value: KeyValue::Release,
        })
    } else if state.contains(ic::MouseState::MIDDLE_BUTTON_DOWN) {
        Some(KeyEvent {
            code: OsCode::BTN_MIDDLE,
            value: KeyValue::Press,
        })
    } else if state.contains(ic::MouseState::MIDDLE_BUTTON_UP) {
        Some(KeyEvent {
            code: OsCode::BTN_MIDDLE,
            value: KeyValue::Release,
        })
    } else if state.contains(ic::MouseState::BUTTON_4_DOWN) {
        Some(KeyEvent {
            code: OsCode::BTN_SIDE,
            value: KeyValue::Press,
        })
    } else if state.contains(ic::MouseState::BUTTON_4_UP) {
        Some(KeyEvent {
            code: OsCode::BTN_SIDE,
            value: KeyValue::Release,
        })
    } else if state.contains(ic::MouseState::BUTTON_5_DOWN) {
        Some(KeyEvent {
            code: OsCode::BTN_EXTRA,
            value: KeyValue::Press,
        })
    } else if state.contains(ic::MouseState::BUTTON_5_UP) {
        Some(KeyEvent {
            code: OsCode::BTN_EXTRA,
            value: KeyValue::Release,
        })
    } else if state.contains(ic::MouseState::WHEEL) {
        let osc = if rolling >= 0 {
            OsCode::MouseWheelUp
        } else {
            OsCode::MouseWheelDown
        };
        if MAPPED_KEYS.lock().contains(&osc) {
            Some(KeyEvent {
                code: osc,
                value: KeyValue::Tap,
            })
        } else {
            None
        }
    } else if state.contains(ic::MouseState::HWHEEL) {
        let osc = if rolling >= 0 {
            OsCode::MouseWheelRight
        } else {
            OsCode::MouseWheelLeft
        };
        if MAPPED_KEYS.lock().contains(&osc) {
            Some(KeyEvent {
                code: osc,
                value: KeyValue::Tap,
            })
        } else {
            None
        }
    } else {
        None
    }
}
