use anyhow::{bail, Result};
use interception as ic;
use parking_lot::Mutex;
use std::sync::mpsc::Sender;
use std::sync::Arc;

use super::PRESSED_KEYS;
use crate::kanata::*;
use crate::keys::{KeyValue, OsCode};

const HWID_ARR_SZ: usize = 128;

impl Kanata {
    pub fn event_loop(kanata: Arc<Mutex<Self>>, tx: Sender<KeyEvent>) -> Result<()> {
        let rx = kanata.lock().kbd_out_rx.take().unwrap();
        let intrcptn = ic::Interception::new().ok_or_else(|| anyhow!("interception driver should init: have you completed the interception driver installation?"))?;
        intrcptn.set_filter(ic::is_keyboard, ic::Filter::KeyFilter(ic::KeyFilter::all()));
        let mut strokes = [ic::Stroke::Keyboard {
            code: ic::ScanCode::Esc,
            state: ic::KeyState::empty(),
            information: 0,
        }; 32];

        let mouse_to_intercept_hwid: Option<[u8; HWID_ARR_SZ]> = kanata
            .lock()
            .intercept_mouse_hwid.as_ref()
            .map(|hwid| {
                hwid.iter().copied().enumerate()
                    .fold([0u8; HWID_ARR_SZ], |mut hwid, idx_byte| {
                        let (i, b) = idx_byte;
                        if i > HWID_ARR_SZ {
                            panic!("windows-interception-mouse-hwid is too long; it should be up to {HWID_ARR_SZ} 8-bit unsigned integers");
                        }
                        hwid[i] = b;
                        hwid
                    })
            });
        if mouse_to_intercept_hwid.is_some() {
            intrcptn.set_filter(
                ic::is_mouse,
                ic::Filter::MouseFilter(
                    ic::MouseState::all()
                        & (!ic::MouseState::WHEEL)
                        & (!ic::MouseState::HWHEEL)
                        & (!ic::MouseState::MOVE),
                ),
            );
        }
        let mut is_dev_interceptable: HashMap<ic::Device, bool> = HashMap::default();

        let mut can_block = false;
        let mut empty_channel_iter_count = 0;
        loop {
            let dev = match (can_block, empty_channel_iter_count) {
                (true, 100..) => {
                    can_block = false;
                    empty_channel_iter_count = 0;
                    intrcptn.wait()
                }
                _ => intrcptn.wait_with_timeout(std::time::Duration::from_millis(1)),
            };
            if dev > 0 {
                let num_strokes = intrcptn.receive(dev, &mut strokes) as usize;
                for i in 0..num_strokes {
                    let mut key_event = match strokes[i] {
                        ic::Stroke::Keyboard { state, .. } => {
                            log::debug!("got stroke {:?}", strokes[i]);
                            let code = match OsCode::try_from(strokes[i]) {
                                Ok(c) => c,
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
                        ic::Stroke::Mouse { state, .. } => {
                            if let Some(hwid) = mouse_to_intercept_hwid {
                                log::trace!("checking mouse stroke {:?}", strokes[i]);
                                if let Some(event) = mouse_state_to_event(
                                    dev,
                                    &hwid,
                                    state,
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
                    tx.send(key_event)?;
                }
            }

            match rx.try_recv() {
                Ok(event) => {
                    empty_channel_iter_count = 0;
                    if event.0 {
                        can_block = true;
                    } else if !event.0 {
                        strokes[0] = event.1 .0;
                        log::debug!("kanata sending {:?} to driver", strokes[0]);
                        match strokes[0] {
                            // Note regarding device numbers:
                            // Keyboard devices are 1-10 and mouse devices are 11-20. Source:
                            // https://github.com/oblitum/Interception/blob/39eecbbc46a52e0402f783b872ef62b0254a896a/library/interception.h#L34
                            ic::Stroke::Keyboard { .. } => {
                                intrcptn.send(1, &strokes[0..1]);
                            }
                            ic::Stroke::Mouse { .. } => {
                                intrcptn.send(11, &strokes[0..1]);
                            }
                        }
                    }
                }
                Err(TryRecvError::Disconnected) => {
                    const ERR: &str = "interception event rx channel disconnected";
                    log::error!("{ERR}");
                    bail!(ERR);
                }
                Err(TryRecvError::Empty) => {
                    empty_channel_iter_count += 1;
                }
            }
        }
    }
}

fn mouse_state_to_event(
    input_dev: ic::Device,
    allowed_hwid: &[u8; HWID_ARR_SZ],
    state: ic::MouseState,
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
    } else {
        None
    }
}
