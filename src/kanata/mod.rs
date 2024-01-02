//! Implements the glue between OS input/output and keyberon state management.

use anyhow::{bail, Result};
use log::{error, info};
use parking_lot::Mutex;
use std::sync::mpsc::{Receiver, SyncSender as Sender, TryRecvError};

use kanata_keyberon::key_code::*;
use kanata_keyberon::layout::*;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::Arc;
use std::time;

use crate::oskbd::{KeyEvent, *};
use crate::tcp_server::ServerMessage;
use crate::ValidatedArgs;
use kanata_parser::cfg;
use kanata_parser::cfg::*;
use kanata_parser::custom_action::*;
use kanata_parser::keys::*;

mod dynamic_macro;
use dynamic_macro::*;

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

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

mod caps_word;
pub use caps_word::*;

type HashSet<T> = rustc_hash::FxHashSet<T>;
type HashMap<K, V> = rustc_hash::FxHashMap<K, V>;

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
    /// A list of mouse speed modifiers in percentages by which mouse travel distance is scaled.
    pub move_mouse_speed_modifiers: Vec<u16>,
    /// The user configuration for backtracking to find valid sequences. See
    /// <../../docs/sequence-adding-chords-ideas.md> for more info.
    pub sequence_backtrack_modcancel: bool,
    /// Tracks sequence progress. Is Some(...) when in sequence mode and None otherwise.
    pub sequence_state: Option<SequenceState>,
    /// Valid sequences defined in the user configuration.
    pub sequences: cfg::KeySeqsToFKeys,
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
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    /// Tracks the Linux/Macos user configuration for device names (instead of paths) that should be
    /// included for interception and processing by kanata.
    pub include_names: Option<Vec<String>>,
    #[cfg(target_os = "linux")]
    /// Tracks the Linux user configuration for device names (instead of paths) that should be
    /// excluded for interception and processing by kanata.
    pub exclude_names: Option<Vec<String>>,
    #[cfg(all(feature = "interception_driver", target_os = "windows"))]
    /// Used to know which input device to treat as a mouse for intercepting and processing inputs
    /// by kanata.
    intercept_mouse_hwid: Option<[u8; HWID_ARR_SZ]>,
    /// User configuration to do logging of layer changes or not.
    log_layer_changes: bool,
    /// Tracks the caps-word state. Is Some(...) if caps-word is active and None otherwise.
    pub caps_word: Option<CapsWordState>,
    /// Config items from `defcfg`.
    #[cfg(target_os = "linux")]
    pub x11_repeat_rate: Option<KeyRepeatSettings>,
    /// Fake key actions that are waiting for a certain duration of keyboard idling.
    pub waiting_for_idle: HashSet<FakeKeyOnIdle>,
    /// Number of ticks since kanata was idle.
    pub ticks_since_idle: u16,
    /// If a mousemove action is active and another mousemove action is activated,
    /// reuse the acceleration state.
    movemouse_inherit_accel_state: bool,
    /// Removes jaggedneess of vertical and horizontal mouse movements when used
    /// simultaneously at the cost of increased mousemove actions latency.
    movemouse_smooth_diagonals: bool,
    /// If movemouse_smooth_diagonals is enabled, the previous mouse actions
    /// gets stored in this buffer and if the next movemouse action is opposite axis
    /// than the one stored in the buffer, both events are outputted at the same time.
    movemouse_buffer: Option<(Axis, CalculatedMouseMove)>,
    /// Configured maximum for dynamic macro recording, to protect users from themselves if they
    /// have accidentally left it on.
    dynamic_macro_max_presses: u16,
    /// Keys that should be unmodded. If non-empty, any modifier should be cleared.
    unmodded_keys: Vec<KeyCode>,
    /// Keys that should be unshifted. If non-empty, left+right shift keys should be cleared.
    unshifted_keys: Vec<KeyCode>,
    /// Keep track of last pressed key for [`CustomAction::Repeat`].
    last_pressed_key: KeyCode,
}

#[derive(PartialEq, Clone, Copy)]
pub enum Axis {
    Vertical,
    Horizontal,
}

impl From<MoveDirection> for Axis {
    fn from(val: MoveDirection) -> Axis {
        match val {
            MoveDirection::Up | MoveDirection::Down => Axis::Vertical,
            MoveDirection::Left | MoveDirection::Right => Axis::Horizontal,
        }
    }
}

#[derive(Clone, Copy)]
pub struct CalculatedMouseMove {
    pub direction: MoveDirection,
    pub distance: u16,
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

#[derive(Clone, Copy)]
pub struct MoveMouseAccelState {
    pub accel_ticks_from_min: u16,
    pub accel_ticks_until_max: u16,
    pub accel_increment: f64,
    pub min_distance: u16,
    pub max_distance: u16,
}

pub struct SequenceState {
    pub sequence: Vec<u16>,
    pub sequence_input_mode: SequenceInputMode,
    pub ticks_until_timeout: u16,
    pub sequence_timeout: u16,
}

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

        let kbd_out = match KbdOut::new(
            #[cfg(target_os = "linux")]
            &args.symlink_path,
        ) {
            Ok(kbd_out) => kbd_out,
            Err(err) => {
                error!("Failed to open the output uinput device. Make sure you've added the user executing kanata to the `uinput` group");
                bail!(err)
            }
        };

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

        #[cfg(target_os = "windows")]
        set_win_altgr_behaviour(cfg.items.windows_altgr);

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
            move_mouse_speed_modifiers: Vec::new(),
            sequence_backtrack_modcancel: cfg.items.sequence_backtrack_modcancel,
            sequence_state: None,
            sequences: cfg.sequences,
            last_tick: time::Instant::now(),
            time_remainder: 0,
            live_reload_requested: false,
            overrides: cfg.overrides,
            override_states: OverrideStates::new(),
            #[cfg(target_os = "macos")]
            include_names: cfg.items.macos_dev_names_include,
            #[cfg(target_os = "linux")]
            kbd_in_paths: cfg.items.linux_dev,
            #[cfg(target_os = "linux")]
            continue_if_no_devices: cfg.items.linux_continue_if_no_devs_found,
            #[cfg(target_os = "linux")]
            include_names: cfg.items.linux_dev_names_include,
            #[cfg(target_os = "linux")]
            exclude_names: cfg.items.linux_dev_names_exclude,
            #[cfg(all(feature = "interception_driver", target_os = "windows"))]
            intercept_mouse_hwid: cfg.items.windows_interception_mouse_hwid,
            dynamic_macro_replay_state: None,
            dynamic_macro_record_state: None,
            dynamic_macros: Default::default(),
            log_layer_changes: cfg.items.log_layer_changes,
            caps_word: None,
            movemouse_smooth_diagonals: cfg.items.movemouse_smooth_diagonals,
            movemouse_inherit_accel_state: cfg.items.movemouse_inherit_accel_state,
            dynamic_macro_max_presses: cfg.items.dynamic_macro_max_presses,
            #[cfg(target_os = "linux")]
            x11_repeat_rate: cfg.items.linux_x11_repeat_delay_rate,
            waiting_for_idle: HashSet::default(),
            ticks_since_idle: 0,
            movemouse_buffer: None,
            unmodded_keys: vec![],
            unshifted_keys: vec![],
            last_pressed_key: KeyCode::No,
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
        #[cfg(target_os = "windows")]
        set_win_altgr_behaviour(cfg.items.windows_altgr);
        self.sequence_backtrack_modcancel = cfg.items.sequence_backtrack_modcancel;
        self.layout = cfg.layout;
        self.key_outputs = cfg.key_outputs;
        self.layer_info = cfg.layer_info;
        self.sequences = cfg.sequences;
        self.overrides = cfg.overrides;
        self.log_layer_changes = cfg.items.log_layer_changes;
        self.movemouse_smooth_diagonals = cfg.items.movemouse_smooth_diagonals;
        self.movemouse_inherit_accel_state = cfg.items.movemouse_inherit_accel_state;
        self.dynamic_macro_max_presses = cfg.items.dynamic_macro_max_presses;

        *MAPPED_KEYS.lock() = cfg.mapped_keys;
        #[cfg(target_os = "linux")]
        Kanata::set_repeat_rate(cfg.items.linux_x11_repeat_delay_rate)?;
        log::info!("Live reload successful");
        Ok(())
    }

    /// Update keyberon layout state for press/release, handle repeat separately
    fn handle_input_event(&mut self, event: &KeyEvent) -> Result<()> {
        log::debug!("process recv ev {event:?}");
        let evc: u16 = event.code.into();
        self.ticks_since_idle = 0;
        let kbrn_ev = match event.value {
            KeyValue::Press => {
                if let Some((macro_id, recorded_macro)) = record_press(
                    &mut self.dynamic_macro_record_state,
                    event.code,
                    self.dynamic_macro_max_presses,
                ) {
                    self.dynamic_macros.insert(macro_id, recorded_macro);
                }
                Event::Press(0, evc)
            }
            KeyValue::Release => {
                record_release(&mut self.dynamic_macro_record_state, event.code);
                Event::Release(0, evc)
            }
            KeyValue::Repeat => {
                let ret = self.handle_repeat(event);
                return ret;
            }
            KeyValue::Tap => {
                self.layout.bm().event(Event::Press(0, evc));
                self.layout.bm().event(Event::Release(0, evc));
                return Ok(());
            }
        };
        self.layout.bm().event(kbrn_ev);
        Ok(())
    }

    /// Advance keyberon layout state and send events based on changes to its state.
    /// Returns the number of ticks that elapsed.
    fn handle_time_ticks(&mut self, tx: &Option<Sender<ServerMessage>>) -> Result<u16> {
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
            self.tick_idle_timeout();

            if let Some(event) = tick_replay_state(&mut self.dynamic_macro_replay_state) {
                self.layout.bm().event(event);
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

        if self.live_reload_requested
            && ((self.prev_keys.is_empty() && self.cur_keys.is_empty())
                || self.ticks_since_idle > 1000)
        {
            // Note regarding the ticks_since_idle check above:
            // After 1 second if live reload is still not done, there might be a key in a stuck
            // state. One known instance where this happens is Win+L to lock the screen in
            // Windows with the LLHOOK mechanism. The release of Win and L keys will not be
            // caught by the kanata process when on the lock screen. However, the OS knows that
            // these keys have released - only the kanata state is wrong. And since kanata has
            // a key in a stuck state, without this 1s fallback, live reload would never
            // activate. Having this fallback allows live reload to happen which resets the
            // kanata states.
            self.live_reload_requested = false;
            if let Err(e) = self.do_live_reload() {
                log::error!("live reload failed {e}");
            }
        }

        #[cfg(feature = "perf_logging")]
        log::info!("ms elapsed: {ms_elapsed}");
        // Note regarding `as` casting. It doesn't really matter if the result would truncate and
        // end up being wrong. Prefer to do the cheaper operation, as compared to doing the min of
        // u16::MAX and ms_elapsed.
        Ok(ms_elapsed as u16)
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
                let scaled_distance =
                    apply_mouse_distance_modifiers(mmsv.distance, &self.move_mouse_speed_modifiers);
                log::debug!("handle_move_mouse: scaled vdistance: {}", scaled_distance);

                let current_move = CalculatedMouseMove {
                    direction: mmsv.direction,
                    distance: scaled_distance,
                };

                if self.movemouse_smooth_diagonals {
                    let axis: Axis = current_move.direction.into();
                    match &self.movemouse_buffer {
                        Some((previous_axis, previous_move)) => {
                            if axis == *previous_axis {
                                self.kbd_out.move_mouse(*previous_move)?;
                                self.movemouse_buffer = Some((axis, current_move));
                            } else {
                                self.kbd_out
                                    .move_mouse_many(&[*previous_move, current_move])?;
                                self.movemouse_buffer = None;
                            }
                        }
                        None => {
                            self.movemouse_buffer = Some((axis, current_move));
                        }
                    }
                } else {
                    self.kbd_out.move_mouse(current_move)?;
                }
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
                let scaled_distance =
                    apply_mouse_distance_modifiers(mmsh.distance, &self.move_mouse_speed_modifiers);
                log::debug!("handle_move_mouse: scaled hdistance: {}", scaled_distance);

                let current_move = CalculatedMouseMove {
                    direction: mmsh.direction,
                    distance: scaled_distance,
                };

                if self.movemouse_smooth_diagonals {
                    let axis: Axis = current_move.direction.into();
                    match &self.movemouse_buffer {
                        Some((previous_axis, previous_move)) => {
                            if axis == *previous_axis {
                                self.kbd_out.move_mouse(*previous_move)?;
                                self.movemouse_buffer = Some((axis, current_move));
                            } else {
                                self.kbd_out
                                    .move_mouse_many(&[*previous_move, current_move])?;
                                self.movemouse_buffer = None;
                            }
                        }
                        None => {
                            self.movemouse_buffer = Some((axis, current_move));
                        }
                    }
                } else {
                    self.kbd_out.move_mouse(current_move)?;
                }
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
                cancel_sequence(state, &mut self.kbd_out)?;
                self.sequence_state = None;
            }
        }
        Ok(())
    }

    fn tick_idle_timeout(&mut self) {
        if self.waiting_for_idle.is_empty() {
            return;
        }
        self.waiting_for_idle.retain(|wfd| {
            if self.ticks_since_idle >= wfd.idle_duration {
                // Process this and return false so that it is not retained.
                let layout = self.layout.bm();
                let Coord { x, y } = wfd.coord;
                handle_fakekey_action(wfd.action, layout, x, y);
                false
            } else {
                true
            }
        })
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

        // Deal with unmodded. Unlike other custom actions, this should come before key presses and
        // releases. I don't quite remember why custom actions come after the key processing, but I
        // remember that it is intentional. However, since unmodded needs to modify the key lists,
        // it should come before.
        match custom_event {
            CustomEvent::Press(custacts) => {
                for custact in custacts.iter() {
                    match custact {
                        CustomAction::Unmodded { keys } => {
                            self.unmodded_keys.extend(keys);
                        }
                        CustomAction::Unshifted { keys } => {
                            self.unshifted_keys.extend(keys);
                        }
                        _ => {}
                    }
                }
            }
            CustomEvent::Release(custacts) => {
                for custact in custacts.iter() {
                    match custact {
                        CustomAction::Unmodded { keys } => {
                            self.unmodded_keys.retain(|k| !keys.contains(k));
                        }
                        CustomAction::Unshifted { keys } => {
                            self.unshifted_keys.retain(|k| !keys.contains(k));
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        if !self.unmodded_keys.is_empty() {
            cur_keys.retain(|k| {
                !matches!(
                    k,
                    KeyCode::LShift
                        | KeyCode::RShift
                        | KeyCode::LGui
                        | KeyCode::RGui
                        | KeyCode::LCtrl
                        | KeyCode::RCtrl
                        | KeyCode::LAlt
                        | KeyCode::RAlt
                )
            });
            cur_keys.extend(self.unmodded_keys.iter());
        }
        if !self.unshifted_keys.is_empty() {
            cur_keys.retain(|k| !matches!(k, KeyCode::LShift | KeyCode::RShift));
            cur_keys.extend(self.unshifted_keys.iter());
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
            self.last_pressed_key = *k;
            match &mut self.sequence_state {
                None => {
                    log::debug!("key press     {:?}", k);
                    if let Err(e) = self.kbd_out.press_key(k.into()) {
                        bail!("failed to press key: {:?}", e);
                    }
                }
                Some(state) => {
                    state.ticks_until_timeout = state.sequence_timeout;

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
                    match state.sequence_input_mode {
                        SequenceInputMode::VisibleBackspaced => {
                            self.kbd_out.press_key(osc)?;
                        }
                        SequenceInputMode::HiddenSuppressed
                        | SequenceInputMode::HiddenDelayType => {}
                    }
                    log::debug!("sequence got {k:?}");

                    use kanata_parser::sequences::*;
                    use kanata_parser::trie::GetOrDescendentExistsResult::*;

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
                            match state.sequence_input_mode {
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
                        match state.sequence_input_mode {
                            SequenceInputMode::HiddenSuppressed
                            | SequenceInputMode::HiddenDelayType => {}
                            SequenceInputMode::VisibleBackspaced => {
                                // Release all keys since they might modify the behaviour of
                                // backspace into an undesirable behaviour, for example deleting
                                // more characters than it should.
                                layout.states.retain(|s| match s {
                                    State::NormalKey { keycode, .. } => {
                                        // Ignore the error, ugly to return it from retain, and
                                        // this is very unlikely to happen anyway.
                                        let _ = self.kbd_out.release_key(keycode.into());
                                        false
                                    }
                                    _ => true,
                                });
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
                        CustomAction::MWheelNotch { direction } => {
                            self.kbd_out
                                .scroll(*direction, HI_RES_SCROLL_UNITS_IN_LO_RES)?;
                        }
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
                            let move_mouse_accel_state = match (
                                self.movemouse_inherit_accel_state,
                                &self.move_mouse_state_horizontal,
                                &self.move_mouse_state_vertical,
                            ) {
                                (
                                    true,
                                    Some(MoveMouseState {
                                        move_mouse_accel_state: Some(s),
                                        ..
                                    }),
                                    _,
                                )
                                | (
                                    true,
                                    _,
                                    Some(MoveMouseState {
                                        move_mouse_accel_state: Some(s),
                                        ..
                                    }),
                                ) => *s,
                                _ => {
                                    let f_max_distance: f64 = *max_distance as f64;
                                    let f_min_distance: f64 = *min_distance as f64;
                                    let f_accel_time: f64 = *accel_time as f64;
                                    let increment =
                                        (f_max_distance - f_min_distance) / f_accel_time;

                                    MoveMouseAccelState {
                                        accel_ticks_from_min: 0,
                                        accel_ticks_until_max: *accel_time,
                                        accel_increment: increment,
                                        min_distance: *min_distance,
                                        max_distance: *max_distance,
                                    }
                                }
                            };

                            match direction {
                                MoveDirection::Up | MoveDirection::Down => {
                                    self.move_mouse_state_vertical = Some(MoveMouseState {
                                        direction: *direction,
                                        distance: *min_distance,
                                        ticks_until_move: 0,
                                        interval: *interval,
                                        move_mouse_accel_state: Some(move_mouse_accel_state),
                                    })
                                }
                                MoveDirection::Left | MoveDirection::Right => {
                                    self.move_mouse_state_horizontal = Some(MoveMouseState {
                                        direction: *direction,
                                        distance: *min_distance,
                                        ticks_until_move: 0,
                                        interval: *interval,
                                        move_mouse_accel_state: Some(move_mouse_accel_state),
                                    })
                                }
                            }
                        }
                        CustomAction::MoveMouseSpeed { speed } => {
                            self.move_mouse_speed_modifiers.push(*speed);
                            log::debug!(
                                "movemousespeed modifiers: {:?}",
                                self.move_mouse_speed_modifiers
                            );
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
                            handle_fakekey_action(*action, layout, x, y);
                        }
                        CustomAction::Delay(delay) => {
                            log::debug!("on-press: sleeping for {delay} ms");
                            std::thread::sleep(std::time::Duration::from_millis((*delay).into()));
                        }
                        CustomAction::SequenceCancel => {
                            if self.sequence_state.is_some() {
                                log::debug!("exiting sequence");
                                let state = self.sequence_state.as_ref().unwrap();
                                cancel_sequence(state, &mut self.kbd_out)?;
                                self.sequence_state = None;
                            }
                        }
                        CustomAction::SequenceLeader(timeout, input_mode) => {
                            if self.sequence_state.is_none()
                                || self.sequence_state.as_ref().unwrap().sequence_input_mode
                                    == SequenceInputMode::HiddenSuppressed
                            {
                                log::debug!("entering sequence mode");
                                self.sequence_state = Some(SequenceState {
                                    sequence: vec![],
                                    sequence_input_mode: *input_mode,
                                    ticks_until_timeout: *timeout,
                                    sequence_timeout: *timeout,
                                });
                            }
                        }
                        CustomAction::Repeat => {
                            let keycode = self.last_pressed_key;
                            let osc: OsCode = keycode.into();
                            log::debug!("repeating a keypress {osc:?}");
                            let mut do_caps_word = false;
                            if !cur_keys.contains(&KeyCode::LShift) {
                                if let Some(ref mut cw) = self.caps_word {
                                    cur_keys.push(keycode);
                                    let prev_len = cur_keys.len();
                                    cw.maybe_add_lsft(cur_keys);
                                    if cur_keys.len() > prev_len {
                                        do_caps_word = true;
                                        self.kbd_out.press_key(OsCode::KEY_LEFTSHIFT)?;
                                    }
                                }
                            }
                            // Release key in case the most recently pressed key is still pressed.
                            self.kbd_out.release_key(osc)?;
                            self.kbd_out.press_key(osc)?;
                            self.kbd_out.release_key(osc)?;
                            if do_caps_word {
                                self.kbd_out.release_key(OsCode::KEY_LEFTSHIFT)?;
                            }
                        }
                        CustomAction::DynamicMacroRecord(macro_id) => {
                            if let Some((macro_id, prev_recorded_macro)) =
                                record_macro(*macro_id, &mut self.dynamic_macro_record_state)
                            {
                                self.dynamic_macros.insert(macro_id, prev_recorded_macro);
                            }
                        }
                        CustomAction::DynamicMacroRecordStop(num_actions_to_remove) => {
                            if let Some((macro_id, prev_recorded_macro)) = stop_macro(
                                &mut self.dynamic_macro_record_state,
                                *num_actions_to_remove,
                            ) {
                                self.dynamic_macros.insert(macro_id, prev_recorded_macro);
                            }
                        }
                        CustomAction::DynamicMacroPlay(macro_id) => {
                            play_macro(
                                *macro_id,
                                &mut self.dynamic_macro_replay_state,
                                &self.dynamic_macros,
                            );
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
                        CustomAction::FakeKeyOnIdle(fkd) => {
                            self.ticks_since_idle = 0;
                            self.waiting_for_idle.insert(*fkd);
                        }
                        CustomAction::FakeKeyOnRelease { .. }
                        | CustomAction::DelayOnRelease(_)
                        | CustomAction::Unmodded { .. }
                        | CustomAction::Unshifted { .. }
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
                        CustomAction::MoveMouse { direction, .. }
                        | CustomAction::MoveMouseAccel { direction, .. } => {
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
                            if self.movemouse_smooth_diagonals {
                                self.movemouse_buffer = None
                            }
                            pbtn
                        }
                        CustomAction::MoveMouseSpeed { speed, .. } => {
                            if let Some(idx) = self
                                .move_mouse_speed_modifiers
                                .iter()
                                .position(|s| *s == *speed)
                            {
                                self.move_mouse_speed_modifiers.remove(idx);
                            }
                            log::debug!(
                                "movemousespeed modifiers: {:?}",
                                self.move_mouse_speed_modifiers
                            );
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
                            handle_fakekey_action(*action, layout, x, y);
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
                for osc in outputs_for_key.iter().rev().copied() {
                    let kc = osc.into();
                    if self.cur_keys.contains(&kc)
                        || self.unshifted_keys.contains(&kc)
                        || self.unmodded_keys.contains(&kc)
                    {
                        log::debug!("repeat    {:?}", KeyCode::from(osc));
                        if let Err(e) = self.kbd_out.write_key(osc, KeyValue::Repeat) {
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
        for osc in outputs_for_key.iter().rev().copied() {
            let kc = osc.into();
            if self.cur_keys.contains(&kc)
                || self.unshifted_keys.contains(&kc)
                || self.unmodded_keys.contains(&kc)
            {
                log::debug!("repeat    {:?}", KeyCode::from(osc));
                if let Err(e) = self.kbd_out.write_key(osc, KeyValue::Repeat) {
                    bail!("could not write key {:?}", e)
                }
                return Ok(());
            }
        }
        Ok(())
    }

    #[cfg(feature = "tcp_server")]
    pub fn change_layer(&mut self, layer_name: String) {
        for (i, l) in self.layer_info.iter().enumerate() {
            if l.name == layer_name {
                self.layout.bm().set_default_layer(i);
                return;
            }
        }
    }

    #[allow(unused_variables)]
    /// Prints the layer. If the TCP server is enabled, then this will also send a notification to
    /// all connected clients.
    fn check_handle_layer_change(&mut self, tx: &Option<Sender<ServerMessage>>) {
        let cur_layer = self.layout.bm().current_layer();
        if cur_layer != self.prev_layer {
            let new = self.layer_info[cur_layer].name.clone();
            self.prev_layer = cur_layer;
            self.print_layer(cur_layer);

            #[cfg(feature = "tcp_server")]
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
        if self.log_layer_changes {
            log::info!("Entered layer:\n\n{}", self.layer_info[layer].cfg_text);
        }
    }

    #[cfg(feature = "tcp_server")]
    pub fn start_notification_loop(
        rx: Receiver<ServerMessage>,
        clients: crate::tcp_server::Connections,
    ) {
        use std::io::Write;
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

    #[cfg(not(feature = "tcp_server"))]
    pub fn start_notification_loop(
        _rx: Receiver<ServerMessage>,
        _clients: crate::tcp_server::Connections,
    ) {
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
            let mut ms_elapsed = 0;

            info!("Starting kanata proper");
            let err = loop {
                let can_block = {
                    let mut k = kanata.lock();
                    let is_idle = k.is_idle();
                    // Note: checking waiting_for_idle can not be part of the computation for
                    // is_idle() since incrementing ticks_since_idle is dependent on the return
                    // value of is_idle().
                    let counting_idle_ticks =
                        !k.waiting_for_idle.is_empty() || k.live_reload_requested;
                    if !is_idle {
                        k.ticks_since_idle = 0;
                    } else if is_idle && counting_idle_ticks {
                        k.ticks_since_idle = k.ticks_since_idle.saturating_add(ms_elapsed);
                        #[cfg(feature = "perf_logging")]
                        log::info!("ticks since idle: {}", k.ticks_since_idle);
                    }
                    is_idle && !counting_idle_ticks
                };
                if can_block {
                    log::trace!("blocking on channel");
                    match rx.recv() {
                        Ok(kev) => {
                            let mut k = kanata.lock();
                            let now = time::Instant::now()
                                .checked_sub(time::Duration::from_millis(1))
                                .expect("subtract 1ms from current time");
                            #[cfg(all(
                                not(feature = "interception_driver"),
                                target_os = "windows"
                            ))]
                            {
                                // If kanata has been blocking for long enough, clear all states.
                                // This won't trigger if there are macros running, or if a key is
                                // held down for a long time and is sending OS repeats. The reason
                                // for this code is in case like Win+L which locks the Windows
                                // desktop. When this happens, the Win key and L key will be stuck
                                // as pressed in the kanata state because LLHOOK kanata cannot read
                                // keys in the lock screen or administrator applications. So this
                                // is heuristic to detect such an issue and clear states assuming
                                // that's what happened.
                                //
                                // Only states in the normal key row are cleared, since those are
                                // the states that might be stuck. A real use case might be to have
                                // a fake key pressed for a long period of time, so make sure those
                                // are not cleared.
                                if (now - k.last_tick) > time::Duration::from_secs(60) {
                                    log::debug!(
                                    "clearing keyberon normal key states due to blocking for a while"
                                );
                                    k.layout.bm().states.retain(|s| {
                                        !matches!(
                                            s,
                                            State::NormalKey {
                                                coord: (NORMAL_KEY_ROW, _),
                                                ..
                                            } | State::LayerModifier {
                                                coord: (NORMAL_KEY_ROW, _),
                                                ..
                                            } | State::Custom {
                                                coord: (NORMAL_KEY_ROW, _),
                                                ..
                                            } | State::RepeatingSequence {
                                                coord: (NORMAL_KEY_ROW, _),
                                                ..
                                            }
                                        )
                                    });
                                }
                            }
                            k.last_tick = now;

                            #[cfg(feature = "perf_logging")]
                            let start = std::time::Instant::now();

                            if let Err(e) = k.handle_input_event(&kev) {
                                break e;
                            }

                            #[cfg(feature = "perf_logging")]
                            log::info!(
                                "[PERF]: handle key event: {} ns",
                                (start.elapsed()).as_nanos()
                            );
                            #[cfg(feature = "perf_logging")]
                            let start = std::time::Instant::now();

                            match k.handle_time_ticks(&tx) {
                                Ok(ms) => ms_elapsed = ms,
                                Err(e) => break e,
                            };

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

                            if let Err(e) = k.handle_input_event(&kev) {
                                break e;
                            }

                            #[cfg(feature = "perf_logging")]
                            log::info!(
                                "[PERF]: handle key event: {} ns",
                                (start.elapsed()).as_nanos()
                            );
                            #[cfg(feature = "perf_logging")]
                            let start = std::time::Instant::now();

                            match k.handle_time_ticks(&tx) {
                                Ok(ms) => ms_elapsed = ms,
                                Err(e) => break e,
                            };

                            #[cfg(feature = "perf_logging")]
                            log::info!(
                                "[PERF]: handle time ticks: {} ns",
                                (start.elapsed()).as_nanos()
                            );
                        }
                        Err(TryRecvError::Empty) => {
                            #[cfg(feature = "perf_logging")]
                            let start = std::time::Instant::now();

                            match k.handle_time_ticks(&tx) {
                                Ok(ms) => ms_elapsed = ms,
                                Err(e) => break e,
                            };

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

    pub fn is_idle(&self) -> bool {
        let pressed_keys_means_not_idle =
            !self.waiting_for_idle.is_empty() || self.live_reload_requested;
        self.layout.b().queue.is_empty()
            && self.layout.b().waiting.is_none()
            && self.layout.b().last_press_tracker.tap_hold_timeout == 0
            && (self.layout.b().oneshot.timeout == 0 || self.layout.b().oneshot.keys.is_empty())
            && self.layout.b().active_sequences.is_empty()
            && self.layout.b().tap_dance_eager.is_none()
            && self.layout.b().action_queue.is_empty()
            && self.sequence_state.is_none()
            && self.scroll_state.is_none()
            && self.hscroll_state.is_none()
            && self.move_mouse_state_vertical.is_none()
            && self.move_mouse_state_horizontal.is_none()
            && self.dynamic_macro_replay_state.is_none()
            && self.caps_word.is_none()
            && !self.layout.b().states.iter().any(|s| {
                matches!(s, State::SeqCustomPending(_) | State::SeqCustomActive(_))
                    || (pressed_keys_means_not_idle && matches!(s, State::NormalKey { .. }))
            })
    }
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

fn apply_mouse_distance_modifiers(initial_distance: u16, mods: &Vec<u16>) -> u16 {
    let mut scaled_distance = initial_distance;
    for &modifier in mods {
        scaled_distance = u16::max(
            1,
            f32::min(
                scaled_distance as f32 * (modifier as f32 / 100f32),
                u16::MAX as f32,
            )
            .round() as u16,
        );
    }
    scaled_distance
}

#[test]
fn apply_speed_modifiers() {
    assert_eq!(apply_mouse_distance_modifiers(15, &vec![]), 15);

    assert_eq!(apply_mouse_distance_modifiers(10, &vec![200u16]), 20);
    assert_eq!(apply_mouse_distance_modifiers(20, &vec![50u16]), 10);

    assert_eq!(apply_mouse_distance_modifiers(5, &vec![33u16]), 2); // 1.65
    assert_eq!(apply_mouse_distance_modifiers(100, &vec![99u16]), 99);

    // Clamping
    assert_eq!(
        apply_mouse_distance_modifiers(65535, &vec![65535u16]),
        65535
    );
    assert_eq!(apply_mouse_distance_modifiers(1, &vec![1u16]), 1);

    // Nice, round calculations equal themselves
    assert_eq!(
        apply_mouse_distance_modifiers(10, &vec![50u16, 200u16]),
        apply_mouse_distance_modifiers(10, &vec![200u16, 50u16])
    );

    // 33% of 20
    assert_eq!(apply_mouse_distance_modifiers(10, &vec![200u16, 33u16]), 7);
    // 200% of 3
    assert_eq!(apply_mouse_distance_modifiers(10, &vec![33u16, 200u16]), 6);
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

fn update_kbd_out(_cfg: &CfgOptions, _kbd_out: &KbdOut) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        _kbd_out.update_unicode_termination(_cfg.linux_unicode_termination);
        _kbd_out.update_unicode_u_code(_cfg.linux_unicode_u_code);
    }
    Ok(())
}

fn cancel_sequence(state: &SequenceState, kbd_out: &mut KbdOut) -> Result<()> {
    match state.sequence_input_mode {
        SequenceInputMode::HiddenDelayType => {
            for code in state.sequence.iter().copied() {
                if let Some(osc) = OsCode::from_u16(code) {
                    kbd_out.press_key(osc)?;
                    kbd_out.release_key(osc)?;
                }
            }
        }
        SequenceInputMode::HiddenSuppressed | SequenceInputMode::VisibleBackspaced => {}
    }
    Ok(())
}

fn handle_fakekey_action<'a, const C: usize, const R: usize, const L: usize, T>(
    action: FakeKeyAction,
    layout: &mut Layout<'a, C, R, L, T>,
    x: u8,
    y: u16,
) where
    T: 'a + std::fmt::Debug + Copy,
{
    match action {
        FakeKeyAction::Press => layout.event(Event::Press(x, y)),
        FakeKeyAction::Release => layout.event(Event::Release(x, y)),
        FakeKeyAction::Tap => {
            layout.event(Event::Press(x, y));
            layout.event(Event::Release(x, y));
        }
        FakeKeyAction::Toggle => {
            match states_has_coord(&layout.states, x, y) {
                true => layout.event(Event::Release(x, y)),
                false => layout.event(Event::Press(x, y)),
            };
        }
    };
}

fn states_has_coord<T>(states: &[State<T>], x: u8, y: u16) -> bool {
    states.iter().any(|s| match s {
        State::NormalKey { coord, .. }
        | State::LayerModifier { coord, .. }
        | State::Custom { coord, .. }
        | State::RepeatingSequence { coord, .. } => *coord == (x, y),
        _ => false,
    })
}
