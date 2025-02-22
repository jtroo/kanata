use parking_lot::Mutex;
use std::convert::TryFrom;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, SyncSender as Sender, TryRecvError, sync_channel};
use std::time;

use super::PRESSED_KEYS;
use crate::kanata::*;

impl Kanata {
    /// Initialize the callback that is passed to the Windows low level hook to receive key events and run the native_windows_gui event loop.
    pub fn event_loop(_cfg: Arc<Mutex<Self>>, tx: Sender<KeyEvent>) -> Result<()> {
        let (preprocess_tx, preprocess_rx) = sync_channel(100);
        start_event_preprocessor(preprocess_rx, tx);

        let _ = KeyboardHook::set_input_cb(move |input_event| {
            // →true if input event was handled, false otherwise, informs input_ev_listener whether to look for the output key event
            let mut key_event = match KeyEvent::try_from(input_event) {
                // InputEvent{code:u32      , up   :bool}
                Ok(ev) => ev, // KeyEvent  {code:OsCode   , value:KeyValue}
                _ => return false,
            }; // Some(OsCode::KEY_0)←0x30        Release0 Press1 Repeat2 Tap WakeUp
            check_for_exit(&key_event); //noop

            let oscode = OsCode::from(input_event.code);
            if !MAPPED_KEYS.lock().contains(&oscode) {
                return false;
            }
            log::debug!("event loop: {}", key_event);
            match key_event.value {
                // Unlike Linux, Windows does not use a separate value for repeat. However, our code needs to differentiate between initial press and repeat press.
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
            try_send_panic(&preprocess_tx, key_event); // Send input_events to the preprocessing loop. Panic if channel somehow gets full or if channel disconnects. Typing input should never trigger a panic based on the channel getting full, assuming regular operation of the program and some other bug isn't the problem. I've tried to crash the program by pressing as many keys on my keyboard at the same time as I could, but was unable to.
            #[cfg(feature = "perf_logging")]
            debug!(
                " 🕐{}μs sent msg to tx→rx@start_processing_loop from event loop@KeyboardHook::set_input_cb",
                (start.elapsed()).as_micros()
            );
            true
        });
        Ok(())
    }
}

fn try_send_panic(tx: &Sender<KeyEvent>, kev: KeyEvent) {
    if let Err(e) = tx.try_send(kev) {
        panic!("failed to send on channel: {e:?}")
    }
}

fn start_event_preprocessor(preprocess_rx: Receiver<KeyEvent>, process_tx: Sender<KeyEvent>) {
    #[derive(Debug, Clone, Copy, PartialEq)]
    enum LctlState {
        Pressed,
        Released,
        Pending,
        PendingReleased,
        None,
    }

    std::thread::spawn(move || {
        let mut lctl_state = LctlState::None;
        loop {
            match preprocess_rx.try_recv() {
                Ok(kev) => match (*ALTGR_BEHAVIOUR.lock(), kev) {
                    (AltGrBehaviour::DoNothing, _) => try_send_panic(&process_tx, kev),
                    (
                        AltGrBehaviour::AddLctlRelease,
                        KeyEvent {
                            value: KeyValue::Release,
                            code: OsCode::KEY_RIGHTALT,
                            ..
                        },
                    ) => {
                        log::debug!("altgr add: adding lctl release");
                        try_send_panic(&process_tx, kev);
                        try_send_panic(
                            &process_tx,
                            KeyEvent::new(OsCode::KEY_LEFTCTRL, KeyValue::Release),
                        );
                        PRESSED_KEYS.lock().remove(&OsCode::KEY_LEFTCTRL);
                    }
                    (
                        AltGrBehaviour::CancelLctlPress,
                        KeyEvent {
                            value: KeyValue::Press,
                            code: OsCode::KEY_LEFTCTRL,
                            ..
                        },
                    ) => {
                        log::debug!("altgr cancel: lctl state->pressed");
                        lctl_state = LctlState::Pressed;
                    }
                    (
                        AltGrBehaviour::CancelLctlPress,
                        KeyEvent {
                            value: KeyValue::Release,
                            code: OsCode::KEY_LEFTCTRL,
                            ..
                        },
                    ) => match lctl_state {
                        LctlState::Pressed => {
                            log::debug!("altgr cancel: lctl state->released");
                            lctl_state = LctlState::Released;
                        }
                        LctlState::Pending => {
                            log::debug!("altgr cancel: lctl state->pending-released");
                            lctl_state = LctlState::PendingReleased;
                        }
                        LctlState::None => try_send_panic(&process_tx, kev),
                        _ => {}
                    },
                    (
                        AltGrBehaviour::CancelLctlPress,
                        KeyEvent {
                            value: KeyValue::Press,
                            code: OsCode::KEY_RIGHTALT,
                            ..
                        },
                    ) => {
                        log::debug!("altgr cancel: lctl state->none");
                        lctl_state = LctlState::None;
                        try_send_panic(&process_tx, kev);
                    }
                    (_, _) => try_send_panic(&process_tx, kev),
                },
                Err(TryRecvError::Empty) => {
                    if *ALTGR_BEHAVIOUR.lock() == AltGrBehaviour::CancelLctlPress {
                        match lctl_state {
                            LctlState::Pressed => {
                                log::debug!("altgr cancel: lctl state->pending");
                                lctl_state = LctlState::Pending;
                            }
                            LctlState::Released => {
                                log::debug!("altgr cancel: lctl state->pending-released");
                                lctl_state = LctlState::PendingReleased;
                            }
                            LctlState::Pending => {
                                log::debug!("altgr cancel: lctl state->send");
                                try_send_panic(
                                    &process_tx,
                                    KeyEvent::new(OsCode::KEY_LEFTCTRL, KeyValue::Press),
                                );
                                lctl_state = LctlState::None;
                            }
                            LctlState::PendingReleased => {
                                log::debug!("altgr cancel: lctl state->send+release");
                                try_send_panic(
                                    &process_tx,
                                    KeyEvent::new(OsCode::KEY_LEFTCTRL, KeyValue::Press),
                                );
                                try_send_panic(
                                    &process_tx,
                                    KeyEvent::new(OsCode::KEY_LEFTCTRL, KeyValue::Release),
                                );
                                lctl_state = LctlState::None;
                            }
                            _ => {}
                        }
                    }
                    std::thread::sleep(time::Duration::from_millis(1));
                }
                Err(TryRecvError::Disconnected) => {
                    panic!("channel disconnected (exthook event_preproces)")
                }
            }
        }
    });
}
