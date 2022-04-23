use anyhow::{bail, Result};
use log::{error, info};

use std::collections::HashSet;
use std::convert::TryFrom;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::time;

use parking_lot::Mutex;
use std::sync::Arc;

use crate::cfg;
use crate::keys::*;
use crate::oskbd::*;

use keyberon::key_code::*;
use keyberon::layout::*;

pub struct Ktrl {
    pub kbd_in_path: PathBuf,
    pub kbd_out: KbdOut,
    pub mapped_keys: [bool; cfg::MAPPED_KEYS_LEN],
    pub key_outputs: cfg::KeyOutputs,
    pub layout: Layout<256, 1, { crate::layers::MAX_LAYERS }>,
    pub prev_keys: Vec<KeyCode>,
    last_tick: time::Instant,
}

#[cfg(target_os = "windows")]
use once_cell::sync::Lazy;
#[cfg(target_os = "windows")]
static PRESSED_KEYS: Lazy<Mutex<HashSet<OsCode>>> = Lazy::new(|| Mutex::new(HashSet::new()));

impl Ktrl {
    /// Create a new configuration from a file.
    pub fn new(cfg: PathBuf) -> Result<Self> {
        let cfg = cfg::Cfg::new_from_file(&cfg)?;

        let kbd_out = match KbdOut::new() {
            Ok(kbd_out) => kbd_out,
            Err(err) => {
                error!("Failed to open the output uinput device. Make sure you've added ktrl to the `uinput` group");
                bail!(err)
            }
        };

        #[cfg(target_os = "linux")]
        let kbd_in_path = cfg
            .items
            .get("linux-dev")
            .expect("linux-dev required in defcfg")
            .into();
        #[cfg(target_os = "windows")]
        let kbd_in_path = "unused".into();

        Ok(Self {
            kbd_in_path,
            kbd_out,
            mapped_keys: cfg.mapped_keys,
            key_outputs: cfg.key_outputs,
            layout: cfg.layout,
            prev_keys: Vec::new(),
            last_tick: time::Instant::now(),
        })
    }

    /// Create a new configuration from a file, wrapped in an Arc<Mutex<_>>
    pub fn new_arc(cfg: PathBuf) -> Result<Arc<Mutex<Self>>> {
        Ok(Arc::new(Mutex::new(Self::new(cfg)?)))
    }

    /// Update keyberon layout state for press/release, handle repeat separately
    fn handle_key_event(&mut self, event: &KeyEvent) -> Result<()> {
        let evc: u32 = event.code.into();
        let kbrn_ev = match event.value {
            KeyValue::Press => Event::Press(0, evc as u8),
            KeyValue::Release => Event::Release(0, evc as u8),
            KeyValue::Repeat => return self.handle_repeat(event),
        };
        self.layout.event(kbrn_ev);
        Ok(())
    }

    /// Advance keyberon layout state and send events based on changes to its state.
    fn handle_time_ticks(&mut self) -> Result<()> {
        let now = time::Instant::now();
        let ms_elapsed = now.duration_since(self.last_tick).as_millis();

        if ms_elapsed > 0 {
            self.last_tick = now;
        }

        for _ in 0..ms_elapsed {
            self.layout.tick();
            let cur_keys: Vec<KeyCode> = self.layout.keycodes().collect();

            // Release keys that are missing from the current state but exist in the previous
            // state. It's important to iterate using a Vec because the order matters. This used to
            // use HashSet force computing `difference` but that iteration order is random which is
            // not what we want.
            for k in &self.prev_keys {
                if cur_keys.contains(k) {
                    continue;
                }
                if let Err(e) = self.kbd_out.release_key(k.into()) {
                    bail!("failed to release key: {:?}", e);
                }
            }
            // Press keys that exist in the current state but are missing from the previous state.
            // Comment above regarding Vec/HashSet also applies here.
            for k in &cur_keys {
                if self.prev_keys.contains(k) {
                    continue;
                }
                if let Err(e) = self.kbd_out.press_key(k.into()) {
                    bail!("failed to press key: {:?}", e);
                }
            }
            self.prev_keys = cur_keys;
        }
        Ok(())
    }

    /// This compares the active keys in the keyberon layout against the potential key outputs for
    /// corresponding physical key in the configuration. If any of keyberon active keys match any
    /// potential physical key output, write the repeat event to the OS.
    fn handle_repeat(&mut self, event: &KeyEvent) -> Result<()> {
        let active_keycodes: HashSet<KeyCode> = self.layout.keycodes().collect();
        let idx: usize = event.code.into();
        let outputs_for_key: &Vec<OsCode> = match &self.key_outputs[idx] {
            None => return Ok(()),
            Some(v) => v,
        };
        let mut output = None;
        for valid_output in outputs_for_key.iter() {
            if active_keycodes.contains(&valid_output.into()) {
                output = Some(valid_output);
                break;
            }
        }
        if let Some(kc) = output {
            if let Err(e) = self.kbd_out.write_key(*kc, KeyValue::Repeat) {
                bail!("could not write key {:?}", e)
            }
        }
        Ok(())
    }

    /// Starts a new thread that processes OS key events and advances the keyberon layout's state.
    pub fn start_processing_loop(ktrl: Arc<Mutex<Self>>, rx: Receiver<KeyEvent>) {
        info!("Ktrl: entering the processing loop");
        std::thread::spawn(move || {
            // This is done to try and work around a weird issue where upon starting ktrl, it seems
            // that enter is being held constantly until any new keycode is sent.
            #[cfg(target_os = "linux")]
            {
                info!("Sending press+release for space repeatedly");
                for _ in 0..1000 {
                    let mut ktrl = ktrl.lock();
                    ktrl.kbd_out.press_key(OsCode::KEY_SPACE).unwrap();
                    ktrl.kbd_out.release_key(OsCode::KEY_SPACE).unwrap();
                    std::thread::sleep(time::Duration::from_millis(1));
                }
            }

            info!("Starting processing loop");
            let err = loop {
                if let Ok(kev) = rx.try_recv() {
                    let mut k = ktrl.lock();
                    if let Err(e) = k.handle_key_event(&kev) {
                        break e;
                    }
                    if let Err(e) = k.handle_time_ticks() {
                        break e;
                    }
                } else {
                    if let Err(e) = ktrl.lock().handle_time_ticks() {
                        break e;
                    }
                    std::thread::sleep(time::Duration::from_millis(1));
                }
            };
            panic!("processing loop encountered error {:?}", err)
        });
    }

    /// Enter an infinite loop that listens for OS key events and sends them to the processing
    /// thread.
    #[cfg(target_os = "linux")]
    pub fn event_loop(ktrl: Arc<Mutex<Self>>, tx: Sender<KeyEvent>) -> Result<()> {
        info!("Ktrl: entering the event loop");

        let (kbd_in, mapped_keys) = {
            let ktrl = ktrl.lock();
            let kbd_in = match KbdIn::new(&ktrl.kbd_in_path) {
                Ok(kbd_in) => kbd_in,
                Err(e) => {
                    bail!("failed to open keyboard device: {}", e)
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
                    let mut ktrl = ktrl.lock();
                    ktrl.kbd_out.write(in_event)?;
                    continue;
                }
            };

            // Check if this keycode is mapped in the configuration. If it hasn't been mapped, send
            // it immediately.
            let kc: usize = key_event.code.into();
            if kc >= cfg::MAPPED_KEYS_LEN || !mapped_keys[kc] {
                let mut ktrl = ktrl.lock();
                ktrl.kbd_out.write_key(key_event.code, key_event.value)?;
                continue;
            }

            // Send key events to the processing loop
            if let Err(e) = tx.send(key_event) {
                bail!("failed to send on mpsc: {}", e)
            }
        }
    }

    #[cfg(target_os = "windows")]
    pub fn event_loop(ktrl: Arc<Mutex<Self>>, tx: Sender<KeyEvent>) -> Result<()> {
        // Display debug and panic output when launched from a terminal.
        unsafe {
            use winapi::um::wincon::*;
            if AttachConsole(ATTACH_PARENT_PROCESS) != 0 {
                panic!("Could not attach to console");
            }
        };
        native_windows_gui::init()?;

        let mapped_keys = {
            let ktrl = ktrl.lock();
            ktrl.mapped_keys
        };

        let _kbhook = KeyboardHook::set_input_cb(move |input_event| {
            if input_event.code as usize >= cfg::MAPPED_KEYS_LEN {
                return false;
            }
            if !mapped_keys[input_event.code as usize] {
                return false;
            }

            let mut key_event = match KeyEvent::try_from(input_event) {
                Ok(ev) => ev,
                _ => return false,
            };

            match key_event.value {
                KeyValue::Release => {
                    PRESSED_KEYS.lock().remove(&key_event.code);
                }
                KeyValue::Press => {
                    if PRESSED_KEYS.lock().contains(&key_event.code) {
                        key_event.value = KeyValue::Repeat;
                    } else {
                        PRESSED_KEYS.lock().insert(key_event.code);
                    }
                }
                _ => {}
            }

            // Send input_events to the processing loop
            if let Err(e) = tx.send(key_event) {
                panic!("failed to send on mpsc: {}", e)
            }
            true
        });

        // The event loop is also required for the low-level keyboard hook to work.
        native_windows_gui::dispatch_thread_events();
        Ok(())
    }
}
