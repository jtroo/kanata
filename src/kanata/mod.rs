//! Implements the glue between OS input/output and keyberon state management.

use anyhow::{bail, Result};
use log::{error, info};

use crossbeam_channel::{Receiver, Sender, TryRecvError};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::net::TcpStream;
use std::path::PathBuf;
use std::time;

use parking_lot::Mutex;
use std::sync::Arc;

use crate::cfg::LayerInfo;
use crate::custom_action::*;
use crate::keys::*;
use crate::oskbd::*;
use crate::tcp_server::ServerMessage;
use crate::{cfg, ValidatedArgs};

use kanata_keyberon::key_code::*;
use kanata_keyberon::layout::*;

pub struct Kanata {
    pub kbd_in_paths: String,
    pub kbd_out: KbdOut,
    pub cfg_path: PathBuf,
    pub mapped_keys: [bool; cfg::MAPPED_KEYS_LEN],
    pub key_outputs: cfg::KeyOutputs,
    pub layout: cfg::KanataLayout,
    pub prev_keys: Vec<KeyCode>,
    pub layer_info: Vec<LayerInfo>,
    pub prev_layer: usize,
    last_tick: time::Instant,
}

use once_cell::sync::Lazy;

static MAPPED_KEYS: Lazy<Mutex<cfg::MappedKeys>> = Lazy::new(|| Mutex::new([false; 256]));

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::*;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

impl Kanata {
    /// Create a new configuration from a file.
    pub fn new(args: &ValidatedArgs) -> Result<Self> {
        let cfg = cfg::Cfg::new_from_file(&args.path)?;

        let kbd_out = match KbdOut::new(
            #[cfg(target_os = "linux")]
            &args.symlink_path,
        ) {
            Ok(kbd_out) => kbd_out,
            Err(err) => {
                error!("Failed to open the output uinput device. Make sure you've added kanata to the `uinput` group");
                bail!(err)
            }
        };

        #[cfg(target_os = "linux")]
        let kbd_in_paths = cfg
            .items
            .get("linux-dev")
            .cloned()
            .expect("linux-dev required in defcfg");
        #[cfg(target_os = "windows")]
        let kbd_in_paths = "unused".into();

        #[cfg(target_os = "windows")]
        unsafe {
            log::info!("Asking Windows to improve timer precision");
            if winapi::um::timeapi::timeBeginPeriod(1) == winapi::um::mmsystem::TIMERR_NOCANDO {
                bail!("failed to improve timer precision");
            }
        }

        set_altgr_behaviour(&cfg)?;

        Ok(Self {
            kbd_in_paths,
            kbd_out,
            cfg_path: args.path.clone(),
            mapped_keys: cfg.mapped_keys,
            key_outputs: cfg.key_outputs,
            layout: cfg.layout,
            layer_info: cfg.layer_info,
            prev_keys: Vec::new(),
            prev_layer: 0,
            last_tick: time::Instant::now(),
        })
    }

    /// Create a new configuration from a file, wrapped in an Arc<Mutex<_>>
    pub fn new_arc(args: &ValidatedArgs) -> Result<Arc<Mutex<Self>>> {
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
    fn handle_time_ticks(&mut self, tx: &Option<Sender<ServerMessage>>) -> Result<()> {
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
            self.check_handle_layer_change(tx);
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
                if let Err(e) = set_altgr_behaviour(&cfg) {
                    log::error!("{}", e);
                    return;
                }
                self.layout = cfg.layout;
                let mut mapped_keys = MAPPED_KEYS.lock();
                *mapped_keys = cfg.mapped_keys;
                self.key_outputs = cfg.key_outputs;
                self.layer_info = cfg.layer_info;
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

    pub fn change_layer(&mut self, layer_name: String) {
        for (i, l) in self.layer_info.iter().enumerate() {
            if l.name == layer_name {
                self.layout.set_default_layer(i);
                return;
            }
        }
    }

    fn check_handle_layer_change(&mut self, tx: &Option<Sender<ServerMessage>>) {
        let cur_layer = self.layout.current_layer();
        if cur_layer != self.prev_layer {
            let new = self.layer_info[cur_layer].name.clone();
            self.prev_layer = cur_layer;
            self.print_layer(cur_layer);

            if let Some(tx) = tx {
                match tx.try_send(ServerMessage::LayerChange { new }) {
                    Ok(_) => {}
                    Err(error) => {
                        log::error!("could not send event notification: {}", error);
                    }
                }
            }
        }
    }

    fn print_layer(&self, layer: usize) {
        log::info!("Entered layer:\n{}", self.layer_info[layer].cfg_text);
    }

    pub fn start_notification_loop(
        rx: Receiver<ServerMessage>,
        clients: Arc<Mutex<HashMap<String, TcpStream>>>,
    ) {
        info!("Kanata: listening for event notifications to relay to connected clients");
        std::thread::spawn(move || {
            loop {
                match rx.recv() {
                    Err(_) => {
                        panic!("channel disconnected")
                    }
                    Ok(event) => {
                        let notification = match event.as_bytes() {
                            Ok(serialized_notification) => serialized_notification,
                            Err(error) => {
                                log::warn!(
                                    "failed to serialize layer change notification: {}",
                                    error
                                );
                                return;
                            }
                        };

                        let mut clients = clients.lock();
                        let mut stale_clients = vec![];
                        for (id, client) in &mut *clients {
                            match client.write(&notification) {
                                Ok(_) => {
                                    log::debug!("layer change notification sent");
                                }
                                Err(_) => {
                                    // the client is no longer connected, let's remove them
                                    stale_clients.push(id.clone());
                                }
                            }
                        }

                        for id in &stale_clients {
                            log::warn!("removing disconnected tcp client: {id}");
                            clients.remove(id);
                        }
                    }
                }
            }
        });
    }

    /// Starts a new thread that processes OS key events and advances the keyberon layout's state.
    pub fn start_processing_loop(
        kanata: Arc<Mutex<Self>>,
        rx: Receiver<KeyEvent>,
        tx: Option<Sender<ServerMessage>>,
    ) {
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
                        if let Err(e) = k.handle_time_ticks(&tx) {
                            break e;
                        }
                    }
                    Err(TryRecvError::Empty) => {
                        if let Err(e) = kanata.lock().handle_time_ticks(&tx) {
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
}

fn set_altgr_behaviour(_cfg: &cfg::Cfg) -> Result<()> {
    #[cfg(target_os = "windows")]
    set_win_altgr_behaviour(_cfg)?;
    Ok(())
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
    let cmds = <&[&[String]]>::clone(&cmds);
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
