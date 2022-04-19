use log::{error, info};

use std::collections::HashSet;
use std::convert::TryFrom;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::time;

use std::sync::Arc;
use std::sync::Mutex;

use crate::cfg;
use crate::keys::*;
use crate::KbdIn;
use crate::KbdOut;

use keyberon::key_code::*;
use keyberon::layout::*;

pub struct KtrlArgs {
    pub kbd_path: PathBuf,
    pub config_path: PathBuf,
}

pub struct Ktrl {
    pub kbd_in_path: PathBuf,
    pub kbd_out: KbdOut,
    pub mapped_keys: [bool; cfg::MAPPED_KEYS_LEN],
    pub key_outputs: cfg::KeyOutputs,
    pub layout: Layout<256, 1, 25>,
    pub prev_keys: HashSet<KeyCode>,
    last_tick: time::Instant,
}

impl Ktrl {
    pub fn new(args: KtrlArgs) -> Result<Self, std::io::Error> {
        let kbd_out = match KbdOut::new() {
            Ok(kbd_out) => kbd_out,
            Err(err) => {
                error!("Failed to open the output uinput device. Make sure you've added ktrl to the `uinput` group");
                return Err(err);
            }
        };

        let mapped_keys = cfg::create_mapped_keys();
        let key_outputs = cfg::create_key_outputs();

        Ok(Self {
            kbd_in_path: args.kbd_path,
            kbd_out,
            mapped_keys,
            key_outputs,
            prev_keys: HashSet::new(),
            layout: cfg::create_layout(),
            last_tick: time::Instant::now(),
        })
    }

    pub fn new_arc(args: KtrlArgs) -> Result<Arc<Mutex<Self>>, std::io::Error> {
        Ok(Arc::new(Mutex::new(Self::new(args)?)))
    }

    fn handle_key_event(&mut self, event: &KeyEvent) -> Result<(), String> {
        info!("handling: {:?}", event);
        let kbrn_ev = match event.value {
            KeyValue::Press => Event::Press(0, event.code as u8),
            KeyValue::Release => Event::Release(0, event.code as u8),
            KeyValue::Repeat => return self.handle_repeat(event),
        };
        // ignore events - handle it when calling tick()
        self.layout.event(kbrn_ev);
        Ok(())
    }

    fn handle_time_ticks(&mut self) {
        let now = time::Instant::now();
        let ms_elapsed = now.duration_since(self.last_tick).as_millis();
        self.last_tick = now;

        for _ in 0..ms_elapsed {
            self.layout.tick();
            let cur_keys: HashSet<KeyCode> = self.layout.keycodes().collect();
            let key_ups = self.prev_keys.difference(&cur_keys);
            let key_downs = cur_keys.difference(&self.prev_keys);
            for kc in key_ups {
                if let Err(e) = self.kbd_out.release_key(kc.into()) {
                    error!("failed to release key: {:?}", e);
                }
            }
            for kc in key_downs {
                if let Err(e) = self.kbd_out.press_key(kc.into()) {
                    error!("failed to press key: {:?}", e);
                }
            }
            self.prev_keys = cur_keys;
        }
    }

    // For a repeat event in the OS input, write key back out to OS if it makes sense to.
    //
    // An example of when it doesn't make sense to write anything to the OS is if it's a HoldTap
    // key being held to toggle a layer.
    //
    // This compares the active keys in the keyberon layout against the potential key outputs for
    // in the configuration. If any of keyberon active keys match any potential configured mapping,
    // write the repeat event to the OS.
    fn handle_repeat(&mut self, event: &KeyEvent) -> Result<(), String> {
        let active_keycodes: HashSet<KeyCode> = self.layout.keycodes().collect();
        let outputs_for_key = match &self.key_outputs[event.code as usize] {
            None => return Ok(()),
            Some(v) => v,
        };
        let mut output = None;
        for valid_output in outputs_for_key {
            if active_keycodes.contains(&valid_output.into()) {
                output = Some(valid_output);
                break;
            }
        }
        if let Some(kc) = output {
            if let Err(e) = self.kbd_out.write_key(*kc, KeyValue::Repeat) {
                return Err(format!("{:?}", e));
            }
        }
        Ok(())
    }

    pub fn start_processing_loop(ktrl: Arc<Mutex<Self>>, rx: Receiver<KeyEvent>) {
        info!("Ktrl: entering the processing loop");
        std::thread::spawn(move || {
            info!("Starting processing loop");
            loop {
                if let Ok(kev) = rx.try_recv() {
                    let mut k = ktrl.lock().unwrap();
                    if let Err(e) = k.handle_key_event(&kev) {
                        error!("Failed to process key event {:?}", e);
                    }
                    k.handle_time_ticks();
                } else {
                    ktrl.lock().unwrap().handle_time_ticks();
                    // Sleep for 1 ms.
                    std::thread::sleep(time::Duration::from_millis(1));
                }
            }
        });
    }

    pub fn event_loop(ktrl: Arc<Mutex<Self>>, tx: Sender<KeyEvent>) -> Result<(), std::io::Error> {
        info!("Ktrl: entering the event loop");

        let (kbd_in, mapped_keys) = {
            let ktrl = ktrl.lock().expect("Failed to lock ktrl (poisoned)");
            let kbd_in = match KbdIn::new(&ktrl.kbd_in_path) {
                Ok(kbd_in) => kbd_in,
                Err(err) => {
                    error!("Failed to open the input keyboard device. Make sure you've added ktrl to the `input` group");
                    return Err(err);
                }
            };
            (kbd_in, ktrl.mapped_keys)
        };

        loop {
            let in_event = kbd_in.read()?;

            // Pass-through non-key events
            let key_event = match KeyEvent::try_from(in_event.clone()) {
                Ok(ev) => ev,
                _ => {
                    let mut ktrl = ktrl.lock().unwrap();
                    ktrl.kbd_out.write(in_event)?;
                    continue;
                }
            };

            // Check if this keycode is mapped in the configuration. If it hasn't been mapped, send
            // it immediately.
            if key_event.code as usize >= cfg::MAPPED_KEYS_LEN
                || !mapped_keys[key_event.code as usize]
            {
                let mut ktrl = ktrl.lock().unwrap();
                ktrl.kbd_out.write_key(key_event.code, key_event.value)?;
                continue;
            }

            // Send key events to the processing loop
            if let Err(e) = tx.send(key_event) {
                error!("Could not send on ch: {}", e.to_string());
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "failed to send on mpsc",
                ));
            }
        }
    }
}
