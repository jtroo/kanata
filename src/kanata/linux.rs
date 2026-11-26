#![cfg_attr(
    feature = "simulated_output",
    allow(dead_code, unused_imports, unused_variables, unused_mut)
)]

use crate::kanata::debounce::debounce::{Debounce, create_debounce_algorithm};
use crate::kanata::millisecond_counting::count_ms_elapsed;
use anyhow::{Result, anyhow, bail};
use evdev::{EventSummary, InputEvent, RelativeAxisCode};
use log::info;
use parking_lot::Mutex;
use std::convert::TryFrom;
use std::sync::Arc;
use std::sync::mpsc::SyncSender as Sender;
use std::time::{Duration, Instant};

use super::*;

impl Kanata {
    /// Enter an infinite loop that listens for OS key events and sends them to the processing
    /// thread.
    pub fn event_loop(kanata: Arc<Mutex<Self>>, tx: Sender<KeyEvent>) -> Result<()> {
        info!("entering the event loop");

        let k = kanata.lock();
        // Debounce settings are only read here, so live reload does not apply them.
        let debounce_tx = (k.debounce_duration_ms > 0).then(|| {
            start_debouncer(
                create_debounce_algorithm(k.debounce_algorithm, k.debounce_duration_ms),
                kanata.clone(),
                tx.clone(),
            )
        });

        let allow_hardware_repeat = k.allow_hardware_repeat;
        let mouse_movement_key = k.mouse_movement_key.clone();
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
                if let Some(ms_mvmt_key) = *mouse_movement_key.lock()
                    && let EventSummary::RelativeAxis(_, _, _) = in_event.destructure()
                {
                    let fake_event = KeyEvent::new(ms_mvmt_key, KeyValue::Tap);
                    if let Err(e) = tx.try_send(fake_event) {
                        bail!("failed to send on channel: {}", e)
                    }
                }

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
                }

                // Debounce key switch events before anything else looks at them, so that
                // held-key tracking, unmapped-key passthrough and the processing loop all see
                // the same events. Scroll events (Tap) do not come from a key switch.
                match &debounce_tx {
                    Some(debounce_tx) if key_event.value != KeyValue::Tap => {
                        if let Err(e) = debounce_tx.try_send(key_event) {
                            bail!("failed to send on channel: {}", e)
                        }
                    }
                    _ => forward_key_event(&kanata, key_event, Some(in_event), &tx)?,
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
    match in_event.destructure() {
        EventSummary::RelativeAxis(_, axis_type, _) => {
            match axis_type {
                RelativeAxisCode::REL_WHEEL | RelativeAxisCode::REL_HWHEEL => {
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
                            ev.destructure(),
                            EventSummary::RelativeAxis(
                                _,
                                RelativeAxisCode::REL_WHEEL_HI_RES
                                    | RelativeAxisCode::REL_HWHEEL_HI_RES,
                                _
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
                RelativeAxisCode::REL_WHEEL_HI_RES | RelativeAxisCode::REL_HWHEEL_HI_RES => {
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

/// Record a key as physically held or released, then either pass it straight to the output
/// when the configuration does not map it, or send it to the processing loop.
///
/// `raw` is the event as read from the device. It is `None` for events coming out of the
/// debouncer: those can be emitted after the device's SYN_REPORT was already passed through,
/// so they are written with a SYN_REPORT of their own instead of being buffered for the next one.
fn forward_key_event(
    kanata: &Mutex<Kanata>,
    key_event: KeyEvent,
    raw: Option<InputEvent>,
    tx: &Sender<KeyEvent>,
) -> Result<()> {
    match key_event.value {
        KeyValue::Release => {
            PRESSED_KEYS.lock().remove(&key_event.code);
        }
        KeyValue::Press => {
            PRESSED_KEYS.lock().insert(key_event.code);
        }
        _ => {}
    }

    // Handle normal keypresses.
    // Check if this keycode is mapped in the configuration.
    // If it hasn't been mapped, send it immediately.
    if !MAPPED_KEYS.lock().contains(&key_event.code) {
        let mut kanata = kanata.lock();
        #[cfg(not(feature = "simulated_output"))]
        match raw {
            Some(raw) => kanata.kbd_out.write_raw(raw),
            None => kanata.kbd_out.write_key(key_event.code, key_event.value),
        }
        .map_err(|e| anyhow!("failed write: {}", e))?;
        return Ok(());
    };

    // Send key events to the processing loop
    if let Err(e) = tx.try_send(key_event) {
        bail!("failed to send on channel: {}", e)
    }
    Ok(())
}

/// Start the thread that debounces key events and then hands them to `forward_key_event`.
/// Returns the channel that feeds it. If the thread stops, sending on that channel fails,
/// which ends the event loop.
fn start_debouncer(
    algorithm: Box<dyn Debounce>,
    kanata: Arc<Mutex<Kanata>>,
    tx: Sender<KeyEvent>,
) -> Sender<KeyEvent> {
    let (debounce_tx, debounce_rx) = std::sync::mpsc::sync_channel(100);
    std::thread::spawn(move || {
        if let Err(e) = run_debouncer(algorithm, debounce_rx, |kev| {
            forward_key_event(&kanata, kev, None, &tx)
        }) {
            log::error!("debounce thread stopped: {e:?}");
        }
    });
    debounce_tx
}

/// Feed received key events through the debounce algorithm, ticking it every millisecond while
/// it has deferred events, and call `forward` with each event it lets through. Returns when the
/// sending side of `rx` is dropped.
fn run_debouncer(
    mut algorithm: Box<dyn Debounce>,
    rx: Receiver<KeyEvent>,
    mut forward: impl FnMut(KeyEvent) -> Result<()>,
) -> Result<()> {
    log::info!(
        "debouncing key events with {} for {} ms",
        algorithm.name(),
        algorithm.debounce_time()
    );
    // The algorithms emit into a channel, which is drained after every call.
    let (out_tx, out_rx) = std::sync::mpsc::sync_channel(100);
    let mut last_tick = Instant::now();
    let mut ms_remainder_in_ns = 0;
    let mut has_pending_deadlines = false;

    loop {
        if has_pending_deadlines {
            // Tick every 1ms until no pending deadlines
            let now = Instant::now();
            let tick_result = count_ms_elapsed(last_tick, now, ms_remainder_in_ns);
            ms_remainder_in_ns = tick_result.ms_remainder_in_ns;
            last_tick = tick_result.last_tick;

            if tick_result.ms_elapsed >= 1 {
                has_pending_deadlines = algorithm.tick(&out_tx, now);
            }

            // Non-blocking check for new events
            match rx.try_recv() {
                Ok(kev) => {
                    log::debug!("Received event: {:?}", kev);
                    has_pending_deadlines = algorithm.process_event(kev, &out_tx);
                }
                Err(TryRecvError::Empty) => {
                    // No events available, wait a small duration to prevent busy looping
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        } else {
            // No pending deadlines, block until a new event arrives
            match rx.recv() {
                Ok(kev) => {
                    log::debug!("Received event: {:?}", kev);
                    has_pending_deadlines = algorithm.process_event(kev, &out_tx);
                }
                Err(_) => return Ok(()),
            }
        }

        for kev in out_rx.try_iter() {
            forward(kev)?;
        }
    }
}

#[cfg(test)]
mod debouncer_tests {
    use super::*;
    use kanata_parser::cfg::debounce_algorithm::DebounceAlgorithm;

    /// Chatter sent through the debounce thread comes out as one clean press and release,
    /// with the release emitted by the thread's own ticking.
    #[test]
    fn chatter_is_removed_before_forwarding() {
        let (tx, rx) = std::sync::mpsc::sync_channel(100);
        let (out_tx, out_rx) = std::sync::mpsc::channel();
        let algorithm = create_debounce_algorithm(DebounceAlgorithm::AsymEagerDeferPk, 20);
        let handle = std::thread::spawn(move || {
            run_debouncer(algorithm, rx, |kev| {
                out_tx.send((kev.code, kev.value)).unwrap();
                Ok(())
            })
        });

        for value in [
            KeyValue::Press,
            KeyValue::Release,
            KeyValue::Press,
            KeyValue::Release,
        ] {
            tx.send(KeyEvent::new(OsCode::KEY_A, value)).unwrap();
        }
        std::thread::sleep(Duration::from_millis(100));
        drop(tx);
        handle.join().unwrap().unwrap();

        let out: Vec<_> = out_rx.try_iter().collect();
        assert_eq!(
            out,
            vec![
                (OsCode::KEY_A, KeyValue::Press),
                (OsCode::KEY_A, KeyValue::Release)
            ]
        );
    }
}
