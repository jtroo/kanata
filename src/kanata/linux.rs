#![cfg_attr(
    feature = "simulated_output",
    allow(dead_code, unused_imports, unused_variables, unused_mut)
)]

use anyhow::{anyhow, bail, Result};
use evdev::{InputEvent, InputEventKind, RelativeAxisType};
use log::info;
use parking_lot::Mutex;
use std::convert::TryFrom;
use std::sync::mpsc::SyncSender as Sender;
use std::sync::Arc;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::*;

impl Kanata {
    /// Enter an infinite loop that listens for OS key events and sends them to the processing
    /// thread.
    pub fn event_loop(kanata: Arc<Mutex<Self>>, tx: Sender<KeyEvent>) -> Result<()> {
        info!("entering the event loop");

        let (preprocess_tx, preprocess_rx) = std::sync::mpsc::sync_channel(100);
        let k = kanata.lock();
        let debounce_duration = Duration::from_millis(k.linux_debounce_duration);
        start_event_preprocessor(preprocess_rx, tx.clone(), debounce_duration);

        let allow_hardware_repeat = k.allow_hardware_repeat;
        let mut kbd_in = match KbdIn::new(
            &k.kbd_in_paths,
            k.continue_if_no_devices,
            k.include_names.clone(),
            k.exclude_names.clone(),
            k.device_detect_mode,
        ) {
            Ok(kbd_in) => kbd_in,
            Err(e) => {
                bail!("failed to open keyboard device(s): {}", e)
            }
        };

        // In some environments, this needs to be done after the input device grab otherwise it
        // does not work on kanata startup.
        Kanata::set_repeat_rate(k.x11_repeat_rate)?;
        drop(k);

        loop {
            let events = kbd_in.read().map_err(|e| anyhow!("failed read: {}", e))?;
            log::trace!("event count: {}\nevents:\n{events:?}", events.len());

            for in_event in events.iter().copied() {
                let key_event = match KeyEvent::try_from(in_event) {
                    Ok(ev) => ev,
                    _ => {
                        // Pass-through non-key and non-scroll events
                        let mut kanata = kanata.lock();
                        #[cfg(not(feature = "simulated_output"))]
                        kanata
                            .kbd_out
                            .write_raw(in_event)
                            .map_err(|e| anyhow!("failed write: {}", e))?;
                        continue;
                    }
                };

                check_for_exit(&key_event);

                if key_event.value == KeyValue::Repeat && !allow_hardware_repeat {
                    continue;
                }

                if key_event.value == KeyValue::Tap {
                    // Scroll event for sure. Only scroll events produce Tap.
                    if !handle_scroll(&kanata, in_event, key_event.code, &events)? {
                        continue;
                    }
                } else {
                    // Handle normal keypresses.
                    // Check if this keycode is mapped in the configuration.
                    // If it hasn't been mapped, send it immediately.
                    if !MAPPED_KEYS.lock().contains(&key_event.code) {
                        let mut kanata = kanata.lock();
                        #[cfg(not(feature = "simulated_output"))]
                        kanata
                            .kbd_out
                            .write_raw(in_event)
                            .map_err(|e| anyhow!("failed write: {}", e))?;
                        continue;
                    };
                }

                // Send key events to the appropriate processing loop based on debounce duration
                let target_tx = if debounce_duration.is_zero() { &tx } else { &preprocess_tx };
                if let Err(e) = target_tx.try_send(key_event) {
                    bail!("failed to send on channel: {}", e);
                }
            }
        }
    }

    pub fn check_release_non_physical_shift(&mut self) -> Result<()> {
        Ok(())
    }

    pub fn set_repeat_rate(s: Option<KeyRepeatSettings>) -> Result<()> {
        if let Some(s) = s {
            log::info!(
                "Using xset to set X11 repeat delay to {} and repeat rate to {}",
                s.delay,
                s.rate,
            );
            let cmd_output = std::process::Command::new("xset")
                .args([
                    "r",
                    "rate",
                    s.delay.to_string().as_str(),
                    s.rate.to_string().as_str(),
                ])
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

/// Returns true if the scroll event should be sent to the processing loop, otherwise returns
/// false.
fn handle_scroll(
    kanata: &Mutex<Kanata>,
    in_event: InputEvent,
    code: OsCode,
    all_events: &[InputEvent],
) -> Result<bool> {
    let direction: MWheelDirection = code.try_into().unwrap();
    let scroll_distance = in_event.value().unsigned_abs() as u16;
    match in_event.kind() {
        InputEventKind::RelAxis(axis_type) => {
            match axis_type {
                RelativeAxisType::REL_WHEEL | RelativeAxisType::REL_HWHEEL => {
                    if MAPPED_KEYS.lock().contains(&code) {
                        return Ok(true);
                    }
                    // If we just used `write_raw` here, some of the scrolls issued by kanata would be
                    // REL_WHEEL_HI_RES + REL_WHEEL and some just REL_WHEEL and an issue like this one
                    // would happen: https://github.com/jtroo/kanata/issues/395
                    //
                    // So to fix this case, we need to use `scroll` which will also send hi-res scrolls
                    // along normal scrolls.
                    //
                    // However, if this is a normal scroll event, it may be sent alongside a hi-res
                    // scroll event. In this scenario, the hi-res event should be used to call
                    // scroll, and not the normal event. Otherwise, too much scrolling will happen.
                    let mut kanata = kanata.lock();
                    if !all_events.iter().any(|ev| {
                        matches!(
                            ev.kind(),
                            InputEventKind::RelAxis(
                                RelativeAxisType::REL_WHEEL_HI_RES
                                    | RelativeAxisType::REL_HWHEEL_HI_RES
                            )
                        )
                    }) {
                        kanata
                            .kbd_out
                            .scroll(direction, scroll_distance * HI_RES_SCROLL_UNITS_IN_LO_RES)
                            .map_err(|e| anyhow!("failed write: {}", e))?;
                    }
                    Ok(false)
                }
                RelativeAxisType::REL_WHEEL_HI_RES | RelativeAxisType::REL_HWHEEL_HI_RES => {
                    if !MAPPED_KEYS.lock().contains(&code) {
                        // Passthrough if the scroll wheel event is not mapped
                        // in the configuration.
                        let mut kanata = kanata.lock();
                        kanata
                            .kbd_out
                            .scroll(direction, scroll_distance)
                            .map_err(|e| anyhow!("failed write: {}", e))?;
                    }
                    // Kanata will not handle high resolution scroll events for now.
                    // Full notch scrolling only.
                    Ok(false)
                }
                _ => unreachable!("expect to be handling a wheel event"),
            }
        }
        _ => unreachable!("expect to be handling a wheel event"),
    }
}

fn start_event_preprocessor(
    preprocess_rx: Receiver<KeyEvent>,
    process_tx: Sender<KeyEvent>,
    debounce_duration: Duration,
) {
    let mut last_key_event_time: HashMap<OsCode, Instant> = HashMap::new();

    std::thread::spawn(move || {
        loop {
            match preprocess_rx.try_recv() {
                Ok(kev) => {
                    let now = Instant::now();
                    let oscode = kev.code;

                    match kev.value {
                        KeyValue::Release => {
                            // Always allow key releases to pass through
                            try_send_panic(&process_tx, kev);
                        }
                        KeyValue::Press => {
                            // Check if the key press is within the debounce duration
                            if let Some(&last_time) = last_key_event_time.get(&oscode) {
                                if now.duration_since(last_time) < debounce_duration {
                                    log::debug!("Debounced key press: {:?}", kev);
                                    continue; // Skip processing this event
                                }
                            }

                            // Update the last processed time for the key press
                            last_key_event_time.insert(oscode, now);

                            // Forward the key press event
                            try_send_panic(&process_tx, kev);
                        }
                        _ => {
                            // Forward other key events (e.g., Repeat, Tap) without debouncing
                            try_send_panic(&process_tx, kev);
                        }
                    }
                }
                Err(TryRecvError::Empty) => {
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(TryRecvError::Disconnected) => {
                    panic!("channel disconnected");
                }
            }
        }
    });
}

fn try_send_panic(tx: &Sender<KeyEvent>, kev: KeyEvent) {
    if let Err(e) = tx.try_send(kev) {
        panic!("failed to send on channel: {e:?}");
    }
}
