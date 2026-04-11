use super::*;
use anyhow::{Result, anyhow, bail};
use log::info;
use parking_lot::Mutex;
use std::convert::TryFrom;
use std::sync::Arc;
use std::sync::mpsc::SyncSender as Sender;
use std::time::Duration;

impl Kanata {
    /// Enter an infinite loop that listens for OS key events and sends them to the processing thread.
    ///
    /// Contains a recovery mechanism: if the DriverKit output connection drops
    /// (daemon crash, not installed, etc.), input devices are released so the
    /// keyboard returns to normal operation. When the connection recovers,
    /// devices are re-seized and remapping resumes.
    ///
    /// Recovery uses `regrab_input()` rather than recreating `KbdIn` to avoid
    /// re-initializing the pqrs client (via `init_sink()`). A second client
    /// causes duplicate connection callbacks that race with the IOHIDManager,
    /// leading to "exclusive access" errors on the input device.
    pub fn event_loop(kanata: Arc<Mutex<Self>>, tx: Sender<KeyEvent>) -> Result<()> {
        info!("entering the event loop");

        let k = kanata.lock();
        let allow_hardware_repeat = k.allow_hardware_repeat;
        let include_names = k.include_names.clone();
        let exclude_names = k.exclude_names.clone();
        drop(k);

        let mut kb = match KbdIn::new(include_names, exclude_names) {
            Ok(kbd_in) => kbd_in,
            Err(e) => bail!("failed to open keyboard device(s): {}", e),
        };

        {
            let kanata = kanata.lock();
            if !kanata
                .kbd_out
                .wait_until_ready(Some(Duration::from_secs(10)))
            {
                log::warn!(
                    "output backend not ready after 10s. Key output may fail until the backend recovers."
                );
            }
        }

        // Startup is done. Stop decorating future `SIGABRT`s with the
        // Karabiner setup hint — any abort from here on is almost
        // certainly a dispatcher/CoreFoundation teardown race (e.g. the
        // `mutex lock failed` one on kill-chord exit), for which the
        // Karabiner hint would be actively misleading.
        crate::oskbd::mark_karabiner_startup_complete();

        info!("keyboard grabbed, entering event processing loop");

        // Start the mouse event tap on a background thread if any mouse buttons
        // are mapped in the config. Similar to the Windows mouse hook.
        // The braces scope-drop the MAPPED_KEYS lock before entering the event loop;
        // the JoinHandle is dropped because the run loop runs for the process lifetime.
        // Subsequent live reloads that introduce mouse keys to a previously
        // mouse-key-free defsrc install the tap lazily via
        // `ensure_mouse_listener_installed_after_reload` (called from
        // `do_live_reload`). The tap callback re-reads `MAPPED_KEYS` per event,
        // so reloads that change *which* mouse keys are mapped also take
        // effect without restart.
        //
        // Clone the mouse_movement_key Arc *before* locking MAPPED_KEYS to keep
        // the project-wide lock order `kanata -> MAPPED_KEYS`. Reversing it here
        // would create a new ordering edge with the rest of this file.
        {
            let mmk = kanata.lock().mouse_movement_key.clone();
            let mapped = MAPPED_KEYS.lock();
            let _ = crate::oskbd::start_mouse_listener(tx.clone(), &mapped, mmk);
        }

        // Toggles `is_screen_grab_paused()` on lock / fast-user-switch.
        // See `oskbd::start_screen_lock_poller` for the design notes.
        crate::oskbd::start_screen_lock_poller();

        loop {
            // --- Event processing loop ---
            let needs_recovery = loop {
                // Check output health before blocking on input
                if !kanata.lock().kbd_out.output_ready() {
                    log::warn!("output backend unavailable — releasing input devices");
                    break true;
                }

                if crate::oskbd::is_screen_grab_paused() {
                    log::info!(
                        "console session paused (lock/user-switch) — releasing input devices"
                    );
                    break true;
                }

                let event = match kb.read() {
                    Ok(ev) => ev,
                    Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                        // Pipe closed by release_input_only() — expected during recovery
                        log::info!("input pipe EOF — devices were released");
                        break true;
                    }
                    Err(e) => return Err(anyhow!("failed read: {}", e)),
                };

                // Re-check after the blocking read: the lock may have
                // landed while we were inside `wait_key`, in which case
                // this event is the first keystroke from the new user
                // and must be dropped, not remapped. See the caveat in
                // `oskbd::macos`.
                if crate::oskbd::is_screen_grab_paused() {
                    log::info!(
                        "console session paused (lock/user-switch) — dropping read event and releasing input devices"
                    );
                    break true;
                }

                let mut key_event = match KeyEvent::try_from(event) {
                    Ok(ev) => ev,
                    _ => {
                        log::debug!("{event:?} is unrecognized!");
                        let mut kanata = kanata.lock();
                        match kanata.kbd_out.write(event) {
                            Ok(()) => continue,
                            Err(e) if e.kind() == std::io::ErrorKind::NotConnected => {
                                log::warn!(
                                    "output backend unavailable during write — releasing input devices"
                                );
                                break true;
                            }
                            Err(e) => return Err(anyhow!("failed write: {}", e)),
                        }
                    }
                };

                check_for_exit(&key_event);

                if key_event.value == KeyValue::Repeat && !allow_hardware_repeat {
                    continue;
                }

                if !MAPPED_KEYS.lock().contains(&key_event.code) {
                    log::debug!("{key_event:?} is not mapped");
                    let mut kanata = kanata.lock();
                    match kanata.kbd_out.write(event) {
                        Ok(()) => continue,
                        Err(e) if e.kind() == std::io::ErrorKind::NotConnected => {
                            log::warn!(
                                "output backend unavailable during write — releasing input devices"
                            );
                            break true;
                        }
                        Err(e) => return Err(anyhow!("failed write: {}", e)),
                    }
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
            };

            if !needs_recovery {
                break Ok(());
            }

            // --- Release input so the keyboard works normally (unseized) ---
            kb.release_input();

            info!(
                "Input devices released. Keyboard is usable (without remapping). \
                 Waiting for the output backend and console session to recover..."
            );

            // Re-grab once both the sink is healthy and the session is
            // unpaused. Sleep explicitly when only the sink is ready,
            // since `wait_until_ready` returns instantly in that case.
            loop {
                let sink_ready = kanata
                    .lock()
                    .kbd_out
                    .wait_until_ready(Some(Duration::from_millis(500)));
                let session_ready = !crate::oskbd::is_screen_grab_paused();
                if sink_ready && session_ready {
                    // Let the direct DriverKit backend finish its callback sequence
                    // before we re-seize input devices. Seizing too early can race
                    // with IOKit enumeration triggered by those callbacks.
                    std::thread::sleep(Duration::from_secs(1));
                    info!("output backend and console session ready — re-grabbing input devices");
                    break;
                }
                if sink_ready && !session_ready {
                    std::thread::sleep(Duration::from_millis(500));
                }
            }

            {
                let mut kanata = kanata.lock();
                kanata
                    .kbd_out
                    .release_tracked_output_keys("output-backend-recovery");
            }
            PRESSED_KEYS.lock().clear();

            // Re-seize input devices using regrab_input() which creates a fresh
            // pipe and listener thread without re-initializing the sink client.
            if !kb.regrab_input() {
                bail!("failed to re-grab keyboard devices after DriverKit recovery");
            }

            info!("keyboard grabbed, entering event processing loop");

            // Back to the event processing loop.
        }
    }

    pub fn check_release_non_physical_shift(&mut self) -> Result<()> {
        Ok(())
    }
}
