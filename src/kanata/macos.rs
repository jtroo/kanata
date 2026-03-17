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

        info!("keyboard grabbed, entering event processing loop");

        loop {
            // --- Event processing loop ---
            let needs_recovery = loop {
                // Check output health before blocking on input
                if !kanata.lock().kbd_out.output_ready() {
                    log::warn!("output backend unavailable — releasing input devices");
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
                 Waiting for the output backend to recover..."
            );

            // --- Wait for the output backend to re-establish the connection ---
            loop {
                if kanata
                    .lock()
                    .kbd_out
                    .wait_until_ready(Some(Duration::from_millis(500)))
                {
                    // Let the direct DriverKit backend finish its callback sequence
                    // before we re-seize input devices. Seizing too early can race
                    // with IOKit enumeration triggered by those callbacks.
                    std::thread::sleep(Duration::from_secs(1));
                    info!("output backend recovered — re-grabbing input devices");
                    break;
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
