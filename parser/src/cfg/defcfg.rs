use super::error::*;
use super::sexpr::SExpr;
use super::HashSet;
use crate::cfg::check_first_expr;
use crate::cfg::parse_colon_separated_text;
use crate::custom_action::*;
use crate::{anyhow_expr, anyhow_span, bail, bail_expr};

#[derive(Debug)]
pub struct CfgOptions {
    pub process_unmapped_keys: bool,
    pub enable_cmd: bool,
    pub sequence_timeout: u16,
    pub sequence_input_mode: SequenceInputMode,
    pub sequence_backtrack_modcancel: bool,
    pub log_layer_changes: bool,
    pub delegate_to_first_layer: bool,
    pub movemouse_inherit_accel_state: bool,
    pub movemouse_smooth_diagonals: bool,
    pub dynamic_macro_max_presses: u16,
    #[cfg(any(target_os = "linux", target_os = "unknown"))]
    pub linux_dev: Vec<String>,
    #[cfg(any(target_os = "linux", target_os = "unknown"))]
    pub linux_dev_names_include: Option<Vec<String>>,
    #[cfg(any(target_os = "linux", target_os = "unknown"))]
    pub linux_dev_names_exclude: Option<Vec<String>>,
    #[cfg(any(target_os = "linux", target_os = "unknown"))]
    pub linux_continue_if_no_devs_found: bool,
    #[cfg(any(target_os = "linux", target_os = "unknown"))]
    pub linux_unicode_u_code: OsCode,
    #[cfg(any(target_os = "linux", target_os = "unknown"))]
    pub linux_unicode_termination: UnicodeTermination,
    #[cfg(any(target_os = "linux", target_os = "unknown"))]
    pub linux_x11_repeat_delay_rate: Option<KeyRepeatSettings>,
    #[cfg(any(target_os = "windows", target_os = "unknown"))]
    pub windows_altgr: AltGrBehaviour,
    #[cfg(any(
        all(feature = "interception_driver", target_os = "windows"),
        target_os = "unknown"
    ))]
    pub windows_interception_mouse_hwid: Option<[u8; HWID_ARR_SZ]>,
}

impl Default for CfgOptions {
    fn default() -> Self {
        Self {
            process_unmapped_keys: false,
            enable_cmd: false,
            sequence_timeout: 1000,
            sequence_input_mode: SequenceInputMode::HiddenSuppressed,
            sequence_backtrack_modcancel: true,
            log_layer_changes: true,
            delegate_to_first_layer: false,
            movemouse_inherit_accel_state: false,
            movemouse_smooth_diagonals: false,
            dynamic_macro_max_presses: 128,
            #[cfg(any(target_os = "linux", target_os = "unknown"))]
            linux_dev: vec![],
            #[cfg(any(target_os = "linux", target_os = "unknown"))]
            linux_dev_names_include: None,
            #[cfg(any(target_os = "linux", target_os = "unknown"))]
            linux_dev_names_exclude: None,
            #[cfg(any(target_os = "linux", target_os = "unknown"))]
            linux_continue_if_no_devs_found: false,
            #[cfg(any(target_os = "linux", target_os = "unknown"))]
            // historically was the only option, so make KEY_U the default
            linux_unicode_u_code: OsCode::KEY_U,
            #[cfg(any(target_os = "linux", target_os = "unknown"))]
            // historically was the only option, so make Enter the default
            linux_unicode_termination: UnicodeTermination::Enter,
            #[cfg(any(target_os = "linux", target_os = "unknown"))]
            linux_x11_repeat_delay_rate: None,
            #[cfg(any(target_os = "windows", target_os = "unknown"))]
            windows_altgr: AltGrBehaviour::DoNothing,
            #[cfg(any(
                all(feature = "interception_driver", target_os = "windows"),
                target_os = "unknown"
            ))]
            windows_interception_mouse_hwid: None,
        }
    }
}

/// Parse configuration entries from an expression starting with defcfg.
pub fn parse_defcfg(expr: &[SExpr]) -> Result<CfgOptions> {
    let mut seen_keys = HashSet::default();
    let mut cfg = CfgOptions::default();
    let mut exprs = check_first_expr(expr.iter(), "defcfg")?;
    // Read k-v pairs from the configuration
    loop {
        let key = match exprs.next() {
            Some(k) => k,
            None => return Ok(cfg),
        };
        let val = match exprs.next() {
            Some(v) => v,
            None => bail_expr!(key, "Found a defcfg option missing a value"),
        };
        match (&key, &val) {
            (SExpr::Atom(k), SExpr::Atom(v)) => {
                if !seen_keys.insert(&k.t) {
                    bail_expr!(key, "Duplicate defcfg option {}", k.t);
                }
                match k.t.as_str() {
                    k @ "sequence-timeout" => {
                        cfg.sequence_timeout = parse_cfg_val_u16(val, k, true)?;
                    }
                    "sequence-input-mode" => {
                        cfg.sequence_input_mode =
                            SequenceInputMode::try_from_str(&v.t.trim_matches('"'))
                                .map_err(|e| anyhow_expr!(val, "{}", e.to_string()))?;
                    }
                    k @ "dynamic-macro-max-presses" => {
                        cfg.dynamic_macro_max_presses = parse_cfg_val_u16(val, k, false)?;
                    }
                    "linux-dev" => {
                        #[cfg(any(target_os = "linux", target_os = "unknown"))]
                        {
                            let paths = v.t.trim_matches('"');
                            cfg.linux_dev = parse_colon_separated_text(paths);
                        }
                    }
                    "linux-dev-names-include" => {
                        #[cfg(any(target_os = "linux", target_os = "unknown"))]
                        {
                            let paths = v.t.trim_matches('"');
                            cfg.linux_dev_names_include = Some(parse_colon_separated_text(paths));
                        }
                    }
                    "linux-dev-names-exclude" => {
                        #[cfg(any(target_os = "linux", target_os = "unknown"))]
                        {
                            let paths = v.t.trim_matches('"');
                            cfg.linux_dev_names_exclude = Some(parse_colon_separated_text(paths));
                        }
                    }
                    _k @ "linux-unicode-u-code" => {
                        #[cfg(any(target_os = "linux", target_os = "unknown"))]
                        {
                            cfg.linux_unicode_u_code = str_to_oscode(v.t.trim_matches('"'))
                                .ok_or_else(|| {
                                    anyhow_expr!(val, "unknown code for {_k}: {}", v.t)
                                })?;
                        }
                    }
                    _k @ "linux-unicode-termination" => {
                        #[cfg(any(target_os = "linux", target_os = "unknown"))]
                        {
                            cfg.linux_unicode_termination = match v.t.trim_matches('"') {
                                "enter" => UnicodeTermination::Enter,
                                "space" => UnicodeTermination::Space,
                                "enter-space" => UnicodeTermination::EnterSpace,
                                "space-enter" => UnicodeTermination::SpaceEnter,
                                _ => bail_expr!(
                                    val,
                                    "{_k} got {}. It accepts: enter|space|enter-space|space-enter",
                                    v.t
                                ),
                            }
                        }
                    }
                    _k @ "linux-x11-repeat-delay-rate" => {
                        #[cfg(any(target_os = "linux", target_os = "unknown"))]
                        {
                            let delay_rate = v.t.trim_matches('"').split(',').collect::<Vec<_>>();
                            let errmsg = anyhow_span!(v, "Invalid value for {_k}.\nExpected two numbers 0-65535 separated by a comma, e.g. 200,25");
                            if delay_rate.len() != 2 {
                                bail!("{:?}", errmsg)
                            }
                            cfg.linux_x11_repeat_delay_rate = Some(KeyRepeatSettings {
                                delay: match str::parse::<u16>(delay_rate[0]) {
                                    Ok(delay) => delay,
                                    Err(_) => bail!("{:?}", errmsg),
                                },
                                rate: match str::parse::<u16>(delay_rate[0]) {
                                    Ok(rate) => rate,
                                    Err(_) => bail!("{:?}", errmsg),
                                },
                            });
                        }
                    }
                    _k @ "windows-altgr" => {
                        #[cfg(any(target_os = "windows", target_os = "unknown"))]
                        {
                            const CANCEL: &str = "cancel-lctl-press";
                            const ADD: &str = "add-lctl-release";
                            cfg.windows_altgr = match v.t.trim_matches('"') {
                                CANCEL => AltGrBehaviour::CancelLctlPress,
                                ADD => AltGrBehaviour::AddLctlRelease,
                                _ => bail_expr!(
                                    val,
                                    "Invalid value for {_k}: {}. Valid values are {},{}",
                                    v.t,
                                    CANCEL,
                                    ADD
                                ),
                            }
                        }
                    }
                    _k @ "windows-interception-mouse-hwid" => {
                        #[cfg(any(
                            all(feature = "interception_driver", target_os = "windows"),
                            target_os = "unknown"
                        ))]
                        {
                            let hwid = v.t.trim_matches('"');
                            log::trace!("win hwid: {hwid}");
                            let hwid_vec = hwid
                                .split_whitespace()
                                .try_fold(vec![], |mut hwid_bytes, hwid_byte| {
                                    hwid_byte.trim_matches(',').parse::<u8>().map(|b| {
                                        hwid_bytes.push(b);
                                        hwid_bytes
                                    })
                                }).map_err(|_| anyhow_expr!(val, "{_k} format is invalid. It should consist of integers separated by commas"))?;
                            let hwid_slice = hwid_vec.iter().copied().enumerate()
                                .try_fold([0u8; HWID_ARR_SZ], |mut hwid, idx_byte| {
                                    let (i, b) = idx_byte;
                                    if i > HWID_ARR_SZ {
                                        bail_expr!(val, "{_k} is too long; it should be up to {HWID_ARR_SZ} 8-bit unsigned integers")
                                    }
                                    hwid[i] = b;
                                    Ok(hwid)
                            });
                            cfg.windows_interception_mouse_hwid = Some(hwid_slice?);
                        }
                    }

                    "process-unmapped-keys" => {
                        cfg.process_unmapped_keys = parse_defcfg_val_bool(val, &k.t)?
                    }
                    "danger-enable-cmd" => cfg.enable_cmd = parse_defcfg_val_bool(val, &k.t)?,
                    "sequence-backtrack-modcancel" => {
                        cfg.sequence_backtrack_modcancel = parse_defcfg_val_bool(val, &k.t)?
                    }
                    "log-layer-changes" => {
                        cfg.log_layer_changes = parse_defcfg_val_bool(val, &k.t)?
                    }
                    "delegate-to-first-layer" => {
                        cfg.delegate_to_first_layer = parse_defcfg_val_bool(val, &k.t)?;
                        if cfg.delegate_to_first_layer {
                            log::info!("delegating transparent keys on other layers to first defined layer");
                        }
                    }
                    "linux-continue-if-no-devs-found" => {
                        #[cfg(any(target_os = "linux", target_os = "unknown"))]
                        {
                            cfg.linux_continue_if_no_devs_found = parse_defcfg_val_bool(val, &k.t)?
                        }
                    }
                    "movemouse-smooth-diagonals" => {
                        cfg.movemouse_smooth_diagonals = parse_defcfg_val_bool(val, &k.t)?
                    }
                    "movemouse-inherit-accel-state" => {
                        cfg.movemouse_inherit_accel_state = parse_defcfg_val_bool(val, &k.t)?
                    }
                    _ => bail_expr!(key, "Unknown defcfg option {}", &k.t),
                };
            }
            (SExpr::List(_), _) => {
                bail_expr!(key, "Lists are not allowed in defcfg");
            }
            (_, SExpr::List(_)) => {
                bail_expr!(val, "Lists are not allowed in defcfg");
            }
        }
    }
}

pub const FALSE_VALUES: [&str; 3] = ["no", "false", "0"];
pub const TRUE_VALUES: [&str; 3] = ["yes", "true", "1"];
pub const BOOLEAN_VALUES: [&str; 6] = ["yes", "true", "1", "no", "false", "0"];

fn parse_defcfg_val_bool(expr: &SExpr, label: &str) -> Result<bool> {
    match &expr {
        SExpr::Atom(v) => {
            let val = v.t.trim_matches('"').to_ascii_lowercase();
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
        SExpr::Atom(v) => Ok(str::parse::<u16>(&v.t.trim_matches('"'))
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

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct KeyRepeatSettings {
    pub delay: u16,
    pub rate: u16,
}

#[cfg(any(target_os = "windows", target_os = "unknown"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AltGrBehaviour {
    DoNothing,
    CancelLctlPress,
    AddLctlRelease,
}

