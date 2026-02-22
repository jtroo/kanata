use super::*;
use anyhow::{Result, anyhow, bail};
use karabiner_driverkit::is_sink_ready;
use log::info;
use parking_lot::Mutex;
use std::convert::TryFrom;
use std::sync::Arc;
use std::sync::mpsc::SyncSender as Sender;

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

        info!("keyboard grabbed, entering event processing loop");

        loop {
            // --- Event processing loop ---
            let needs_recovery = loop {
                // Check output health before blocking on input
                if !is_sink_ready() {
                    log::warn!("DriverKit output lost — releasing input devices");
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

                let mut key_event = match KeyEvent::try_from(event) {
                    Ok(ev) => ev,
                    _ => {
                        log::debug!("{event:?} is unrecognized!");
                        let mut kanata = kanata.lock();
                        match kanata.kbd_out.write(event) {
                            Ok(()) => continue,
                            Err(e) if e.kind() == std::io::ErrorKind::NotConnected => {
                                log::warn!(
                                    "DriverKit output lost during write — releasing input devices"
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
                                "DriverKit output lost during write — releasing input devices"
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
                 Waiting for DriverKit output to recover..."
            );

            // --- Wait for the pqrs client to re-establish the connection ---
            loop {
                std::thread::sleep(std::time::Duration::from_millis(500));
                if is_sink_ready() {
                    // Let the pqrs client's callback sequence finish before
                    // we re-seize input devices. The client fires several
                    // callbacks in quick succession (connected, driver_connected,
                    // virtual_hid_keyboard_ready); seizing too early can race
                    // with IOKit enumeration triggered by those callbacks.
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    info!("DriverKit output recovered — re-grabbing input devices");
                    break;
                }
            }

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
