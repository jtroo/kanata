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
    pub fn event_loop(kanata: Arc<Mutex<Self>>, tx: Sender<SupportedInputEvent>) -> Result<()> {
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

        // In some environments, this needs to be done after the input device grab otherwise it
        // does not work on kanata startup.
        Kanata::set_repeat_rate(&k.defcfg_items)?;
        drop(k);

        loop {
            let events = kbd_in.read().map_err(|e| anyhow!("failed read: {}", e))?;
            log::trace!("{events:?}");

            for in_event in events.into_iter() {
                let supported_in_event = match SupportedInputEvent::try_from(in_event) {
                    Ok(ev) => ev,
                    _ => {
                        // Pass-through non-key and non-scroll events
                        let mut kanata = kanata.lock();
                        kanata
                            .kbd_out
                            .write_raw(in_event)
                            .map_err(|e| anyhow!("failed write: {}", e))?;
                        continue;
                    }
                };

                // TODO: Perhaps this should be exposed as config option?
                // Hardcoding this would disable hi-res scrolls whenever kanata is running,
                // regardless of whether user remaps mouse wheel.
                // But allowing passthrough when scroll is mapped might
                // show some side effects, such as scroll still working while remapped.
                // Another idea is to disable corresponding hi-res scroll
                // events for the scroll events that are listed in defsrc.
                let allow_hi_res_scroll_events_passthrough = false;

                let osc: OsCode = match supported_in_event {
                    SupportedInputEvent::KeyEvent(kev) => kev.code,
                    SupportedInputEvent::ScrollEvent(
                        sev @ ScrollEvent {
                            kind: ScrollEventKind::Standard,
                            ..
                        },
                    ) => sev
                        .try_into()
                        .expect("standard scroll should have OsCode mapping"),
                    SupportedInputEvent::ScrollEvent(ScrollEvent {
                        kind: ScrollEventKind::HiRes,
                        ..
                    }) => {
                        if allow_hi_res_scroll_events_passthrough {
                            let mut kanata = kanata.lock();
                            kanata
                                .kbd_out
                                .write_raw(in_event)
                                .map_err(|e| anyhow!("failed write: {}", e))?;
                        }
                        continue;
                    }
                };

                // Check if this keycode is mapped in the configuration. If it hasn't been mapped, send
                // it immediately.
                let is_mapped = MAPPED_KEYS.lock().contains(&osc);

                if let SupportedInputEvent::KeyEvent(key_event) = supported_in_event {
                    check_for_exit(&key_event);
                }

                if !is_mapped {
                    let mut kanata = kanata.lock();
                    kanata
                        .kbd_out
                        .write_raw(in_event)
                        .map_err(|e| anyhow!("failed write: {}", e))?;
                    continue;
                }

                // Send key events to the processing loop
                if let Err(e) = tx.send(supported_in_event) {
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
