use anyhow::{anyhow, bail, Result};
use log::info;
use parking_lot::Mutex;
use std::convert::TryFrom;
use std::sync::mpsc::Sender;
use std::sync::Arc;

use super::*;

impl Kanata {
    /// Enter an infinite loop that listens for OS key events and sends them to the processing
    /// thread.
    pub fn event_loop(kanata: Arc<Mutex<Self>>, tx: Sender<KeyEvent>) -> Result<()> {
        info!("entering the event loop");

        let k = kanata.lock();
        let mut kbd_in = match KbdIn::new(
            &k.kbd_in_paths,
            k.continue_if_no_devices,
            k.include_names.clone(),
            k.exclude_names.clone(),
        ) {
            Ok(kbd_in) => kbd_in,
            Err(e) => {
                bail!("failed to open keyboard device(s): {}", e)
            }
        };
        drop(k);

        loop {
            let events = kbd_in.read().map_err(|e| anyhow!("failed read: {}", e))?;
            log::trace!("{events:?}");

            for in_event in events.into_iter() {
                let key_event = match KeyEvent::try_from(in_event) {
                    Ok(ev) => ev,
                    _ => {
                        // Pass-through non-key events
                        let mut kanata = kanata.lock();
                        kanata
                            .kbd_out
                            .write_raw(in_event)
                            .map_err(|e| anyhow!("failed write: {}", e))?;
                        continue;
                    }
                };

                check_for_exit(&key_event);

                // Check if this keycode is mapped in the configuration. If it hasn't been mapped, send
                // it immediately.
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

    pub fn set_repeat_rate(cfg_items: &HashMap<String, String>) -> Result<()> {
        if let Some(x11_rpt_str) = cfg_items.get("linux-x11-repeat-delay-rate") {
            let delay_rate = x11_rpt_str.split(',').collect::<Vec<_>>();
            let errmsg = format!("Invalid value for linux-x11-repeat-delay-rate: \"{x11_rpt_str}\".\nExpected two numbers 0-65535 separated by a comma, e.g. 200,25");
            if delay_rate.len() != 2 {
                log::error!("{errmsg}");
            }
            str::parse::<u16>(delay_rate[0]).map_err(|e| {
                log::error!("{errmsg}");
                e
            })?;
            str::parse::<u16>(delay_rate[1]).map_err(|e| {
                log::error!("{errmsg}");
                e
            })?;
            log::info!(
                "Using xset to set X11 repeat delay to {} and repeat rate to {}",
                delay_rate[0],
                delay_rate[1]
            );
            let cmd_output = std::process::Command::new("xset")
                .args(["r", "rate", delay_rate[0], delay_rate[1]])
                .output()
                .map_err(|e| {
                    log::error!("failed to run xset: {e:?}");
                    e
                })?;
            log::info!(
                "xset stdout: {}",
                String::from_utf8_lossy(&cmd_output.stdout)
            );
            log::info!(
                "xset stderr: {}",
                String::from_utf8_lossy(&cmd_output.stderr)
            );
        }
        Ok(())
    }
}
