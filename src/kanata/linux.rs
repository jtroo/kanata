use anyhow::{anyhow, bail, Result};
use crossbeam_channel::Sender;
use log::info;
use parking_lot::Mutex;
use std::convert::TryFrom;
use std::sync::Arc;

use super::*;

impl Kanata {
    /// Enter an infinite loop that listens for OS key events and sends them to the processing
    /// thread.
    pub fn event_loop(kanata: Arc<Mutex<Self>>, tx: Sender<KeyEvent>) -> Result<()> {
        info!("entering the event loop");
        {
            let mut mapped_keys = MAPPED_KEYS.lock();
            *mapped_keys = kanata.lock().mapped_keys.clone();
        }

        let k = kanata.lock();
        let mut kbd_in = match KbdIn::new(&k.kbd_in_paths, k.continue_if_no_devices) {
            Ok(kbd_in) => kbd_in,
            Err(e) => {
                bail!("failed to open keyboard device(s): {}", e)
            }
        };
        drop(k);

        loop {
            let events = kbd_in.read().map_err(|e| anyhow!("failed read: {}", e))?;
            log::trace!("{events:?}");

            // Pass-through non-key events
            for in_event in events.into_iter() {
                let key_event = match KeyEvent::try_from(in_event) {
                    Ok(ev) => ev,
                    _ => {
                        let mut kanata = kanata.lock();
                        kanata
                            .kbd_out
                            .write_raw(in_event)
                            .map_err(|e| anyhow!("failed write: {}", e))?;
                        continue;
                    }
                };

                // Check if this keycode is mapped in the configuration. If it hasn't been mapped, send
                // it immediately.
                check_for_exit(&key_event);
                if !MAPPED_KEYS.lock().contains(&key_event.code) {
                    let mut kanata = kanata.lock();
                    kanata
                        .kbd_out
                        .write_key(key_event.code, key_event.value)
                        .map_err(|e| anyhow!("failed write key: {}", e))?;
                    continue;
                }

                // Send key events to the processing loop
                if let Err(e) = tx.send(key_event) {
                    bail!("failed to send on channel: {}", e)
                }
            }
        }
    }

    pub fn check_release_non_physical_shift(&mut self) -> Result<()> {
        Ok(())
    }
}
