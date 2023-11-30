use anyhow::{anyhow, bail, Result};
use driverkit::KeyEvent as dKeyEvent;
use driverkit::{grab_kb, send_key, wait_key};
use log::info;
use parking_lot::Mutex;
use std::convert::TryFrom;
use std::sync::mpsc::SyncSender as Sender;
use std::sync::Arc;

use super::*;

static PRESSED_KEYS: Lazy<Mutex<HashSet<OsCode>>> = Lazy::new(|| Mutex::new(HashSet::default()));

impl Kanata {
    /// Enter an infinite loop that listens for OS key events and sends them to the processing
    /// thread.
    pub fn event_loop(kanata: Arc<Mutex<Self>>, tx: Sender<KeyEvent>) -> Result<()> {
        info!("entering the event loop");

        let mut kb = match KbdIn::new() {
            Ok(kbd_in) => kbd_in,
            Err(e) => bail!("failed to open keyboard device(s): {}", e)
        };

        loop {

            //let mut event = dKeyEvent { value: 0, page: 0, code: 0, };
            //let _key = wait_key(&mut event);

            let event = kb.read().map_err(|e| anyhow!("failed read: {}", e))?;

            //let mut key_event = KeyEvent::try_from(event).map_err(|e| anyhow!("failed read: {:?}", e))?;

            let mut key_event = match KeyEvent::try_from(event) {
                    Ok(ev) => ev,
                    _ => {
                        // Pass-through non-key and non-scroll events
                        let mut kanata = kanata.lock();
                        kanata.kbd_out.write(event.clone()).map_err(|e| anyhow!("failed write: {}", e))?;
                        continue;
                    }
                };

            check_for_exit(&key_event);

            if !MAPPED_KEYS.lock().contains(&key_event.code) {
                log::debug!("{key_event:?} is not mapped");
                let mut kanata = kanata.lock();
                kanata
                    .kbd_out
                    .write(event.clone())
                    .map_err(|e| anyhow!("failed write: {}", e))?;
                //send_key(&mut event.to_driverkit_event()  );
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
            tx.try_send(key_event)?;
        }
    }

    pub fn check_release_non_physical_shift(&mut self) -> Result<()> {
        Ok(())
    }
}
