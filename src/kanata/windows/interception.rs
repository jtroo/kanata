use anyhow::{Result, anyhow};
use kanata_interception as ic;
use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::mpsc::SyncSender as Sender;

use super::PRESSED_KEYS;
use crate::kanata::*;
use crate::oskbd::KeyValue;
use kanata_parser::keys::OsCode;

impl Kanata {
    pub fn event_loop_inner(kanata: Arc<Mutex<Self>>, tx: Sender<KeyEvent>) -> Result<()> {
        let intrcptn = ic::Interception::new().ok_or_else(|| anyhow!("interception driver should init: have you completed the interception driver installation?"))?;
        intrcptn.set_filter(ic::is_keyboard, ic::Filter::KeyFilter(ic::KeyFilter::all()));
        let mut strokes = [ic::Stroke::Keyboard {
            code: ic::ScanCode::Esc,
            state: ic::KeyState::empty(),
            information: 0,
        }; 32];

        let keyboards_to_intercept_hwids = kanata.lock().intercept_kb_hwids.clone();
        let keyboards_to_intercept_hwids_exclude = kanata.lock().intercept_kb_hwids_exclude.clone();
        let mouse_to_intercept_hwids: Option<Vec<[u8; HWID_ARR_SZ]>> =
            kanata.lock().intercept_mouse_hwids.clone();
        let mouse_to_intercept_excluded_hwids: Option<Vec<[u8; HWID_ARR_SZ]>> =
            kanata.lock().intercept_mouse_hwids_exclude.clone();
        let mouse_movement_key = kanata.lock().mouse_movement_key.clone();
        if mouse_to_intercept_hwids.is_some() || mouse_to_intercept_excluded_hwids.is_some() {
            if mouse_movement_key.lock().is_some() {
                intrcptn.set_filter(ic::is_mouse, ic::Filter::MouseFilter(ic::MouseState::all()));
            } else {
                intrcptn.set_filter(
                    ic::is_mouse,
                    ic::Filter::MouseFilter(ic::MouseState::all() & (!ic::MouseState::MOVE)),
                );
            }
        }
        let mut is_dev_interceptable: HashMap<ic::Device, bool> = HashMap::default();
        loop {
            let dev = intrcptn.wait();
            if dev > 0 {
                let num_strokes = intrcptn.receive(dev, &mut strokes) as usize;
                for i in 0..num_strokes {
                    let mut key_event = match strokes[i] {
                        ic::Stroke::Keyboard { state, .. } => {
                            if !is_device_interceptable(
                                dev,
                                &intrcptn,
                                &keyboards_to_intercept_hwids,
                                &keyboards_to_intercept_hwids_exclude,
                                &mut is_dev_interceptable,
                            ) {
                                log::debug!("stroke {:?} is from undesired device", strokes[i]);
                                intrcptn.send(dev, &strokes[i..i + 1]);
                                continue;
                            }
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
                        ic::Stroke::Mouse {
                            state,
                            rolling,
                            flags,
                            ..
                        } => {
                            let allow_this_dev = is_device_interceptable(
                                dev,
                                &intrcptn,
                                &mouse_to_intercept_hwids,
                                &mouse_to_intercept_excluded_hwids,
                                &mut is_dev_interceptable,
                            );

                            if allow_this_dev {
                                log::trace!("checking mouse stroke {:?}", strokes[i]);

                                if let Some(ms_mvmt_key) = *mouse_movement_key.lock() {
                                    if flags.contains(ic::MouseFlags::MOVE_RELATIVE) {
                                        tx.try_send(KeyEvent::new(ms_mvmt_key, KeyValue::Tap))?;
                                    }
                                };
                            }

                            if let (true, Some(event)) =
                                (allow_this_dev, mouse_state_to_event(state, rolling))
                            {
                                event
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
    pub fn event_loop(
        kanata: Arc<Mutex<Self>>,
        tx: Sender<KeyEvent>,
        #[cfg(feature = "gui")] ui: crate::gui::system_tray_ui::SystemTrayUi,
    ) -> Result<()> {
        #[cfg(not(feature = "gui"))]
        {
            Self::event_loop_inner(kanata, tx)
        }
        #[cfg(feature = "gui")]
        {
            std::thread::spawn(move || -> Result<()> { Self::event_loop_inner(kanata, tx) });
            let _ui = ui; // prevents thread from panicking on exiting via a GUI
            native_windows_gui::dispatch_thread_events();
            Ok(())
        }
    }
}

fn is_device_interceptable(
    input_dev: ic::Device,
    intrcptn: &ic::Interception,
    allowed_hwids: &Option<Vec<[u8; HWID_ARR_SZ]>>,
    excluded_hwids: &Option<Vec<[u8; HWID_ARR_SZ]>>,
    cache: &mut HashMap<ic::Device, bool>,
) -> bool {
    match (allowed_hwids, excluded_hwids) {
        (None, None) => true,
        (Some(allowed), None) => match cache.get(&input_dev) {
            Some(v) => *v,
            None => {
                let mut hwid = [0u8; HWID_ARR_SZ];
                log::trace!("getting hardware id for input dev: {input_dev}");
                let res = intrcptn.get_hardware_id(input_dev, &mut hwid);
                let dev_is_interceptable = allowed.contains(&hwid);
                log::info!(
                    "include check - res {res}; device #{input_dev} is intercepted: {dev_is_interceptable}; hwid {hwid:?} "
                );
                cache.insert(input_dev, dev_is_interceptable);
                dev_is_interceptable
            }
        },
        (None, Some(excluded)) => match cache.get(&input_dev) {
            Some(v) => *v,
            None => {
                let mut hwid = [0u8; HWID_ARR_SZ];
                log::trace!("getting hardware id for input dev: {input_dev}");
                let res = intrcptn.get_hardware_id(input_dev, &mut hwid);
                let dev_is_interceptable = !excluded.contains(&hwid);
                log::info!(
                    "exclude check - res {res}; device #{input_dev} is intercepted: {dev_is_interceptable}; hwid {hwid:?} "
                );
                cache.insert(input_dev, dev_is_interceptable);
                dev_is_interceptable
            }
        },
        _ => unreachable!("excluded and allowed should be mutually exclusive"),
    }
}
fn mouse_state_to_event(state: ic::MouseState, rolling: i16) -> Option<KeyEvent> {
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
