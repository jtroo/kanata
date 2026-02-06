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
    /// Contains an outer recovery loop: if the DriverKit output connection drops
    /// (daemon crash, not installed, etc.), input devices are released so the
    /// keyboard returns to normal operation. When the connection recovers,
    /// devices are re-seized and remapping resumes.
    pub fn event_loop(kanata: Arc<Mutex<Self>>, tx: Sender<KeyEvent>) -> Result<()> {
        info!("entering the event loop");

        let k = kanata.lock();
        let allow_hardware_repeat = k.allow_hardware_repeat;
        let include_names = k.include_names.clone();
        let exclude_names = k.exclude_names.clone();
        drop(k);

        loop {
            // --- (Re)create KbdIn and grab input devices ---
            let mut kb = match KbdIn::new(include_names.clone(), exclude_names.clone()) {
                Ok(kbd_in) => kbd_in,
                Err(e) => bail!("failed to open keyboard device(s): {}", e),
            };

            info!("keyboard grabbed, entering event processing loop");

            // --- Inner event processing loop ---
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
            drop(kb);

            info!(
                "Input devices released. Keyboard is usable (without remapping). \
                 Waiting for DriverKit output to recover..."
            );

            // --- Wait for the pqrs client heartbeat to re-establish the connection ---
            loop {
                std::thread::sleep(std::time::Duration::from_millis(500));
                if is_sink_ready() {
                    info!("DriverKit output recovered — re-grabbing input devices");
                    break;
                }
            }

            // The outer loop will create a new KbdIn and resume remapping.
        }
    }

    pub fn check_release_non_physical_shift(&mut self) -> Result<()> {
        Ok(())
    }
}
