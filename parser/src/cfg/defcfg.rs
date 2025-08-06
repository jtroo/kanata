use super::HashSet;
use super::sexpr::SExpr;
use super::{TrimAtomQuotes, error::*};
use crate::cfg::check_first_expr;
use crate::custom_action::*;
use crate::keys::*;
#[allow(unused)]
use crate::{anyhow_expr, anyhow_span, bail, bail_expr, bail_span};

#[cfg(any(target_os = "linux", target_os = "unknown"))]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DeviceDetectMode {
    KeyboardOnly,
    KeyboardMice,
    Any,
}
#[cfg(any(target_os = "linux", target_os = "unknown"))]
impl std::fmt::Display for DeviceDetectMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[cfg(any(target_os = "linux", target_os = "unknown"))]
#[derive(Debug, Clone)]
pub struct CfgLinuxOptions {
    pub linux_dev: Vec<String>,
    pub linux_dev_names_include: Option<Vec<String>>,
    pub linux_dev_names_exclude: Option<Vec<String>>,
    pub linux_continue_if_no_devs_found: bool,
    pub linux_unicode_u_code: crate::keys::OsCode,
    pub linux_unicode_termination: UnicodeTermination,
    pub linux_x11_repeat_delay_rate: Option<KeyRepeatSettings>,
    pub linux_use_trackpoint_property: bool,
    pub linux_output_name: String,
    pub linux_output_bus_type: LinuxCfgOutputBusType,
    pub linux_device_detect_mode: Option<DeviceDetectMode>,
}
#[cfg(any(target_os = "linux", target_os = "unknown"))]
impl Default for CfgLinuxOptions {
    fn default() -> Self {
        Self {
            linux_dev: vec![],
            linux_dev_names_include: None,
            linux_dev_names_exclude: None,
            linux_continue_if_no_devs_found: false,
            // historically was the only option, so make KEY_U the default
            linux_unicode_u_code: crate::keys::OsCode::KEY_U,
            // historically was the only option, so make Enter the default
            linux_unicode_termination: UnicodeTermination::Enter,
            linux_x11_repeat_delay_rate: None,
            linux_use_trackpoint_property: false,
            linux_output_name: "kanata".to_owned(),
            linux_output_bus_type: LinuxCfgOutputBusType::BusI8042,
            linux_device_detect_mode: None,
        }
    }
}
#[cfg(any(target_os = "linux", target_os = "unknown"))]
#[derive(Debug, Clone, Copy)]
pub enum LinuxCfgOutputBusType {
    BusUsb,
    BusI8042,
}

#[cfg(any(target_os = "macos", target_os = "unknown"))]
#[derive(Debug, Default, Clone)]
pub struct CfgMacosOptions {
    pub macos_dev_names_include: Option<Vec<String>>,
    pub macos_dev_names_exclude: Option<Vec<String>>,
}

#[cfg(any(
    all(feature = "interception_driver", target_os = "windows"),
    target_os = "unknown"
))]
#[derive(Debug, Clone, Default)]
pub struct CfgWinterceptOptions {
    pub windows_interception_mouse_hwids: Option<Vec<[u8; HWID_ARR_SZ]>>,
    pub windows_interception_mouse_hwids_exclude: Option<Vec<[u8; HWID_ARR_SZ]>>,
    pub windows_interception_keyboard_hwids: Option<Vec<[u8; HWID_ARR_SZ]>>,
    pub windows_interception_keyboard_hwids_exclude: Option<Vec<[u8; HWID_ARR_SZ]>>,
}

#[cfg(any(target_os = "windows", target_os = "unknown"))]
#[derive(Debug, Clone, Default)]
pub struct CfgWindowsOptions {
    pub windows_altgr: AltGrBehaviour,
    pub sync_keystates: bool,
}

#[cfg(all(any(target_os = "windows", target_os = "unknown"), feature = "gui"))]
#[derive(Debug, Clone)]
pub struct CfgOptionsGui {
    /// File name / path to the tray icon file.
    pub tray_icon: Option<String>,
    /// Whether to match layer names to icon files without an explicit 'icon' field
    pub icon_match_layer_name: bool,
    /// Show tooltip on layer changes showing layer icons
    pub tooltip_layer_changes: bool,
    /// Show tooltip on layer changes for the default/base layer
    pub tooltip_no_base: bool,
    /// Show tooltip on layer changes even for layers without an icon
    pub tooltip_show_blank: bool,
    /// Show tooltip on layer changes for this duration (ms)
    pub tooltip_duration: u16,
    /// Show system notification message on config reload
    pub notify_cfg_reload: bool,
    /// Disable sound for the system notification message on config reload
    pub notify_cfg_reload_silent: bool,
    /// Show system notification message on errors
    pub notify_error: bool,
    /// Set tooltip size (width, height)
    pub tooltip_size: (u16, u16),
}
#[cfg(all(any(target_os = "windows", target_os = "unknown"), feature = "gui"))]
impl Default for CfgOptionsGui {
    fn default() -> Self {
        Self {
            tray_icon: None,
            icon_match_layer_name: true,
            tooltip_layer_changes: false,
            tooltip_show_blank: false,
            tooltip_no_base: true,
            tooltip_duration: 500,
            notify_cfg_reload: true,
            notify_cfg_reload_silent: false,
            notify_error: true,
            tooltip_size: (24, 24),
        }
    }
}

#[derive(Debug)]
pub struct CfgOptions {
    pub process_unmapped_keys: bool,
    pub process_unmapped_keys_exceptions: Option<Vec<(OsCode, SExpr)>>,
    pub block_unmapped_keys: bool,
    pub allow_hardware_repeat: bool,
    pub start_alias: Option<String>,
    pub enable_cmd: bool,
    pub sequence_timeout: u16,
    pub sequence_input_mode: SequenceInputMode,
    pub sequence_backtrack_modcancel: bool,
    pub sequence_always_on: bool,
    pub log_layer_changes: bool,
    pub delegate_to_first_layer: bool,
    pub movemouse_inherit_accel_state: bool,
    pub movemouse_smooth_diagonals: bool,
    pub override_release_on_activation: bool,
    pub dynamic_macro_max_presses: u16,
    pub dynamic_macro_replay_delay_behaviour: ReplayDelayBehaviour,
    pub concurrent_tap_hold: bool,
    pub rapid_event_delay: u16,
    pub trans_resolution_behavior_v2: bool,
    pub chords_v2_min_idle: u16,
    #[cfg(any(
        all(target_os = "windows", feature = "interception_driver"),
        target_os = "linux",
        target_os = "unknown"
    ))]
    pub mouse_movement_key: Option<OsCode>,
    #[cfg(any(target_os = "linux", target_os = "unknown"))]
    pub linux_opts: CfgLinuxOptions,
    #[cfg(any(target_os = "macos", target_os = "unknown"))]
    pub macos_opts: CfgMacosOptions,
    #[cfg(any(target_os = "windows", target_os = "unknown"))]
    pub windows_opts: CfgWindowsOptions,
    #[cfg(any(
        all(feature = "interception_driver", target_os = "windows"),
        target_os = "unknown"
    ))]
    pub wintercept_opts: CfgWinterceptOptions,
    #[cfg(all(any(target_os = "windows", target_os = "unknown"), feature = "gui"))]
    pub gui_opts: CfgOptionsGui,
}

impl Default for CfgOptions {
    fn default() -> Self {
        Self {
            process_unmapped_keys: false,
            process_unmapped_keys_exceptions: None,
            block_unmapped_keys: false,
            allow_hardware_repeat: true,
            start_alias: None,
            enable_cmd: false,
            sequence_timeout: 1000,
            sequence_input_mode: SequenceInputMode::HiddenSuppressed,
            sequence_backtrack_modcancel: true,
            sequence_always_on: false,
            log_layer_changes: true,
            delegate_to_first_layer: false,
            movemouse_inherit_accel_state: false,
            movemouse_smooth_diagonals: false,
            override_release_on_activation: false,
            dynamic_macro_max_presses: 128,
            dynamic_macro_replay_delay_behaviour: ReplayDelayBehaviour::Recorded,
            concurrent_tap_hold: false,
            rapid_event_delay: 5,
            trans_resolution_behavior_v2: true,
            chords_v2_min_idle: 5,
            #[cfg(any(
                all(target_os = "windows", feature = "interception_driver"),
                target_os = "linux",
                target_os = "unknown"
            ))]
            mouse_movement_key: None,
            #[cfg(any(target_os = "linux", target_os = "unknown"))]
            linux_opts: Default::default(),
            #[cfg(any(target_os = "windows", target_os = "unknown"))]
            windows_opts: Default::default(),
            #[cfg(any(
                all(feature = "interception_driver", target_os = "windows"),
                target_os = "unknown"
            ))]
            wintercept_opts: Default::default(),
            #[cfg(any(target_os = "macos", target_os = "unknown"))]
            macos_opts: Default::default(),
            #[cfg(all(any(target_os = "windows", target_os = "unknown"), feature = "gui"))]
            gui_opts: Default::default(),
        }
    }
}

/// Parse configuration entries from an expression starting with defcfg.
pub fn parse_defcfg(expr: &[SExpr]) -> Result<CfgOptions> {
    let mut seen_keys = HashSet::default();
    let mut cfg = CfgOptions::default();
    let mut exprs = check_first_expr(expr.iter(), "defcfg")?;
    let mut is_process_unmapped_keys_defined = false;
    // Read k-v pairs from the configuration
    loop {
        let key = match exprs.next() {
            Some(k) => k,
            None => {
                if !is_process_unmapped_keys_defined {
                    log::warn!(
                        "The item process-unmapped-keys is not defined in defcfg. Consider whether process-unmapped-keys should be yes vs. no."
                    );
                }
                return Ok(cfg);
            }
        };
        let val = match exprs.next() {
            Some(v) => v,
            None => bail_expr!(key, "Found a defcfg option missing a value"),
        };
        match key {
            SExpr::Atom(k) => {
                let label = k.t.as_str();
                if !seen_keys.insert(label) {
                    bail_expr!(key, "Duplicate defcfg option {}", label);
                }
                match label {
                    "sequence-timeout" => {
                        cfg.sequence_timeout = parse_cfg_val_u16(val, label, true)?;
                    }
                    "sequence-input-mode" => {
                        let v = sexpr_to_str_or_err(val, label)?;
                        cfg.sequence_input_mode = SequenceInputMode::try_from_str(v)
                            .map_err(|e| anyhow_expr!(val, "{}", e.to_string()))?;
                    }
                    "sequence-always-on" => {
                        cfg.sequence_always_on = parse_defcfg_val_bool(val, label)?
                    }
                    "dynamic-macro-max-presses" => {
                        cfg.dynamic_macro_max_presses = parse_cfg_val_u16(val, label, false)?;
                    }
                    "dynamic-macro-replay-delay-behaviour" => {
                        cfg.dynamic_macro_replay_delay_behaviour = val
                            .atom(None)
                            .map(|v| match v {
                                "constant" => Ok(ReplayDelayBehaviour::Constant),
                                "recorded" => Ok(ReplayDelayBehaviour::Recorded),
                                _ => bail_expr!(
                                    val,
                                    "this option must be one of: constant | recorded"
                                ),
                            })
                            .ok_or_else(|| {
                                anyhow_expr!(val, "this option must be one of: constant | recorded")
                            })??;
                    }
                    "linux-dev" => {
                        #[cfg(any(target_os = "linux", target_os = "unknown"))]
                        {
                            cfg.linux_opts.linux_dev = parse_dev(val)?;
                            if cfg.linux_opts.linux_dev.is_empty() {
                                bail_expr!(
                                    val,
                                    "device list is empty, no devices will be intercepted"
                                );
                            }
                        }
                    }
                    "linux-dev-names-include" => {
                        #[cfg(any(target_os = "linux", target_os = "unknown"))]
                        {
                            let dev_names = parse_dev(val)?;
                            if dev_names.is_empty() {
                                log::warn!("linux-dev-names-include is empty");
                            }
                            cfg.linux_opts.linux_dev_names_include = Some(dev_names);
                        }
                    }
                    "linux-dev-names-exclude" => {
                        #[cfg(any(target_os = "linux", target_os = "unknown"))]
                        {
                            cfg.linux_opts.linux_dev_names_exclude = Some(parse_dev(val)?);
                        }
                    }
                    "linux-unicode-u-code" => {
                        #[cfg(any(target_os = "linux", target_os = "unknown"))]
                        {
                            let v = sexpr_to_str_or_err(val, label)?;
                            cfg.linux_opts.linux_unicode_u_code = crate::keys::str_to_oscode(v)
                                .ok_or_else(|| {
                                    anyhow_expr!(val, "unknown code for {label}: {}", v)
                                })?;
                        }
                    }
                    "linux-unicode-termination" => {
                        #[cfg(any(target_os = "linux", target_os = "unknown"))]
                        {
                            let v = sexpr_to_str_or_err(val, label)?;
                            cfg.linux_opts.linux_unicode_termination = match v {
                                "enter" => UnicodeTermination::Enter,
                                "space" => UnicodeTermination::Space,
                                "enter-space" => UnicodeTermination::EnterSpace,
                                "space-enter" => UnicodeTermination::SpaceEnter,
                                _ => bail_expr!(
                                    val,
                                    "{label} got {}. It accepts: enter|space|enter-space|space-enter",
                                    v
                                ),
                            }
                        }
                    }
                    "linux-x11-repeat-delay-rate" => {
                        #[cfg(any(target_os = "linux", target_os = "unknown"))]
                        {
                            let v = sexpr_to_str_or_err(val, label)?;
                            let delay_rate = v.split(',').collect::<Vec<_>>();
                            const ERRMSG: &str = "Invalid value for linux-x11-repeat-delay-rate.\nExpected two numbers 0-65535 separated by a comma, e.g. 200,25";
                            if delay_rate.len() != 2 {
                                bail_expr!(val, "{}", ERRMSG)
                            }
                            cfg.linux_opts.linux_x11_repeat_delay_rate = Some(KeyRepeatSettings {
                                delay: match str::parse::<u16>(delay_rate[0]) {
                                    Ok(delay) => delay,
                                    Err(_) => bail_expr!(val, "{}", ERRMSG),
                                },
                                rate: match str::parse::<u16>(delay_rate[1]) {
                                    Ok(rate) => rate,
                                    Err(_) => bail_expr!(val, "{}", ERRMSG),
                                },
                            });
                        }
                    }
                    "linux-use-trackpoint-property" => {
                        #[cfg(any(target_os = "linux", target_os = "unknown"))]
                        {
                            cfg.linux_opts.linux_use_trackpoint_property =
                                parse_defcfg_val_bool(val, label)?
                        }
                    }
                    "linux-output-device-name" => {
                        #[cfg(any(target_os = "linux", target_os = "unknown"))]
                        {
                            let device_name = sexpr_to_str_or_err(val, label)?;
                            if device_name.is_empty() {
                                log::warn!(
                                    "linux-output-device-name is empty, using kanata as default value"
                                );
                            } else {
                                cfg.linux_opts.linux_output_name = device_name.to_owned();
                            }
                        }
                    }
                    "linux-output-device-bus-type" => {
                        let bus_type = sexpr_to_str_or_err(val, label)?;
                        match bus_type {
                            "USB" | "I8042" => {}
                            _ => bail_expr!(
                                val,
                                "Invalid value for linux-output-device-bus-type.\nExpected one of: USB or I8042"
                            ),
                        };
                        #[cfg(any(target_os = "linux", target_os = "unknown"))]
                        {
                            let bus_type = match bus_type {
                                "USB" => LinuxCfgOutputBusType::BusUsb,
                                "I8042" => LinuxCfgOutputBusType::BusI8042,
                                _ => unreachable!("validated earlier"),
                            };
                            cfg.linux_opts.linux_output_bus_type = bus_type;
                        }
                    }
                    "linux-device-detect-mode" => {
                        let detect_mode = sexpr_to_str_or_err(val, label)?;
                        match detect_mode {
                            "any" | "keyboard-only" | "keyboard-mice" => {}
                            _ => bail_expr!(
                                val,
                                "Invalid value for linux-device-detect-mode.\nExpected one of: any | keyboard-only | keyboard-mice"
                            ),
                        };
                        #[cfg(any(target_os = "linux", target_os = "unknown"))]
                        {
                            let detect_mode = Some(match detect_mode {
                                "any" => DeviceDetectMode::Any,
                                "keyboard-only" => DeviceDetectMode::KeyboardOnly,
                                "keyboard-mice" => DeviceDetectMode::KeyboardMice,
                                _ => unreachable!("validated earlier"),
                            });
                            cfg.linux_opts.linux_device_detect_mode = detect_mode;
                        }
                    }
                    "windows-altgr" => {
                        #[cfg(any(target_os = "windows", target_os = "unknown"))]
                        {
                            const CANCEL: &str = "cancel-lctl-press";
                            const ADD: &str = "add-lctl-release";
                            let v = sexpr_to_str_or_err(val, label)?;
                            cfg.windows_opts.windows_altgr = match v {
                                CANCEL => AltGrBehaviour::CancelLctlPress,
                                ADD => AltGrBehaviour::AddLctlRelease,
                                _ => bail_expr!(
                                    val,
                                    "Invalid value for {label}: {}. Valid values are {},{}",
                                    v,
                                    CANCEL,
                                    ADD
                                ),
                            }
                        }
                    }
                    "windows-sync-keystates" => {
                        #[cfg(any(target_os = "windows", target_os = "unknown"))]
                        {
                            cfg.windows_opts.sync_keystates = parse_defcfg_val_bool(val, label)?;
                        }
                    }
                    "windows-interception-mouse-hwid" => {
                        #[cfg(any(
                            all(feature = "interception_driver", target_os = "windows"),
                            target_os = "unknown"
                        ))]
                        {
                            if cfg
                                .wintercept_opts
                                .windows_interception_mouse_hwids_exclude
                                .is_some()
                            {
                                bail_expr!(
                                    val,
                                    "{label} and windows-interception-mouse-hwid-exclude cannot both be included"
                                );
                            }
                            let v = sexpr_to_str_or_err(val, label)?;
                            let hwid = v;
                            log::trace!("win hwid: {hwid}");
                            let hwid_vec = hwid
                                .split(',')
                                .try_fold(vec![], |mut hwid_bytes, hwid_byte| {
                                    hwid_byte.trim_matches(' ').parse::<u8>().map(|b| {
                                        hwid_bytes.push(b);
                                        hwid_bytes
                                    })
                                }).map_err(|_| anyhow_expr!(val, "{label} format is invalid. It should consist of numbers [0,255] separated by commas"))?;
                            let hwid_slice = hwid_vec.iter().copied().enumerate()
                                .try_fold([0u8; HWID_ARR_SZ], |mut hwid, idx_byte| {
                                    let (i, b) = idx_byte;
                                    if i > HWID_ARR_SZ {
                                        bail_expr!(val, "{label} is too long; it should be up to {HWID_ARR_SZ} numbers [0,255]")
                                    }
                                    hwid[i] = b;
                                    Ok(hwid)
                            })?;
                            match cfg
                                .wintercept_opts
                                .windows_interception_mouse_hwids
                                .as_mut()
                            {
                                Some(v) => {
                                    v.push(hwid_slice);
                                }
                                None => {
                                    cfg.wintercept_opts.windows_interception_mouse_hwids =
                                        Some(vec![hwid_slice]);
                                }
                            }
                            cfg.wintercept_opts
                                .windows_interception_mouse_hwids
                                .as_mut()
                                .unwrap()
                                .shrink_to_fit();
                        }
                    }
                    "windows-interception-mouse-hwids" => {
                        #[cfg(any(
                            all(feature = "interception_driver", target_os = "windows"),
                            target_os = "unknown"
                        ))]
                        {
                            if cfg
                                .wintercept_opts
                                .windows_interception_mouse_hwids_exclude
                                .is_some()
                            {
                                bail_expr!(
                                    val,
                                    "{label} and windows-interception-mouse-hwid-exclude cannot both be included"
                                );
                            }
                            let parsed_hwids = sexpr_to_hwids_vec(
                                val,
                                label,
                                "entry in windows-interception-mouse-hwids",
                            )?;
                            match cfg
                                .wintercept_opts
                                .windows_interception_mouse_hwids
                                .as_mut()
                            {
                                Some(v) => {
                                    v.extend(parsed_hwids);
                                }
                                None => {
                                    cfg.wintercept_opts.windows_interception_mouse_hwids =
                                        Some(parsed_hwids);
                                }
                            }
                            cfg.wintercept_opts
                                .windows_interception_mouse_hwids
                                .as_mut()
                                .unwrap()
                                .shrink_to_fit();
                        }
                    }
                    "windows-interception-mouse-hwids-exclude" => {
                        #[cfg(any(
                            all(feature = "interception_driver", target_os = "windows"),
                            target_os = "unknown"
                        ))]
                        {
                            if cfg
                                .wintercept_opts
                                .windows_interception_mouse_hwids
                                .is_some()
                            {
                                bail_expr!(
                                    val,
                                    "{label} and windows-interception-mouse-hwid(s) cannot both be used"
                                );
                            }
                            let parsed_hwids = sexpr_to_hwids_vec(
                                val,
                                label,
                                "entry in windows-interception-mouse-hwids-exclude",
                            )?;
                            cfg.wintercept_opts.windows_interception_mouse_hwids_exclude =
                                Some(parsed_hwids);
                        }
                    }
                    "windows-interception-keyboard-hwids" => {
                        #[cfg(any(
                            all(feature = "interception_driver", target_os = "windows"),
                            target_os = "unknown"
                        ))]
                        {
                            if cfg
                                .wintercept_opts
                                .windows_interception_keyboard_hwids_exclude
                                .is_some()
                            {
                                bail_expr!(
                                    val,
                                    "{label} and windows-interception-keyboard-hwid-exclude cannot both be used"
                                );
                            }
                            let parsed_hwids = sexpr_to_hwids_vec(
                                val,
                                label,
                                "entry in windows-interception-keyboard-hwids",
                            )?;
                            cfg.wintercept_opts.windows_interception_keyboard_hwids =
                                Some(parsed_hwids);
                        }
                    }
                    "windows-interception-keyboard-hwids-exclude" => {
                        #[cfg(any(
                            all(feature = "interception_driver", target_os = "windows"),
                            target_os = "unknown"
                        ))]
                        {
                            if cfg
                                .wintercept_opts
                                .windows_interception_keyboard_hwids
                                .is_some()
                            {
                                bail_expr!(
                                    val,
                                    "{label} and windows-interception-keyboard-hwid cannot both be used"
                                );
                            }
                            let parsed_hwids = sexpr_to_hwids_vec(
                                val,
                                label,
                                "entry in windows-interception-keyboard-hwids-exclude",
                            )?;
                            cfg.wintercept_opts
                                .windows_interception_keyboard_hwids_exclude = Some(parsed_hwids);
                        }
                    }
                    "macos-dev-names-include" => {
                        #[cfg(any(target_os = "macos", target_os = "unknown"))]
                        {
                            let dev_names = parse_dev(val)?;
                            if dev_names.is_empty() {
                                log::warn!("macos-dev-names-include is empty");
                            }
                            cfg.macos_opts.macos_dev_names_include = Some(dev_names);
                        }
                    }
                    "macos-dev-names-exclude" => {
                        #[cfg(any(target_os = "macos", target_os = "unknown"))]
                        {
                            let dev_names = parse_dev(val)?;
                            if dev_names.is_empty() {
                                log::warn!("macos-dev-names-exclude is empty");
                            }
                            cfg.macos_opts.macos_dev_names_exclude = Some(dev_names);
                        }
                    }
                    "tray-icon" => {
                        #[cfg(all(
                            any(target_os = "windows", target_os = "unknown"),
                            feature = "gui"
                        ))]
                        {
                            let icon_path = sexpr_to_str_or_err(val, label)?;
                            if icon_path.is_empty() {
                                log::warn!("tray-icon is empty");
                            }
                            cfg.gui_opts.tray_icon = Some(icon_path.to_string());
                        }
                    }
                    "icon-match-layer-name" => {
                        #[cfg(all(
                            any(target_os = "windows", target_os = "unknown"),
                            feature = "gui"
                        ))]
                        {
                            cfg.gui_opts.icon_match_layer_name = parse_defcfg_val_bool(val, label)?
                        }
                    }
                    "tooltip-layer-changes" => {
                        #[cfg(all(
                            any(target_os = "windows", target_os = "unknown"),
                            feature = "gui"
                        ))]
                        {
                            cfg.gui_opts.tooltip_layer_changes = parse_defcfg_val_bool(val, label)?
                        }
                    }
                    "tooltip-show-blank" => {
                        #[cfg(all(
                            any(target_os = "windows", target_os = "unknown"),
                            feature = "gui"
                        ))]
                        {
                            cfg.gui_opts.tooltip_show_blank = parse_defcfg_val_bool(val, label)?
                        }
                    }
                    "tooltip-no-base" => {
                        #[cfg(all(
                            any(target_os = "windows", target_os = "unknown"),
                            feature = "gui"
                        ))]
                        {
                            cfg.gui_opts.tooltip_no_base = parse_defcfg_val_bool(val, label)?
                        }
                    }
                    "tooltip-duration" => {
                        #[cfg(all(
                            any(target_os = "windows", target_os = "unknown"),
                            feature = "gui"
                        ))]
                        {
                            cfg.gui_opts.tooltip_duration = parse_cfg_val_u16(val, label, false)?
                        }
                    }
                    "notify-cfg-reload" => {
                        #[cfg(all(
                            any(target_os = "windows", target_os = "unknown"),
                            feature = "gui"
                        ))]
                        {
                            cfg.gui_opts.notify_cfg_reload = parse_defcfg_val_bool(val, label)?
                        }
                    }
                    "notify-cfg-reload-silent" => {
                        #[cfg(all(
                            any(target_os = "windows", target_os = "unknown"),
                            feature = "gui"
                        ))]
                        {
                            cfg.gui_opts.notify_cfg_reload_silent =
                                parse_defcfg_val_bool(val, label)?
                        }
                    }
                    "notify-error" => {
                        #[cfg(all(
                            any(target_os = "windows", target_os = "unknown"),
                            feature = "gui"
                        ))]
                        {
                            cfg.gui_opts.notify_error = parse_defcfg_val_bool(val, label)?
                        }
                    }
                    "tooltip-size" => {
                        #[cfg(all(
                            any(target_os = "windows", target_os = "unknown"),
                            feature = "gui"
                        ))]
                        {
                            let v = sexpr_to_str_or_err(val, label)?;
                            let tooltip_size = v.split(',').collect::<Vec<_>>();
                            const ERRMSG: &str = "Invalid value for tooltip-size.\nExpected two numbers 0-65535 separated by a comma, e.g. 24,24";
                            if tooltip_size.len() != 2 {
                                bail_expr!(val, "{}", ERRMSG)
                            }
                            cfg.gui_opts.tooltip_size = (
                                match str::parse::<u16>(tooltip_size[0]) {
                                    Ok(w) => w,
                                    Err(_) => bail_expr!(val, "{}", ERRMSG),
                                },
                                match str::parse::<u16>(tooltip_size[1]) {
                                    Ok(h) => h,
                                    Err(_) => bail_expr!(val, "{}", ERRMSG),
                                },
                            );
                        }
                    }

                    "process-unmapped-keys" => {
                        is_process_unmapped_keys_defined = true;
                        if let Some(list) = val.list(None) {
                            let err = "Expected (all-except key1 ... keyN).";
                            if list.len() < 2 {
                                bail_expr!(val, "{err}");
                            }
                            match list[0].atom(None) {
                                Some("all-except") => {}
                                _ => {
                                    bail_expr!(val, "{err}");
                                }
                            };
                            // Note: deflocalkeys should already be parsed when parsing defcfg,
                            // so can use safely use str_to_oscode here; it will include user
                            // configurations already.
                            let mut key_exceptions: Vec<(OsCode, SExpr)> = vec![];
                            for key_expr in list[1..].iter() {
                                let key = key_expr.atom(None).and_then(str_to_oscode).ok_or_else(
                                    || anyhow_expr!(key_expr, "Expected a known key name."),
                                )?;
                                if key_exceptions.iter().any(|k_exc| k_exc.0 == key) {
                                    bail_expr!(key_expr, "Duplicate key name is not allowed.");
                                }
                                key_exceptions.push((key, key_expr.clone()));
                            }
                            cfg.process_unmapped_keys = true;
                            cfg.process_unmapped_keys_exceptions = Some(key_exceptions);
                        } else {
                            cfg.process_unmapped_keys = parse_defcfg_val_bool(val, label)?
                        }
                    }

                    "block-unmapped-keys" => {
                        cfg.block_unmapped_keys = parse_defcfg_val_bool(val, label)?
                    }
                    "allow-hardware-repeat" => {
                        cfg.allow_hardware_repeat = parse_defcfg_val_bool(val, label)?
                    }
                    "alias-to-trigger-on-load" => {
                        cfg.start_alias = parse_defcfg_val_string(val, label)?
                    }
                    "danger-enable-cmd" => cfg.enable_cmd = parse_defcfg_val_bool(val, label)?,
                    "sequence-backtrack-modcancel" => {
                        cfg.sequence_backtrack_modcancel = parse_defcfg_val_bool(val, label)?
                    }
                    "log-layer-changes" => {
                        cfg.log_layer_changes = parse_defcfg_val_bool(val, label)?
                    }
                    "delegate-to-first-layer" => {
                        cfg.delegate_to_first_layer = parse_defcfg_val_bool(val, label)?;
                        if cfg.delegate_to_first_layer {
                            log::info!(
                                "delegating transparent keys on other layers to first defined layer"
                            );
                        }
                    }
                    "linux-continue-if-no-devs-found" => {
                        #[cfg(any(target_os = "linux", target_os = "unknown"))]
                        {
                            cfg.linux_opts.linux_continue_if_no_devs_found =
                                parse_defcfg_val_bool(val, label)?
                        }
                    }
                    "movemouse-smooth-diagonals" => {
                        cfg.movemouse_smooth_diagonals = parse_defcfg_val_bool(val, label)?
                    }
                    "movemouse-inherit-accel-state" => {
                        cfg.movemouse_inherit_accel_state = parse_defcfg_val_bool(val, label)?
                    }
                    "override-release-on-activation" => {
                        cfg.override_release_on_activation = parse_defcfg_val_bool(val, label)?
                    }
                    "concurrent-tap-hold" => {
                        cfg.concurrent_tap_hold = parse_defcfg_val_bool(val, label)?
                    }
                    "rapid-event-delay" => {
                        cfg.rapid_event_delay = parse_cfg_val_u16(val, label, false)?
                    }
                    "transparent-key-resolution" => {
                        let v = sexpr_to_str_or_err(val, label)?;
                        cfg.trans_resolution_behavior_v2 = match v {
                            "to-base-layer" => false,
                            "layer-stack" => true,
                            _ => bail_expr!(
                                val,
                                "{label} got {}. It accepts: 'to-base-layer' or 'layer-stack'",
                                v
                            ),
                        };
                    }
                    "chords-v2-min-idle" | "chords-v2-min-idle-experimental" => {
                        if label == "chords-v2-min-idle-experimental" {
                            log::warn!(
                                "You should replace chords-v2-min-idle-experimental with chords-v2-min-idle\n\
                                        Using -experimental will be invalid in the future."
                            )
                        }
                        let min_idle = parse_cfg_val_u16(val, label, true)?;
                        if min_idle < 5 {
                            bail_expr!(val, "{label} must be 5-65535");
                        }
                        cfg.chords_v2_min_idle = min_idle;
                    }
                    "mouse-movement-key" => {
                        #[cfg(any(
                            all(target_os = "windows", feature = "interception_driver"),
                            target_os = "linux",
                            target_os = "unknown"
                        ))]
                        {
                            if let Some(keystr) = parse_defcfg_val_string(val, label)? {
                                if let Some(key) = str_to_oscode(&keystr) {
                                    cfg.mouse_movement_key = Some(key);
                                } else {
                                    bail_expr!(val, "{label} not a recognised key code");
                                }
                            } else {
                                bail_expr!(val, "{label} not a string for a key code");
                            }
                        }
                    }
                    _ => bail_expr!(key, "Unknown defcfg option {}", label),
                };
            }
            SExpr::List(_) => {
                bail_expr!(key, "Lists are not allowed in as keys in defcfg");
            }
        }
    }
}

fn parse_defcfg_val_string(expr: &SExpr, _label: &str) -> Result<Option<String>> {
    match expr {
        SExpr::Atom(v) => Ok(Some(v.t.clone())),
        _ => Ok(None),
    }
}

pub const FALSE_VALUES: [&str; 3] = ["no", "false", "0"];
pub const TRUE_VALUES: [&str; 3] = ["yes", "true", "1"];
pub const BOOLEAN_VALUES: [&str; 6] = ["yes", "true", "1", "no", "false", "0"];

fn parse_defcfg_val_bool(expr: &SExpr, label: &str) -> Result<bool> {
    match &expr {
        SExpr::Atom(v) => {
            let val = v.t.trim_atom_quotes().to_ascii_lowercase();
            if TRUE_VALUES.contains(&val.as_str()) {
                Ok(true)
            } else if FALSE_VALUES.contains(&val.as_str()) {
                Ok(false)
            } else {
                bail_expr!(
                    expr,
                    "The value for {label} must be one of: {}",
                    BOOLEAN_VALUES.join(", ")
                );
            }
        }
        SExpr::List(_) => {
            bail_expr!(
                expr,
                "The value for {label} cannot be a list, it must be one of: {}",
                BOOLEAN_VALUES.join(", "),
            )
        }
    }
}

fn parse_cfg_val_u16(expr: &SExpr, label: &str, exclude_zero: bool) -> Result<u16> {
    let start = if exclude_zero { 1 } else { 0 };
    match &expr {
        SExpr::Atom(v) => Ok(str::parse::<u16>(v.t.trim_atom_quotes())
            .ok()
            .and_then(|u| {
                if exclude_zero && u == 0 {
                    None
                } else {
                    Some(u)
                }
            })
            .ok_or_else(|| anyhow_expr!(expr, "{label} must be {start}-65535"))?),
        SExpr::List(_) => {
            bail_expr!(
                expr,
                "The value for {label} cannot be a list, it must be a number {start}-65535",
            )
        }
    }
}

pub fn parse_colon_separated_text(paths: &str) -> Vec<String> {
    let mut all_paths = vec![];
    let mut full_dev_path = String::new();
    let mut dev_path_iter = paths.split(':').peekable();
    while let Some(dev_path) = dev_path_iter.next() {
        if dev_path.ends_with('\\') && dev_path_iter.peek().is_some() {
            full_dev_path.push_str(dev_path.trim_end_matches('\\'));
            full_dev_path.push(':');
            continue;
        } else {
            full_dev_path.push_str(dev_path);
        }
        all_paths.push(full_dev_path.clone());
        full_dev_path.clear();
    }
    all_paths.shrink_to_fit();
    all_paths
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "unknown"))]
pub fn parse_dev(val: &SExpr) -> Result<Vec<String>> {
    Ok(match val {
        SExpr::Atom(a) => {
            let devs = parse_colon_separated_text(a.t.trim_atom_quotes());
            if devs.len() == 1 && devs[0].is_empty() {
                bail_expr!(val, "an empty string is not a valid device name or path")
            }
            devs
        }
        SExpr::List(l) => {
            let r: Result<Vec<String>> =
                l.t.iter()
                    .try_fold(Vec::with_capacity(l.t.len()), |mut acc, expr| match expr {
                        SExpr::Atom(path) => {
                            let trimmed_path = path.t.trim_atom_quotes().to_string();
                            if trimmed_path.is_empty() {
                                bail_span!(
                                    path,
                                    "an empty string is not a valid device name or path"
                                )
                            }
                            acc.push(trimmed_path);
                            Ok(acc)
                        }
                        SExpr::List(inner_list) => {
                            bail_span!(inner_list, "expected strings, found a list")
                        }
                    });

            r?
        }
    })
}

fn sexpr_to_str_or_err<'a>(expr: &'a SExpr, label: &str) -> Result<&'a str> {
    match expr {
        SExpr::Atom(a) => Ok(a.t.trim_atom_quotes()),
        SExpr::List(_) => bail_expr!(expr, "The value for {label} can't be a list"),
    }
}

#[cfg(any(
    all(feature = "interception_driver", target_os = "windows"),
    target_os = "unknown"
))]
fn sexpr_to_list_or_err<'a>(expr: &'a SExpr, label: &str) -> Result<&'a [SExpr]> {
    match expr {
        SExpr::Atom(_) => bail_expr!(expr, "The value for {label} must be a list"),
        SExpr::List(l) => Ok(&l.t),
    }
}

#[cfg(any(
    all(feature = "interception_driver", target_os = "windows"),
    target_os = "unknown"
))]
fn sexpr_to_hwids_vec(
    val: &SExpr,
    label: &str,
    entry_label: &str,
) -> Result<Vec<[u8; HWID_ARR_SZ]>> {
    let hwids = sexpr_to_list_or_err(val, label)?;
    let mut parsed_hwids = vec![];
    for hwid_expr in hwids.iter() {
        let hwid = sexpr_to_str_or_err(hwid_expr, entry_label)?;
        log::trace!("win hwid: {hwid}");
        let hwid_vec = hwid
            .split(',')
            .try_fold(vec![], |mut hwid_bytes, hwid_byte| {
                hwid_byte.trim_matches(' ').parse::<u8>().map(|b| {
                    hwid_bytes.push(b);
                    hwid_bytes
                })
            }).map_err(|_| anyhow_expr!(hwid_expr, "Entry in {label} is invalid. Entries should be numbers [0,255] separated by commas"))?;
        let hwid_slice = hwid_vec.iter().copied().enumerate()
            .try_fold([0u8; HWID_ARR_SZ], |mut hwid, idx_byte| {
                let (i, b) = idx_byte;
                if i > HWID_ARR_SZ {
                    bail_expr!(hwid_expr, "entry in {label} is too long; it should be up to {HWID_ARR_SZ} 8-bit unsigned integers")
                }
                hwid[i] = b;
                Ok(hwid)
        });
        parsed_hwids.push(hwid_slice?);
    }
    parsed_hwids.shrink_to_fit();
    Ok(parsed_hwids)
}

#[cfg(any(target_os = "linux", target_os = "unknown"))]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct KeyRepeatSettings {
    pub delay: u16,
    pub rate: u16,
}

#[cfg(any(target_os = "linux", target_os = "unknown"))]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum UnicodeTermination {
    Enter,
    Space,
    SpaceEnter,
    EnterSpace,
}

#[cfg(any(target_os = "windows", target_os = "unknown"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AltGrBehaviour {
    DoNothing,
    CancelLctlPress,
    AddLctlRelease,
}

#[cfg(any(target_os = "windows", target_os = "unknown"))]
impl Default for AltGrBehaviour {
    fn default() -> Self {
        Self::DoNothing
    }
}

#[cfg(any(
    all(feature = "interception_driver", target_os = "windows"),
    target_os = "unknown"
))]
pub const HWID_ARR_SZ: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayDelayBehaviour {
    /// Always use a fixed number of ticks between presses and releases.
    /// This is the original kanata behaviour.
    /// This means that held action activations like in tap-hold do not behave as intended.
    Constant,
    /// Use the recorded number of ticks between presses and releases.
    /// This is newer behaviour.
    Recorded,
}
