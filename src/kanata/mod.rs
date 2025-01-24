//! Implements the glue between OS input/output and keyberon state management.

#[cfg(all(target_os = "windows", feature = "gui"))]
use crate::gui::win::*;
use anyhow::{bail, Result};
use kanata_parser::sequences::*;
use log::{error, info};
use parking_lot::Mutex;
use std::sync::mpsc::{Receiver, SyncSender as Sender, TryRecvError};

#[cfg(feature = "passthru_ahk")]
use std::sync::mpsc::Sender as ASender;

use kanata_keyberon::action::ReleasableState;
use kanata_keyberon::key_code::*;
use kanata_keyberon::layout::{CustomEvent, Event, Layout, State};

use std::path::PathBuf;
use std::sync::Arc;
use std::time;

use crate::oskbd::{KeyEvent, *};
#[cfg(feature = "tcp_server")]
use crate::tcp_server::simple_sexpr_to_json_array;
#[cfg(feature = "tcp_server")]
use crate::SocketAddrWrapper;
use crate::ValidatedArgs;
use kanata_parser::cfg;
use kanata_parser::cfg::list_actions::*;
use kanata_parser::cfg::*;
use kanata_parser::custom_action::*;
pub use kanata_parser::keys::*;
use kanata_tcp_protocol::ServerMessage;

mod dynamic_macro;
use dynamic_macro::*;

mod key_repeat;

mod sequences;
use sequences::*;

pub mod cfg_forced;
use cfg_forced::*;

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
use macos::*;

mod output_logic;
use output_logic::*;

#[cfg(target_os = "unknown")]
mod unknown;
#[cfg(target_os = "unknown")]
use unknown::*;

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
    /// This can be cleared and reused in various procedures as buffer space.
    pub cur_keys: Vec<KeyCode>,
    /// Reusable vec (to save on allocations) that stores the active output keys from the previous
    /// tick. This must only be updated once per tick and must not be modified outside of the one
    /// procedure that updates it.
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
    /// The user configuration for sequences be permanently on.
    pub sequence_always_on: bool,
    /// Default sequence input mode for use with always-on.
    pub sequence_input_mode: SequenceInputMode,
    /// Default sequence timeout for use with always-on.
    pub sequence_timeout: u16,
    /// Tracks sequence progress. Is Some(...) when in sequence mode and None otherwise.
    pub sequence_state: SequenceState,
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
    last_tick: instant::Instant,
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
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    /// Tracks the Linux/Macos user configuration for device names (instead of paths) that should be
    /// excluded for interception and processing by kanata.
    pub exclude_names: Option<Vec<String>>,
    #[cfg(target_os = "windows")]
    /// Tracks whether Kanata should try to synchronize keystates with the Windows OS.
    /// Has no effect on Interception. Fixes some use cases related to admin window permissions and
    /// potentially locking via Win+L.
    pub windows_sync_keystates: bool,
    #[cfg(all(feature = "interception_driver", target_os = "windows"))]
    /// Used to know which input device to treat as a mouse for intercepting and processing inputs
    /// by kanata.
    intercept_mouse_hwids: Option<Vec<[u8; HWID_ARR_SZ]>>,
    #[cfg(all(feature = "interception_driver", target_os = "windows"))]
    /// Used to know which mouse input devices to exclude from processing inputs by kanata. This is
    /// mutually exclusive from `intercept_mouse_hwids` and kanata will panic if both are included.
    intercept_mouse_hwids_exclude: Option<Vec<[u8; HWID_ARR_SZ]>>,
    #[cfg(all(feature = "interception_driver", target_os = "windows"))]
    /// Used to know which input device to treat as a keyboard for intercepting and processing inputs
    /// by kanata.
    intercept_kb_hwids: Option<Vec<[u8; HWID_ARR_SZ]>>,
    #[cfg(all(feature = "interception_driver", target_os = "windows"))]
    /// Used to know which keyboard input devices to exclude from processing inputs by kanata. This
    /// is mutually exclusive from `intercept_kb_hwids` and kanata will panic if both are included.
    intercept_kb_hwids_exclude: Option<Vec<[u8; HWID_ARR_SZ]>>,
    /// User configuration to do logging of layer changes or not.
    log_layer_changes: bool,
    /// Tracks the caps-word state. Is Some(...) if caps-word is active and None otherwise.
    pub caps_word: Option<CapsWordState>,
    /// Config items from `defcfg`.
    #[cfg(target_os = "linux")]
    pub x11_repeat_rate: Option<KeyRepeatSettings>,
    /// Determines what types of devices to grab based on autodetection mode.
    #[cfg(target_os = "linux")]
    pub device_detect_mode: DeviceDetectMode,
    /// Fake key actions that are waiting for a certain duration of keyboard idling.
    pub waiting_for_idle: HashSet<FakeKeyOnIdle>,
    /// Fake key actions that are being held and are pending release.
    /// The key is the coordinate and the value is the number of ticks until release should be
    /// done.
    pub vkeys_pending_release: HashMap<Coord, u16>,
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
    override_release_on_activation: bool,
    /// Configured maximum for dynamic macro recording, to protect users from themselves if they
    /// have accidentally left it on.
    dynamic_macro_max_presses: u16,
    /// Determines behaviour of replayed dynamic macros.
    dynamic_macro_replay_behaviour: ReplayBehaviour,
    /// Keys that should be unmodded. If non-empty, any modifier should be cleared.
    unmodded_keys: Vec<KeyCode>,
    /// Modifiers to be cleared in case the above is non-empty.
    unmodded_mods: UnmodMods,
    /// Keys that should be unshifted. If non-empty, left+right shift keys should be cleared.
    unshifted_keys: Vec<KeyCode>,
    /// Keep track of last pressed key for [`CustomAction::Repeat`].
    last_pressed_key: KeyCode,
    #[cfg(feature = "tcp_server")]
    /// Names of fake keys mapped to their index in the fake keys row
    pub virtual_keys: HashMap<String, usize>,
    /// The maximum value of switch's key-timing item in the configuration.
    pub switch_max_key_timing: u16,
    #[cfg(feature = "tcp_server")]
    tcp_server_address: Option<SocketAddrWrapper>,
    #[cfg(all(target_os = "windows", feature = "gui"))]
    /// Various GUI-related options.
    pub gui_opts: CfgOptionsGui,
    pub allow_hardware_repeat: bool,
    /// When > 0, it means macros should be cancelled on the next press.
    /// Upon cancelling this should be set to 0.
    pub macro_on_press_cancel_duration: u32,
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

use once_cell::sync::Lazy;

static MAPPED_KEYS: Lazy<Mutex<cfg::MappedKeys>> =
    Lazy::new(|| Mutex::new(cfg::MappedKeys::default()));

impl Kanata {
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
            #[cfg(target_os = "linux")]
            cfg.options.linux_opts.linux_use_trackpoint_property,
            #[cfg(target_os = "linux")]
            match cfg.options.linux_opts.linux_output_bus_type {
                LinuxCfgOutputBusType::BusUsb => evdev::BusType::BUS_USB,
                LinuxCfgOutputBusType::BusI8042 => evdev::BusType::BUS_I8042,
            },
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
                winapi::um::winbase::REALTIME_PRIORITY_CLASS,
            );
        }

        update_kbd_out(&cfg.options, &kbd_out)?;

        #[cfg(target_os = "windows")]
        set_win_altgr_behaviour(cfg.options.windows_opts.windows_altgr);

        *MAPPED_KEYS.lock() = cfg.mapped_keys;
        #[cfg(feature = "zippychord")]
        {
            zch().zch_configure(cfg.zippy.unwrap_or_default());
        }

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
            sequence_backtrack_modcancel: cfg.options.sequence_backtrack_modcancel,
            sequence_always_on: cfg.options.sequence_always_on,
            sequence_input_mode: cfg.options.sequence_input_mode,
            sequence_timeout: cfg.options.sequence_timeout,
            sequence_state: SequenceState::new(),
            sequences: cfg.sequences,
            last_tick: instant::Instant::now(),
            time_remainder: 0,
            live_reload_requested: false,
            overrides: cfg.overrides,
            override_states: OverrideStates::new(),
            #[cfg(target_os = "macos")]
            include_names: cfg.options.macos_opts.macos_dev_names_include,
            #[cfg(target_os = "macos")]
            exclude_names: cfg.options.macos_opts.macos_dev_names_exclude,
            #[cfg(target_os = "linux")]
            kbd_in_paths: cfg.options.linux_opts.linux_dev,
            #[cfg(target_os = "linux")]
            continue_if_no_devices: cfg.options.linux_opts.linux_continue_if_no_devs_found,
            #[cfg(target_os = "linux")]
            include_names: cfg.options.linux_opts.linux_dev_names_include,
            #[cfg(target_os = "linux")]
            exclude_names: cfg.options.linux_opts.linux_dev_names_exclude,
            #[cfg(target_os = "windows")]
            windows_sync_keystates: cfg.options.windows_opts.sync_keystates,
            #[cfg(all(feature = "interception_driver", target_os = "windows"))]
            intercept_mouse_hwids: cfg.options.wintercept_opts.windows_interception_mouse_hwids,
            #[cfg(all(feature = "interception_driver", target_os = "windows"))]
            intercept_mouse_hwids_exclude: cfg
                .options
                .wintercept_opts
                .windows_interception_mouse_hwids_exclude,
            #[cfg(all(feature = "interception_driver", target_os = "windows"))]
            intercept_kb_hwids: cfg
                .options
                .wintercept_opts
                .windows_interception_keyboard_hwids,
            #[cfg(all(feature = "interception_driver", target_os = "windows"))]
            intercept_kb_hwids_exclude: cfg
                .options
                .wintercept_opts
                .windows_interception_keyboard_hwids_exclude,
            dynamic_macro_replay_state: None,
            dynamic_macro_record_state: None,
            dynamic_macros: Default::default(),
            log_layer_changes: get_forced_log_layer_changes()
                .unwrap_or(cfg.options.log_layer_changes),
            caps_word: None,
            movemouse_smooth_diagonals: cfg.options.movemouse_smooth_diagonals,
            override_release_on_activation: cfg.options.override_release_on_activation,
            movemouse_inherit_accel_state: cfg.options.movemouse_inherit_accel_state,
            dynamic_macro_max_presses: cfg.options.dynamic_macro_max_presses,
            dynamic_macro_replay_behaviour: ReplayBehaviour {
                delay: cfg.options.dynamic_macro_replay_delay_behaviour,
            },
            #[cfg(target_os = "linux")]
            x11_repeat_rate: cfg.options.linux_opts.linux_x11_repeat_delay_rate,
            #[cfg(target_os = "linux")]
            device_detect_mode: cfg
                .options
                .linux_opts
                .linux_device_detect_mode
                .expect("parser should default to some"),
            waiting_for_idle: HashSet::default(),
            vkeys_pending_release: HashMap::default(),
            ticks_since_idle: 0,
            movemouse_buffer: None,
            unmodded_keys: vec![],
            unmodded_mods: UnmodMods::empty(),
            unshifted_keys: vec![],
            last_pressed_key: KeyCode::No,
            #[cfg(feature = "tcp_server")]
            virtual_keys: cfg.fake_keys,
            switch_max_key_timing: cfg.switch_max_key_timing,
            #[cfg(feature = "tcp_server")]
            tcp_server_address: args.tcp_server_address.clone(),
            #[cfg(all(target_os = "windows", feature = "gui"))]
            gui_opts: cfg.options.gui_opts,
            allow_hardware_repeat: cfg.options.allow_hardware_repeat,
            macro_on_press_cancel_duration: 0,
        })
    }

    /// Create a new configuration from a file, wrapped in an Arc<Mutex<_>>
    pub fn new_arc(args: &ValidatedArgs) -> Result<Arc<Mutex<Self>>> {
        Ok(Arc::new(Mutex::new(Self::new(args)?)))
    }

    pub fn new_from_str(cfg: &str, file_content: HashMap<String, String>) -> Result<Self> {
        let cfg = match cfg::new_from_str(cfg, file_content) {
            Ok(c) => c,
            Err(e) => {
                bail!("{e:?}");
            }
        };

        let kbd_out = match KbdOut::new(
            #[cfg(target_os = "linux")]
            &None,
            #[cfg(target_os = "linux")]
            cfg.options.linux_opts.linux_use_trackpoint_property,
            #[cfg(target_os = "linux")]
            match cfg.options.linux_opts.linux_output_bus_type {
                LinuxCfgOutputBusType::BusUsb => evdev::BusType::BUS_USB,
                LinuxCfgOutputBusType::BusI8042 => evdev::BusType::BUS_I8042,
            },
        ) {
            Ok(kbd_out) => kbd_out,
            Err(err) => {
                error!("Failed to open the output uinput device. Make sure you've added the user executing kanata to the `uinput` group");
                bail!(err)
            }
        };

        *MAPPED_KEYS.lock() = cfg.mapped_keys;
        #[cfg(feature = "zippychord")]
        {
            zch().zch_configure(cfg.zippy.unwrap_or_default());
        }

        Ok(Self {
            kbd_out,
            cfg_paths: vec!["config string".into()],
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
            sequence_backtrack_modcancel: cfg.options.sequence_backtrack_modcancel,
            sequence_always_on: cfg.options.sequence_always_on,
            sequence_input_mode: cfg.options.sequence_input_mode,
            sequence_timeout: cfg.options.sequence_timeout,
            sequence_state: SequenceState::new(),
            sequences: cfg.sequences,
            last_tick: instant::Instant::now(),
            time_remainder: 0,
            live_reload_requested: false,
            overrides: cfg.overrides,
            override_states: OverrideStates::new(),
            #[cfg(target_os = "macos")]
            include_names: cfg.options.macos_opts.macos_dev_names_include,
            #[cfg(target_os = "macos")]
            exclude_names: cfg.options.macos_opts.macos_dev_names_exclude,
            #[cfg(target_os = "linux")]
            kbd_in_paths: cfg.options.linux_opts.linux_dev,
            #[cfg(target_os = "linux")]
            continue_if_no_devices: cfg.options.linux_opts.linux_continue_if_no_devs_found,
            #[cfg(target_os = "linux")]
            include_names: cfg.options.linux_opts.linux_dev_names_include,
            #[cfg(target_os = "linux")]
            exclude_names: cfg.options.linux_opts.linux_dev_names_exclude,
            #[cfg(target_os = "windows")]
            windows_sync_keystates: cfg.options.windows_opts.sync_keystates,
            #[cfg(all(feature = "interception_driver", target_os = "windows"))]
            intercept_mouse_hwids: cfg.options.wintercept_opts.windows_interception_mouse_hwids,
            #[cfg(all(feature = "interception_driver", target_os = "windows"))]
            intercept_mouse_hwids_exclude: cfg
                .options
                .wintercept_opts
                .windows_interception_mouse_hwids_exclude,
            #[cfg(all(feature = "interception_driver", target_os = "windows"))]
            intercept_kb_hwids: cfg
                .options
                .wintercept_opts
                .windows_interception_keyboard_hwids,
            #[cfg(all(feature = "interception_driver", target_os = "windows"))]
            intercept_kb_hwids_exclude: cfg
                .options
                .wintercept_opts
                .windows_interception_keyboard_hwids_exclude,
            dynamic_macro_replay_state: None,
            dynamic_macro_record_state: None,
            dynamic_macros: Default::default(),
            log_layer_changes: get_forced_log_layer_changes()
                .unwrap_or(cfg.options.log_layer_changes),
            caps_word: None,
            movemouse_smooth_diagonals: cfg.options.movemouse_smooth_diagonals,
            override_release_on_activation: cfg.options.override_release_on_activation,
            movemouse_inherit_accel_state: cfg.options.movemouse_inherit_accel_state,
            dynamic_macro_max_presses: cfg.options.dynamic_macro_max_presses,
            dynamic_macro_replay_behaviour: ReplayBehaviour {
                delay: cfg.options.dynamic_macro_replay_delay_behaviour,
            },
            #[cfg(target_os = "linux")]
            x11_repeat_rate: cfg.options.linux_opts.linux_x11_repeat_delay_rate,
            #[cfg(target_os = "linux")]
            device_detect_mode: cfg
                .options
                .linux_opts
                .linux_device_detect_mode
                .expect("parser should default to some"),
            waiting_for_idle: HashSet::default(),
            vkeys_pending_release: HashMap::default(),
            ticks_since_idle: 0,
            movemouse_buffer: None,
            unmodded_keys: vec![],
            unmodded_mods: UnmodMods::empty(),
            unshifted_keys: vec![],
            last_pressed_key: KeyCode::No,
            #[cfg(feature = "tcp_server")]
            virtual_keys: cfg.fake_keys,
            switch_max_key_timing: cfg.switch_max_key_timing,
            #[cfg(feature = "tcp_server")]
            tcp_server_address: None,
            #[cfg(all(target_os = "windows", feature = "gui"))]
            gui_opts: cfg.options.gui_opts,
            allow_hardware_repeat: cfg.options.allow_hardware_repeat,
            macro_on_press_cancel_duration: 0,
        })
    }

    #[cfg(feature = "passthru_ahk")]
    pub fn new_with_output_channel(
        args: &ValidatedArgs,
        tx: Option<ASender<InputEvent>>,
    ) -> Result<Arc<Mutex<Self>>> {
        let mut k = Self::new(args)?;
        k.kbd_out.tx_kout = tx;
        Ok(Arc::new(Mutex::new(k)))
    }

    fn do_live_reload(&mut self, _tx: &Option<Sender<ServerMessage>>) -> Result<()> {
        let cfg = match cfg::new_from_file(&self.cfg_paths[self.cur_cfg_idx]) {
            Ok(c) => c,
            Err(e) => {
                log::error!("{e:?}");
                bail!("failed to parse config file");
            }
        };
        update_kbd_out(&cfg.options, &self.kbd_out)?;
        #[cfg(target_os = "windows")]
        set_win_altgr_behaviour(cfg.options.windows_opts.windows_altgr);
        self.sequence_backtrack_modcancel = cfg.options.sequence_backtrack_modcancel;
        self.sequence_always_on = cfg.options.sequence_always_on;
        self.sequence_input_mode = cfg.options.sequence_input_mode;
        self.sequence_timeout = cfg.options.sequence_timeout;
        self.layout = cfg.layout;
        self.key_outputs = cfg.key_outputs;
        self.layer_info = cfg.layer_info;
        self.sequences = cfg.sequences;
        self.overrides = cfg.overrides;
        self.log_layer_changes =
            get_forced_log_layer_changes().unwrap_or(cfg.options.log_layer_changes);
        self.movemouse_smooth_diagonals = cfg.options.movemouse_smooth_diagonals;
        self.override_release_on_activation = cfg.options.override_release_on_activation;
        self.movemouse_inherit_accel_state = cfg.options.movemouse_inherit_accel_state;
        self.dynamic_macro_max_presses = cfg.options.dynamic_macro_max_presses;
        self.dynamic_macro_replay_behaviour = ReplayBehaviour {
            delay: cfg.options.dynamic_macro_replay_delay_behaviour,
        };
        self.switch_max_key_timing = cfg.switch_max_key_timing;
        #[cfg(feature = "tcp_server")]
        {
            self.virtual_keys = cfg.fake_keys;
        }
        #[cfg(target_os = "windows")]
        {
            self.windows_sync_keystates = cfg.options.windows_opts.sync_keystates;
        }
        #[cfg(all(target_os = "windows", feature = "gui"))]
        {
            self.gui_opts.tray_icon = cfg.options.gui_opts.tray_icon;
            self.gui_opts.icon_match_layer_name = cfg.options.gui_opts.icon_match_layer_name;
            self.gui_opts.tooltip_layer_changes = cfg.options.gui_opts.tooltip_layer_changes;
            self.gui_opts.tooltip_no_base = cfg.options.gui_opts.tooltip_no_base;
            self.gui_opts.tooltip_show_blank = cfg.options.gui_opts.tooltip_show_blank;
            self.gui_opts.tooltip_duration = cfg.options.gui_opts.tooltip_duration;
            self.gui_opts.notify_cfg_reload = cfg.options.gui_opts.notify_cfg_reload;
            self.gui_opts.notify_cfg_reload_silent = cfg.options.gui_opts.notify_cfg_reload_silent;
            self.gui_opts.notify_error = cfg.options.gui_opts.notify_error;
            self.gui_opts.tooltip_size = cfg.options.gui_opts.tooltip_size;
        }
        #[cfg(feature = "zippychord")]
        {
            zch().zch_configure(cfg.zippy.unwrap_or_default());
        }

        *MAPPED_KEYS.lock() = cfg.mapped_keys;
        #[cfg(target_os = "linux")]
        Kanata::set_repeat_rate(cfg.options.linux_opts.linux_x11_repeat_delay_rate)?;
        log::info!("Live reload successful");
        #[cfg(feature = "tcp_server")]
        if let Some(tx) = _tx {
            match tx.try_send(ServerMessage::ConfigFileReload {
                new: self.cfg_paths[self.cur_cfg_idx]
                    .to_str()
                    .unwrap()
                    .to_string(),
            }) {
                Ok(_) => {}
                Err(error) => {
                    log::error!(
                        "could not send ConfigFileReload event notification: {}",
                        error
                    );
                }
            }
        }

        let cur_layer = self.layout.bm().current_layer();
        self.prev_layer = cur_layer;
        self.print_layer(cur_layer);
        self.macro_on_press_cancel_duration = 0;

        #[cfg(not(target_os = "linux"))]
        {
            PRESSED_KEYS.lock().clear();
        }

        #[cfg(feature = "tcp_server")]
        if let Some(tx) = _tx {
            let new = self.layer_info[cur_layer].name.clone();
            match tx.try_send(ServerMessage::LayerChange { new }) {
                Ok(_) => {}
                Err(error) => {
                    log::error!("could not send LayerChange event notification: {}", error);
                }
            }
        }
        #[cfg(all(target_os = "windows", feature = "gui"))]
        send_gui_cfg_notice();
        Ok(())
    }

    /// Update keyberon layout state for press/release, handle repeat separately
    pub fn handle_input_event(&mut self, event: &KeyEvent) -> Result<()> {
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
                if self.macro_on_press_cancel_duration > 0 {
                    log::debug!("cancelling all macros: other press");
                    self.macro_on_press_cancel_duration = 0;
                    let layout = self.layout.bm();
                    layout.active_sequences.clear();
                    layout.states.retain(|s| {
                        !matches!(s, State::FakeKey { .. } | State::RepeatingSequence { .. })
                    });
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
            KeyValue::WakeUp => {
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
        let now = instant::Instant::now();
        let ns_elapsed = now.duration_since(self.last_tick).as_nanos();
        let ns_elapsed_with_rem = ns_elapsed + self.time_remainder;
        let ms_elapsed = ns_elapsed_with_rem / NS_IN_MS;
        self.time_remainder = ns_elapsed_with_rem % NS_IN_MS;

        self.tick_ms(ms_elapsed, tx)?;

        self.last_tick = match ms_elapsed {
            0 => self.last_tick,
            1..=10 => now,
            // If too many ms elapsed, probably doing a tight loop of something that's quite
            // expensive, e.g. click spamming. To avoid a growing ms_elapsed due to trying and
            // failing to catch up, reset last_tick to the "actual now" instead the "past now"
            // even though that means ticks will be missed - meaning there will be fewer than
            // 1000 ticks in 1ms on average. In practice, there will already be fewer than 1000
            // ticks in 1ms when running expensive operations, this just avoids having tens to
            // thousands of ticks all happening as soon as the expensive operations end.
            _ => instant::Instant::now(),
        };

        self.check_handle_layer_change(tx);

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
            if let Err(e) = self.do_live_reload(tx) {
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

    pub fn tick_ms(&mut self, ms_elapsed: u128, _tx: &Option<Sender<ServerMessage>>) -> Result<()> {
        let mut extra_ticks: u16 = 0;
        for _ in 0..ms_elapsed {
            self.tick_states(_tx)?;
            if let Some(event) = tick_replay_state(
                &mut self.dynamic_macro_replay_state,
                self.dynamic_macro_replay_behaviour,
            ) {
                self.layout.bm().event(event.key_event());
                extra_ticks = extra_ticks.saturating_add(event.delay());
                log::debug!("dyn macro extra ticks: {extra_ticks}, ms_elapsed: {ms_elapsed}");
            }
        }
        for i in 0..(extra_ticks.saturating_sub(ms_elapsed as u16)) {
            self.tick_states(_tx)?;
            if tick_replay_state(
                &mut self.dynamic_macro_replay_state,
                self.dynamic_macro_replay_behaviour,
            )
            .is_some()
            {
                log::error!("overshot to next event at iteration #{i}, the code is broken!");
                break;
            }
        }
        Ok(())
    }

    fn tick_held_vkeys(&mut self) {
        if self.vkeys_pending_release.is_empty() {
            return;
        }
        let layout = self.layout.bm();
        self.vkeys_pending_release.retain(|coord, deadline| {
            *deadline = deadline.saturating_sub(1);
            match deadline {
                0 => {
                    layout.event(Event::Release(coord.x, coord.y));
                    false
                }
                _ => true,
            }
        });
    }

    fn tick_states(&mut self, _tx: &Option<Sender<ServerMessage>>) -> Result<()> {
        self.live_reload_requested |= self.handle_keystate_changes(_tx)?;
        self.handle_scrolling()?;
        self.handle_move_mouse()?;
        self.tick_sequence_state()?;
        self.tick_idle_timeout();
        self.macro_on_press_cancel_duration = self.macro_on_press_cancel_duration.saturating_sub(1);
        tick_record_state(&mut self.dynamic_macro_record_state);
        zippy_tick(self.caps_word.is_some());
        self.prev_keys.clear();
        self.prev_keys.append(&mut self.cur_keys);
        self.tick_held_vkeys();
        #[cfg(feature = "simulated_output")]
        {
            self.kbd_out.tick();
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
        if let Some(state) = self.sequence_state.get_active() {
            state.ticks_until_timeout -= 1;
            if state.ticks_until_timeout == 0 {
                log::debug!("sequence timeout; exiting sequence state");
                cancel_sequence(state, &mut self.kbd_out)?;
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
    fn handle_keystate_changes(&mut self, _tx: &Option<Sender<ServerMessage>>) -> Result<bool> {
        let layout = self.layout.bm();
        let custom_event = layout.tick();
        let mut live_reload_requested = false;
        let cur_keys = &mut self.cur_keys;
        cur_keys.extend(layout.keycodes());
        let mut reverse_release_order = false;

        // Deal with unmodded. Unlike other custom actions, this should come before key presses and
        // releases. I don't quite remember why custom actions come after the key processing, but I
        // remember that it is intentional. However, since unmodded needs to modify the key lists,
        // it should come before.
        match custom_event {
            CustomEvent::Press(custacts) => {
                for custact in custacts.iter() {
                    match custact {
                        CustomAction::Unmodded { keys, mods } => {
                            self.unmodded_keys.extend(keys.iter());
                            self.unmodded_mods = *mods;
                        }
                        CustomAction::Unshifted { keys } => {
                            self.unshifted_keys.extend(keys.iter());
                        }
                        _ => {}
                    }
                }
            }
            CustomEvent::Release(custacts) => {
                for custact in custacts.iter() {
                    match custact {
                        CustomAction::Unmodded { keys, mods: _ } => {
                            self.unmodded_keys.retain(|k| !keys.contains(k));
                        }
                        CustomAction::Unshifted { keys } => {
                            self.unshifted_keys.retain(|k| !keys.contains(k));
                        }
                        CustomAction::ReverseReleaseOrder => {
                            reverse_release_order = true;
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        if !self.unmodded_keys.is_empty() {
            for mod_key in self.unmodded_mods.iter() {
                let kc = match mod_key {
                    UnmodMods::LSft => KeyCode::LShift,
                    UnmodMods::RSft => KeyCode::RShift,
                    UnmodMods::LAlt => KeyCode::LAlt,
                    UnmodMods::RAlt => KeyCode::RAlt,
                    UnmodMods::LCtl => KeyCode::LCtrl,
                    UnmodMods::RCtl => KeyCode::RCtrl,
                    UnmodMods::LMet => KeyCode::LGui,
                    UnmodMods::RMet => KeyCode::RGui,
                    _ => unreachable!("all bits of u8 should be covered"), // test_unmodmods_bits
                };
                cur_keys.retain(|k| *k != kc);
            }
            cur_keys.extend(self.unmodded_keys.iter());
        }
        if !self.unshifted_keys.is_empty() {
            cur_keys.retain(|k| !matches!(k, KeyCode::LShift | KeyCode::RShift));
            cur_keys.extend(self.unshifted_keys.iter());
        }

        self.overrides
            .override_keys(cur_keys, &mut self.override_states);
        mark_overridden_nonmodkeys_for_eager_erasure(&self.override_states, &mut layout.states);
        if self.override_release_on_activation {
            for removed in self.override_states.removed_oscs() {
                if !removed.is_modifier() {
                    layout.states.retain(|s| {
                        s.release_state(ReleasableState::KeyCode(removed.into()))
                            .is_some()
                    });
                }
            }
        }

        if let Some(caps_word) = &mut self.caps_word {
            if caps_word.maybe_add_lsft(cur_keys) == CapsWordNextState::End {
                self.caps_word = None;
            }
        }

        // Release keys that do not exist in the current state but exist in the previous state.
        // This used to use a HashSet but it was changed to a Vec because the order of operations
        // matters.
        //
        // BUG(sequences):
        //
        // With hidden-delay-type or hidden-suppressed,
        // sequences will unexpectedly send releases
        // for the presses that would otherwise have happened.
        // This is because the press is skipped but the keys make it
        // into `self.prev_keys` and the OS release event is sent in the code below.
        //
        // There haven't been any reports of negative consequences of this behaviour,
        // but it is unusual and ideally wouldn't happen, so I tried to fix it anyway.
        // But I was unsuccessful. Approach tried:
        //
        // - clear `self.cur_keys` and `layout.states` of outputted keys
        //   when a sequence is active, for the impacted sequence modes.
        //
        // This approach fails because it keeping `layout.states` intact
        // is necessary to complete chorded sequences, e.g. `S-(a b c)`.
        // Clearing the `lsft` means the above sequence is impossible to complete.
        //
        // Another approach that might work, which has not been attempted,
        // is to keep track of oskbd events that have actually been sent.
        // Then, a release can only be sent if an un-released corresponding press
        // has been pressed in the past.
        // However, this doesn't seem worth the:
        //
        // - runtime cost
        // - work involved to add the code
        // - ongoing burden of maintaining that code
        //
        // Given that there appears to be no practical negative consequences for this bug
        // remaining.
        log::trace!("{:?}", &self.prev_keys);
        let mut fwd_release = self.prev_keys.iter();
        let mut rev_release = self.prev_keys.iter().rev();
        let keys: &mut dyn Iterator<Item = &KeyCode> = match reverse_release_order {
            false => &mut fwd_release,
            true => &mut rev_release,
        };
        for k in keys {
            if cur_keys.contains(k) {
                continue;
            }
            log::debug!("key release   {:?}", k);
            if let Err(e) = release_key(&mut self.kbd_out, k.into()) {
                bail!("failed to release key: {:?}", e);
            }
        }

        if cur_keys.is_empty() && !self.prev_keys.is_empty() {
            if let Some(state) = self.sequence_state.get_active() {
                use kanata_parser::trie::GetOrDescendentExistsResult::*;
                state.overlapped_sequence.push(KEY_OVERLAP_MARKER);
                match self
                    .sequences
                    .get_or_descendant_exists(&state.overlapped_sequence)
                {
                    HasValue((i, j)) => {
                        do_successful_sequence_termination(
                            &mut self.kbd_out,
                            state,
                            layout,
                            i,
                            j,
                            EndSequenceType::Overlap,
                        )?;
                    }
                    NotInTrie => {
                        // Overwrite overlapped with non-overlapped tracking
                        state.overlapped_sequence.clear();
                        state
                            .overlapped_sequence
                            .extend(state.sequence.iter().copied());
                    }
                    InTrie => {}
                }
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

            if self.sequence_always_on && self.sequence_state.is_inactive() {
                self.sequence_state
                    .activate(self.sequence_input_mode, self.sequence_timeout);
            }

            if let Some(state) = self.sequence_state.get_active() {
                do_sequence_press_logic(
                    state,
                    k,
                    get_mod_mask_for_cur_keys(cur_keys),
                    &mut self.kbd_out,
                    &self.sequences,
                    self.sequence_backtrack_modcancel,
                    layout,
                )?;
            } else {
                log::debug!("key press     {:?}", k);
                if let Err(e) = press_key(&mut self.kbd_out, k.into()) {
                    bail!("failed to press key: {:?}", e);
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
                        CustomAction::LiveReloadNum(n) => {
                            let n = usize::from(*n);
                            live_reload_requested = true;
                            match self.cfg_paths.get(n) {
                                Some(path) => {
                                    self.cur_cfg_idx = n;
                                    log::info!("Requested live reload of file: {}", path.display(),);
                                }
                                None => {
                                    log::error!("Requested live reload of config file number {}, but only {} config files were passed", n+1, self.cfg_paths.len());
                                }
                            }
                        }
                        CustomAction::LiveReloadFile(path) => {
                            let path = PathBuf::from(path);

                            let result = self
                                .cfg_paths
                                .iter()
                                .enumerate()
                                .find(|(_idx, fpath)| **fpath == path);

                            match result {
                                Some((index, _path)) => {
                                    log::info!(
                                        "Requested live reload of file with path: {}",
                                        path.display(),
                                    );
                                    live_reload_requested = true;
                                    self.cur_cfg_idx = index;
                                }
                                None => {
                                    log::error!("Requested live reload of file with path {}, but no such path was passed as an argument to Kanata", path.display());
                                }
                            }
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
                            cmds.push((
                                Some(log::Level::Info),
                                Some(log::Level::Error),
                                _cmd.clone(),
                            ));
                        }
                        CustomAction::CmdLog(_log_level, _error_log_level, _cmd) => {
                            #[cfg(feature = "cmd")]
                            cmds.push((
                                _log_level.get_level(),
                                _error_log_level.get_level(),
                                _cmd.clone(),
                            ));
                        }
                        CustomAction::CmdOutputKeys(_cmd) => {
                            #[cfg(feature = "cmd")]
                            {
                                let cmd = _cmd.clone();
                                // Maybe improvement in the future:
                                // A delay here, as in KeyAction::Delay, will pause the entire
                                // state machine loop. That is _probably_ OK, but ideally this
                                // would be done in a separate thread or somehow
                                for key_action in keys_for_cmd_output(&cmd) {
                                    match key_action {
                                        KeyAction::Press(osc) => press_key(&mut self.kbd_out, osc)?,
                                        KeyAction::Release(osc) => {
                                            release_key(&mut self.kbd_out, osc)?
                                        }
                                        KeyAction::Delay(delay) => std::thread::sleep(
                                            std::time::Duration::from_millis(u64::from(delay)),
                                        ),
                                    }
                                }
                            }
                        }
                        CustomAction::PushMessage(_message) => {
                            log::debug!("Action push-msg");
                            #[cfg(feature = "tcp_server")]
                            if let Some(tx) = _tx {
                                let message = simple_sexpr_to_json_array(_message);
                                log::debug!("Action push-msg message: {}", message);
                                match tx.try_send(ServerMessage::MessagePush { message }) {
                                    Ok(_) => {}
                                    Err(error) => {
                                        log::error!(
                                            "could not send {} event notification: {}",
                                            PUSH_MESSAGE,
                                            error
                                        );
                                    }
                                }
                            }
                            #[cfg(feature = "tcp_server")]
                            if self.tcp_server_address.is_none() {
                                log::warn!("{} was used, but TCP server is not running. did you specify a port?", PUSH_MESSAGE);
                            }
                            #[cfg(not(feature = "tcp_server"))]
                            log::warn!(
                                "{} was used, but Kanata was compiled with TCP server disabled.",
                                PUSH_MESSAGE
                            );
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
                            std::thread::sleep(time::Duration::from_millis((*delay).into()));
                        }
                        CustomAction::SequenceCancel => {
                            if let Some(state) = self.sequence_state.get_active() {
                                log::debug!("pressed cancel sequence key");
                                cancel_sequence(state, &mut self.kbd_out)?;
                            }
                        }
                        CustomAction::SequenceLeader(timeout, input_mode) => {
                            if self.sequence_state.is_inactive() {
                                log::debug!("entering sequence mode");
                                self.sequence_state.activate(*input_mode, *timeout);
                            } else if *input_mode == SequenceInputMode::HiddenSuppressed {
                                log::debug!("retriggering sequence mode");
                                self.sequence_state.activate(*input_mode, *timeout);
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
                                        press_key(&mut self.kbd_out, OsCode::KEY_LEFTSHIFT)?;
                                    }
                                }
                            }
                            // Release key in case the most recently pressed key is still pressed.
                            release_key(&mut self.kbd_out, osc)?;
                            press_key(&mut self.kbd_out, osc)?;
                            release_key(&mut self.kbd_out, osc)?;
                            if do_caps_word {
                                self.kbd_out.release_key(OsCode::KEY_LEFTSHIFT)?;
                            }
                        }
                        CustomAction::DynamicMacroRecord(macro_id) => {
                            if let Some((macro_id, prev_recorded_macro)) =
                                begin_record_macro(*macro_id, &mut self.dynamic_macro_record_state)
                            {
                                log::debug!("saving macro {prev_recorded_macro:?}");
                                self.dynamic_macros.insert(macro_id, prev_recorded_macro);
                            }
                        }
                        CustomAction::DynamicMacroRecordStop(num_actions_to_remove) => {
                            if let Some((macro_id, prev_recorded_macro)) = stop_macro(
                                &mut self.dynamic_macro_record_state,
                                *num_actions_to_remove,
                            ) {
                                log::debug!("saving macro {prev_recorded_macro:?}");
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
                        CustomAction::CancelMacroOnNextPress(duration) => {
                            self.macro_on_press_cancel_duration = *duration;
                        }
                        CustomAction::SendArbitraryCode(code) => {
                            self.kbd_out.write_code(*code as u32, KeyValue::Press)?;
                        }
                        CustomAction::CapsWord(cfg) => match cfg.repress_behaviour {
                            CapsWordRepressBehaviour::Overwrite => {
                                self.caps_word = Some(CapsWordState::new(cfg));
                            }
                            CapsWordRepressBehaviour::Toggle => {
                                self.caps_word = match self.caps_word {
                                    Some(_) => None,
                                    None => Some(CapsWordState::new(cfg)),
                                };
                            }
                        },
                        CustomAction::SetMouse { x, y } => {
                            self.kbd_out.set_mouse(*x, *y)?;
                        }
                        CustomAction::FakeKeyOnIdle(fkd) => {
                            self.ticks_since_idle = 0;
                            self.waiting_for_idle.insert(*fkd);
                        }
                        CustomAction::FakeKeyHoldForDuration(fk_hfd) => {
                            let duration = fk_hfd.hold_duration;
                            self.vkeys_pending_release.entry(fk_hfd.coord)
                                .and_modify(|d| *d = duration)
                                .or_insert_with(|| {
                                    let Coord { x, y } = fk_hfd.coord;
                                    layout.event(Event::Press(x, y));
                                    duration
                                });
                        }
                        CustomAction::FakeKeyOnRelease { .. }
                        | CustomAction::DelayOnRelease(_)
                        | CustomAction::Unmodded { .. }
                        | CustomAction::Unshifted { .. }
                        // Note: ReverseReleaseOrder is already handled earlier on.
                        | CustomAction::ReverseReleaseOrder { .. }
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
                            std::thread::sleep(time::Duration::from_millis((*delay).into()));
                            pbtn
                        }
                        CustomAction::FakeKeyOnRelease { coord, action } => {
                            let (x, y) = (coord.x, coord.y);
                            log::debug!("fake key on release {action:?} {x:?},{y:?}");
                            handle_fakekey_action(*action, layout, x, y);
                            pbtn
                        }
                        CustomAction::CancelMacroOnRelease => {
                            log::debug!("cancelling all macros: releasable macro");
                            layout.active_sequences.clear();
                            self.macro_on_press_cancel_duration = 0;
                            layout.states.retain(|s| {
                                !matches!(
                                    s,
                                    State::FakeKey { .. } | State::RepeatingSequence { .. }
                                )
                            });
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
            #[cfg(all(target_os = "windows", feature = "gui"))]
            send_gui_notice();
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
                            match client.write_all(&notification) {
                                Ok(_) => {
                                    log::debug!("layer change notification sent");
                                }
                                Err(e) => {
                                    log::warn!(
                                        "removing tcp client where write failed: {id}, {e:?}"
                                    );
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

            #[cfg(not(feature = "passthru_ahk"))]
            info!(
                "You may forcefully exit kanata by pressing lctl+spc+esc at any time. \
                        These keys refer to defsrc input, meaning BEFORE kanata remaps keys."
            );

            #[cfg(all(not(feature = "interception_driver"), target_os = "windows"))]
            let mut idle_clear_happened = false;
            #[cfg(all(not(feature = "interception_driver"), target_os = "windows"))]
            let mut last_input_time = instant::Instant::now();

            let err = loop {
                let can_block = {
                    let mut k = kanata.lock();
                    k.can_block_update_idle_waiting(ms_elapsed)
                };
                if can_block {
                    #[cfg(all(
                        target_os = "windows",
                        not(feature = "interception_driver"),
                        not(feature = "simulated_input"),
                    ))]
                    kanata.lock().win_synchronize_keystates();

                    log::trace!("blocking on channel");
                    match rx.recv() {
                        Ok(kev) => {
                            let mut k = kanata.lock();
                            let now = instant::Instant::now()
                                .checked_sub(time::Duration::from_millis(1))
                                .expect("subtract 1ms from current time");

                            #[cfg(all(
                                not(feature = "interception_driver"),
                                target_os = "windows"
                            ))]
                            {
                                // If kanata has been inactive for long enough, clear all states.
                                // This won't trigger if there are macros running, or if a key is
                                // held down for a long time and is sending OS repeats. The reason
                                // for this code is in cases like Win+L which locks the Windows
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
                                if (now - last_input_time)
                                    > time::Duration::from_secs(LLHOOK_IDLE_TIME_SECS_CLEAR_INPUTS)
                                {
                                    log::debug!(
                                        "clearing keyberon normal key states due to inactivity"
                                    );
                                    let layout = k.layout.bm();
                                    release_normalkey_states(layout);
                                    PRESSED_KEYS.lock().clear();
                                }
                            }
                            k.last_tick = now;

                            #[cfg(feature = "perf_logging")]
                            let start = instant::Instant::now();

                            if let Err(e) = k.handle_input_event(&kev) {
                                break e;
                            }
                            #[cfg(all(
                                not(feature = "interception_driver"),
                                target_os = "windows"
                            ))]
                            {
                                last_input_time = now;
                            }
                            #[cfg(all(
                                not(feature = "interception_driver"),
                                target_os = "windows"
                            ))]
                            {
                                idle_clear_happened = false;
                            }

                            #[cfg(feature = "perf_logging")]
                            log::info!(
                                "[PERF]: handle key event: {} ns",
                                (start.elapsed()).as_nanos()
                            );
                            #[cfg(feature = "perf_logging")]
                            let start = instant::Instant::now();

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
                            let start = instant::Instant::now();

                            if let Err(e) = k.handle_input_event(&kev) {
                                break e;
                            }
                            #[cfg(all(
                                not(feature = "interception_driver"),
                                target_os = "windows"
                            ))]
                            {
                                last_input_time = instant::Instant::now();
                            }
                            #[cfg(all(
                                not(feature = "interception_driver"),
                                target_os = "windows"
                            ))]
                            {
                                idle_clear_happened = false;
                            }

                            #[cfg(feature = "perf_logging")]
                            log::info!(
                                "[PERF]: handle key event: {} ns",
                                (start.elapsed()).as_nanos()
                            );
                            #[cfg(feature = "perf_logging")]
                            let start = instant::Instant::now();

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
                            let start = instant::Instant::now();

                            match k.handle_time_ticks(&tx) {
                                Ok(ms) => ms_elapsed = ms,
                                Err(e) => break e,
                            };

                            #[cfg(feature = "perf_logging")]
                            log::info!(
                                "[PERF]: handle time ticks: {} ns",
                                (start.elapsed()).as_nanos()
                            );

                            #[cfg(all(
                                not(feature = "interception_driver"),
                                target_os = "windows"
                            ))]
                            {
                                // If kanata has been inactive for long enough, clear all states.
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
                                if (instant::Instant::now() - (last_input_time))
                                    > time::Duration::from_secs(LLHOOK_IDLE_TIME_SECS_CLEAR_INPUTS)
                                    && !idle_clear_happened
                                {
                                    idle_clear_happened = true;
                                    log::debug!(
                                        "clearing keyberon normal key states due to inactivity"
                                    );
                                    let layout = k.layout.bm();
                                    release_normalkey_states(layout);
                                    PRESSED_KEYS.lock().clear();
                                }
                            }

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

    /// Returns `true` if kanata's processing thread loop can block on the channel instead of doing
    /// a non-blocking channel read and then sleeping for ~1ms.
    ///
    /// In addition to doing the logic for the above, this mutates the `waiting_for_idle` state
    /// used by the `on-idle` action for virtual keys.
    pub fn can_block_update_idle_waiting(&mut self, ms_elapsed: u16) -> bool {
        let k = self;
        let is_idle = k.is_idle();
        // Note: checking waiting_for_idle can not be part of the computation for
        // is_idle() since incrementing ticks_since_idle is dependent on the return
        // value of is_idle().
        let counting_idle_ticks = !k.waiting_for_idle.is_empty() || k.live_reload_requested;
        if !is_idle {
            k.ticks_since_idle = 0;
        } else if is_idle && counting_idle_ticks {
            k.ticks_since_idle = k.ticks_since_idle.saturating_add(ms_elapsed);
            #[cfg(feature = "perf_logging")]
            log::info!("ticks since idle: {}", k.ticks_since_idle);
        }
        // NOTE: this check must not be part of `is_idle` because its falsiness
        // does not mean that kanata is in a non-idle state, just that we
        // haven't done enough ticks yet to properly compute key-timing.
        let passed_max_switch_timing_check = k
            .layout
            .b()
            .historical_keys
            .iter_hevents()
            .next()
            .map(|he| he.ticks_since_occurrence >= k.switch_max_key_timing)
            .unwrap_or(true);
        let chordsv2_accepts_chords = k
            .layout
            .b()
            .chords_v2
            .as_ref()
            .map(|cv2| cv2.accepts_chords_chv2())
            .unwrap_or(true);
        is_idle && !counting_idle_ticks && passed_max_switch_timing_check && chordsv2_accepts_chords
    }

    pub fn is_idle(&self) -> bool {
        let pressed_keys_means_not_idle =
            !self.waiting_for_idle.is_empty() || self.live_reload_requested;
        self.layout.b().queue.is_empty()
            && zippy_is_idle()
            && self.layout.b().waiting.is_none()
            && self.layout.b().last_press_tracker.tap_hold_timeout == 0
            && (self.layout.b().oneshot.timeout == 0 || self.layout.b().oneshot.keys.is_empty())
            && self.layout.b().active_sequences.is_empty()
            && self.layout.b().tap_dance_eager.is_none()
            && self.layout.b().action_queue.is_empty()
            && self.sequence_state.is_inactive()
            && self.scroll_state.is_none()
            && self.hscroll_state.is_none()
            && self.move_mouse_state_vertical.is_none()
            && self.macro_on_press_cancel_duration == 0
            && self.move_mouse_state_horizontal.is_none()
            && self.dynamic_macro_replay_state.is_none()
            && self.caps_word.is_none()
            && self.vkeys_pending_release.is_empty()
            && !self.layout.b().states.iter().any(|s| {
                matches!(s, State::SeqCustomPending(_) | State::SeqCustomActive(_))
                    || (pressed_keys_means_not_idle && matches!(s, State::NormalKey { .. }))
            })
            && self
                .layout
                .b()
                .chords_v2
                .as_ref()
                .map(|cv2| cv2.is_idle_chv2())
                .unwrap_or(true)
    }
}

#[test]
fn test_unmodmods_bits() {
    assert_eq!(UnmodMods::empty().bits(), 0u8);
    assert_eq!(UnmodMods::all().bits(), 255u8);
}

#[cfg(feature = "cmd")]
fn run_multi_cmd(cmds: Vec<(Option<log::Level>, Option<log::Level>, Vec<String>)>) {
    std::thread::spawn(move || {
        for (cmd_log_level, cmd_error_log_level, cmd) in cmds {
            if let Err(e) = run_cmd_in_thread(cmd, cmd_log_level, cmd_error_log_level).join() {
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

#[cfg(feature = "passthru_ahk")]
/// Clean kanata's state without exiting
pub fn clean_state(kanata: &Arc<Mutex<Kanata>>, tick: u128) -> Result<()> {
    let mut k = kanata.lock();
    #[cfg(all(not(feature = "interception_driver"), target_os = "windows"))]
    let layout = k.layout.bm();
    #[cfg(all(not(feature = "interception_driver"), target_os = "windows"))]
    release_normalkey_states(layout);
    k.tick_ms(tick, &None)?;
    #[cfg(not(target_os = "linux"))]
    {
        let mut k_pressed = PRESSED_KEYS.lock();
        for key_os in k_pressed.clone() {
            k.kbd_out.release_key(key_os)?;
        }
        k_pressed.clear();
    }
    Ok(())
}

/// Checks if kanata should exit based on the fixed key combination of:
/// Lctl+Spc+Esc
fn check_for_exit(_event: &KeyEvent) {
    #[cfg(not(feature = "passthru_ahk"))]
    {
        use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
        static IS_LCL_PRESSED: AtomicBool = AtomicBool::new(false);
        static IS_SPC_PRESSED: AtomicBool = AtomicBool::new(false);
        static IS_ESC_PRESSED: AtomicBool = AtomicBool::new(false);
        let is_pressed = match _event.value {
            KeyValue::Press => true,
            KeyValue::Release => false,
            _ => return,
        };
        match _event.code {
            OsCode::KEY_ESC => IS_ESC_PRESSED.store(is_pressed, SeqCst),
            OsCode::KEY_SPACE => IS_SPC_PRESSED.store(is_pressed, SeqCst),
            OsCode::KEY_LEFTCTRL => IS_LCL_PRESSED.store(is_pressed, SeqCst),
            _ => return,
        }
        const EXIT_MSG: &str = "pressed LControl+Space+Escape, exiting";
        if IS_ESC_PRESSED.load(SeqCst) && IS_SPC_PRESSED.load(SeqCst) && IS_LCL_PRESSED.load(SeqCst)
        {
            log::info!("{EXIT_MSG}");
            #[cfg(all(target_os = "windows", feature = "gui"))]
            {
                #[cfg(not(feature = "interception_driver"))]
                native_windows_gui::stop_thread_dispatch();
                #[cfg(feature = "interception_driver")]
                send_gui_exit_notice(); // interception driver is running in another thread to allow
                                        // GUI take the main one, so it's calling check_for_exit
                                        // from a thread that has no access to the main one, so
                                        // can't stop main thread's dispatch
            }
            #[cfg(all(
                not(target_os = "linux"),
                not(all(target_os = "windows", feature = "gui"))
            ))]
            {
                panic!("{EXIT_MSG}");
            }
            #[cfg(target_os = "linux")]
            {
                signal_hook::low_level::raise(signal_hook::consts::SIGTERM).expect("raise signal");
            }
        }
    }
}

fn update_kbd_out(_cfg: &CfgOptions, _kbd_out: &KbdOut) -> Result<()> {
    #[cfg(all(not(feature = "simulated_output"), target_os = "linux"))]
    {
        _kbd_out.update_unicode_termination(_cfg.linux_opts.linux_unicode_termination);
        _kbd_out.update_unicode_u_code(_cfg.linux_opts.linux_unicode_u_code);
    }
    Ok(())
}

pub fn handle_fakekey_action<'a, const C: usize, const R: usize, T>(
    action: FakeKeyAction,
    layout: &mut Layout<'a, C, R, T>,
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

#[cfg(all(not(feature = "interception_driver"), target_os = "windows"))]
fn release_normalkey_states<'a, const C: usize, const R: usize, T>(layout: &mut Layout<'a, C, R, T>)
where
    T: 'a + std::fmt::Debug + Copy,
{
    let mut coords_to_release = vec![];
    for state in layout.states.iter().copied() {
        match state {
            State::NormalKey {
                coord: (NORMAL_KEY_ROW, y),
                ..
            }
            | State::LayerModifier {
                coord: (NORMAL_KEY_ROW, y),
                ..
            }
            | State::Custom {
                coord: (NORMAL_KEY_ROW, y),
                ..
            }
            | State::RepeatingSequence {
                coord: (NORMAL_KEY_ROW, y),
                ..
            } => {
                coords_to_release.push((NORMAL_KEY_ROW, y));
            }
            _ => {}
        }
    }
    for coord in coords_to_release.into_iter() {
        layout.event(Event::Release(coord.0, coord.1));
    }
}
