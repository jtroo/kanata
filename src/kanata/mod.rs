//! Implements the glue between OS input/output and keyberon state management.

use anyhow::{anyhow, bail, Result};
use crossbeam_channel::{Receiver, Sender, TryRecvError};
use log::{error, info};
use parking_lot::Mutex;

use kanata_keyberon::key_code::*;
use kanata_keyberon::layout::*;

use std::collections::VecDeque;
use std::io::Write;
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::SeqCst};
use std::sync::Arc;
use std::time;

use crate::cfg::LayerInfo;
use crate::custom_action::*;
use crate::keys::*;
use crate::oskbd::*;
use crate::tcp_server::ServerMessage;
use crate::{cfg, ValidatedArgs};

type HashSet<T> = rustc_hash::FxHashSet<T>;
type HashMap<K, V> = rustc_hash::FxHashMap<K, V>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DynamicMacroItem {
    Press(OsCode),
    Release(OsCode),
    EndMacro(u16),
}

pub struct Kanata {
    pub kbd_in_paths: Vec<String>,
    pub kbd_out: KbdOut,
    pub cfg_path: PathBuf,
    pub mapped_keys: cfg::MappedKeys,
    pub key_outputs: cfg::KeyOutputs,
    pub layout: cfg::KanataLayout,
    pub cur_keys: Vec<KeyCode>,
    pub prev_keys: Vec<KeyCode>,
    pub layer_info: Vec<LayerInfo>,
    pub prev_layer: usize,
    pub scroll_state: Option<ScrollState>,
    pub hscroll_state: Option<ScrollState>,
    pub move_mouse_state_vertical: Option<MoveMouseState>,
    pub move_mouse_state_horizontal: Option<MoveMouseState>,
    pub sequence_timeout: u16,
    pub sequence_state: Option<SequenceState>,
    pub sequences: cfg::KeySeqsToFKeys,
    pub sequence_input_mode: SequenceInputMode,
    pub dynamic_macros: HashMap<u16, Vec<DynamicMacroItem>>,
    pub dynamic_macro_replay_state: Option<DynamicMacroReplayState>,
    pub dynamic_macro_record_state: Option<DynamicMacroRecordState>,
    last_tick: time::Instant,
    live_reload_requested: bool,
    #[cfg(target_os = "linux")]
    continue_if_no_devices: bool,
    #[cfg(all(feature = "interception_driver", target_os = "windows"))]
    kbd_out_rx: Receiver<(bool, InputEvent)>,
    #[cfg(all(feature = "interception_driver", target_os = "windows"))]
    intercept_mouse_hwid: Option<Vec<u8>>,
}

pub struct ScrollState {
    pub direction: MWheelDirection,
    pub interval: u16,
    pub ticks_until_scroll: u16,
    pub distance: u16,
}

pub struct MoveMouseState {
    pub direction: MoveDirection,
    pub interval: u16,
    pub ticks_until_move: u16,
    pub distance: u16,
    pub move_mouse_accel_state: Option<MoveMouseAccelState>,
}

pub struct MoveMouseAccelState {
    pub accel_ticks_from_min: u16,
    pub accel_ticks_until_max: u16,
    pub accel_increment: f64,
    pub min_distance: u16,
    pub max_distance: u16,
}

pub struct SequenceState {
    pub sequence: Vec<u16>,
    pub ticks_until_timeout: u16,
}

/// This controls the behaviour of kanata when sequence mode is initiated by the sequence leader
/// action.
///
/// - `HiddenSuppressed` hides the keys typed as part of the sequence and does not output the keys
///   typed when an invalid sequence is the result of an invalid sequence character or a timeout.
/// - `HiddenDelayType` hides the keys typed as part of the sequence and outputs the keys when an
///   typed when an invalid sequence is the result of an invalid sequence character or a timeout.
/// - `VisibleBackspaced` will type the keys that are typed as part of the sequence but will
///   backspace the typed sequence keys before performing the fake key tap when a valid sequence is
///   the result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SequenceInputMode {
    HiddenSuppressed,
    HiddenDelayType,
    VisibleBackspaced,
}

const SEQ_INPUT_MODE_CFG_NAME: &str = "sequence-input-mode";
const SEQ_VISIBLE_BACKSPACED: &str = "visible-backspaced";
const SEQ_HIDDEN_SUPPRESSED: &str = "hidden-suppressed";
const SEQ_HIDDEN_DELAY_TYPE: &str = "hidden-delay-type";

impl SequenceInputMode {
    fn try_from_str(s: &str) -> Result<Self> {
        match s {
            SEQ_VISIBLE_BACKSPACED => Ok(SequenceInputMode::VisibleBackspaced),
            SEQ_HIDDEN_SUPPRESSED => Ok(SequenceInputMode::HiddenSuppressed),
            SEQ_HIDDEN_DELAY_TYPE => Ok(SequenceInputMode::HiddenDelayType),
            _ => Err(anyhow!("{SEQ_INPUT_MODE_CFG_NAME} mode must be one of: {SEQ_VISIBLE_BACKSPACED}, {SEQ_HIDDEN_SUPPRESSED}, {SEQ_HIDDEN_DELAY_TYPE}"))
        }
    }
}

pub struct DynamicMacroReplayState {
    pub active_macros: HashSet<u16>,
    pub delay_remaining: u16,
    pub macro_items: VecDeque<DynamicMacroItem>,
}

pub struct DynamicMacroRecordState {
    pub starting_macro_key: u16,
    pub macro_items: Vec<DynamicMacroItem>,
}

static LAST_PRESSED_KEY: AtomicU32 = AtomicU32::new(0);

const SEQUENCE_TIMEOUT_ERR: &str = "sequence-timeout should be a number (1-65535)";
const SEQUENCE_TIMEOUT_DEFAULT: u16 = 1000;

use once_cell::sync::Lazy;

static MAPPED_KEYS: Lazy<Mutex<cfg::MappedKeys>> =
    Lazy::new(|| Mutex::new(cfg::MappedKeys::default()));

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
        let cfg = cfg::new_from_file(&args.path)?;

        #[cfg(all(feature = "interception_driver", target_os = "windows"))]
        let (kbd_out_tx, kbd_out_rx) = crossbeam_channel::unbounded();
        #[cfg(all(feature = "interception_driver", target_os = "windows"))]
        let intercept_mouse_hwid = cfg
            .items
            .get("windows-interception-mouse-hwid")
            .map(|hwid: &String| {
                log::trace!("win hwid: {hwid}");
                hwid.split_whitespace()
                    .try_fold(vec![], |mut hwid_bytes, hwid_byte| {
                        hwid_byte.trim_matches(',').parse::<u8>().map(|b| {
                            hwid_bytes.push(b);
                            hwid_bytes
                        })
                    })
                    .ok()
            })
            .unwrap_or_default();

        let kbd_out = match KbdOut::new(
            #[cfg(target_os = "linux")]
            &args.symlink_path,
            #[cfg(all(feature = "interception_driver", target_os = "windows"))]
            kbd_out_tx,
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
            .map(|paths| parse_dev_paths(&paths))
            .unwrap_or_default();
        #[cfg(not(target_os = "linux"))]
        let kbd_in_paths = vec![];

        #[cfg(target_os = "windows")]
        unsafe {
            log::info!("Asking Windows to improve timer precision");
            if winapi::um::timeapi::timeBeginPeriod(1) == winapi::um::mmsystem::TIMERR_NOCANDO {
                bail!("failed to improve timer precision");
            }
        }

        set_altgr_behaviour(&cfg)?;

        let sequence_timeout = cfg
            .items
            .get("sequence-timeout")
            .map(|s| match str::parse::<u16>(s) {
                Ok(0) | Err(_) => Err(anyhow!("{SEQUENCE_TIMEOUT_ERR}")),
                Ok(t) => Ok(t),
            })
            .unwrap_or(Ok(SEQUENCE_TIMEOUT_DEFAULT))?;
        let sequence_input_mode = cfg
            .items
            .get(SEQ_INPUT_MODE_CFG_NAME)
            .map(|s| SequenceInputMode::try_from_str(s.as_str()))
            .unwrap_or(Ok(SequenceInputMode::HiddenSuppressed))?;

        Ok(Self {
            kbd_in_paths,
            kbd_out,
            cfg_path: args.path.clone(),
            mapped_keys: cfg.mapped_keys,
            key_outputs: cfg.key_outputs,
            layout: cfg.layout,
            layer_info: cfg.layer_info,
            cur_keys: Vec::new(),
            prev_keys: Vec::new(),
            prev_layer: 0,
            scroll_state: None,
            hscroll_state: None,
            move_mouse_state_vertical: None,
            move_mouse_state_horizontal: None,
            sequence_timeout,
            sequence_state: None,
            sequences: cfg.sequences,
            sequence_input_mode,
            last_tick: time::Instant::now(),
            live_reload_requested: false,
            #[cfg(target_os = "linux")]
            continue_if_no_devices: cfg
                .items
                .get("linux-continue-if-no-devs-found")
                .map(|s| matches!(s.to_lowercase().as_str(), "yes" | "true"))
                .unwrap_or_default(),

            #[cfg(all(feature = "interception_driver", target_os = "windows"))]
            kbd_out_rx,
            #[cfg(all(feature = "interception_driver", target_os = "windows"))]
            intercept_mouse_hwid,
            dynamic_macro_replay_state: None,
            dynamic_macro_record_state: None,
            dynamic_macros: Default::default(),
        })
    }

    /// Create a new configuration from a file, wrapped in an Arc<Mutex<_>>
    pub fn new_arc(args: &ValidatedArgs) -> Result<Arc<Mutex<Self>>> {
        Ok(Arc::new(Mutex::new(Self::new(args)?)))
    }

    /// Update keyberon layout state for press/release, handle repeat separately
    fn handle_key_event(&mut self, event: &KeyEvent) -> Result<()> {
        let evc: u16 = event.code.into();
        let kbrn_ev = match event.value {
            KeyValue::Press => {
                if let Some(state) = &mut self.dynamic_macro_record_state {
                    state.macro_items.push(DynamicMacroItem::Press(event.code));
                }
                Event::Press(0, evc)
            }
            KeyValue::Release => {
                if let Some(state) = &mut self.dynamic_macro_record_state {
                    state
                        .macro_items
                        .push(DynamicMacroItem::Release(event.code));
                }
                Event::Release(0, evc)
            }
            KeyValue::Repeat => return self.handle_repeat(event),
        };
        self.layout.bm().event(kbrn_ev);
        Ok(())
    }

    /// Advance keyberon layout state and send events based on changes to its state.
    fn handle_time_ticks(&mut self, tx: &Option<Sender<ServerMessage>>) -> Result<()> {
        let now = time::Instant::now();
        let ms_elapsed = now.duration_since(self.last_tick).as_millis();

        for _ in 0..ms_elapsed {
            self.live_reload_requested = self.handle_keystate_changes()?;
            self.handle_scrolling()?;
            self.handle_move_mouse()?;
            self.tick_sequence_state()?;
            self.tick_dynamic_macro_state()?;

            if self.live_reload_requested && self.prev_keys.is_empty() && self.cur_keys.is_empty() {
                self.live_reload_requested = false;
                if let Err(e) = self.do_live_reload() {
                    log::error!("live reload failed {e}");
                }
            }

            self.prev_keys.clear();
            self.prev_keys.append(&mut self.cur_keys);
        }

        if ms_elapsed > 0 {
            self.last_tick = now;

            // Handle layer change outside the loop. I don't see any practical scenario where it
            // would make a difference, so may as well reduce the amount of processing.
            self.check_handle_layer_change(tx);
        }

        Ok(())
    }

    fn handle_scrolling(&mut self) -> Result<()> {
        if let Some(scroll_state) = &mut self.scroll_state {
            if scroll_state.ticks_until_scroll == 0 {
                scroll_state.ticks_until_scroll = scroll_state.interval - 1;
                self.kbd_out
                    .scroll(scroll_state.direction, scroll_state.distance)?;
            } else {
                scroll_state.ticks_until_scroll -= 1;
            }
        }
        if let Some(hscroll_state) = &mut self.hscroll_state {
            if hscroll_state.ticks_until_scroll == 0 {
                hscroll_state.ticks_until_scroll = hscroll_state.interval - 1;
                self.kbd_out
                    .scroll(hscroll_state.direction, hscroll_state.distance)?;
            } else {
                hscroll_state.ticks_until_scroll -= 1;
            }
        }
        Ok(())
    }

    fn handle_move_mouse(&mut self) -> Result<()> {
        if let Some(mmsv) = &mut self.move_mouse_state_vertical {
            if let Some(mmas) = &mut mmsv.move_mouse_accel_state {
                if mmas.accel_ticks_until_max != 0 {
                    let increment =
                        (mmas.accel_increment * f64::from(mmas.accel_ticks_from_min)) as u16;
                    mmsv.distance = mmas.min_distance + increment;
                    mmas.accel_ticks_from_min += 1;
                    mmas.accel_ticks_until_max -= 1;
                } else {
                    mmsv.distance = mmas.max_distance;
                }
            }
            if mmsv.ticks_until_move == 0 {
                mmsv.ticks_until_move = mmsv.interval - 1;
                self.kbd_out.move_mouse(mmsv.direction, mmsv.distance)?;
            } else {
                mmsv.ticks_until_move -= 1;
            }
        }
        if let Some(mmsh) = &mut self.move_mouse_state_horizontal {
            if let Some(mmas) = &mut mmsh.move_mouse_accel_state {
                if mmas.accel_ticks_until_max != 0 {
                    let increment =
                        (mmas.accel_increment * f64::from(mmas.accel_ticks_from_min)) as u16;
                    mmsh.distance = mmas.min_distance + increment;
                    mmas.accel_ticks_from_min += 1;
                    mmas.accel_ticks_until_max -= 1;
                } else {
                    mmsh.distance = mmas.max_distance;
                }
            }
            if mmsh.ticks_until_move == 0 {
                mmsh.ticks_until_move = mmsh.interval - 1;
                self.kbd_out.move_mouse(mmsh.direction, mmsh.distance)?;
            } else {
                mmsh.ticks_until_move -= 1;
            }
        }
        Ok(())
    }

    fn tick_sequence_state(&mut self) -> Result<()> {
        if let Some(state) = &mut self.sequence_state {
            state.ticks_until_timeout -= 1;
            if state.ticks_until_timeout == 0 {
                log::debug!("sequence timeout; exiting sequence state");
                match self.sequence_input_mode {
                    SequenceInputMode::HiddenDelayType => {
                        for code in state.sequence.iter().copied() {
                            if let Some(osc) = OsCode::from_u16(code) {
                                self.kbd_out.press_key(osc)?;
                                self.kbd_out.release_key(osc)?;
                            }
                        }
                    }
                    SequenceInputMode::HiddenSuppressed | SequenceInputMode::VisibleBackspaced => {}
                }
                self.sequence_state = None;
            }
        }
        Ok(())
    }

    fn tick_dynamic_macro_state(&mut self) -> Result<()> {
        let mut clear_replaying_macro = false;
        if let Some(state) = &mut self.dynamic_macro_replay_state {
            state.delay_remaining = state.delay_remaining.saturating_sub(1);
            if state.delay_remaining == 0 {
                match state.macro_items.pop_front() {
                    None => clear_replaying_macro = true,
                    Some(i) => match i {
                        DynamicMacroItem::Press(k) => {
                            self.layout.bm().event(Event::Press(0, k.into()))
                        }
                        DynamicMacroItem::Release(k) => {
                            self.layout.bm().event(Event::Release(0, k.into()))
                        }
                        DynamicMacroItem::EndMacro(key) => {
                            state.active_macros.remove(&key);
                        }
                    },
                }
                state.delay_remaining = 5;
            }
        }
        if clear_replaying_macro {
            log::debug!("finished macro replay");
            self.dynamic_macro_replay_state = None;
        }
        Ok(())
    }

    /// Sends OS key events according to the change in key state between the current and the
    /// previous keyberon keystate. Also processes any custom actions.
    ///
    /// Updates self.cur_keys.
    ///
    /// Returns whether live reload was requested.
    fn handle_keystate_changes(&mut self) -> Result<bool> {
        let layout = self.layout.bm();
        let custom_event = layout.tick();
        let mut live_reload_requested = false;
        let cur_keys = &mut self.cur_keys;
        cur_keys.extend(layout.keycodes());

        // Release keys that do not in the current state but exist in the previous state. This used
        // to use a HashSet but it was changed to a Vec because the order of operations matters.
        for k in &self.prev_keys {
            if cur_keys.contains(k) {
                continue;
            }
            log::debug!("key release   {:?}", k);
            if let Err(e) = self.kbd_out.release_key(k.into()) {
                bail!("failed to release key: {:?}", e);
            }
        }

        // Press keys that exist in the current state but are missing from the previous state.
        // Comment above regarding Vec/HashSet also applies here.
        for k in cur_keys.iter() {
            log::trace!("{k:?} is pressed");
            if self.prev_keys.contains(k) {
                log::trace!("{k:?} is contained");
                continue;
            }
            LAST_PRESSED_KEY.store(OsCode::from(k).into(), SeqCst);
            match &mut self.sequence_state {
                None => {
                    log::debug!("key press     {:?}", k);
                    if let Err(e) = self.kbd_out.press_key(k.into()) {
                        bail!("failed to press key: {:?}", e);
                    }
                }
                Some(state) => {
                    state.ticks_until_timeout = self.sequence_timeout;
                    let osc = OsCode::from(*k);
                    state.sequence.push(u16::from(osc));
                    match self.sequence_input_mode {
                        SequenceInputMode::VisibleBackspaced => {
                            self.kbd_out.press_key(osc)?;
                        }
                        SequenceInputMode::HiddenSuppressed
                        | SequenceInputMode::HiddenDelayType => {}
                    }
                    log::debug!("sequence got {k:?}");
                    if let Some((x, y)) = self.sequences.get(&state.sequence) {
                        log::debug!("sequence complete; tapping fake key");
                        match self.sequence_input_mode {
                            SequenceInputMode::HiddenSuppressed
                            | SequenceInputMode::HiddenDelayType => {}
                            SequenceInputMode::VisibleBackspaced => {
                                for _ in state.sequence.iter() {
                                    self.kbd_out.press_key(OsCode::KEY_BACKSPACE)?;
                                    self.kbd_out.release_key(OsCode::KEY_BACKSPACE)?;
                                }
                            }
                        }
                        // Make sure to unpress any keys that were pressed as part of the sequence
                        // so that the keyberon internal sequence mechanism can do press+unpress of
                        // them.
                        for k in state.sequence.iter() {
                            layout.states.retain(|s| match s {
                                State::NormalKey { keycode, .. } => {
                                    KeyCode::from(OsCode::from(*k as u32)) != *keycode
                                }
                                _ => true,
                            });
                        }
                        self.sequence_state = None;
                        layout.event(Event::Press(*x, *y));
                        layout.event(Event::Release(*x, *y));
                    } else if self.sequences.get_raw_descendant(&state.sequence).is_none() {
                        log::debug!("got invalid sequence; exiting sequence mode");
                        match self.sequence_input_mode {
                            SequenceInputMode::HiddenDelayType => {
                                for code in state.sequence.iter().copied() {
                                    if let Some(osc) = OsCode::from_u16(code) {
                                        self.kbd_out.press_key(osc)?;
                                        self.kbd_out.release_key(osc)?;
                                    }
                                }
                            }
                            SequenceInputMode::HiddenSuppressed
                            | SequenceInputMode::VisibleBackspaced => {}
                        }
                        self.sequence_state = None;
                    }
                }
            }
        }

        // Handle custom events. This used to be in a separate function but lifetime issues cause
        // it to now be here.
        match custom_event {
            CustomEvent::Press(custacts) => {
                let mut cmds = vec![];
                let mut prev_mouse_btn = None;
                for custact in custacts.iter() {
                    match custact {
                        // For unicode, only send on the press. No repeat action is supported for this for
                        // now.
                        CustomAction::Unicode(c) => self.kbd_out.send_unicode(*c)?,
                        CustomAction::LiveReload => {
                            live_reload_requested = true;
                            log::info!("Requested live reload")
                        }
                        CustomAction::Mouse(btn) => {
                            log::debug!("click     {:?}", btn);
                            if let Some(pbtn) = prev_mouse_btn {
                                log::debug!("unclick   {:?}", pbtn);
                                self.kbd_out.release_btn(pbtn)?;
                            }
                            self.kbd_out.click_btn(*btn)?;
                            prev_mouse_btn = Some(*btn);
                        }
                        CustomAction::MouseTap(btn) => {
                            log::debug!("click     {:?}", btn);
                            self.kbd_out.click_btn(*btn)?;
                            log::debug!("unclick   {:?}", btn);
                            self.kbd_out.release_btn(*btn)?;
                        }
                        CustomAction::MWheel {
                            direction,
                            interval,
                            distance,
                        } => match direction {
                            MWheelDirection::Up | MWheelDirection::Down => {
                                self.scroll_state = Some(ScrollState {
                                    direction: *direction,
                                    distance: *distance,
                                    ticks_until_scroll: 0,
                                    interval: *interval,
                                })
                            }
                            MWheelDirection::Left | MWheelDirection::Right => {
                                self.hscroll_state = Some(ScrollState {
                                    direction: *direction,
                                    distance: *distance,
                                    ticks_until_scroll: 0,
                                    interval: *interval,
                                })
                            }
                        },
                        CustomAction::MoveMouse {
                            direction,
                            interval,
                            distance,
                        } => match direction {
                            MoveDirection::Up | MoveDirection::Down => {
                                self.move_mouse_state_vertical = Some(MoveMouseState {
                                    direction: *direction,
                                    distance: *distance,
                                    ticks_until_move: 0,
                                    interval: *interval,
                                    move_mouse_accel_state: None,
                                })
                            }
                            MoveDirection::Left | MoveDirection::Right => {
                                self.move_mouse_state_horizontal = Some(MoveMouseState {
                                    direction: *direction,
                                    distance: *distance,
                                    ticks_until_move: 0,
                                    interval: *interval,
                                    move_mouse_accel_state: None,
                                })
                            }
                        },
                        CustomAction::MoveMouseAccel {
                            direction,
                            interval,
                            accel_time,
                            min_distance,
                            max_distance,
                        } => {
                            let f_max_distance: f64 = *max_distance as f64;
                            let f_min_distance: f64 = *min_distance as f64;
                            let f_accel_time: f64 = *accel_time as f64;
                            let increment = (f_max_distance - f_min_distance) / f_accel_time;
                            match direction {
                                MoveDirection::Up | MoveDirection::Down => {
                                    self.move_mouse_state_vertical = Some(MoveMouseState {
                                        direction: *direction,
                                        distance: *min_distance,
                                        ticks_until_move: 0,
                                        interval: *interval,
                                        move_mouse_accel_state: Some(MoveMouseAccelState {
                                            accel_ticks_from_min: 0,
                                            accel_ticks_until_max: *accel_time,
                                            accel_increment: increment,
                                            min_distance: *min_distance,
                                            max_distance: *max_distance,
                                        }),
                                    })
                                }
                                MoveDirection::Left | MoveDirection::Right => {
                                    self.move_mouse_state_horizontal = Some(MoveMouseState {
                                        direction: *direction,
                                        distance: *min_distance,
                                        ticks_until_move: 0,
                                        interval: *interval,
                                        move_mouse_accel_state: Some(MoveMouseAccelState {
                                            accel_ticks_from_min: 0,
                                            accel_ticks_until_max: *accel_time,
                                            accel_increment: increment,
                                            min_distance: *min_distance,
                                            max_distance: *max_distance,
                                        }),
                                    })
                                }
                            }
                        }

                        CustomAction::Cmd(cmd) => {
                            cmds.push(cmd.clone());
                        }
                        CustomAction::FakeKey { coord, action } => {
                            let (x, y) = (coord.x, coord.y);
                            log::debug!(
                                "fake key on press   {action:?} {:?},{x:?},{y:?} {:?}",
                                layout.default_layer,
                                layout.layers[layout.default_layer][x as usize][y as usize]
                            );
                            match action {
                                FakeKeyAction::Press => layout.event(Event::Press(x, y)),
                                FakeKeyAction::Release => layout.event(Event::Release(x, y)),
                                FakeKeyAction::Tap => {
                                    layout.event(Event::Press(x, y));
                                    layout.event(Event::Release(x, y));
                                }
                            }
                        }
                        CustomAction::Delay(delay) => {
                            log::debug!("on-press: sleeping for {delay} ms");
                            std::thread::sleep(std::time::Duration::from_millis((*delay).into()));
                        }
                        CustomAction::SequenceLeader => {
                            if self.sequence_state.is_none()
                                && self.sequence_input_mode != SequenceInputMode::HiddenSuppressed
                            {
                                log::debug!("entering sequence mode");
                                self.sequence_state = Some(SequenceState {
                                    sequence: vec![],
                                    ticks_until_timeout: self.sequence_timeout,
                                });
                            }
                        }
                        CustomAction::Repeat => {
                            let key = OsCode::from(LAST_PRESSED_KEY.load(SeqCst));
                            log::debug!("repeating a keypress {key:?}");
                            // Release key in case the most recently pressed key is still pressed.
                            self.kbd_out.release_key(key)?;
                            self.kbd_out.press_key(key)?;
                            self.kbd_out.release_key(key)?;
                        }
                        CustomAction::DynamicMacroRecord(key) => {
                            let mut stop_record = false;
                            let mut new_recording = None;
                            match &mut self.dynamic_macro_record_state {
                                None => {
                                    log::debug!("starting dynamic macro {key} recording");
                                    self.dynamic_macro_record_state =
                                        Some(DynamicMacroRecordState {
                                            starting_macro_key: *key,
                                            macro_items: vec![],
                                        })
                                }
                                Some(state) => {
                                    // remove the last item, since it's almost certainly a "macro
                                    // record" key action which we don't want to keep.
                                    state.macro_items.remove(state.macro_items.len() - 1);
                                    self.dynamic_macros.insert(
                                        state.starting_macro_key,
                                        state.macro_items.clone(),
                                    );
                                    if state.starting_macro_key == *key {
                                        log::debug!(
                                            "same record pressed. saving and stopping dynamic macro {} recording",
                                            state.starting_macro_key
                                        );
                                        stop_record = true;
                                    } else {
                                        log::debug!(
                                            "saving dynamic macro {} recording then starting new macro recording {key}",
                                            state.starting_macro_key,
                                        );
                                        new_recording = Some(key);
                                    }
                                }
                            }
                            if stop_record {
                                self.dynamic_macro_record_state = None;
                            } else if let Some(key) = new_recording {
                                log::debug!("starting new dynamic macro {key} recording");
                                self.dynamic_macro_record_state = Some(DynamicMacroRecordState {
                                    starting_macro_key: *key,
                                    macro_items: vec![],
                                });
                            }
                        }
                        CustomAction::DynamicMacroRecordStop => {
                            if let Some(state) = &mut self.dynamic_macro_record_state {
                                // remove the last item, since it's almost certainly a "macro
                                // record stop" key action which we don't want to keep.
                                state.macro_items.remove(state.macro_items.len() - 1);
                                log::debug!(
                                    "saving and stopping dynamic macro {} recording",
                                    state.starting_macro_key
                                );
                                self.dynamic_macros
                                    .insert(state.starting_macro_key, state.macro_items.clone());
                            }
                            self.dynamic_macro_record_state = None;
                        }
                        CustomAction::DynamicMacroPlay(key) => {
                            match &mut self.dynamic_macro_replay_state {
                                None => {
                                    log::debug!("replaying macro {key}");
                                    self.dynamic_macro_replay_state =
                                        self.dynamic_macros.get(key).map(|macro_items| {
                                            let mut active_macros = HashSet::default();
                                            active_macros.insert(*key);
                                            DynamicMacroReplayState {
                                                active_macros,
                                                delay_remaining: 0,
                                                macro_items: macro_items.clone().into(),
                                            }
                                        });
                                }
                                Some(state) => {
                                    if state.active_macros.contains(key) {
                                        log::warn!("refusing to recurse into macro {key}");
                                    } else if let Some(items) = self.dynamic_macros.get(key) {
                                        log::debug!(
                                            "prepending macro {key} items to current replay"
                                        );
                                        state.active_macros.insert(*key);
                                        state
                                            .macro_items
                                            .push_front(DynamicMacroItem::EndMacro(*key));
                                        for item in items.iter().copied().rev() {
                                            state.macro_items.push_front(item);
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                run_multi_cmd(cmds);
            }
            CustomEvent::Release(custacts) => {
                // Unclick only the last mouse button
                if let Some(Err(e)) = custacts
                    .iter()
                    .fold(None, |pbtn, ac| match ac {
                        CustomAction::Mouse(btn) => Some(btn),
                        CustomAction::MWheel { direction, .. } => {
                            match direction {
                                MWheelDirection::Up | MWheelDirection::Down => {
                                    if let Some(ss) = &self.scroll_state {
                                        if ss.direction == *direction {
                                            self.scroll_state = None;
                                        }
                                    }
                                }
                                MWheelDirection::Left | MWheelDirection::Right => {
                                    if let Some(ss) = &self.hscroll_state {
                                        if ss.direction == *direction {
                                            self.hscroll_state = None;
                                        }
                                    }
                                }
                            }
                            pbtn
                        }
                        CustomAction::MoveMouse { direction, .. } => {
                            match direction {
                                MoveDirection::Up | MoveDirection::Down => {
                                    if let Some(move_mouse_state_vertical) =
                                        &self.move_mouse_state_vertical
                                    {
                                        if move_mouse_state_vertical.direction == *direction {
                                            self.move_mouse_state_vertical = None;
                                        }
                                    }
                                }
                                MoveDirection::Left | MoveDirection::Right => {
                                    if let Some(move_mouse_state_horizontal) =
                                        &self.move_mouse_state_horizontal
                                    {
                                        if move_mouse_state_horizontal.direction == *direction {
                                            self.move_mouse_state_horizontal = None;
                                        }
                                    }
                                }
                            }
                            pbtn
                        }
                        CustomAction::MoveMouseAccel { direction, .. } => {
                            match direction {
                                MoveDirection::Up | MoveDirection::Down => {
                                    if let Some(move_mouse_state_vertical) =
                                        &self.move_mouse_state_vertical
                                    {
                                        if move_mouse_state_vertical.direction == *direction {
                                            self.move_mouse_state_vertical = None;
                                        }
                                    }
                                }
                                MoveDirection::Left | MoveDirection::Right => {
                                    if let Some(move_mouse_state_horizontal) =
                                        &self.move_mouse_state_horizontal
                                    {
                                        if move_mouse_state_horizontal.direction == *direction {
                                            self.move_mouse_state_horizontal = None;
                                        }
                                    }
                                }
                            }
                            pbtn
                        }

                        CustomAction::Delay(delay) => {
                            log::debug!("on-press: sleeping for {delay} ms");
                            std::thread::sleep(std::time::Duration::from_millis((*delay).into()));
                            pbtn
                        }
                        CustomAction::FakeKeyOnRelease { coord, action } => {
                            let (x, y) = (coord.x, coord.y);
                            log::debug!("fake key on release {action:?} {x:?},{y:?}");
                            match action {
                                FakeKeyAction::Press => layout.event(Event::Press(x, y)),
                                FakeKeyAction::Release => layout.event(Event::Release(x, y)),
                                FakeKeyAction::Tap => {
                                    layout.event(Event::Press(x, y));
                                    layout.event(Event::Release(x, y));
                                }
                            }
                            pbtn
                        }
                        CustomAction::CancelMacroOnRelease => {
                            log::debug!("cancelling all macros");
                            layout.active_sequences.clear();
                            layout
                                .states
                                .retain(|s| !matches!(s, State::FakeKey { .. }));
                            pbtn
                        }
                        _ => pbtn,
                    })
                    .map(|btn| {
                        log::debug!("unclick   {:?}", btn);
                        self.kbd_out.release_btn(*btn)
                    })
                {
                    bail!(e);
                }
            }
            _ => {}
        };

        self.check_release_non_physical_shift()?;
        Ok(live_reload_requested)
    }

    fn do_live_reload(&mut self) -> Result<()> {
        let cfg = cfg::new_from_file(&self.cfg_path)?;
        set_altgr_behaviour(&cfg).map_err(|e| anyhow!("failed to set altgr behaviour {e})"))?;
        self.sequence_timeout = cfg
            .items
            .get("sequence-timeout")
            .map(|s| match str::parse::<u16>(s) {
                Ok(0) | Err(_) => Err(anyhow!("{SEQUENCE_TIMEOUT_ERR}")),
                Ok(t) => Ok(t),
            })
            .unwrap_or(Ok(SEQUENCE_TIMEOUT_DEFAULT))?;
        self.sequence_input_mode = cfg
            .items
            .get(SEQ_INPUT_MODE_CFG_NAME)
            .map(|s| SequenceInputMode::try_from_str(s.as_str()))
            .unwrap_or(Ok(SequenceInputMode::HiddenSuppressed))?;
        self.layout = cfg.layout;
        let mut mapped_keys = MAPPED_KEYS.lock();
        *mapped_keys = cfg.mapped_keys;
        self.key_outputs = cfg.key_outputs;
        self.layer_info = cfg.layer_info;
        self.sequences = cfg.sequences;
        log::info!("Live reload successful");
        Ok(())
    }

    /// This compares the active keys in the keyberon layout against the potential key outputs for
    /// corresponding physical key in the configuration. If any of keyberon active keys match any
    /// potential physical key output, write the repeat event to the OS.
    fn handle_repeat(&mut self, event: &KeyEvent) -> Result<()> {
        if self.sequence_state.is_some() {
            // While in sequence mode, don't send key repeats. I can't imagine it's a helpful use
            // case for someone trying to type in a sequence that they want to rely on key repeats
            // to finish a sequence. I suppose one might want to do repeat in order to try and
            // cancel an input sequence... I'll wait for a user created issue to deal with this.
            return Ok(());
        }
        let active_keycodes: HashSet<KeyCode> = self.layout.bm().keycodes().collect();
        let current_layer = self.layout.bm().current_layer();
        if current_layer % 2 == 1 {
            // Prioritize checking the active layer in case a layer-while-held is active.
            if let Some(outputs_for_key) = self.key_outputs[current_layer].get(&event.code) {
                log::debug!("key outs for active layer-while-held: {outputs_for_key:?};");
                for kc in outputs_for_key.iter().rev() {
                    if active_keycodes.contains(&kc.into()) {
                        log::debug!("repeat    {:?}", KeyCode::from(*kc));
                        if let Err(e) = self.kbd_out.write_key(*kc, KeyValue::Repeat) {
                            bail!("could not write key {:?}", e)
                        }
                        return Ok(());
                    }
                }
            } else {
                log::debug!("empty layer-while-held outputs, probably transparent");
            }
        }
        // Try matching a key on the default layer.
        //
        // This code executes in two cases:
        // 1. current layer is the default layer
        // 2. current layer is layer-while-held but did not find a match in the code above, e.g. a
        //    transparent key was pressed.
        let outputs_for_key =
            match self.key_outputs[self.layout.bm().default_layer].get(&event.code) {
                None => return Ok(()),
                Some(v) => v,
            };
        log::debug!("key outs for default layer: {outputs_for_key:?};");
        for kc in outputs_for_key.iter().rev() {
            if active_keycodes.contains(&kc.into()) {
                log::debug!("repeat    {:?}", KeyCode::from(*kc));
                if let Err(e) = self.kbd_out.write_key(*kc, KeyValue::Repeat) {
                    bail!("could not write key {:?}", e)
                }
                return Ok(());
            }
        }
        Ok(())
    }

    pub fn change_layer(&mut self, layer_name: String) {
        for (i, l) in self.layer_info.iter().enumerate() {
            if l.name == layer_name {
                self.layout.bm().set_default_layer(i);
                return;
            }
        }
    }

    fn check_handle_layer_change(&mut self, tx: &Option<Sender<ServerMessage>>) {
        let cur_layer = self.layout.bm().current_layer();
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
        log::info!("Entered layer:\n\n{}", self.layer_info[layer].cfg_text);
    }

    pub fn start_notification_loop(
        rx: Receiver<ServerMessage>,
        clients: Arc<Mutex<HashMap<String, TcpStream>>>,
    ) {
        info!("listening for event notifications to relay to connected clients");
        std::thread::spawn(move || {
            loop {
                match rx.recv() {
                    Err(_) => {
                        panic!("channel disconnected")
                    }
                    Ok(event) => {
                        let notification = event.as_bytes();
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
        info!("entering the processing loop");
        std::thread::spawn(move || {
            info!("Init: catching only releases and sending immediately");
            for _ in 0..500 {
                if let Ok(kev) = rx.try_recv() {
                    if kev.value == KeyValue::Release {
                        let mut k = kanata.lock();
                        info!("Init: releasing {:?}", kev.code);
                        k.kbd_out.release_key(kev.code).expect("key released");
                    }
                }
                std::thread::sleep(time::Duration::from_millis(1));
            }

            info!("Starting kanata proper");
            let err = loop {
                if kanata.lock().can_block() {
                    log::trace!("blocking on channel");
                    #[cfg(all(feature = "interception_driver", target_os = "windows"))]
                    kanata.lock().kbd_out.notify_can_block().unwrap();
                    match rx.recv() {
                        Ok(kev) => {
                            let mut k = kanata.lock();
                            k.last_tick = time::Instant::now()
                                .checked_sub(time::Duration::from_millis(1))
                                .unwrap();

                            #[cfg(feature = "perf_logging")]
                            let start = std::time::Instant::now();

                            if let Err(e) = k.handle_key_event(&kev) {
                                break e;
                            }

                            #[cfg(feature = "perf_logging")]
                            log::info!(
                                "[PERF]: handle key event: {} ns",
                                (start.elapsed()).as_nanos()
                            );
                            #[cfg(feature = "perf_logging")]
                            let start = std::time::Instant::now();

                            if let Err(e) = k.handle_time_ticks(&tx) {
                                break e;
                            }

                            #[cfg(feature = "perf_logging")]
                            log::info!(
                                "[PERF]: handle time ticks: {} ns",
                                (start.elapsed()).as_nanos()
                            );
                        }
                        Err(_) => {
                            log::error!("channel disconnected");
                            return;
                        }
                    }
                } else {
                    let mut k = kanata.lock();
                    match rx.try_recv() {
                        Ok(kev) => {
                            #[cfg(feature = "perf_logging")]
                            let start = std::time::Instant::now();

                            if let Err(e) = k.handle_key_event(&kev) {
                                break e;
                            }

                            #[cfg(feature = "perf_logging")]
                            log::info!(
                                "[PERF]: handle key event: {} ns",
                                (start.elapsed()).as_nanos()
                            );
                            #[cfg(feature = "perf_logging")]
                            let start = std::time::Instant::now();

                            if let Err(e) = k.handle_time_ticks(&tx) {
                                break e;
                            }

                            #[cfg(feature = "perf_logging")]
                            log::info!(
                                "[PERF]: handle time ticks: {} ns",
                                (start.elapsed()).as_nanos()
                            );
                        }
                        Err(TryRecvError::Empty) => {
                            #[cfg(feature = "perf_logging")]
                            let start = std::time::Instant::now();

                            if let Err(e) = k.handle_time_ticks(&tx) {
                                break e;
                            }

                            #[cfg(feature = "perf_logging")]
                            log::info!(
                                "[PERF]: handle time ticks: {} ns",
                                (start.elapsed()).as_nanos()
                            );

                            std::thread::sleep(time::Duration::from_millis(1));
                        }
                        Err(TryRecvError::Disconnected) => {
                            log::error!("channel disconnected");
                            return;
                        }
                    }
                }
            };
            panic!("processing loop encountered error {:?}", err)
        });
    }

    pub fn can_block(&self) -> bool {
        self.layout.b().stacked.is_empty()
            && self.layout.b().waiting.is_none()
            && self.layout.b().last_press_tracker.tap_hold_timeout == 0
            && (self.layout.b().oneshot.timeout == 0 || self.layout.b().oneshot.keys.is_empty())
            && self.layout.b().active_sequences.is_empty()
            && self.layout.b().tap_dance_eager.is_none()
            && self.sequence_state.is_none()
            && self.scroll_state.is_none()
            && self.hscroll_state.is_none()
            && self.move_mouse_state_vertical.is_none()
            && self.move_mouse_state_horizontal.is_none()
            && self.dynamic_macro_replay_state.is_none()
    }
}

fn set_altgr_behaviour(_cfg: &cfg::Cfg) -> Result<()> {
    #[cfg(target_os = "windows")]
    set_win_altgr_behaviour(_cfg)?;
    Ok(())
}

#[cfg(feature = "cmd")]
fn run_cmd(cmd_and_args: Vec<String>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut args = cmd_and_args.iter().cloned();
        let mut cmd = std::process::Command::new(
            args.next()
                .expect("parsing should have forbidden empty cmd"),
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
fn run_multi_cmd(cmds: Vec<Vec<String>>) {
    std::thread::spawn(move || {
        for cmd in cmds {
            if let Err(e) = run_cmd(cmd).join() {
                log::error!("problem joining thread {:?}", e);
            }
        }
    });
}

#[cfg(not(feature = "cmd"))]
fn run_multi_cmd(_cmds: Vec<Vec<String>>) {}

/// Checks if kanata should exit based on the fixed key combination of:
/// Lctl+Spc+Esc
fn check_for_exit(event: &KeyEvent) {
    static IS_LCL_PRESSED: AtomicBool = AtomicBool::new(false);
    static IS_SPC_PRESSED: AtomicBool = AtomicBool::new(false);
    static IS_ESC_PRESSED: AtomicBool = AtomicBool::new(false);
    let is_pressed = match event.value {
        KeyValue::Press => true,
        KeyValue::Release => false,
        _ => return,
    };
    match event.code {
        OsCode::KEY_ESC => IS_ESC_PRESSED.store(is_pressed, SeqCst),
        OsCode::KEY_SPACE => IS_SPC_PRESSED.store(is_pressed, SeqCst),
        OsCode::KEY_LEFTCTRL => IS_LCL_PRESSED.store(is_pressed, SeqCst),
        _ => return,
    }
    const EXIT_MSG: &str = "pressed LControl+Space+Escape, exiting";
    if IS_ESC_PRESSED.load(SeqCst) && IS_SPC_PRESSED.load(SeqCst) && IS_LCL_PRESSED.load(SeqCst) {
        #[cfg(not(target_os = "linux"))]
        {
            log::info!("{EXIT_MSG}");
            panic!("{EXIT_MSG}");
        }
        #[cfg(target_os = "linux")]
        {
            log::info!("{EXIT_MSG}");
            signal_hook::low_level::raise(signal_hook::consts::SIGTERM).unwrap();
        }
    }
}
