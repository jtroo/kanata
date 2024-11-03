use parking_lot::Mutex;
use std::convert::TryFrom;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender as Sender, TryRecvError};
use std::sync::Arc;
use std::time;

use super::PRESSED_KEYS;
use crate::kanata::*;

impl Kanata {
    /// Initialize the callback that is passed to the Windows low level hook to receive key events
    /// and run the native_windows_gui event loop.
    pub fn event_loop(
        _cfg: Arc<Mutex<Self>>,
        tx: Sender<KeyEvent>,
        #[cfg(all(target_os = "windows", feature = "gui"))]
        ui: crate::gui::system_tray_ui::SystemTrayUi,
    ) -> Result<()> {
        // Display debug and panic output when launched from a terminal.
        #[cfg(not(feature = "gui"))]
        unsafe {
            use winapi::um::wincon::*;
            if AttachConsole(ATTACH_PARENT_PROCESS) != 0 {
                panic!("Could not attach to console");
            }
        };

        let (preprocess_tx, preprocess_rx) = sync_channel(100);
        start_event_preprocessor(preprocess_rx, tx);

        // This callback should return `false` if the input event is **not** handled by the
        // callback and `true` if the input event **is** handled by the callback. Returning false
        // informs the callback caller that the input event should be handed back to the OS for
        // normal processing.
        let _kbhook = KeyboardHook::set_input_cb(move |input_event| {
            let mut key_event = match KeyEvent::try_from(input_event) {
                Ok(ev) => ev,
                _ => return false,
            };

            check_for_exit(&key_event);
            let oscode = OsCode::from(input_event.code);
            if !MAPPED_KEYS.lock().contains(&oscode) {
                return false;
            }

            // Unlike Linux, Windows does not use a separate value for repeat. However, our code
            // needs to differentiate between initial press and repeat press.
            log::debug!("event loop: {:?}", key_event);
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

            // Send input_events to the preprocessing loop. Panic if channel somehow gets full or if
            // channel disconnects. Typing input should never trigger a panic based on the channel
            // getting full, assuming regular operation of the program and some other bug isn't the
            // problem. I've tried to crash the program by pressing as many keys on my keyboard at
            // the same time as I could, but was unable to.
            try_send_panic(&preprocess_tx, key_event);
            true
        });

        #[cfg(all(target_os = "windows", feature = "gui"))]
        let _ui = ui; // prevents thread from panicking on exiting via a GUI
                      // The event loop is also required for the low-level keyboard hook to work.
        native_windows_gui::dispatch_thread_events();
        Ok(())
    }

    /// On Windows with LLHOOK/SendInput APIs,
    /// Kanata does not have as much control
    /// over the full system's keystates as one would want;
    /// unlike in Linux or with the Interception driver.
    /// Sometimes Kanata can miss events; e.g. a release is
    /// missed and a keystate remains pressed within Kanata (1),
    /// or a press is missed in Kanata but the release is caught,
    /// and thus the keystate remains pressed within the Windows system
    /// because Kanata consumed the release and didn't know what to do about it (2).
    ///
    /// For (1), `release_normalkey_states` theoretically fixes the issue
    /// after 60s of Kanata being idle,
    /// but that is a long time and doesn't seem to work consistently.
    /// Unfortunately this does not seem to be easily fixable in all cases.
    /// For example, a press consumed by Kanata could result in
    /// **only** a `(layer-while-held ...)` action as the output;
    /// if the corresponding release were missed,
    /// Kanata has no information available from the larger Windows system
    /// to confirm that the physical key is actually released
    /// but that the process didn't see the event.
    /// E.g. there is the `GetKeyboardState` API
    /// and this will be useful when the missed release has a key output,
    /// but not with the layer example.
    /// There does not appear to be any "raw input" mechanism
    /// to see the snapshot of the current state of physical keyboard keys.
    ///
    /// For (2), consider that this might be fixed purely within Kanata
    /// by checking Kanata's active action states,
    /// and if there are no active states corresponding to a released event,
    /// to send a release of the original input.
    /// This would result in extra release events though;
    /// for example if the `A` key action is `(macro a)`,
    /// the above logic will result in a second SendInput release event of `A`.
    ///
    /// The solution makes use of the following states:
    /// - `MAPPED_KEYS` (MK)
    /// - `GetKeyboardState` WinAPI (GKS)
    /// - `PRESSED_KEYS` (PK)
    /// - `self.prev_keys` (SPV)
    ///
    /// If a discrepancy is detected,
    /// this procedure releases Windows keys via SendInput
    /// and/or clears internal Kanata states.
    ///
    /// The checks are:
    /// 1. For all of SPV, check that it is pressed in GKS.
    ///    If a key is not pressed, find the coordinate of this state.
    ///    Clear in PK and clear all states with the same coordinate as key output.
    /// 2. For all active in GKS and exists in MK, check it is in SPV.
    ///    If not in SPV, call SendInput to release.
    #[cfg(not(feature = "simulated_input"))]
    pub(crate) fn win_synchronize_keystates(&mut self) {
        use winapi::um::winuser::*;
        use winapi::um::errhandlingapi::*;
        use kanata_keyberon::layout::*;

        log::debug!("synchronizing win keystates");
        let mut win_key_states = [0u8; 256];
        let res = unsafe { GetKeyboardState(win_key_states.as_mut_ptr()) };
        if res == 0 {
            let err_code = unsafe { GetLastError() };
            log::error!("GetKeyboardState returned error code: {err_code}");
            return;
        }

        for pvk in self.prev_keys.iter() {
            // Each pvk is expected to be pressed.
            let osc: OsCode = pvk.into();
            let idx = usize::from(osc);
            let wks = win_key_states[idx];
            let is_pressed_in_windows = wks >= 0b1000000;
            if is_pressed_in_windows {
                continue;
            }

            log::error!("Unexpected keycode is pressed in kanata but not in Windows. Clearing it: {pvk}");
            // Need to clear internal state about this key.
                // find coordinate(s) in keyberon associated with pvk
                let mut coords_to_clear = Vec::<KCoord>::new();
                let layout = self.layout.bm();
                layout
                    .states
                    .retain(|s| {
                        let retain = match s.keycode() {
                            Some(k) => k != *pvk,
                            _ => true,
                        };
                        if !retain {
                            if let Some(coord) = s.coord() {
                                coords_to_clear.push(coord);
                            }
                        }
                        retain
                    });

                // Clear other states other than keycode associated with a keycode that needs to be
                // cleaned up.
                layout.states.retain(|s| {
                    match s.coord() {
                        Some(c) => !coords_to_clear.contains(&c),
                        None => false,
                    }
                });

                // Clear PRESSED_KEYS for coordinates associated with real and not virtual keys
                let mut pressed_keys = PRESSED_KEYS.lock();
                for osc in coords_to_clear
                    .iter()
                    .copied()
                    .filter_map(|c| {
                        match c {
                            (FAKE_KEY_ROW, _) => None,
                            (_, kc) => Some(OsCode::from(kc)),
                        }
                    })
                {
                    pressed_keys.remove(&osc);
                }
                drop(pressed_keys);
        }

        for (vk, state) in win_key_states.iter().copied().enumerate() {
        }
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
                    panic!("channel disconnected")
                }
            }
        }
    });
}
