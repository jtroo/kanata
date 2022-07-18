//! Implements the glue between OS input/output and keyberon state management.

use anyhow::{bail, Result};
use log::{error, info};

use crossbeam_channel::{Receiver, Sender, TryRecvError};
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::time;

use parking_lot::Mutex;
use std::sync::Arc;

use crate::custom_action::*;
use crate::keys::*;
use crate::oskbd::*;
use crate::{cfg, ValidatedArgs};

use kanata_keyberon::key_code::*;
use kanata_keyberon::layout::*;

#[derive(Debug, Serialize)]
pub enum EventNotification {
    LayerChange { old: String, new: String },
}

impl EventNotification {
    pub fn as_bytes(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_string(self)?.as_bytes().to_vec())
    }
}

pub struct NotificationServer {
    pub port: i32,
    pub connections: Arc<Mutex<HashMap<String, TcpStream>>>,
}

impl NotificationServer {
    pub fn new(port: i32) -> Self {
        let server = Self {
            port,
            connections: Arc::new(Mutex::new(HashMap::new())),
        };

        server
    }

    pub fn start(&mut self) {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.port))
            .expect("Could not start the server");

        let cl = self.connections.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        let addr = stream
                            .peer_addr()
                            .expect("could not find peer address")
                            .to_string();

                        cl.lock().insert(addr, stream);
                    }
                    Err(_) => log::error!("not able to accept client connection"),
                }
            }
        });
    }
}

pub struct Kanata {
    pub kbd_in_path: PathBuf,
    pub kbd_out: KbdOut,
    pub cfg_path: PathBuf,
    pub mapped_keys: [bool; cfg::MAPPED_KEYS_LEN],
    pub key_outputs: cfg::KeyOutputs,
    pub layout: cfg::KanataLayout,
    pub prev_keys: Vec<KeyCode>,
    pub layer_strings: Vec<String>,
    pub layer_names: Vec<String>,
    pub prev_layer: usize,
    pub server: NotificationServer,
    last_tick: time::Instant,
}

use once_cell::sync::Lazy;
use serde::Serialize;

static MAPPED_KEYS: Lazy<Mutex<cfg::MappedKeys>> = Lazy::new(|| Mutex::new([false; 256]));

#[cfg(target_os = "windows")]
static PRESSED_KEYS: Lazy<Mutex<HashSet<OsCode>>> = Lazy::new(|| Mutex::new(HashSet::new()));

impl Kanata {
    /// Create a new configuration from a file.
    pub fn new(args: ValidatedArgs) -> Result<Self> {
        let cfg = cfg::Cfg::new_from_file(&args.path)?;

        let kbd_out = match KbdOut::new() {
            Ok(kbd_out) => kbd_out,
            Err(err) => {
                error!("Failed to open the output uinput device. Make sure you've added kanata to the `uinput` group");
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

        let mut server = NotificationServer::new(args.port);
        server.start();
        Ok(Self {
            kbd_in_path,
            kbd_out,
            cfg_path: args.path,
            mapped_keys: cfg.mapped_keys,
            key_outputs: cfg.key_outputs,
            layout: cfg.layout,
            layer_strings: cfg.layer_strings,
            layer_names: cfg.layer_names,
            prev_keys: Vec::new(),
            prev_layer: 0,
            server,
            last_tick: time::Instant::now(),
        })
    }

    /// Create a new configuration from a file, wrapped in an Arc<Mutex<_>>
    pub fn new_arc(args: ValidatedArgs) -> Result<Arc<Mutex<Self>>> {
        Ok(Arc::new(Mutex::new(Self::new(args)?)))
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

        let mut live_reload_requested = false;

        for _ in 0..ms_elapsed {
            live_reload_requested |= self.tick_handle_custom_event()?;

            let cur_keys = self.handle_keystate_changes()?;

            if live_reload_requested && self.prev_keys.is_empty() && cur_keys.is_empty() {
                live_reload_requested = false;
                self.do_reload();
            }

            self.prev_keys = cur_keys;
        }

        if ms_elapsed > 0 {
            self.last_tick = now;

            // Handle layer change outside the loop. I don't see any practical scenario where it
            // would make a difference, so may as well reduce the amount of processing.
            self.check_handle_layer_change();
        }

        Ok(())
    }

    /// Returns true if live reload is requested and false otherwise.
    fn tick_handle_custom_event(&mut self) -> Result<bool> {
        let mut live_reload_requested = false;
        match self.layout.tick() {
            CustomEvent::Press(custact) => match custact {
                // For unicode, only send on the press. No repeat action is supported for this for
                // now.
                CustomAction::Unicode(c) => self.kbd_out.send_unicode(*c)?,
                CustomAction::MultiUnicode(chars) => {
                    for c in chars.iter() {
                        self.kbd_out.send_unicode(*c)?;
                    }
                }
                CustomAction::LiveReload => {
                    live_reload_requested = true;
                    log::info!("Requested live reload")
                }
                CustomAction::Mouse(btn) => {
                    log::debug!("press     {:?}", btn);
                    self.kbd_out.click_btn(*btn)?;
                }
                CustomAction::MultiMouse(btns) => {
                    assert!(!btns.is_empty());
                    for i in 0..btns.len() - 1 {
                        let btn = btns[i];
                        log::debug!("press     {:?}", btn);
                        self.kbd_out.click_btn(btn)?;
                        log::debug!("release   {:?}", btn);
                        self.kbd_out.release_btn(btn)?;
                    }
                    let btn = btns[btns.len() - 1];
                    log::debug!("press     {:?}", btn);
                    self.kbd_out.click_btn(btn)?;
                }
                CustomAction::Cmd(cmd) => {
                    run_cmd(cmd);
                }
                CustomAction::MultiCmd(cmds) => {
                    run_multi_cmd(cmds);
                }
            },
            CustomEvent::Release(CustomAction::Mouse(btn)) => {
                log::debug!("release   {:?}", btn);
                self.kbd_out.release_btn(*btn)?;
            }
            CustomEvent::Release(CustomAction::MultiMouse(btns)) => {
                assert!(!btns.is_empty());
                let btn = btns[btns.len() - 1];
                log::debug!("release   {:?}", btn);
                self.kbd_out.release_btn(btn)?;
            }
            _ => {}
        };
        Ok(live_reload_requested)
    }

    /// Sends OS key events according to the change in key state between the current and the
    /// previous keyberon keystate. Returns the current keys.
    fn handle_keystate_changes(&mut self) -> Result<Vec<KeyCode>> {
        let cur_keys: Vec<KeyCode> = self.layout.keycodes().collect();
        // Release keys that are missing from the current state but exist in the previous
        // state. It's important to iterate using a Vec because the order matters. This used to
        // use HashSet force computing `difference` but that iteration order is random which is
        // not what we want.
        for k in &self.prev_keys {
            if cur_keys.contains(k) {
                continue;
            }
            log::debug!("release   {:?}", k);
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
            log::debug!("press     {:?}", k);
            if let Err(e) = self.kbd_out.press_key(k.into()) {
                bail!("failed to press key: {:?}", e);
            }
        }
        Ok(cur_keys)
    }

    fn do_reload(&mut self) {
        match cfg::Cfg::new_from_file(&self.cfg_path) {
            Err(e) => {
                log::error!("Could not reload configuration:\n{}", e);
            }
            Ok(cfg) => {
                self.layout = cfg.layout;
                let mut mapped_keys = MAPPED_KEYS.lock();
                *mapped_keys = cfg.mapped_keys;
                self.key_outputs = cfg.key_outputs;
                self.layer_strings = cfg.layer_strings;
                log::info!("Live reload successful")
            }
        };
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
            log::debug!("repeat    {:?}", KeyCode::from(*kc));
            if let Err(e) = self.kbd_out.write_key(*kc, KeyValue::Repeat) {
                bail!("could not write key {:?}", e)
            }
        }
        Ok(())
    }

    fn check_handle_layer_change(&mut self) {
        let cur_layer = self.layout.current_layer();
        if cur_layer != self.prev_layer {
            let old = &self.layer_names[self.prev_layer];
            let new = &self.layer_names[cur_layer];
            self.prev_layer = cur_layer;
            self.print_layer(cur_layer);

            let raw_notification = EventNotification::LayerChange {
                old: old.clone(),
                new: new.clone(),
            };

            let notification = match raw_notification.as_bytes() {
                Ok(serialized_notification) => serialized_notification,
                Err(error) => {
                    log::warn!("failed to serialize layer change notification: {}", error);
                    return;
                }
            };

            let mut clients = self.server.connections.lock();
            let mut stale_clients = vec![];
            for (id, client) in &mut *clients {
                match client.write(&notification) {
                    Ok(_) => {
                        log::debug!("layer change notification sent");
                    }
                    Err(_) => {
                        // the client is no longer connected, let's remove them
                        stale_clients.push(id.clone());
                        log::debug!("removing disconnected notification client");
                    }
                }
            }

            for id in &stale_clients {
                clients.remove(id);
            }
        }
    }

    fn print_layer(&self, layer: usize) {
        log::info!("Entered layer:\n{}", self.layer_strings[layer]);
    }

    /// Starts a new thread that processes OS key events and advances the keyberon layout's state.
    pub fn start_processing_loop(kanata: Arc<Mutex<Self>>, rx: Receiver<KeyEvent>) {
        info!("Kanata: entering the processing loop");
        std::thread::spawn(move || {
            info!("Init: catching only releases and sending immediately");
            for _ in 0..500 {
                if let Ok(kev) = rx.try_recv() {
                    if kev.value == KeyValue::Release {
                        let mut k = kanata.lock();
                        info!("Init: releasing {:?}", kev.code);
                        k.kbd_out
                            .release_key(kev.code)
                            .expect("could not release key");
                    }
                }
                std::thread::sleep(time::Duration::from_millis(1));
            }

            info!("Starting kanata proper");
            let err = loop {
                match rx.try_recv() {
                    Ok(kev) => {
                        let mut k = kanata.lock();
                        if let Err(e) = k.handle_key_event(&kev) {
                            break e;
                        }
                        if let Err(e) = k.handle_time_ticks() {
                            break e;
                        }
                    }
                    Err(TryRecvError::Empty) => {
                        if let Err(e) = kanata.lock().handle_time_ticks() {
                            break e;
                        }
                        std::thread::sleep(time::Duration::from_millis(1));
                    }
                    Err(TryRecvError::Disconnected) => {
                        panic!("channel disconnected")
                    }
                }
            };
            panic!("processing loop encountered error {:?}", err)
        });
    }

    /// Enter an infinite loop that listens for OS key events and sends them to the processing
    /// thread.
    #[cfg(target_os = "linux")]
    pub fn event_loop(kanata: Arc<Mutex<Self>>, tx: Sender<KeyEvent>) -> Result<()> {
        info!("Kanata: entering the event loop");
        {
            let mut mapped_keys = MAPPED_KEYS.lock();
            *mapped_keys = kanata.lock().mapped_keys;
        }

        let kbd_in = match KbdIn::new(&kanata.lock().kbd_in_path) {
            Ok(kbd_in) => kbd_in,
            Err(e) => {
                bail!("failed to open keyboard device: {}", e)
            }
        };

        loop {
            let in_event = kbd_in.read()?;

            // Pass-through non-key events
            let key_event = match KeyEvent::try_from(in_event.clone()) {
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

    /// Initialize the callback that is passed to the Windows low level hook to receive key events
    /// and run the native_windows_gui event loop.
    #[cfg(target_os = "windows")]
    pub fn event_loop(kanata: Arc<Mutex<Self>>, tx: Sender<KeyEvent>) -> Result<()> {
        // Display debug and panic output when launched from a terminal.
        unsafe {
            use winapi::um::wincon::*;
            if AttachConsole(ATTACH_PARENT_PROCESS) != 0 {
                panic!("Could not attach to console");
            }
        };
        native_windows_gui::init()?;
        {
            let mut mapped_keys = MAPPED_KEYS.lock();
            *mapped_keys = kanata.lock().mapped_keys;
        }

        // This callback should return `false` if the input event is **not** handled by the
        // callback and `true` if the input event **is** handled by the callback. Returning false
        // informs the callback caller that the input event should be handed back to the OS for
        // normal processing.
        let _kbhook = KeyboardHook::set_input_cb(move |input_event| {
            if input_event.code as usize >= cfg::MAPPED_KEYS_LEN {
                return false;
            }
            if !MAPPED_KEYS.lock()[input_event.code as usize] {
                return false;
            }

            let mut key_event = match KeyEvent::try_from(input_event) {
                Ok(ev) => ev,
                _ => return false,
            };

            // Unlike Linux, Windows does not use a separate value for repeat. However, our code
            // needs to differentiate between initial press and repeat press.
            log::debug!("event loop: {:?}", key_event);
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

            // Send input_events to the processing loop. Panic if channel somehow gets full or if
            // channel disconnects. Typing input should never trigger a panic based on the channel
            // getting full, assuming regular operation of the program and some other bug isn't the
            // problem. I've tried to crash the program by pressing as many keys on my keyboard at
            // the same time as I could, but was unable to.
            if let Err(e) = tx.try_send(key_event) {
                panic!("failed to send on channel: {:?}", e)
            }
            true
        });

        // The event loop is also required for the low-level keyboard hook to work.
        native_windows_gui::dispatch_thread_events();
        Ok(())
    }
}

#[cfg(feature = "cmd")]
fn run_cmd(cmd_and_args: &'static [String]) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut args = cmd_and_args.iter().cloned();
        let mut cmd = std::process::Command::new(
            args.next()
                .expect("Parsing should have forbidden empty cmd"),
        );
        for arg in args {
            cmd.arg(arg);
        }
        match cmd.output() {
            Ok(output) => {
                log::info!(
                    "Successfully ran cmd {}\nstdout:\n{}\nstderr:\n{}",
                    {
                        let mut printable_cmd = Vec::new();
                        printable_cmd.push(format!("{:?}", cmd.get_program()));
                        let printable_cmd = cmd.get_args().fold(printable_cmd, |mut cmd, arg| {
                            cmd.push(format!("{:?}", arg));
                            cmd
                        });
                        printable_cmd.join(" ")
                    },
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            Err(e) => log::error!("Failed to execute cmd: {}", e),
        };
    })
}

#[cfg(feature = "cmd")]
fn run_multi_cmd(cmds: &'static [&'static [String]]) {
    let cmds = cmds.clone();
    std::thread::spawn(move || {
        for cmd in cmds {
            if let Err(e) = run_cmd(cmd).join() {
                log::error!("problem joining thread {:?}", e);
            }
        }
    });
}

#[cfg(not(feature = "cmd"))]
fn run_cmd(_cmd_and_args: &[String]) {}

#[cfg(not(feature = "cmd"))]
fn run_multi_cmd(_cmds: &'static [&'static [String]]) {}
