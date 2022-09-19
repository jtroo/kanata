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

        let mut kbd_in = match KbdIn::new(&kanata.lock().kbd_in_paths) {
            Ok(kbd_in) => kbd_in,
            Err(e) => {
                bail!("failed to open keyboard device: {}", e)
            }
        };

        loop {
            let events = kbd_in.read().map_err(|e| anyhow!("failed read: {}", e))?;
            log::trace!("{events:?}");

            // Pass-through non-key events
            for in_event in events.into_iter() {
                let mut key_event = match KeyEvent::try_from(in_event) {
                    Ok(ev) => ev,
                    _ => {
                        let mut kanata = kanata.lock();
                        kanata
                            .kbd_out
                            .write(in_event)
                            .map_err(|e| anyhow!("failed write: {}", e))?;
                        continue;
                    }
                };

                // Check if this keycode is mapped in the configuration. If it hasn't been mapped, send
                // it immediately.
                check_for_exit(&key_event);
                if !MAPPED_KEYS.lock().contains(&key_event.code) {
                    log::debug!("{key_event:?} is not mapped");
                    let mut kanata = kanata.lock();
                    kanata
                        .kbd_out
                        .write_key(key_event.code, key_event.value)
                        .map_err(|e| anyhow!("failed write key: {}", e))?;

                    // #139: send an event that is guaranteed to map to no-op to the processing loop so
                    // that it will process tap-hold-press and tap-hold-release even for unmapped keys.
                    key_event.code = OsCode::KEY_RESERVED;
                }

                // Send key events to the processing loop
                tx.send(key_event).unwrap();
            }
        }
    }

    pub fn check_release_non_physical_shift(&mut self) -> Result<()> {
        Ok(())
    }
}
