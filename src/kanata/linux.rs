use anyhow::{bail, Result};
use crossbeam_channel::Sender;
use log::info;
use parking_lot::Mutex;
use std::convert::TryFrom;
use std::sync::Arc;

use crate::cfg;

use super::*;

impl Kanata {
    /// Enter an infinite loop that listens for OS key events and sends them to the processing
    /// thread.
    #[cfg(target_os = "linux")]
    pub fn event_loop(kanata: Arc<Mutex<Self>>, tx: Sender<KeyEvent>) -> Result<()> {
        info!("entering the event loop");
        {
            let mut mapped_keys = MAPPED_KEYS.lock();
            *mapped_keys = kanata.lock().mapped_keys;
        }

        let mut kbd_in = match KbdIn::new(&kanata.lock().kbd_in_paths) {
            Ok(kbd_in) => kbd_in,
            Err(e) => {
                bail!("failed to open keyboard device: {}", e)
            }
        };

        loop {
            let events = kbd_in.read()?;
            log::trace!("{events:?}");

            // Pass-through non-key events
            for in_event in events.into_iter() {
                let key_event = match KeyEvent::try_from(in_event) {
                    Ok(ev) => ev,
                    _ => {
                        let mut kanata = kanata.lock();
                        kanata.kbd_out.write(in_event)?;
                        continue;
                    }
                };

                // Check if this keycode is mapped in the configuration. If it hasn't been mapped, send
                // it immediately.
                let kc: usize = key_event.code.into();
                if kc >= cfg::MAPPED_KEYS_LEN || !MAPPED_KEYS.lock()[kc] {
                    let mut kanata = kanata.lock();
                    kanata.kbd_out.write_key(key_event.code, key_event.value)?;
                    continue;
                }

                // Send key events to the processing loop
                if let Err(e) = tx.send(key_event) {
                    bail!("failed to send on channel: {}", e)
                }
            }
        }
    }
}
