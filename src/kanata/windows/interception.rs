use anyhow::{bail, Result};
use crossbeam_channel::Sender;
use interception as ic;
use parking_lot::Mutex;
use std::sync::Arc;

use crate::kanata::*;
use crate::keys::{KeyValue, OsCode};

static PRESSED_KEYS: Lazy<Mutex<HashSet<OsCode>>> = Lazy::new(|| Mutex::new(HashSet::new()));

impl Kanata {
    pub fn event_loop(kanata: Arc<Mutex<Self>>, tx: Sender<KeyEvent>) -> Result<()> {
        let rx = kanata.lock().kbd_out_rx.clone();
        *MAPPED_KEYS.lock() = kanata.lock().mapped_keys.clone();
        let intrcptn = ic::Interception::new().expect("interception driver should init: have you completed the interception driver installation?");
        intrcptn.set_filter(ic::is_keyboard, ic::Filter::KeyFilter(ic::KeyFilter::all()));
        let mut strokes = [ic::Stroke::Keyboard {
            code: ic::ScanCode::Esc,
            state: ic::KeyState::empty(),
            information: 0,
        }; 32];

        loop {
            let dev = intrcptn.wait_with_timeout(std::time::Duration::from_millis(1));
            if dev > 0 {
                let num_strokes = intrcptn.receive(dev, &mut strokes);
                let num_strokes = num_strokes as usize;

                for i in 0..num_strokes {
                    log::debug!("got stroke {:?}", strokes[i]);
                    let mut key_event = match strokes[i] {
                        ic::Stroke::Keyboard { state, .. } => {
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
                        _ => {
                            intrcptn.send(dev, &strokes[i..i + 1]);
                            continue;
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
                    tx.send(key_event).unwrap();
                }
            }

            match rx.try_recv() {
                Ok(event) => {
                    log::debug!("kanata sending {:?} to driver", event.0);
                    strokes[0] = event.0;
                    match event.0 {
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
                Err(TryRecvError::Disconnected) => {
                    const ERR: &str = "interception event rx channel disconnected";
                    log::error!("{ERR}");
                    bail!(ERR);
                }
                Err(TryRecvError::Empty) => {}
            }
        }
    }
}
