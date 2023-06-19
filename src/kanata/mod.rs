//! Implements the glue between OS input/output and keyberon state management.

use anyhow::{anyhow, bail, Result};
use log::{error, info};
use parking_lot::Mutex;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};

use kanata_keyberon::key_code::*;
use kanata_keyberon::layout::*;

use std::collections::VecDeque;
use std::io::Write;
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::SeqCst};
use std::sync::Arc;
use std::time;

use crate::cfg::*;
use crate::custom_action::*;
use crate::keys::*;
use crate::oskbd::*;
use crate::tcp_server::ServerMessage;
use crate::{cfg, ValidatedArgs};

#[cfg(feature = "cmd")]
mod cmd;
#[cfg(feature = "cmd")]
use cmd::*;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::*;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

mod caps_word;
pub use caps_word::*;

type HashSet<T> = rustc_hash::FxHashSet<T>;
type HashMap<K, V> = rustc_hash::FxHashMap<K, V>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DynamicMacroItem {
    Press(OsCode),
    Release(OsCode),
    EndMacro(u16),
}

pub struct Kanata {
    /// Handle to some OS keyboard output mechanism.
    pub kbd_out: KbdOut,
    /// Paths to one or more configuration files that define kanata's behaviour.
    pub cfg_paths: Vec<PathBuf>,
    /// Index into `cfg_paths`, used to know which file to live reload. Changes when cycling
    /// through the configuration files.
    pub cur_cfg_idx: usize,
    /// The potential key outputs of every key input. Used for managing key repeat.
    pub key_outputs: cfg::KeyOutputs,
    /// Handle to the keyberon library layout.
    pub layout: cfg::KanataLayout,
    /// Reusable vec (to save on allocations) that stores the currently active output keys.
    pub cur_keys: Vec<KeyCode>,
    /// Reusable vec (to save on allocations) that stores the active output keys from the previous
    /// tick.
    pub prev_keys: Vec<KeyCode>,
    /// Used for printing layer info to the info log when changing layers.
    pub layer_info: Vec<LayerInfo>,
    /// Used to track when a layer change occurs.
    pub prev_layer: usize,
    /// Vertical scrolling state tracker. Is Some(...) when a vertical scrolling action is active
    /// and None otherwise.
    pub scroll_state: Option<ScrollState>,
    /// Horizontal scrolling state. Is Some(...) when a horizontal scrolling action is active and
    /// None otherwise.
    pub hscroll_state: Option<ScrollState>,
    /// Vertical mouse movement state. Is Some(...) when vertical mouse movement is active and None
    /// otherwise.
    pub move_mouse_state_vertical: Option<MoveMouseState>,
    /// Horizontal mouse movement state. Is Some(...) when horizontal mouse movement is active and
    /// None otherwise.
    pub move_mouse_state_horizontal: Option<MoveMouseState>,
    /// The number of ticks defined in the user configuration for sequence timeout.
    pub sequence_timeout: u16,
    /// The user configuration for backtracking to find valid sequences. See
    /// <../../docs/sequence-adding-chords-ideas.md> for more info.
    pub sequence_backtrack_modcancel: bool,
    /// Tracks sequence progress. Is Some(...) when in sequence mode and None otherwise.
    pub sequence_state: Option<SequenceState>,
    /// Valid sequences defined in the user configuration.
    pub sequences: cfg::KeySeqsToFKeys,
    /// Stores the user configuration for the sequence input mode.
    pub sequence_input_mode: SequenceInputMode,
    /// Stores the user recored dynamic macros.
    pub dynamic_macros: HashMap<u16, Vec<DynamicMacroItem>>,
    /// Tracks the progress of an active dynamic macro. Is Some(...) when a dynamic macro is being
    /// replayed and None otherwise.
    pub dynamic_macro_replay_state: Option<DynamicMacroReplayState>,
    /// Tracks the the inputs for a dynamic macro recording. Is Some(...) when a dynamic macro is
    /// being recorded and None otherwise.
    pub dynamic_macro_record_state: Option<DynamicMacroRecordState>,
    /// Global overrides defined in the user configuration.
    pub overrides: Overrides,
    /// Reusable allocations to help with computing whether overrides are active based on key
    /// outputs.
    pub override_states: OverrideStates,
    /// Time of the last tick to know how many tick iterations to run, to achieve a 1ms tick
    /// interval more closely.
    last_tick: time::Instant,
    /// Tracks the non-whole-millisecond gaps between ticks to know when to do another tick
    /// iteration without sleeping, to achive a 1ms tick interval more closely.
    time_remainder: u128,
    /// Is true if a live reload was requested by the user and false otherwise.
    live_reload_requested: bool,
    #[cfg(target_os = "linux")]
    /// Linux input paths in the user configuration.
    pub kbd_in_paths: Vec<String>,
    #[cfg(target_os = "linux")]
    /// Tracks the Linux user configuration to continue or abort if no devices are found.
    continue_if_no_devices: bool,
    #[cfg(target_os = "linux")]
    /// Tracks the Linux user configuration for device names (instead of paths) that should be
    /// included for interception and processing by kanata.
    pub include_names: Option<Vec<String>>,
    #[cfg(target_os = "linux")]
    /// Tracks the Linux user configuration for device names (instead of paths) that should be
    /// excluded for interception and processing by kanata.
    pub exclude_names: Option<Vec<String>>,
    #[cfg(all(feature = "interception_driver", target_os = "windows"))]
    /// Used to know which input device to treat as a mouse for intercepting and processing inputs
    /// by kanata.
    intercept_mouse_hwid: Option<Vec<u8>>,
    /// User configuration to do logging of layer changes or not.
    log_layer_changes: bool,
    /// Tracks the caps-word state. Is Some(...) if caps-word is active and None otherwise.
    pub caps_word: Option<CapsWordState>,
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
    pub starting_macro_id: u16,
    pub macro_items: Vec<DynamicMacroItem>,
}

impl DynamicMacroRecordState {
    fn add_release_for_all_unreleased_presses(&mut self) {
        let mut pressed_oscs = HashSet::default();
        for item in self.macro_items.iter() {
            match item {
                DynamicMacroItem::Press(osc) => pressed_oscs.insert(*osc),
                DynamicMacroItem::Release(osc) => pressed_oscs.remove(osc),
                DynamicMacroItem::EndMacro(_) => false,
            };
        }
        // Hopefully release order doesn't matter here since a HashSet is being used
        for osc in pressed_oscs.into_iter() {
            self.macro_items.push(DynamicMacroItem::Release(osc));
        }
    }
}

static LAST_PRESSED_KEY: AtomicU32 = AtomicU32::new(0);

const SEQUENCE_TIMEOUT_ERR: &str = "sequence-timeout should be a number (1-65535)";
const SEQUENCE_TIMEOUT_DEFAULT: u16 = 1000;

use once_cell::sync::Lazy;

static MAPPED_KEYS: Lazy<Mutex<cfg::MappedKeys>> =
    Lazy::new(|| Mutex::new(cfg::MappedKeys::default()));

impl Kanata {
    /// Create a new configuration from a file.
    pub fn new(args: &ValidatedArgs) -> Result<Self> {
        let cfg = match cfg::new_from_file(&args.paths[0]) {
            Ok(c) => c,
            Err(e) => {
                log::error!("{e:?}");
                bail!("failed to parse file");
            }
        };

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
            .map(|paths| parse_colon_separated_text(&paths))
            .unwrap_or_default();
        #[cfg(target_os = "linux")]
        let include_names = cfg
            .items
            .get("linux-dev-names-include")
            .cloned()
            .map(|paths| parse_colon_separated_text(&paths));
        #[cfg(target_os = "linux")]
        let exclude_names = cfg
            .items
            .get("linux-dev-names-exclude")
            .cloned()
            .map(|paths| parse_colon_separated_text(&paths));
        Kanata::set_repeat_rate(&cfg.items)?;

        #[cfg(target_os = "windows")]
        unsafe {
            log::info!("Asking Windows to improve timer precision");
            if winapi::um::timeapi::timeBeginPeriod(1) == winapi::um::mmsystem::TIMERR_NOCANDO {
                bail!("failed to improve timer precision");
            }
        }

        #[cfg(target_os = "windows")]
        unsafe {
            log::info!("Asking Windows to increase process priority");
            winapi::um::processthreadsapi::SetPriorityClass(
                winapi::um::processthreadsapi::GetCurrentProcess(),
                winapi::um::winbase::HIGH_PRIORITY_CLASS,
            );
        }

        update_kbd_out(&cfg.items, &kbd_out)?;
        set_altgr_behaviour(&cfg)?;

        let sequence_timeout = cfg
            .items
            .get("sequence-timeout")
            .map(|s| match str::parse::<u16>(s) {
                Ok(0) | Err(_) => Err(anyhow!("{SEQUENCE_TIMEOUT_ERR}")),
                Ok(t) => Ok(t),
            })
            .unwrap_or(Ok(SEQUENCE_TIMEOUT_DEFAULT))?;
        let sequence_backtrack_modcancel = cfg
            .items
            .get("sequence-backtrack-modcancel")
            .map(|s| !FALSE_VALUES.contains(&s.to_lowercase().as_str()))
            .unwrap_or(true);
        let sequence_input_mode = cfg
            .items
            .get(SEQ_INPUT_MODE_CFG_NAME)
            .map(|s| SequenceInputMode::try_from_str(s.as_str()))
            .unwrap_or(Ok(SequenceInputMode::HiddenSuppressed))?;
        let log_layer_changes = cfg
            .items
            .get("log-layer-changes")
            .map(|s| !FALSE_VALUES.contains(&s.to_lowercase().as_str()))
            .unwrap_or(true);

        *MAPPED_KEYS.lock() = cfg.mapped_keys;

        Ok(Self {
            kbd_out,
            cfg_paths: args.paths.clone(),
            cur_cfg_idx: 0,
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
            sequence_backtrack_modcancel,
            sequence_state: None,
            sequences: cfg.sequences,
            sequence_input_mode,
            last_tick: time::Instant::now(),
            time_remainder: 0,
            live_reload_requested: false,
            overrides: cfg.overrides,
            override_states: OverrideStates::new(),
            #[cfg(target_os = "linux")]
            kbd_in_paths,
            #[cfg(target_os = "linux")]
            continue_if_no_devices: cfg
                .items
                .get("linux-continue-if-no-devs-found")
                .map(|s| TRUE_VALUES.contains(&s.to_lowercase().as_str()))
                .unwrap_or_default(),
            #[cfg(target_os = "linux")]
            include_names,
            #[cfg(target_os = "linux")]
            exclude_names,
            #[cfg(all(feature = "interception_driver", target_os = "windows"))]
            intercept_mouse_hwid,
            dynamic_macro_replay_state: None,
            dynamic_macro_record_state: None,
            dynamic_macros: Default::default(),
            log_layer_changes,
            caps_word: None,
        })
    }

    /// Create a new configuration from a file, wrapped in an Arc<Mutex<_>>
    pub fn new_arc(args: &ValidatedArgs) -> Result<Arc<Mutex<Self>>> {
        Ok(Arc::new(Mutex::new(Self::new(args)?)))
    }

    fn do_live_reload(&mut self) -> Result<()> {
        let cfg = match cfg::new_from_file(&self.cfg_paths[self.cur_cfg_idx]) {
            Ok(c) => c,
            Err(e) => {
                log::error!("{e:?}");
                bail!("failed to parse config file");
            }
        };
        update_kbd_out(&cfg.items, &self.kbd_out)?;
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
        let log_layer_changes = cfg
            .items
            .get("log-layer-changes")
            .map(|s| !FALSE_VALUES.contains(&s.to_lowercase().as_str()))
            .unwrap_or(true);
        self.sequence_backtrack_modcancel = cfg
            .items
            .get("sequence-backtrack-modcancel")
            .map(|s| !FALSE_VALUES.contains(&s.to_lowercase().as_str()))
            .unwrap_or(true);
        self.layout = cfg.layout;
        self.key_outputs = cfg.key_outputs;
        self.layer_info = cfg.layer_info;
        self.sequences = cfg.sequences;
        self.overrides = cfg.overrides;
        self.log_layer_changes = log_layer_changes;
        *MAPPED_KEYS.lock() = cfg.mapped_keys;
        Kanata::set_repeat_rate(&cfg.items)?;
        log::info!("Live reload successful");
        Ok(())
    }

    /// Update keyberon layout state for press/release, handle repeat separately
    fn handle_key_event(&mut self, event: &KeyEvent) -> Result<()> {
        log::debug!("process recv ev {event:?}");
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
            KeyValue::Repeat => {
                let ret = self.handle_repeat(event);
                return ret;
            }
        };
        self.layout.bm().event(kbrn_ev);
        Ok(())
    }

    /// Advance keyberon layout state and send events based on changes to its state.
    fn handle_time_ticks(&mut self, tx: &Option<Sender<ServerMessage>>) -> Result<()> {
        const NS_IN_MS: u128 = 1_000_000;
        let now = time::Instant::now();
        let ns_elapsed = now.duration_since(self.last_tick).as_nanos();
        let ns_elapsed_with_rem = ns_elapsed + self.time_remainder;
        let ms_elapsed = ns_elapsed_with_rem / NS_IN_MS;
        self.time_remainder = ns_elapsed_with_rem % NS_IN_MS;

        for _ in 0..ms_elapsed {
            self.live_reload_requested |= self.handle_keystate_changes()?;
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
            self.last_tick = match ms_elapsed {
                0..=10 => now,
                // If too many ms elapsed, probably doing a tight loop of something that's quite
                // expensive, e.g. click spamming. To avoid a growing ms_elapsed due to trying and
                // failing to catch up, reset last_tick to the "actual now" instead the "past now"
                // even though that means ticks will be missed - meaning there will be fewer than
                // 1000 ticks in 1ms on average. In practice, there will already be fewer than 1000
                // ticks in 1ms when running expensive operations, this just avoids having tens to
                // thousands of ticks all happening as soon as the expensive operations end.
                _ => time::Instant::now(),
            };

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
                        DynamicMacroItem::EndMacro(macro_id) => {
                            state.active_macros.remove(&macro_id);
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
        self.overrides
            .override_keys(cur_keys, &mut self.override_states);
        if let Some(caps_word) = &mut self.caps_word {
            if caps_word.maybe_add_lsft(cur_keys) == CapsWordNextState::End {
                self.caps_word = None;
            }
        }

        // Release keys that do not exist in the current state but exist in the previous state.
        // This used to use a HashSet but it was changed to a Vec because the order of operations
        // matters.
        log::trace!("{:?}", &self.prev_keys);
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
        log::trace!("{cur_keys:?}");
        for k in cur_keys.iter() {
            if self.prev_keys.contains(k) {
                log::trace!("{k:?} is old press");
                continue;
            }
            // Note - keyberon can return duplicates of a key in the keycodes()
            // iterator. Instead of trying to fix it in the keyberon library, It
            // seems better to fix it in the kanata logic. Keyberon iterates over
            // its internal state array with very simple filtering logic when
            // calling keycodes(). It would be troublesome to add deduplication
            // logic there and is easier to add here since we already have
            // allocations and logic.
            self.prev_keys.push(*k);
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

                    // Transform to OsCode and convert modifiers other than altgr/ralt (same key
                    // different names) to the left version, since that's how chords get
                    // transformed when building up sequences.
                    let osc = match OsCode::from(*k) {
                        OsCode::KEY_RIGHTSHIFT => OsCode::KEY_LEFTSHIFT,
                        OsCode::KEY_RIGHTMETA => OsCode::KEY_LEFTMETA,
                        OsCode::KEY_RIGHTCTRL => OsCode::KEY_LEFTCTRL,
                        osc => osc,
                    };

                    // Modify the upper unused bits of the u16 to signify that the key is activated
                    // alongside a modifier.
                    let pushed_into_seq = {
                        let mut base = u16::from(osc);
                        for k in cur_keys.iter().copied() {
                            base |= mod_mask_for_keycode(k);
                        }
                        base
                    };

                    state.sequence.push(pushed_into_seq);
                    match self.sequence_input_mode {
                        SequenceInputMode::VisibleBackspaced => {
                            self.kbd_out.press_key(osc)?;
                        }
                        SequenceInputMode::HiddenSuppressed
                        | SequenceInputMode::HiddenDelayType => {}
                    }
                    log::debug!("sequence got {k:?}");

                    use crate::sequences::*;
                    use crate::trie::GetOrDescendentExistsResult::*;

                    // Check for invalid sequence termination.
                    let mut res = self.sequences.get_or_descendant_exists(&state.sequence);
                    if res == NotInTrie {
                        let is_invalid_termination = if self.sequence_backtrack_modcancel
                            && (pushed_into_seq & MASK_MODDED > 0)
                        {
                            let mut no_valid_seqs = true;
                            // If applicable, check again with modifier bits unset.
                            for i in (0..state.sequence.len()).rev() {
                                // Safety: proper bounds are immediately above.
                                // Note - can't use iter_mut due to borrowing issues.
                                *unsafe { state.sequence.get_unchecked_mut(i) } &= MASK_KEYCODES;
                                res = self.sequences.get_or_descendant_exists(&state.sequence);
                                if res != NotInTrie {
                                    no_valid_seqs = false;
                                    break;
                                }
                            }
                            no_valid_seqs
                        } else {
                            true
                        };
                        if is_invalid_termination {
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
                            continue;
                        }
                    }

                    // Check for and handle valid termination.
                    if let HasValue((i, j)) = res {
                        log::debug!("sequence complete; tapping fake key");
                        match self.sequence_input_mode {
                            SequenceInputMode::HiddenSuppressed
                            | SequenceInputMode::HiddenDelayType => {}
                            SequenceInputMode::VisibleBackspaced => {
                                for k in state.sequence.iter() {
                                    // Check for pressed modifiers and don't input backspaces for
                                    // those since they don't output characters that can be
                                    // backspaced.
                                    let kc = OsCode::from(*k & MASK_KEYCODES);
                                    if matches!(
                                        kc,
                                        // Known bug: most non-characters-outputting keys are not
                                        // listed. I'm too lazy to list them all. Just use
                                        // character-outputting keys (and modifiers) in sequences
                                        // please! Or switch to a different input mode? It doesn't
                                        // really make sense to use non-typing characters other
                                        // than modifiers does it? Since those would probably be
                                        // further away from the home row, so why use them? If one
                                        // desired to fix this, a shorter list of keys would
                                        // probably be the list of keys that **do** output
                                        // characters than those that don't.
                                        OsCode::KEY_LEFTSHIFT
                                            | OsCode::KEY_RIGHTSHIFT
                                            | OsCode::KEY_LEFTMETA
                                            | OsCode::KEY_RIGHTMETA
                                            | OsCode::KEY_LEFTCTRL
                                            | OsCode::KEY_RIGHTCTRL
                                            | OsCode::KEY_LEFTALT
                                            | OsCode::KEY_RIGHTALT
                                    ) {
                                        continue;
                                    }

                                    self.kbd_out.press_key(OsCode::KEY_BACKSPACE)?;
                                    self.kbd_out.release_key(OsCode::KEY_BACKSPACE)?;
                                }
                            }
                        }

                        // Make sure to unpress any keys that were pressed as part of the sequence
                        // so that the keyberon internal sequence mechanism can do press+unpress of
                        // them.
                        for k in state.sequence.iter() {
                            let kc = KeyCode::from(OsCode::from(*k & MASK_KEYCODES));
                            layout.states.retain(|s| match s {
                                State::NormalKey { keycode, .. } => kc != *keycode,
                                _ => true,
                            });
                        }
                        layout.event(Event::Press(i, j));
                        layout.event(Event::Release(i, j));
                        self.sequence_state = None;
                    }
                }
            }
        }

        // Handle custom events. This used to be in a separate function but lifetime issues cause
        // it to now be here.
        match custom_event {
            CustomEvent::Press(custacts) => {
                #[cfg(feature = "cmd")]
                let mut cmds = vec![];
                let mut prev_mouse_btn = None;
                for custact in custacts.iter() {
                    match custact {
                        // For unicode, only send on the press. No repeat action is supported for this for
                        // now.
                        CustomAction::Unicode(c) => self.kbd_out.send_unicode(*c)?,
                        CustomAction::LiveReload => {
                            live_reload_requested = true;
                            log::info!(
                                "Requested live reload of file: {}",
                                self.cfg_paths[self.cur_cfg_idx].display()
                            );
                        }
                        CustomAction::LiveReloadNext => {
                            live_reload_requested = true;
                            self.cur_cfg_idx = if self.cur_cfg_idx == self.cfg_paths.len() - 1 {
                                0
                            } else {
                                self.cur_cfg_idx + 1
                            };
                            log::info!(
                                "Requested live reload of next file: {}",
                                self.cfg_paths[self.cur_cfg_idx].display()
                            );
                        }
                        CustomAction::LiveReloadPrev => {
                            live_reload_requested = true;
                            self.cur_cfg_idx = match self.cur_cfg_idx {
                                0 => self.cfg_paths.len() - 1,
                                i => i - 1,
                            };
                            log::info!(
                                "Requested live reload of prev file: {}",
                                self.cfg_paths[self.cur_cfg_idx].display()
                            );
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
                        CustomAction::Cmd(_cmd) => {
                            #[cfg(feature = "cmd")]
                            cmds.push(_cmd.clone());
                        }
                        CustomAction::CmdOutputKeys(_cmd) => {
                            #[cfg(feature = "cmd")]
                            {
                                for (key_action, osc) in keys_for_cmd_output(_cmd) {
                                    match key_action {
                                        KeyAction::Press => self.kbd_out.press_key(osc)?,
                                        KeyAction::Release => self.kbd_out.release_key(osc)?,
                                    }
                                }
                            }
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
                                || self.sequence_input_mode == SequenceInputMode::HiddenSuppressed
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
                            let mut do_caps_word = false;
                            if !cur_keys.contains(&KeyCode::LShift) {
                                if let Some(ref mut cw) = self.caps_word {
                                    cur_keys.push(key.into());
                                    let prev_len = cur_keys.len();
                                    cw.maybe_add_lsft(cur_keys);
                                    if cur_keys.len() > prev_len {
                                        do_caps_word = true;
                                        self.kbd_out.press_key(OsCode::KEY_LEFTSHIFT)?;
                                    }
                                }
                            }
                            // Release key in case the most recently pressed key is still pressed.
                            self.kbd_out.release_key(key)?;
                            self.kbd_out.press_key(key)?;
                            self.kbd_out.release_key(key)?;
                            if do_caps_word {
                                self.kbd_out.release_key(OsCode::KEY_LEFTSHIFT)?;
                            }
                        }
                        CustomAction::DynamicMacroRecord(macro_id) => {
                            let mut stop_record = false;
                            let mut new_recording = None;
                            match &mut self.dynamic_macro_record_state {
                                None => {
                                    log::info!("starting dynamic macro {macro_id} recording");
                                    self.dynamic_macro_record_state =
                                        Some(DynamicMacroRecordState {
                                            starting_macro_id: *macro_id,
                                            macro_items: vec![],
                                        })
                                }
                                Some(ref mut state) => {
                                    // remove the last item, since it's almost certainly a "macro
                                    // record" key press action which we don't want to keep.
                                    state.macro_items.remove(state.macro_items.len() - 1);
                                    state.add_release_for_all_unreleased_presses();
                                    self.dynamic_macros
                                        .insert(state.starting_macro_id, state.macro_items.clone());
                                    if state.starting_macro_id == *macro_id {
                                        log::info!(
                                            "same macro id pressed. saving and stopping dynamic macro {} recording",
                                            state.starting_macro_id
                                        );
                                        stop_record = true;
                                    } else {
                                        log::info!(
                                            "saving dynamic macro {} recording then starting new macro recording {macro_id}",
                                            state.starting_macro_id,
                                        );
                                        new_recording = Some(macro_id);
                                    }
                                }
                            }
                            if stop_record {
                                self.dynamic_macro_record_state = None;
                            } else if let Some(macro_id) = new_recording {
                                log::info!("starting new dynamic macro {macro_id} recording");
                                self.dynamic_macro_record_state = Some(DynamicMacroRecordState {
                                    starting_macro_id: *macro_id,
                                    macro_items: vec![],
                                });
                            }
                        }
                        CustomAction::DynamicMacroRecordStop(num_actions_to_remove) => {
                            if let Some(state) = &mut self.dynamic_macro_record_state {
                                // remove the last item independently of `num_actions_to_remove`
                                // since it's almost certainly a "macro record stop" key press
                                // action which we don't want to keep.
                                state.macro_items.remove(state.macro_items.len() - 1);
                                log::info!(
                                    "saving and stopping dynamic macro {} recording with {num_actions_to_remove} actions at the end removed",
                                    state.starting_macro_id,
                                );
                                state.macro_items.truncate(
                                    state
                                        .macro_items
                                        .len()
                                        .saturating_sub(*num_actions_to_remove as usize),
                                );
                                state.add_release_for_all_unreleased_presses();
                                self.dynamic_macros
                                    .insert(state.starting_macro_id, state.macro_items.clone());
                            }
                            self.dynamic_macro_record_state = None;
                        }
                        CustomAction::DynamicMacroPlay(macro_id) => {
                            match &mut self.dynamic_macro_replay_state {
                                None => {
                                    log::info!("replaying macro {macro_id}");
                                    self.dynamic_macro_replay_state =
                                        self.dynamic_macros.get(macro_id).map(|macro_items| {
                                            let mut active_macros = HashSet::default();
                                            active_macros.insert(*macro_id);
                                            DynamicMacroReplayState {
                                                active_macros,
                                                delay_remaining: 0,
                                                macro_items: macro_items.clone().into(),
                                            }
                                        });
                                }
                                Some(state) => {
                                    if state.active_macros.contains(macro_id) {
                                        log::warn!("refusing to recurse into macro {macro_id}");
                                    } else if let Some(items) = self.dynamic_macros.get(macro_id) {
                                        log::debug!(
                                            "prepending macro {macro_id} items to current replay"
                                        );
                                        state.active_macros.insert(*macro_id);
                                        state
                                            .macro_items
                                            .push_front(DynamicMacroItem::EndMacro(*macro_id));
                                        for item in items.iter().copied().rev() {
                                            state.macro_items.push_front(item);
                                        }
                                    }
                                }
                            }
                        }
                        CustomAction::SendArbitraryCode(code) => {
                            self.kbd_out.write_code(*code as u32, KeyValue::Press)?;
                        }
                        CustomAction::CapsWord(cfg) => {
                            self.caps_word = Some(CapsWordState::new(cfg));
                        }
                        CustomAction::SetMouse { x, y } => {
                            self.kbd_out.set_mouse(*x, *y)?;
                        }
                        CustomAction::FakeKeyOnRelease { .. }
                        | CustomAction::DelayOnRelease(_)
                        | CustomAction::CancelMacroOnRelease => {}
                    }
                }
                #[cfg(feature = "cmd")]
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
                        CustomAction::SendArbitraryCode(code) => {
                            if let Err(e) = self.kbd_out.write_code(*code as u32, KeyValue::Release)
                            {
                                log::error!("failed to send arbitrary code {e:?}");
                            }
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

    /// This compares the active keys in the keyberon layout against the potential key outputs for
    /// corresponding physical key in the configuration. If any of keyberon active keys match any
    /// potential physical key output, write the repeat event to the OS.
    fn handle_repeat(&mut self, event: &KeyEvent) -> Result<()> {
        let ret = self.handle_repeat_actual(event);
        // The cur_keys Vec is re-used for processing, for efficiency reasons to avoid allocation.
        // Unlike prev_keys which has useful info for the next call to handle_time_ticks, cur_keys
        // can be reused and cleared — it just needs to be empty for the next handle_time_ticks
        // call.
        self.cur_keys.clear();
        ret
    }

    fn handle_repeat_actual(&mut self, event: &KeyEvent) -> Result<()> {
        if self.sequence_state.is_some() {
            // While in sequence mode, don't send key repeats. I can't imagine it's a helpful use
            // case for someone trying to type in a sequence that they want to rely on key repeats
            // to finish a sequence. I suppose one might want to do repeat in order to try and
            // cancel an input sequence... I'll wait for a user created issue to deal with this.
            return Ok(());
        }
        self.cur_keys.extend(self.layout.bm().keycodes());
        self.overrides
            .override_keys(&mut self.cur_keys, &mut self.override_states);
        let current_layer = self.layout.bm().current_layer();
        if current_layer % 2 == 1 {
            // Prioritize checking the active layer in case a layer-while-held is active.
            if let Some(outputs_for_key) = self.key_outputs[current_layer].get(&event.code) {
                log::debug!("key outs for active layer-while-held: {outputs_for_key:?};");
                for kc in outputs_for_key.iter().rev() {
                    if self.cur_keys.contains(&kc.into()) {
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
            if self.cur_keys.contains(&kc.into()) {
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

    /// Prints the layer. If the TCP server is enabled, then this will also send a notification to
    /// all connected clients.
    fn check_handle_layer_change(&mut self, tx: &Option<Sender<ServerMessage>>) {
        let cur_layer = self.layout.bm().current_layer();
        if cur_layer != self.prev_layer {
            let new = self.layer_info[cur_layer].name.clone();
            self.prev_layer = cur_layer;
            self.print_layer(cur_layer);

            if let Some(tx) = tx {
                match tx.send(ServerMessage::LayerChange { new }) {
                    Ok(_) => {}
                    Err(error) => {
                        log::error!("could not send event notification: {}", error);
                    }
                }
            }
        }
    }

    fn print_layer(&self, layer: usize) {
        if self.log_layer_changes {
            log::info!("Entered layer:\n\n{}", self.layer_info[layer].cfg_text);
        }
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
        nodelay: bool,
    ) {
        info!("entering the processing loop");
        std::thread::spawn(move || {
            if !nodelay {
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
            }

            info!("Starting kanata proper");
            let err = loop {
                if kanata.lock().can_block() {
                    log::trace!("blocking on channel");
                    match rx.recv() {
                        Ok(kev) => {
                            let mut k = kanata.lock();
                            k.last_tick = time::Instant::now()
                                .checked_sub(time::Duration::from_millis(1))
                                .expect("subtract 1ms from current time");

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

                            drop(k);
                            std::thread::sleep(time::Duration::from_millis(1));
                        }
                        Err(TryRecvError::Disconnected) => {
                            log::error!("channel disconnected");
                            return;
                        }
                    }
                }
            };
            panic!("processing loop encountered error {err:?}")
        });
    }

    pub fn can_block(&self) -> bool {
        self.layout.b().queue.is_empty()
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
            && self.caps_word.is_none()
            && !self
                .layout
                .b()
                .states
                .iter()
                .any(|s| matches!(s, State::SeqCustomPending(_) | State::SeqCustomActive(_)))
    }
}

fn set_altgr_behaviour(_cfg: &cfg::Cfg) -> Result<()> {
    #[cfg(target_os = "windows")]
    set_win_altgr_behaviour(_cfg)?;
    Ok(())
}

#[cfg(feature = "cmd")]
fn run_multi_cmd(cmds: Vec<Vec<String>>) {
    std::thread::spawn(move || {
        for cmd in cmds {
            if let Err(e) = run_cmd_in_thread(cmd).join() {
                log::error!("problem joining thread {:?}", e);
            }
        }
    });
}

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
            signal_hook::low_level::raise(signal_hook::consts::SIGTERM).expect("raise signal");
        }
    }
}

fn update_kbd_out(_cfg: &HashMap<String, String>, _kbd_out: &KbdOut) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        _kbd_out.update_unicode_termination(
                _cfg.get("linux-unicode-termination").map(|s| {
                match s.as_str() {
                    "enter" => Ok(UnicodeTermination::Enter),
                    "space" => Ok(UnicodeTermination::Space),
                    "enter-space" => Ok(UnicodeTermination::EnterSpace),
                    "space-enter" => Ok(UnicodeTermination::SpaceEnter),
                    _ => Err(anyhow!("linux-unicode-termination got {s}. It accepts: enter|space|enter-space|space-enter")),
                }
            }).unwrap_or(Ok(_kbd_out.unicode_termination.get()))?);
        _kbd_out.update_unicode_u_code(
            _cfg.get("linux-unicode-u-code")
                .map(|s| {
                    str_to_oscode(s)
                        .ok_or_else(|| anyhow!("unknown code for linux-unicode-u-code {s}"))
                })
                .unwrap_or(Ok(_kbd_out.unicode_u_code.get()))?,
        );
    }
    Ok(())
}
