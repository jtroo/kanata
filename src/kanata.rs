//! Implements the glue between OS input/output and keyberon state management.

use anyhow::{bail, Result};
use log::{error, info};

use crossbeam_channel::{Receiver, Sender, TryRecvError};
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
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

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AltGrBehaviour {
    DoNothing,
    CancelLctlPress,
    AddLctlRelease,
}

pub struct Kanata {
    pub kbd_in_path: PathBuf,
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
static PRESSED_KEYS: Lazy<Mutex<HashSet<OsCode>>> = Lazy::new(|| Mutex::new(HashSet::new()));

#[cfg(target_os = "windows")]
static ALTGR_BEHAVIOUR: Lazy<Mutex<AltGrBehaviour>> =
    Lazy::new(|| Mutex::new(AltGrBehaviour::DoNothing));

impl Kanata {
    /// Create a new configuration from a file.
    pub fn new(args: &ValidatedArgs) -> Result<Self> {
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

        #[cfg(target_os = "windows")]
        unsafe {
            log::info!("Asking Windows to improve timer precision");
            if winapi::um::timeapi::timeBeginPeriod(1) == winapi::um::mmsystem::TIMERR_NOCANDO {
                bail!("failed to improve timer precision");
            }
        }

        set_altgr_behaviour(&cfg)?;

        Ok(Self {
            kbd_in_path,
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
            log::trace!("{in_event:?}");

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

        let (preprocess_tx, preprocess_rx) = crossbeam_channel::bounded(10);
        start_event_preprocessor(preprocess_rx, tx);

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
            try_send_panic(&preprocess_tx, key_event);
            true
        });

        // The event loop is also required for the low-level keyboard hook to work.
        native_windows_gui::dispatch_thread_events();
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn try_send_panic(tx: &Sender<KeyEvent>, kev: KeyEvent) {
    if let Err(e) = tx.try_send(kev) {
        panic!("failed to send on channel: {:?}", e)
    }
}

#[cfg(target_os = "windows")]
fn start_event_preprocessor(preprocess_rx: Receiver<KeyEvent>, process_tx: Sender<KeyEvent>) {
    #[derive(Debug, Clone, Copy, PartialEq)]
    enum LctlState {
        Pressed,
        Released,
        Pending,
        PendingReleased,
        None,
    }

    std::thread::spawn(move || {
        let mut lctl_state = LctlState::None;
        loop {
            match preprocess_rx.try_recv() {
                Ok(kev) => match (*ALTGR_BEHAVIOUR.lock(), kev) {
                    (AltGrBehaviour::DoNothing, _) => try_send_panic(&process_tx, kev),
                    (
                        AltGrBehaviour::AddLctlRelease,
                        KeyEvent {
                            value: KeyValue::Release,
                            code: OsCode::KEY_RIGHTALT,
                            ..
                        },
                    ) => {
                        log::debug!("altgr add: adding lctl release");
                        try_send_panic(&process_tx, kev);
                        try_send_panic(
                            &process_tx,
                            KeyEvent::new(OsCode::KEY_LEFTCTRL, KeyValue::Release),
                        );
                        PRESSED_KEYS.lock().remove(&OsCode::KEY_LEFTCTRL);
                    }
                    (
                        AltGrBehaviour::CancelLctlPress,
                        KeyEvent {
                            value: KeyValue::Press,
                            code: OsCode::KEY_LEFTCTRL,
                            ..
                        },
                    ) => {
                        log::debug!("altgr cancel: lctl state->pressed");
                        lctl_state = LctlState::Pressed;
                    }
                    (
                        AltGrBehaviour::CancelLctlPress,
                        KeyEvent {
                            value: KeyValue::Release,
                            code: OsCode::KEY_LEFTCTRL,
                            ..
                        },
                    ) => match lctl_state {
                        LctlState::Pressed => {
                            log::debug!("altgr cancel: lctl state->released");
                            lctl_state = LctlState::Released;
                        }
                        LctlState::Pending => {
                            log::debug!("altgr cancel: lctl state->pending-released");
                            lctl_state = LctlState::PendingReleased;
                        }
                        LctlState::None => try_send_panic(&process_tx, kev),
                        _ => {}
                    },
                    (
                        AltGrBehaviour::CancelLctlPress,
                        KeyEvent {
                            value: KeyValue::Press,
                            code: OsCode::KEY_RIGHTALT,
                            ..
                        },
                    ) => {
                        log::debug!("altgr cancel: lctl state->none");
                        lctl_state = LctlState::None;
                        try_send_panic(&process_tx, kev);
                    }
                    (_, _) => try_send_panic(&process_tx, kev),
                },
                Err(TryRecvError::Empty) => {
                    if *ALTGR_BEHAVIOUR.lock() == AltGrBehaviour::CancelLctlPress {
                        match lctl_state {
                            LctlState::Pressed => {
                                log::debug!("altgr cancel: lctl state->pending");
                                lctl_state = LctlState::Pending;
                            }
                            LctlState::Released => {
                                log::debug!("altgr cancel: lctl state->pending-released");
                                lctl_state = LctlState::PendingReleased;
                            }
                            LctlState::Pending => {
                                log::debug!("altgr cancel: lctl state->send");
                                try_send_panic(
                                    &process_tx,
                                    KeyEvent::new(OsCode::KEY_LEFTCTRL, KeyValue::Press),
                                );
                                lctl_state = LctlState::None;
                            }
                            LctlState::PendingReleased => {
                                log::debug!("altgr cancel: lctl state->send+release");
                                try_send_panic(
                                    &process_tx,
                                    KeyEvent::new(OsCode::KEY_LEFTCTRL, KeyValue::Press),
                                );
                                try_send_panic(
                                    &process_tx,
                                    KeyEvent::new(OsCode::KEY_LEFTCTRL, KeyValue::Release),
                                );
                                lctl_state = LctlState::None;
                            }
                            _ => {}
                        }
                    }
                    std::thread::sleep(time::Duration::from_millis(1));
                }
                Err(TryRecvError::Disconnected) => {
                    panic!("channel disconnected")
                }
            }
        }
    });
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

fn set_altgr_behaviour(_cfg: &cfg::Cfg) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        *ALTGR_BEHAVIOUR.lock() = {
            const CANCEL: &str = "cancel-lctl-press";
            const ADD: &str = "add-lctl-release";
            match _cfg.items.get("windows-altgr") {
                None => AltGrBehaviour::DoNothing,
                Some(cfg_val) => match cfg_val.as_str() {
                    CANCEL => AltGrBehaviour::CancelLctlPress,
                    ADD => AltGrBehaviour::AddLctlRelease,
                    _ => bail!(
                        "Invalid value for windows-altgr: {}. Valid values are {},{}",
                        cfg_val,
                        CANCEL,
                        ADD
                    ),
                },
            }
        };
    }
    Ok(())
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
