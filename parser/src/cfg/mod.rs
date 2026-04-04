//! This parses the configuration language to create a `kanata_keyberon::layout::Layout` as well as
//! associated metadata to help with processing.
//!
//! How the configuration maps to keyberon:
//!
//! If the mapped keys are defined as:
//!
//! (defsrc
//!     esc  1    2    3    4
//! )
//!
//! and the layers are:
//!
//! (deflayer one
//!     _   a    s    d    _
//! )
//!
//! (deflayer two
//!     _   a    o    e    _
//! )
//!
//! Then the keyberon layers will be as follows:
//!
//! (xx means unimportant and _ means transparent)
//!
//! layers[0] = { xx, esc, a, s, d, 4, xx... }
//! layers[1] = { xx, _  , a, s, d, _, xx... }
//! layers[2] = { xx, esc, a, o, e, 4, xx... }
//! layers[3] = { xx, _  , a, o, e, _, xx... }
//!
//! Note that this example isn't practical, but `(defsrc esc 1 2 3 4)` is used because these keys
//! are at the beginning of the array. The column index for layers is the numerical value of
//! the key from `keys::OsCode`.
//!
//! In addition, there are two versions of each layer. One version delegates transparent entries to
//! the key defined in defsrc, while the other keeps them as actually transparent. This is to match
//! the behaviour in kmonad.
//!
//! The specific values in example above applies to Linux, but the same logic applies to Windows.

pub(crate) mod alloc;
use alloc::*;
mod arbitrary_code;
use arbitrary_code::*;
mod caps_word;
use caps_word::*;
mod chord;
use chord::*;
mod chord_v1;
use chord_v1::*;
mod clipboard;
use clipboard::*;
mod cmd;
use cmd::*;
mod custom_tap_hold;
use custom_tap_hold::*;
mod defcfg;
pub use defcfg::*;
mod defhands;
use defhands::{
    parse_defhands, parse_tap_hold_opposite_hand, parse_tap_hold_opposite_hand_release,
};
mod deflocalkeys;
use deflocalkeys::*;
mod defsrc;
use defsrc::*;
mod deflayer;
use deflayer::*;
mod deftemplate;
pub use deftemplate::*;
mod error;
pub use error::*;
mod fake_key;
use fake_key::*;
mod fork;
pub use fake_key::{FAKE_KEY_ROW, NORMAL_KEY_ROW};
use fork::*;
mod is_a_button;
use is_a_button::*;
mod live_reload;
use live_reload::*;
mod key_outputs;
pub use key_outputs::*;
mod key_override;
pub use key_override::*;
pub mod layer_opts;
use layer_opts::*;
pub mod list_actions;
use list_actions::*;
mod r#macro;
use r#macro::*;
mod mouse;
use mouse::*;
mod multi;
use multi::*;
mod oneshot;
use oneshot::*;
mod r#override;
use r#override::*;
mod permutations;
use permutations::*;
mod platform;
use platform::*;
mod push_msg;
pub use push_msg::SimpleSExpr;
use push_msg::*;
mod releases;
use releases::*;
mod sequence;
use sequence::*;
pub mod sexpr;
use sexpr::*;
mod str_ext;
pub use str_ext::*;
mod switch;
pub use switch::*;
mod tap_dance;
use tap_dance::*;
mod tap_hold;
use tap_hold::*;
mod unicode;
use unicode::*;
mod unmod;
use unmod::*;
mod vars;
use vars::*;
mod zippychord;
pub use zippychord::*;

use crate::custom_action::*;
use crate::keys::*;
use crate::layers::*;
use crate::lsp_hints::{self, *};
use crate::sequences::*;
use crate::trie::Trie;

use anyhow::anyhow;
use ordered_float::OrderedFloat;

use std::cell::Cell;
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use kanata_keyberon::action::*;
use kanata_keyberon::chord::ChordsV2;
use kanata_keyberon::key_code::*;
use kanata_keyberon::layout::*;

type HashSet<T> = rustc_hash::FxHashSet<T>;
type HashMap<K, V> = rustc_hash::FxHashMap<K, V>;

#[cfg(test)]
mod tests;
#[cfg(test)]
pub use sexpr::parse;

#[macro_export]
macro_rules! bail {
    ($err:expr $(,)?) => {
        return Err(ParseError::from(anyhow::anyhow!($err)))
    };
    ($fmt:expr, $($arg:tt)*) => {
        return Err(ParseError::from(anyhow::anyhow!($fmt, $($arg)*)))
    };
}

#[macro_export]
macro_rules! bail_expr {
    ($expr:expr, $fmt:expr $(,)?) => {
        return Err(ParseError::from_expr($expr, format!($fmt)))
    };
    ($expr:expr, $fmt:expr, $($arg:tt)*) => {
        return Err(ParseError::from_expr($expr, format!($fmt, $($arg)*)))
    };
}

#[macro_export]
macro_rules! err_expr {
    ($expr:expr, $fmt:expr $(,)?) => {
        Err(ParseError::from_expr($expr, format!($fmt)))
    };
    ($expr:expr, $fmt:expr, $($arg:tt)*) => {
        Err(ParseError::from_expr($expr, format!($fmt, $($arg)*)))
    };
}

#[macro_export]
macro_rules! bail_span {
    ($expr:expr, $fmt:expr $(,)?) => {
        return Err(ParseError::from_spanned($expr, format!($fmt)))
    };
    ($expr:expr, $fmt:expr, $($arg:tt)*) => {
        return Err(ParseError::from_spanned($expr, format!($fmt, $($arg)*)))
    };
}

#[macro_export]
macro_rules! err_span {
    ($expr:expr, $fmt:expr $(,)?) => {
        Err(ParseError::from_spanned($expr, format!($fmt)))
    };
    ($expr:expr, $fmt:expr, $($arg:tt)*) => {
        Err(ParseError::from_spanned($expr, format!($fmt, $($arg)*)))
    };
}

#[macro_export]
macro_rules! anyhow_expr {
    ($expr:expr, $fmt:expr $(,)?) => {
        ParseError::from_expr($expr, format!($fmt))
    };
    ($expr:expr, $fmt:expr, $($arg:tt)*) => {
        ParseError::from_expr($expr, format!($fmt, $($arg)*))
    };
}

#[macro_export]
macro_rules! anyhow_span {
    ($expr:expr, $fmt:expr $(,)?) => {
        ParseError::from_spanned($expr, format!($fmt))
    };
    ($expr:expr, $fmt:expr, $($arg:tt)*) => {
        ParseError::from_spanned($expr, format!($fmt, $($arg)*))
    };
}

pub struct FileContentProvider<'a> {
    /// A function to load content of a file from a filepath.
    /// Optionally, it could implement caching and a mechanism preventing "file" and "./file"
    /// from loading twice.
    get_file_content_fn: &'a mut dyn FnMut(&Path) -> std::result::Result<String, String>,
}

impl<'a> FileContentProvider<'a> {
    pub fn new(
        get_file_content_fn: &'a mut impl FnMut(&Path) -> std::result::Result<String, String>,
    ) -> Self {
        Self {
            get_file_content_fn,
        }
    }
    pub fn get_file_content(&mut self, filename: &Path) -> std::result::Result<String, String> {
        (self.get_file_content_fn)(filename)
    }
}

pub type KanataAction = Action<'static, KanataCustom>;
type KLayout = Layout<'static, KEYS_IN_ROW, 2, KanataCustom>;

type TapHoldCustomFunc = fn(&[OsCode], &Allocations) -> &'static custom_tap_hold::CustomTapHoldFn;

pub type BorrowedKLayout<'a> = Layout<'a, KEYS_IN_ROW, 2, &'a CustomAction>;
pub type KeySeqsToFKeys = Trie<(u8, u16)>;

pub struct KanataLayout {
    layout: KLayout,
    _allocations: Arc<Allocations>,
}

impl KanataLayout {
    fn new(layout: KLayout, a: Arc<Allocations>) -> Self {
        Self {
            layout,
            _allocations: a,
        }
    }

    /// bm stands for borrow mut.
    pub fn bm(&mut self) -> &mut BorrowedKLayout<'_> {
        // shrink the lifetime
        unsafe { std::mem::transmute(&mut self.layout) }
    }

    /// b stands for borrow.
    pub fn b(&self) -> &BorrowedKLayout<'_> {
        // shrink the lifetime
        unsafe { std::mem::transmute(&self.layout) }
    }
}

pub struct Cfg {
    /// The list of keys that kanata should be processing. Keys that are missing from `mapped_keys`
    /// that are received from the OS input mechanism will be forwarded to OS output mechanism
    /// without going through kanata's processing.
    pub mapped_keys: MappedKeys,
    /// The potential outputs for a physical key position. The intention behind this is for sending
    /// key repeats.
    pub key_outputs: KeyOutputs,
    /// Layer info used for printing to the logs.
    pub layer_info: Vec<LayerInfo>,
    /// Configuration items in `defcfg`.
    pub options: CfgOptions,
    /// The keyberon layout state machine struct.
    pub layout: KanataLayout,
    /// Sequences defined in `defseq`.
    pub sequences: KeySeqsToFKeys,
    /// Overrides defined in `defoverrides`.
    pub overrides: Overrides,
    /// Mapping of fake key name to its column in the fake key row.
    pub fake_keys: HashMap<String, usize>,
    /// The maximum value of switch's key-timing item in the configuration.
    pub max_key_timing_check: u16,
    /// Zipchord-like configuration.
    pub zippy: Option<(ZchPossibleChords, ZchConfig)>,
}

/// Parse a new configuration from a file.
pub fn new_from_file(p: &Path) -> MResult<Cfg> {
    parse_cfg(p)
}

pub fn new_from_str(cfg_text: &str, file_content: HashMap<String, String>) -> MResult<Cfg> {
    let mut s = ParserState::default();
    let icfg = parse_cfg_raw_string(
        cfg_text,
        &mut s,
        &PathBuf::from("configuration"),
        &mut FileContentProvider {
            get_file_content_fn: &mut move |fname| match file_content
                .get(fname.to_string_lossy().as_ref())
            {
                Some(s) => Ok(s.clone()),
                None => Err("File is not known".into()),
            },
        },
        DEF_LOCAL_KEYS,
        Err("environment variables are not supported".into()),
    )?;
    log::info!("config file is valid");
    Ok(populate_cfg_with_icfg(icfg, s))
}

pub type MappedKeys = HashSet<OsCode>;

#[derive(Debug)]
pub struct LayerInfo {
    pub name: String,
    pub cfg_text: String,
    pub icon: Option<String>,
}

#[allow(clippy::type_complexity)] // return type is not pub
fn parse_cfg(p: &Path) -> MResult<Cfg> {
    let mut s = ParserState::default();
    let icfg = parse_cfg_raw(p, &mut s)?;
    log::info!("config file is valid");
    Ok(populate_cfg_with_icfg(icfg, s))
}

fn populate_cfg_with_icfg(icfg: IntermediateCfg, s: ParserState) -> Cfg {
    let (layers, allocations) = icfg.klayers.get();
    let key_outputs = create_key_outputs(&layers, &icfg.overrides, &icfg.chords_v2);
    let max_key_timing_check = std::cmp::max(
        s.max_key_timing_check.get(),
        icfg.options.tap_hold_require_prior_idle,
    );
    let mut layout = KanataLayout::new(
        Layout::new_with_trans_action_settings(
            s.a.sref(s.defsrc_layer),
            layers,
            icfg.options.trans_resolution_behavior_v2,
            icfg.options.delegate_to_first_layer,
        ),
        allocations,
    );
    layout.bm().chords_v2 = icfg.chords_v2;
    layout.bm().quick_tap_hold_timeout = icfg.options.concurrent_tap_hold;
    layout.bm().tap_hold_require_prior_idle = icfg.options.tap_hold_require_prior_idle;
    layout.bm().oneshot.pause_input_processing_delay = icfg.options.rapid_event_delay;
    if let Some(s) = icfg.start_action {
        layout
            .bm()
            .action_queue
            .push_front(Some(((1, 0), 0, s, Default::default())));
    }
    let mut fake_keys: HashMap<String, usize> = s
        .virtual_keys
        .iter()
        .map(|(k, v)| (k.clone(), v.0))
        .collect();
    fake_keys.shrink_to_fit();
    Cfg {
        options: icfg.options,
        mapped_keys: icfg.mapped_keys,
        layer_info: icfg.layer_info,
        key_outputs,
        layout,
        sequences: icfg.sequences,
        overrides: icfg.overrides,
        fake_keys,
        max_key_timing_check,
        zippy: icfg.zippy,
    }
}

#[derive(Debug)]
pub struct IntermediateCfg {
    pub options: CfgOptions,
    pub mapped_keys: MappedKeys,
    pub layer_info: Vec<LayerInfo>,
    pub klayers: KanataLayers,
    pub sequences: KeySeqsToFKeys,
    pub overrides: Overrides,
    pub chords_v2: Option<ChordsV2<'static, KanataCustom>>,
    pub start_action: Option<&'static KanataAction>,
    pub zippy: Option<(ZchPossibleChords, ZchConfig)>,
}

// A snapshot of enviroment variables, or an error message with an explanation
// why env vars are not supported.
pub type EnvVars = std::result::Result<Vec<(String, String)>, String>;

#[allow(clippy::type_complexity)] // return type is not pub
fn parse_cfg_raw(p: &Path, s: &mut ParserState) -> MResult<IntermediateCfg> {
    const INVALID_PATH_ERROR: &str = "The provided config file path is not valid";

    let mut loaded_files: HashSet<PathBuf> = HashSet::default();

    let mut get_file_content_fn_impl = |filepath: &Path| {
        // Make the include paths relative to main config file instead of kanata executable.
        let filepath_relative_to_loaded_kanata_cfg = if filepath.is_absolute() {
            filepath.to_owned()
        } else {
            let relative_main_cfg_file_dir = p.parent().ok_or(INVALID_PATH_ERROR)?;
            relative_main_cfg_file_dir.join(filepath)
        };

        let Ok(abs_filepath) = filepath_relative_to_loaded_kanata_cfg.canonicalize() else {
            log::info!(
                "Failed to resolve relative path: {}. Ignoring this file.",
                filepath_relative_to_loaded_kanata_cfg.to_string_lossy()
            );
            return Ok("".to_owned());
        };

        // Forbid loading the same file multiple times.
        // This prevents a potential recursive infinite loop of includes
        // (if includes within includes were to be allowed).
        if !loaded_files.insert(abs_filepath.clone()) {
            return Err("The provided config file was already included before".to_string());
        };

        std::fs::read_to_string(abs_filepath.to_str().ok_or(INVALID_PATH_ERROR)?)
            .map_err(|e| format!("Failed to include file: {e}"))
    };
    let mut file_content_provider = FileContentProvider::new(&mut get_file_content_fn_impl);

    // `get_file_content_fn_impl` already uses CWD of the main config path,
    // so we need to provide only the name, not the whole path.
    let cfg_file_name: PathBuf = p
        .file_name()
        .ok_or_else(|| miette::miette!(INVALID_PATH_ERROR))?
        .into();
    let text = file_content_provider
        .get_file_content(&cfg_file_name)
        .map_err(|e| miette::miette!(e))?;

    let env_vars: EnvVars = Ok(std::env::vars().collect());

    parse_cfg_raw_string(
        &text,
        s,
        p,
        &mut file_content_provider,
        DEF_LOCAL_KEYS,
        env_vars,
    )
    .map_err(|e| e.into())
}

fn expand_includes(
    xs: Vec<TopLevel>,
    file_content_provider: &mut FileContentProvider,
    _lsp_hints: &mut LspHints,
) -> Result<Vec<TopLevel>> {
    let include_is_first_atom = gen_first_atom_filter("include");
    xs.iter().try_fold(Vec::new(), |mut acc, spanned_exprs| {
        if include_is_first_atom(&&spanned_exprs.t) {
            let mut exprs =
                check_first_expr(spanned_exprs.t.iter(), "include").expect("can't fail");

            let expr = exprs.next().ok_or(anyhow_span!(
                spanned_exprs,
                "Every include block must contain exactly one filepath"
            ))?;

            let spanned_filepath = match expr {
                SExpr::Atom(filepath) => filepath,
                SExpr::List(_) => {
                    bail_expr!(expr, "Filepath cannot be a list")
                }
            };

            if let Some(expr) = exprs.next() {
                bail_expr!(
                    expr,
                    "Multiple filepaths are not allowed in include blocks. If you want to include multiple files, create a new include block for each of them."
                )
            };
            let include_file_path = spanned_filepath.t.trim_atom_quotes();
            let file_content = file_content_provider.get_file_content(Path::new(include_file_path))
                .map_err(|e| anyhow_span!(spanned_filepath, "{e}"))?;
            let tree = sexpr::parse(&file_content, include_file_path)?;
            acc.extend(tree);

            #[cfg(feature = "lsp")]
            _lsp_hints.reference_locations.include.push_from_atom(spanned_filepath);

            Ok(acc)
        } else {
            acc.push(spanned_exprs.clone());
            Ok(acc)
        }
    })
}

#[cfg(feature = "lsp")]
thread_local! {
    pub(crate) static LSP_VARIABLE_REFERENCES: RefCell<crate::lsp_hints::ReferencesMap> =
        RefCell::new(crate::lsp_hints::ReferencesMap::default());
}

#[allow(clippy::type_complexity)] // return type is not pub
pub fn parse_cfg_raw_string(
    text: &str,
    s: &mut ParserState,
    cfg_path: &Path,
    file_content_provider: &mut FileContentProvider,
    def_local_keys_variant_to_apply: &str,
    env_vars: EnvVars,
) -> Result<IntermediateCfg> {
    let mut lsp_hints: LspHints = Default::default();

    let spanned_root_exprs = sexpr::parse(text, &cfg_path.to_string_lossy())
        .and_then(|xs| expand_includes(xs, file_content_provider, &mut lsp_hints))
        .and_then(|xs| {
            filter_platform_specific_cfg(xs, def_local_keys_variant_to_apply, &mut lsp_hints)
        })
        .and_then(|xs| filter_env_specific_cfg(xs, &env_vars, &mut lsp_hints))
        .and_then(|xs| expand_templates(xs, &mut lsp_hints))?;

    if let Some(spanned) = spanned_root_exprs
        .iter()
        .find(gen_first_atom_filter_spanned("include"))
    {
        bail_span!(spanned, "Nested includes are not allowed.")
    }

    let root_exprs: Vec<_> = spanned_root_exprs.iter().map(|t| t.t.clone()).collect();

    error_on_unknown_top_level_atoms(&spanned_root_exprs)?;

    let mut local_keys: Option<HashMap<String, OsCode>> = None;
    clear_custom_str_oscode_mapping();
    for def_local_keys_variant in DEFLOCALKEYS_VARIANTS {
        let Some((result, _span)) = spanned_root_exprs
            .iter()
            .find(gen_first_atom_filter_spanned(def_local_keys_variant))
            .map(|x| {
                (
                    parse_deflocalkeys(def_local_keys_variant, &x.t),
                    x.span.clone(),
                )
            })
        else {
            continue;
        };

        let mapping = result?;
        if def_local_keys_variant == &def_local_keys_variant_to_apply {
            assert!(
                local_keys.is_none(),
                ">1 mutually exclusive deflocalkeys variants were parsed"
            );
            local_keys = Some(mapping);
        } else {
            #[cfg(feature = "lsp")]
            lsp_hints.inactive_code.push(lsp_hints::InactiveCode {
                span: _span,
                reason: format!(
                    "Another localkeys variant is currently active: {def_local_keys_variant_to_apply}"
                    ),
            })
        }

        if let Some(spanned) = spanned_root_exprs
            .iter()
            .filter(gen_first_atom_filter_spanned(def_local_keys_variant))
            .nth(1)
        {
            bail_span!(
                spanned,
                "Only one {def_local_keys_variant} is allowed, found more. Delete the extras."
            )
        }
    }
    replace_custom_str_oscode_mapping(&local_keys.unwrap_or_default());

    #[allow(unused_mut)]
    let mut cfg = root_exprs
        .iter()
        .find(gen_first_atom_filter("defcfg"))
        .map(|cfg| parse_defcfg(cfg))
        .transpose()?
        .unwrap_or_else(|| {
            log::warn!("No defcfg is defined. Consider whether the process-unmapped-keys defcfg option should be yes vs. no. Adding defcfg with process-unmapped-keys defined will remove this warning.");
            Default::default()
        });
    if let Some(spanned) = spanned_root_exprs
        .iter()
        .filter(gen_first_atom_filter_spanned("defcfg"))
        .nth(1)
    {
        bail_span!(
            spanned,
            "Only one defcfg is allowed, found more. Delete the extras."
        )
    }
    let src_expr = root_exprs
        .iter()
        .find(gen_first_atom_filter("defsrc"))
        .ok_or_else(|| anyhow!("Exactly one defsrc must exist; found none"))?;
    if let Some(spanned) = spanned_root_exprs
        .iter()
        .filter(gen_first_atom_filter_spanned("defsrc"))
        .nth(1)
    {
        bail_span!(
            spanned,
            "Exactly one defsrc is allowed, found more. Delete the extras."
        )
    }
    let (mut mapped_keys, mapping_order, _mouse_in_defsrc) = parse_defsrc(src_expr, &cfg)?;
    #[cfg(any(target_os = "linux", target_os = "android", target_os = "unknown"))]
    if cfg.linux_opts.linux_device_detect_mode.is_none() {
        cfg.linux_opts.linux_device_detect_mode = Some(match _mouse_in_defsrc {
            MouseInDefsrc::MouseUsed => DeviceDetectMode::Any,
            MouseInDefsrc::NoMouse => DeviceDetectMode::KeyboardMice,
        });
    }

    let var_exprs = root_exprs
        .iter()
        .filter(gen_first_atom_filter("defvar"))
        .collect::<Vec<_>>();
    let vars = parse_vars(&var_exprs, &mut lsp_hints)?;

    let deflayer_labels = [DEFLAYER, DEFLAYER_MAPPED];
    let deflayer_filter = |exprs: &&Vec<SExpr>| -> bool {
        if exprs.is_empty() {
            return false;
        }
        if let SExpr::Atom(atom) = &exprs[0] {
            deflayer_labels.contains(&atom.t.as_str())
        } else {
            false
        }
    };
    let deflayer_spanned_filter =
        |exprs: &&Spanned<Vec<SExpr>>| -> bool { deflayer_filter(&&exprs.t) };
    let layer_exprs = spanned_root_exprs
        .iter()
        .filter(deflayer_spanned_filter)
        .map(|e| match e.t[0].atom(None).unwrap() {
            DEFLAYER => SpannedLayerExprs::DefsrcMapping(e.clone()),
            DEFLAYER_MAPPED => SpannedLayerExprs::CustomMapping(e.clone()),
            _ => unreachable!(),
        })
        .collect::<Vec<_>>();
    if layer_exprs.is_empty() {
        bail!("No deflayer expressions exist. At least one layer must be defined.")
    }

    let (layer_idxs, layer_icons) =
        parse_layer_indexes(&layer_exprs, mapping_order.len(), &vars, &mut lsp_hints)?;
    let mut sorted_idxs: Vec<(&String, &usize)> =
        layer_idxs.iter().map(|tuple| (tuple.0, tuple.1)).collect();

    sorted_idxs.sort_by_key(|f| f.1);

    #[allow(clippy::needless_collect)]
    // Clippy suggests using the sorted_idxs iter directly and manipulating it
    // to produce the layer_names vec when creating Vec<LayerInfo> below
    let layer_names = sorted_idxs
        .into_iter()
        .map(|(name, _)| (*name).clone())
        .collect::<Vec<_>>();

    let layer_strings = spanned_root_exprs
        .iter()
        .filter(|expr| deflayer_filter(&&expr.t))
        .map(|expr| expr.span.file_content()[expr.span.clone()].to_string())
        .collect::<Vec<_>>();

    let layer_info: Vec<LayerInfo> = layer_names
        .into_iter()
        .zip(layer_strings)
        .map(|(name, cfg_text)| LayerInfo {
            name: name.clone(),
            cfg_text,
            icon: layer_icons.get(&name).unwrap_or(&None).clone(),
        })
        .collect();

    let defsrc_layer = create_defsrc_layer();

    let deflayer_filter = |exprs: &&Vec<SExpr>| -> bool {
        if exprs.is_empty() {
            return false;
        }
        if let SExpr::Atom(atom) = &exprs[0] {
            deflayer_labels.contains(&atom.t.as_str())
        } else {
            false
        }
    };
    let layer_exprs = root_exprs
        .iter()
        .filter(deflayer_filter)
        .map(|e| match e[0].atom(None).unwrap() {
            DEFLAYER => LayerExprs::DefsrcMapping(e.clone()),
            DEFLAYER_MAPPED => LayerExprs::CustomMapping(e.clone()),
            _ => unreachable!(),
        })
        .collect::<Vec<_>>();

    *s = ParserState {
        a: s.a.clone(),
        layer_exprs,
        layer_idxs,
        mapping_order,
        defsrc_layer,
        is_cmd_enabled: {
            #[cfg(feature = "cmd")]
            {
                if cfg.enable_cmd {
                    log::warn!("DANGER! cmd action is enabled.");
                    true
                } else {
                    false
                }
            }
            #[cfg(not(feature = "cmd"))]
            {
                log::info!("NOTE: kanata was compiled to never allow cmd");
                false
            }
        },
        delegate_to_first_layer: cfg.delegate_to_first_layer,
        default_sequence_timeout: cfg.sequence_timeout,
        default_sequence_input_mode: cfg.sequence_input_mode,
        block_unmapped_keys: cfg.block_unmapped_keys,
        lsp_hints: RefCell::new(lsp_hints),
        vars,
        max_key_timing_check: Cell::new(cfg.rapid_event_delay),
        ..Default::default()
    };

    let defhands_exprs = root_exprs
        .iter()
        .filter(gen_first_atom_filter("defhands"))
        .collect::<Vec<_>>();
    match defhands_exprs.len() {
        0 => {}
        1 => {
            let hand_map = parse_defhands(defhands_exprs[0], s)?;
            s.hand_map = Some(s.a.sref(hand_map));
        }
        _ => {
            let spanned = spanned_root_exprs
                .iter()
                .filter(gen_first_atom_filter_spanned("defhands"))
                .nth(1)
                .expect(">= 2 defhands");
            bail_span!(
                spanned,
                "Only one defhands block is allowed, found more. Delete the extras."
            );
        }
    }

    let chords_exprs = spanned_root_exprs
        .iter()
        .filter(gen_first_atom_filter_spanned("defchords"))
        .collect::<Vec<_>>();
    parse_chord_groups(&chords_exprs, s)?;

    let fake_keys_exprs = root_exprs
        .iter()
        .filter(gen_first_atom_filter("deffakekeys"))
        .collect::<Vec<_>>();
    parse_fake_keys(&fake_keys_exprs, s)?;

    let vkeys_exprs = root_exprs
        .iter()
        .filter(gen_first_atom_filter("defvirtualkeys"))
        .collect::<Vec<_>>();
    parse_virtual_keys(&vkeys_exprs, s)?;

    let sequence_exprs = root_exprs
        .iter()
        .filter(gen_first_atom_filter("defseq"))
        .collect::<Vec<_>>();
    let sequences = parse_sequences(&sequence_exprs, s)?;

    let alias_exprs = spanned_root_exprs
        .iter()
        .filter(gen_first_atom_start_filter_spanned("defalias"))
        .collect::<Vec<_>>();
    parse_aliases(&alias_exprs, s, &env_vars)?;

    let start_action = cfg
        .start_alias
        .as_ref()
        .and_then(|start| s.aliases.get(start).copied());
    if let (Some(_), None) = (cfg.start_alias.as_ref(), start_action) {
        bail!("alias-to-trigger-on-load was given, but alias could not be found")
    }

    let mut klayers = parse_layers(s, &mut mapped_keys, &cfg)?;

    resolve_chord_groups(&mut klayers, s)?;
    let layers = s.a.bref_slice(klayers);
    s.layers = layers;

    let override_exprs = root_exprs
        .iter()
        .filter(gen_first_atom_filter("defoverrides"))
        .collect::<Vec<_>>();
    let (overrides, overrides_v1_exists) = match override_exprs.len() {
        0 => (Overrides::new(&[]), false),
        1 => (parse_overrides(override_exprs[0], s)?, true),
        _ => {
            let spanned = spanned_root_exprs
                .iter()
                .filter(gen_first_atom_filter_spanned("defoverrides"))
                .nth(1)
                .expect(">= 2 overrides");
            bail_span!(
                spanned,
                "Only one defoverrides allowed, found more. Delete the extras."
            )
        }
    };

    let overridesv2_exprs = root_exprs
        .iter()
        .filter(gen_first_atom_filter("defoverridesv2"))
        .collect::<Vec<_>>();
    let overrides = match overridesv2_exprs.len() {
        0 => overrides,
        1 => match overrides_v1_exists {
            false => parse_overridesv2(overridesv2_exprs[0], s)?,
            true => {
                let spanned = spanned_root_exprs
                    .iter()
                    .find(gen_first_atom_filter_spanned("defoverridesv2"))
                    .expect("1 overridesv2");
                bail_span!(
                    spanned,
                    "Only one of defoverrides or defoverridesv2 allowed, found both. Delete one of them."
                )
            }
        },
        _ => {
            let spanned = spanned_root_exprs
                .iter()
                .filter(gen_first_atom_filter_spanned("defoverridesv2"))
                .nth(1)
                .expect(">= 2 overridesv2");
            bail_span!(
                spanned,
                "Only one defoverridesv2 allowed, found more. Delete the extras."
            )
        }
    };

    let defchordsv2_filter = |exprs: &&Vec<SExpr>| -> bool {
        if exprs.is_empty() {
            return false;
        }
        if let SExpr::Atom(atom) = &exprs[0] {
            matches!(atom.t.as_str(), "defchordsv2" | "defchordsv2-experimental")
        } else {
            false
        }
    };
    let defchordsv2_spanned_filter =
        |exprs: &&Spanned<Vec<SExpr>>| -> bool { defchordsv2_filter(&&exprs.t) };

    s.pctx.trans_forbidden_reason = Some("Transparent action is forbidden within chordsv2");
    let chords_v2_exprs = root_exprs
        .iter()
        .filter(defchordsv2_filter)
        .collect::<Vec<_>>();
    let chords_v2 = match chords_v2_exprs.len() {
        0 => None,
        1 => {
            let cfks = parse_defchordv2(chords_v2_exprs[0], s)?;
            Some(ChordsV2::new(cfks, cfg.chords_v2_min_idle))
        }
        _ => {
            let spanned = spanned_root_exprs
                .iter()
                .filter(defchordsv2_spanned_filter)
                .nth(1)
                .expect("> 2 overrides");
            bail_span!(
                spanned,
                "Only one defchordsv2 allowed, found more.\nDelete the extras."
            )
        }
    };
    s.pctx.trans_forbidden_reason = None;
    if chords_v2.is_some() && !cfg.concurrent_tap_hold {
        return Err(anyhow!(
            "With defchordsv2 defined, concurrent-tap-hold in defcfg must be true.\n\
            It is currently false or unspecified."
        )
        .into());
    }

    let defzippy_filter = |exprs: &&Vec<SExpr>| -> bool {
        if exprs.is_empty() {
            return false;
        }
        if let SExpr::Atom(atom) = &exprs[0] {
            matches!(atom.t.as_str(), "defzippy" | "defzippy-experimental")
        } else {
            false
        }
    };
    let defzippy_spanned_filter =
        |exprs: &&Spanned<Vec<SExpr>>| -> bool { defzippy_filter(&&exprs.t) };

    let zippy_exprs = root_exprs
        .iter()
        .filter(defzippy_filter)
        .collect::<Vec<_>>();
    let zippy = match zippy_exprs.len() {
        0 => None,
        1 => {
            let zippy = parse_zippy(zippy_exprs[0], s, file_content_provider)?;
            Some(zippy)
        }
        _ => {
            let spanned = spanned_root_exprs
                .iter()
                .filter(defzippy_spanned_filter)
                .nth(1)
                .expect("> 2 overrides");
            bail_span!(
                spanned,
                "Only one defzippy allowed, found more.\nDelete the extras."
            )
        }
    };

    #[cfg(feature = "lsp")]
    LSP_VARIABLE_REFERENCES.with_borrow_mut(|refs| {
        s.lsp_hints
            .borrow_mut()
            .reference_locations
            .variable
            .0
            .extend(refs.0.drain());
    });

    let klayers = unsafe { KanataLayers::new(layers, s.a.clone()) };
    Ok(IntermediateCfg {
        options: cfg,
        mapped_keys,
        layer_info,
        klayers,
        sequences,
        overrides,
        chords_v2,
        start_action,
        zippy,
    })
}

fn error_on_unknown_top_level_atoms(exprs: &[Spanned<Vec<SExpr>>]) -> Result<()> {
    for expr in exprs {
        expr.t
            .first()
            .ok_or_else(|| {
                anyhow_span!(
                    expr,
                    "Found empty list as a configuration item, you should delete this"
                )
            })?
            .atom(None)
            .map(|a| match a {
                "defcfg"
                | "defalias"
                | "defaliasenvcond"
                | "defsrc"
                | DEFLAYER
                | DEFLAYER_MAPPED
                | "defoverrides"
                | "defoverridesv2"
                | "deflocalkeys-macos"
                | "deflocalkeys-linux"
                | "deflocalkeys-win"
                | "deflocalkeys-winiov2"
                | "deflocalkeys-wintercept"
                | "deffakekeys"
                | "defvirtualkeys"
                | "defchords"
                | "defvar"
                | "deftemplate"
                | "defchordsv2"
                | "defchordsv2-experimental"
                | "defzippy"
                | "defzippy-experimental"
                | "defseq"
                | "defhands" => Ok(()),
                _ => err_span!(expr, "Found unknown configuration item"),
            })
            .ok_or_else(|| {
                anyhow_expr!(
                    expr.t.first().expect("not empty"),
                    "Invalid: found list as first item in a configuration item"
                )
            })??;
    }
    Ok(())
}

/// Return a closure that filters a root expression by the content of the first element. The
/// closure returns true if the first element is an atom that matches the input `a` and false
/// otherwise.
fn gen_first_atom_filter(a: &str) -> impl Fn(&&Vec<SExpr>) -> bool {
    let a = a.to_owned();
    move |expr| {
        if expr.is_empty() {
            return false;
        }
        if let SExpr::Atom(atom) = &expr[0] {
            atom.t == a
        } else {
            false
        }
    }
}

/// Return a closure that filters a root expression by the content of the first element. The
/// closure returns true if the first element is an atom that starts with the input `a` and false
/// otherwise.
fn gen_first_atom_start_filter_spanned(a: &str) -> impl Fn(&&Spanned<Vec<SExpr>>) -> bool {
    let a = a.to_owned();
    move |expr| {
        if expr.t.is_empty() {
            return false;
        }
        if let SExpr::Atom(atom) = &expr.t[0] {
            atom.t.starts_with(&a)
        } else {
            false
        }
    }
}

/// Return a closure that filters a root expression by the content of the first element. The
/// closure returns true if the first element is an atom that matches the input `a` and false
/// otherwise.
fn gen_first_atom_filter_spanned(a: &str) -> impl Fn(&&Spanned<Vec<SExpr>>) -> bool {
    let a = a.to_owned();
    move |expr| {
        if expr.t.is_empty() {
            return false;
        }
        if let SExpr::Atom(atom) = &expr.t[0] {
            atom.t == a
        } else {
            false
        }
    }
}

/// Consumes the first element and returns the rest of the iterator. Returns `Ok` if the first
/// element is an atom and equals `expected_first`.
fn check_first_expr<'a>(
    mut exprs: impl Iterator<Item = &'a SExpr>,
    expected_first: &str,
) -> Result<impl Iterator<Item = &'a SExpr>> {
    let first_atom = exprs
        .next()
        .ok_or_else(|| anyhow!("Passed empty list to {expected_first}"))?
        .atom(None)
        .ok_or_else(|| anyhow!("First entry is expected to be an atom for {expected_first}"))?;
    if first_atom != expected_first {
        bail!("Passed non-{expected_first} expression to {expected_first}: {first_atom}");
    }
    Ok(exprs)
}

#[derive(Debug, Copy, Clone)]
enum MouseInDefsrc {
    MouseUsed,
    NoMouse,
}

type Aliases = HashMap<String, &'static KanataAction>;

#[derive(Debug, Clone)]
enum LayerExprs {
    DefsrcMapping(Vec<SExpr>),
    CustomMapping(Vec<SExpr>),
}

#[derive(Debug, Clone)]
enum SpannedLayerExprs {
    DefsrcMapping(Spanned<Vec<SExpr>>),
    CustomMapping(Spanned<Vec<SExpr>>),
}

#[derive(Debug, Clone, Default)]
pub struct ParserContext {
    is_within_defvirtualkeys: bool,
    trans_forbidden_reason: Option<&'static str>,
}

#[derive(Debug)]
pub struct ParserState {
    layers: KLayers,
    layer_exprs: Vec<LayerExprs>,
    aliases: Aliases,
    layer_idxs: LayerIndexes,
    mapping_order: Vec<usize>,
    virtual_keys: HashMap<String, (usize, &'static KanataAction)>,
    chord_groups: HashMap<String, ChordGroup>,
    defsrc_layer: [KanataAction; KEYS_IN_ROW],
    vars: HashMap<String, SExpr>,
    is_cmd_enabled: bool,
    delegate_to_first_layer: bool,
    default_sequence_timeout: u16,
    default_sequence_input_mode: SequenceInputMode,
    block_unmapped_keys: bool,
    max_key_timing_check: Cell<u16>,
    multi_action_nest_count: Cell<u16>,
    pctx: ParserContext,
    pub lsp_hints: RefCell<LspHints>,
    hand_map: Option<&'static custom_tap_hold::HandMap>,
    a: Arc<Allocations>,
}

impl ParserState {
    fn vars(&self) -> Option<&HashMap<String, SExpr>> {
        Some(&self.vars)
    }
}

impl Default for ParserState {
    fn default() -> Self {
        let default_cfg = CfgOptions::default();
        Self {
            layers: Default::default(),
            layer_exprs: Default::default(),
            aliases: Default::default(),
            layer_idxs: Default::default(),
            mapping_order: Default::default(),
            defsrc_layer: [KanataAction::NoOp; KEYS_IN_ROW],
            virtual_keys: Default::default(),
            chord_groups: Default::default(),
            vars: Default::default(),
            is_cmd_enabled: default_cfg.enable_cmd,
            delegate_to_first_layer: default_cfg.delegate_to_first_layer,
            default_sequence_timeout: default_cfg.sequence_timeout,
            default_sequence_input_mode: default_cfg.sequence_input_mode,
            block_unmapped_keys: default_cfg.block_unmapped_keys,
            max_key_timing_check: Cell::new(0),
            multi_action_nest_count: Cell::new(0),
            lsp_hints: Default::default(),
            hand_map: None,
            a: unsafe { Allocations::new() },
            pctx: ParserContext::default(),
        }
    }
}

/// Parse alias->action mappings from multiple exprs starting with defalias.
/// Mutates the input `s` by storing aliases inside.
fn parse_aliases(
    exprs: &[&Spanned<Vec<SExpr>>],
    s: &mut ParserState,
    env_vars: &EnvVars,
) -> Result<()> {
    for expr in exprs {
        handle_standard_defalias(&expr.t, s)?;
        handle_envcond_defalias(expr, s, env_vars)?;
    }
    Ok(())
}

fn handle_standard_defalias(expr: &[SExpr], s: &mut ParserState) -> Result<()> {
    let subexprs = match check_first_expr(expr.iter(), "defalias") {
        Ok(s) => s,
        Err(_) => return Ok(()),
    };
    read_alias_name_action_pairs(subexprs, s)
}

fn handle_envcond_defalias(
    exprs: &Spanned<Vec<SExpr>>,
    s: &mut ParserState,
    env_vars: &EnvVars,
) -> Result<()> {
    let mut subexprs = match check_first_expr(exprs.t.iter(), "defaliasenvcond") {
        Ok(exprs) => exprs,
        Err(_) => return Ok(()),
    };

    let conderr = "defaliasenvcond must have a list with 2 strings as the first parameter:\n\
            (<env var name> <env var value>)";

    // Check that there is a list containing the environment variable name and value that
    // determines if this defalias entry should be used. If there is no match, return early.
    match subexprs.next() {
        Some(expr) => {
            let envcond = expr.list(s.vars()).ok_or_else(|| {
                anyhow_expr!(expr, "Found a string, but expected a list.\n{conderr}")
            })?;
            if envcond.len() != 2 {
                bail_expr!(expr, "List has the incorrect number of items.\n{conderr}");
            }
            let env_var_name = envcond[0].atom(s.vars()).ok_or_else(|| {
                anyhow_expr!(
                    expr,
                    "Environment variable name must be a string, not a list.\n{conderr}"
                )
            })?;
            let env_var_value = envcond[1].atom(s.vars()).ok_or_else(|| {
                anyhow_expr!(
                    expr,
                    "Environment variable value must be a string, not a list.\n{conderr}"
                )
            })?;
            match env_vars {
                Ok(vars) => {
                    let values_of_matching_vars: Vec<_> = vars
                        .iter()
                        .filter_map(|(k, v)| if k == env_var_name { Some(v) } else { None })
                        .collect();
                    if values_of_matching_vars.is_empty() {
                        let msg = format!("Env var '{env_var_name}' is not set");
                        #[cfg(feature = "lsp")]
                        s.lsp_hints
                            .borrow_mut()
                            .inactive_code
                            .push(lsp_hints::InactiveCode {
                                span: exprs.span.clone(),
                                reason: msg.clone(),
                            });
                        log::info!("{msg}, skipping associated aliases");
                        return Ok(());
                    } else if !values_of_matching_vars.iter().any(|&v| v == env_var_value) {
                        let msg =
                            format!("Env var '{env_var_name}' is set, but value doesn't match");
                        #[cfg(feature = "lsp")]
                        s.lsp_hints
                            .borrow_mut()
                            .inactive_code
                            .push(lsp_hints::InactiveCode {
                                span: exprs.span.clone(),
                                reason: msg.clone(),
                            });
                        log::info!("{msg}, skipping associated aliases");
                        return Ok(());
                    }
                }
                Err(err) => {
                    bail_expr!(expr, "{err}");
                }
            }
            log::info!("Found env var ({env_var_name} {env_var_value}), using associated aliases");
        }
        None => bail_expr!(&exprs.t[0], "Missing a list item.\n{conderr}"),
    };
    read_alias_name_action_pairs(subexprs, s)
}

fn read_alias_name_action_pairs<'a>(
    mut exprs: impl Iterator<Item = &'a SExpr>,
    s: &mut ParserState,
) -> Result<()> {
    // Read k-v pairs from the configuration
    while let Some(alias_expr) = exprs.next() {
        let alias = match alias_expr {
            SExpr::Atom(a) => &a.t,
            _ => bail_expr!(
                alias_expr,
                "Alias names cannot be lists. Invalid alias: {:?}",
                alias_expr
            ),
        };
        let action = match exprs.next() {
            Some(v) => v,
            None => bail_expr!(alias_expr, "Found alias without an action - add an action"),
        };
        let action = parse_action(action, s)?;
        if s.aliases.insert(alias.into(), action).is_some() {
            bail_expr!(alias_expr, "Duplicate alias: {}", alias);
        }
        #[cfg(feature = "lsp")]
        s.lsp_hints
            .borrow_mut()
            .definition_locations
            .alias
            .insert(alias.into(), alias_expr.span());
    }
    Ok(())
}

/// Parse a `kanata_keyberon::action::Action` from a `SExpr`.
fn parse_action(expr: &SExpr, s: &ParserState) -> Result<&'static KanataAction> {
    expr.atom(s.vars())
        .map(|a| parse_action_atom(&Spanned::new(a.into(), expr.span()), s))
        .unwrap_or_else(|| {
            expr.list(s.vars())
                .map(|l| parse_action_list(l, s))
                .expect("must be atom or list")
        })
        .map_err(|mut e| {
            if e.span.is_none() {
                e.span = Some(expr.span())
            };
            e
        })
}

/// Returns a single custom action in the proper wrapped type.
fn custom(ca: CustomAction, a: &Allocations) -> Result<&'static KanataAction> {
    Ok(a.sref(Action::Custom(a.sref(ca))))
}

/// Parse a `kanata_keyberon::action::Action` from a string.
fn parse_action_atom(ac_span: &Spanned<String>, s: &ParserState) -> Result<&'static KanataAction> {
    let ac = &*ac_span.t;
    if is_list_action(ac) {
        bail_span!(
            ac_span,
            "This is a list action and must be in parentheses: ({ac} ...)"
        );
    }
    match ac {
        "_" | "‗" | "≝" => {
            if let Some(trans_forbidden_reason) = s.pctx.trans_forbidden_reason {
                bail_span!(ac_span, "{trans_forbidden_reason}");
            } else {
                return Ok(s.a.sref(Action::Trans));
            }
        }
        "XX" | "✗" | "∅" | "•" => {
            return Ok(s.a.sref(Action::NoOp));
        }
        "lrld" => return custom(CustomAction::LiveReload, &s.a),
        "lrld-next" | "lrnx" => return custom(CustomAction::LiveReloadNext, &s.a),
        "lrld-prev" | "lrpv" => return custom(CustomAction::LiveReloadPrev, &s.a),
        "sldr" => {
            return custom(
                CustomAction::SequenceLeader(
                    s.default_sequence_timeout,
                    s.default_sequence_input_mode,
                ),
                &s.a,
            );
        }
        "scnl" => return custom(CustomAction::SequenceCancel, &s.a),
        "mlft" | "mouseleft" => return custom(CustomAction::Mouse(Btn::Left), &s.a),
        "mrgt" | "mouseright" => return custom(CustomAction::Mouse(Btn::Right), &s.a),
        "mmid" | "mousemid" => return custom(CustomAction::Mouse(Btn::Mid), &s.a),
        "mfwd" | "mouseforward" => return custom(CustomAction::Mouse(Btn::Forward), &s.a),
        "mbck" | "mousebackward" => return custom(CustomAction::Mouse(Btn::Backward), &s.a),
        "mltp" | "mousetapleft" => return custom(CustomAction::MouseTap(Btn::Left), &s.a),
        "mrtp" | "mousetapright" => return custom(CustomAction::MouseTap(Btn::Right), &s.a),
        "mmtp" | "mousetapmid" => return custom(CustomAction::MouseTap(Btn::Mid), &s.a),
        "mftp" | "mousetapforward" => return custom(CustomAction::MouseTap(Btn::Forward), &s.a),
        "mbtp" | "mousetapbackward" => return custom(CustomAction::MouseTap(Btn::Backward), &s.a),
        "mwu" | "mousewheelup" => {
            return custom(
                CustomAction::MWheelNotch {
                    direction: MWheelDirection::Up,
                },
                &s.a,
            );
        }
        "mwd" | "mousewheeldown" => {
            return custom(
                CustomAction::MWheelNotch {
                    direction: MWheelDirection::Down,
                },
                &s.a,
            );
        }
        "mwl" | "mousewheelleft" => {
            return custom(
                CustomAction::MWheelNotch {
                    direction: MWheelDirection::Left,
                },
                &s.a,
            );
        }
        "mwr" | "mousewheelright" => {
            return custom(
                CustomAction::MWheelNotch {
                    direction: MWheelDirection::Right,
                },
                &s.a,
            );
        }
        "rpt" | "repeat" | "rpt-key" => return custom(CustomAction::Repeat, &s.a),
        "rpt-any" => return Ok(s.a.sref(Action::Repeat)),
        "dynamic-macro-record-stop" => {
            return custom(CustomAction::DynamicMacroRecordStop(0), &s.a);
        }
        "reverse-release-order" => match s.multi_action_nest_count.get() {
            0 => bail_span!(
                ac_span,
                "reverse-release-order is only allowed inside of a (multi ...) action list"
            ),
            _ => return custom(CustomAction::ReverseReleaseOrder, &s.a),
        },
        "use-defsrc" => {
            return Ok(s.a.sref(Action::Src));
        }
        "mvmt" | "mousemovement" | "🖰mv" => {
            bail_span!(ac_span, "{ac} can only be used as an input")
        }
        _ => {}
    };
    if let Some(oscode) = str_to_oscode(ac) {
        if matches!(ac, "comp" | "cmp") {
            log::warn!(
                "comp/cmp/cmps is not actually a compose key even though its correpsonding code is KEY_COMPOSE. Its actual functionality is context menu which somewhat behaves like right-click.\nTo remove this warning, replace this usage with an equivalent key name such as: menu"
            );
        }
        return Ok(s.a.sref(k(oscode.into())));
    }
    if let Some(alias) = ac.strip_prefix('@') {
        return match s.aliases.get(alias) {
            Some(ac) => {
                #[cfg(feature = "lsp")]
                s.lsp_hints
                    .borrow_mut()
                    .reference_locations
                    .alias
                    .push(alias, ac_span.span.clone());
                Ok(*ac)
            }
            None => match s.pctx.is_within_defvirtualkeys {
                true => bail_span!(
                    ac_span,
                    "Aliases are not usable within defvirtualkeys. You may use vars or templates.",
                ),
                false => bail_span!(
                    ac_span,
                    "Referenced unknown alias {}. Note that order of declarations matter.",
                    alias
                ),
            },
        };
    }
    if let Some(unisym) = ac.strip_prefix('🔣') {
        // TODO: when unicode accepts multiple chars, change this to feed the whole string, not just the first char
        return custom(
            CustomAction::Unicode(unisym.chars().next().expect("1 char")),
            &s.a,
        );
    }
    // Parse a sequence like `C-S-v` or `C-A-del`
    let (mut keys, unparsed_str) = parse_mod_prefix(ac)?;
    keys.push(
        str_to_oscode(unparsed_str)
            .ok_or_else(|| {
                // check aliases
                if s.aliases.contains_key(ac) {
                    anyhow!("Unknown key/action: {ac}. If you meant to use an alias, prefix it with '@' symbol: @{ac}")
                } else if s.vars.contains_key(ac) {
                    anyhow!("Unknown key/action: {ac}. If you meant to use a variable, prefix it with '$' symbol: ${ac}")
                } else {
                    anyhow!("Unknown key/action: {ac}")
                }
            })?
            .into(),
    );
    if keys.contains(&KEY_OVERLAP) {
        bail!("O- is only valid in sequences for lists of keys");
    }
    Ok(s.a.sref(Action::MultipleKeyCodes(s.a.sref(s.a.sref_vec(keys)))))
}

/// Parse a `kanata_keyberon::action::Action` from a `SExpr::List`.
fn parse_action_list(ac: &[SExpr], s: &ParserState) -> Result<&'static KanataAction> {
    if ac.is_empty() {
        return Ok(s.a.sref(Action::NoOp));
    }
    let ac_type = match &ac[0] {
        SExpr::Atom(a) => &a.t,
        _ => bail!("All list actions must start with string and not a list"),
    };
    if !is_list_action(ac_type) {
        bail_expr!(&ac[0], "Unknown action type: {ac_type}");
    }
    match ac_type.as_str() {
        LAYER_SWITCH => parse_layer_base(&ac[1..], s),
        LAYER_TOGGLE | LAYER_WHILE_HELD => parse_layer_toggle(&ac[1..], s),
        TAP_HOLD => parse_tap_hold(&ac[1..], s, HoldTapConfig::Default),
        TAP_HOLD_PRESS | TAP_HOLD_PRESS_A => {
            parse_tap_hold(&ac[1..], s, HoldTapConfig::HoldOnOtherKeyPress)
        }
        TAP_HOLD_ORDER => parse_tap_hold_order(&ac[1..], s),
        TAP_HOLD_RELEASE | TAP_HOLD_RELEASE_A => {
            parse_tap_hold(&ac[1..], s, HoldTapConfig::PermissiveHold)
        }
        TAP_HOLD_PRESS_TIMEOUT | TAP_HOLD_PRESS_TIMEOUT_A => {
            parse_tap_hold_timeout(&ac[1..], s, HoldTapConfig::HoldOnOtherKeyPress)
        }
        TAP_HOLD_RELEASE_TIMEOUT | TAP_HOLD_RELEASE_TIMEOUT_A => {
            parse_tap_hold_timeout(&ac[1..], s, HoldTapConfig::PermissiveHold)
        }
        TAP_HOLD_RELEASE_KEYS_TAP_RELEASE => parse_tap_hold_keys_trigger_tap_release(&ac[1..], s),
        TAP_HOLD_KEYS => parse_tap_hold_keys_named_lists(&ac[1..], s),
        TAP_HOLD_RELEASE_KEYS | TAP_HOLD_RELEASE_KEYS_A => {
            parse_tap_hold_keys(&ac[1..], s, TAP_HOLD_RELEASE_KEYS, custom_tap_hold_release)
        }
        TAP_HOLD_EXCEPT_KEYS | TAP_HOLD_EXCEPT_KEYS_A => {
            parse_tap_hold_keys(&ac[1..], s, TAP_HOLD_EXCEPT_KEYS, custom_tap_hold_except)
        }
        TAP_HOLD_TAP_KEYS | TAP_HOLD_TAP_KEYS_A => {
            parse_tap_hold_keys(&ac[1..], s, TAP_HOLD_TAP_KEYS, custom_tap_hold_tap_keys)
        }
        TAP_HOLD_OPPOSITE_HAND => parse_tap_hold_opposite_hand(&ac[1..], s),
        TAP_HOLD_OPPOSITE_HAND_RELEASE => parse_tap_hold_opposite_hand_release(&ac[1..], s),
        MULTI => parse_multi(&ac[1..], s),
        MACRO => parse_macro(&ac[1..], s, RepeatMacro::No),
        MACRO_REPEAT | MACRO_REPEAT_A => parse_macro(&ac[1..], s, RepeatMacro::Yes),
        MACRO_RELEASE_CANCEL | MACRO_RELEASE_CANCEL_A => {
            parse_macro_release_cancel(&ac[1..], s, RepeatMacro::No)
        }
        MACRO_REPEAT_RELEASE_CANCEL | MACRO_REPEAT_RELEASE_CANCEL_A => {
            parse_macro_release_cancel(&ac[1..], s, RepeatMacro::Yes)
        }
        MACRO_CANCEL_ON_NEXT_PRESS => {
            parse_macro_cancel_on_next_press(&ac[1..], s, RepeatMacro::No)
        }
        MACRO_REPEAT_CANCEL_ON_NEXT_PRESS => {
            parse_macro_cancel_on_next_press(&ac[1..], s, RepeatMacro::Yes)
        }
        MACRO_CANCEL_ON_NEXT_PRESS_CANCEL_ON_RELEASE => {
            parse_macro_cancel_on_next_press_cancel_on_release(&ac[1..], s, RepeatMacro::No)
        }
        MACRO_REPEAT_CANCEL_ON_NEXT_PRESS_CANCEL_ON_RELEASE => {
            parse_macro_cancel_on_next_press_cancel_on_release(&ac[1..], s, RepeatMacro::Yes)
        }
        UNICODE | SYM => parse_unicode(&ac[1..], s),
        ONE_SHOT | ONE_SHOT_PRESS | ONE_SHOT_PRESS_A => {
            parse_one_shot(&ac[1..], s, OneShotEndConfig::EndOnFirstPress)
        }
        ONE_SHOT_RELEASE | ONE_SHOT_RELEASE_A => {
            parse_one_shot(&ac[1..], s, OneShotEndConfig::EndOnFirstRelease)
        }
        ONE_SHOT_PRESS_PCANCEL | ONE_SHOT_PRESS_PCANCEL_A => {
            parse_one_shot(&ac[1..], s, OneShotEndConfig::EndOnFirstPressOrRepress)
        }
        ONE_SHOT_RELEASE_PCANCEL | ONE_SHOT_RELEASE_PCANCEL_A => {
            parse_one_shot(&ac[1..], s, OneShotEndConfig::EndOnFirstReleaseOrRepress)
        }
        ONE_SHOT_PAUSE_PROCESSING => parse_one_shot_pause_processing(&ac[1..], s),
        TAP_DANCE => parse_tap_dance(&ac[1..], s, TapDanceConfig::Lazy),
        TAP_DANCE_EAGER => parse_tap_dance(&ac[1..], s, TapDanceConfig::Eager),
        CHORD => parse_chord(&ac[1..], s),
        RELEASE_KEY | RELEASE_KEY_A => parse_release_key(&ac[1..], s),
        RELEASE_LAYER | RELEASE_LAYER_A => parse_release_layer(&ac[1..], s),
        ON_PRESS_FAKEKEY | ON_PRESS_FAKEKEY_A => parse_on_press_fake_key_op(&ac[1..], s),
        ON_RELEASE_FAKEKEY | ON_RELEASE_FAKEKEY_A => parse_on_release_fake_key_op(&ac[1..], s),
        ON_PRESS_DELAY | ON_PRESS_FAKEKEY_DELAY | ON_PRESS_FAKEKEY_DELAY_A => {
            parse_fake_key_delay(&ac[1..], s)
        }
        ON_RELEASE_DELAY | ON_RELEASE_FAKEKEY_DELAY | ON_RELEASE_FAKEKEY_DELAY_A => {
            parse_on_release_fake_key_delay(&ac[1..], s)
        }
        ON_IDLE_FAKEKEY => parse_on_idle_fakekey(&ac[1..], s),
        ON_PRESS | ON_PRESS_A => parse_on_press(&ac[1..], s),
        ON_RELEASE | ON_RELEASE_A => parse_on_release(&ac[1..], s),
        ON_IDLE => parse_on_idle(&ac[1..], s),
        ON_PHYSICAL_IDLE => parse_on_physical_idle(&ac[1..], s),
        HOLD_FOR_DURATION => parse_hold_for_duration(&ac[1..], s),
        MWHEEL_UP | MWHEEL_UP_A => parse_mwheel(&ac[1..], MWheelDirection::Up, s),
        MWHEEL_DOWN | MWHEEL_DOWN_A => parse_mwheel(&ac[1..], MWheelDirection::Down, s),
        MWHEEL_LEFT | MWHEEL_LEFT_A => parse_mwheel(&ac[1..], MWheelDirection::Left, s),
        MWHEEL_RIGHT | MWHEEL_RIGHT_A => parse_mwheel(&ac[1..], MWheelDirection::Right, s),
        MWHEEL_ACCEL_UP => parse_mwheel_accel(&ac[1..], MWheelDirection::Up, s),
        MWHEEL_ACCEL_DOWN => parse_mwheel_accel(&ac[1..], MWheelDirection::Down, s),
        MWHEEL_ACCEL_LEFT => parse_mwheel_accel(&ac[1..], MWheelDirection::Left, s),
        MWHEEL_ACCEL_RIGHT => parse_mwheel_accel(&ac[1..], MWheelDirection::Right, s),
        MOVEMOUSE_UP | MOVEMOUSE_UP_A => parse_move_mouse(&ac[1..], MoveDirection::Up, s),
        MOVEMOUSE_DOWN | MOVEMOUSE_DOWN_A => parse_move_mouse(&ac[1..], MoveDirection::Down, s),
        MOVEMOUSE_LEFT | MOVEMOUSE_LEFT_A => parse_move_mouse(&ac[1..], MoveDirection::Left, s),
        MOVEMOUSE_RIGHT | MOVEMOUSE_RIGHT_A => parse_move_mouse(&ac[1..], MoveDirection::Right, s),
        MOVEMOUSE_ACCEL_UP | MOVEMOUSE_ACCEL_UP_A => {
            parse_move_mouse_accel(&ac[1..], MoveDirection::Up, s)
        }
        MOVEMOUSE_ACCEL_DOWN | MOVEMOUSE_ACCEL_DOWN_A => {
            parse_move_mouse_accel(&ac[1..], MoveDirection::Down, s)
        }
        MOVEMOUSE_ACCEL_LEFT | MOVEMOUSE_ACCEL_LEFT_A => {
            parse_move_mouse_accel(&ac[1..], MoveDirection::Left, s)
        }
        MOVEMOUSE_ACCEL_RIGHT | MOVEMOUSE_ACCEL_RIGHT_A => {
            parse_move_mouse_accel(&ac[1..], MoveDirection::Right, s)
        }
        MOVEMOUSE_SPEED | MOVEMOUSE_SPEED_A => parse_move_mouse_speed(&ac[1..], s),
        SETMOUSE | SETMOUSE_A => parse_set_mouse(&ac[1..], s),
        DYNAMIC_MACRO_RECORD => parse_dynamic_macro_record(&ac[1..], s),
        DYNAMIC_MACRO_PLAY => parse_dynamic_macro_play(&ac[1..], s),
        ARBITRARY_CODE => parse_arbitrary_code(&ac[1..], s),
        CMD => parse_cmd(&ac[1..], s, CmdType::Standard),
        CMD_OUTPUT_KEYS => parse_cmd(&ac[1..], s, CmdType::OutputKeys),
        CMD_LOG => parse_cmd_log(&ac[1..], s),
        PUSH_MESSAGE => parse_push_message(&ac[1..], s),
        FORK => parse_fork(&ac[1..], s),
        CAPS_WORD | CAPS_WORD_A => {
            parse_caps_word(&ac[1..], CapsWordRepressBehaviour::Overwrite, s)
        }
        CAPS_WORD_CUSTOM | CAPS_WORD_CUSTOM_A => {
            parse_caps_word_custom(&ac[1..], CapsWordRepressBehaviour::Overwrite, s)
        }
        CAPS_WORD_TOGGLE | CAPS_WORD_TOGGLE_A => {
            parse_caps_word(&ac[1..], CapsWordRepressBehaviour::Toggle, s)
        }
        CAPS_WORD_CUSTOM_TOGGLE | CAPS_WORD_CUSTOM_TOGGLE_A => {
            parse_caps_word_custom(&ac[1..], CapsWordRepressBehaviour::Toggle, s)
        }
        DYNAMIC_MACRO_RECORD_STOP_TRUNCATE => parse_macro_record_stop_truncate(&ac[1..], s),
        SWITCH => parse_switch(&ac[1..], s),
        SEQUENCE => parse_sequence_start(&ac[1..], s),
        SEQUENCE_NOERASE => parse_sequence_noerase(&ac[1..], s),
        UNMOD => parse_unmod(UNMOD, &ac[1..], s),
        UNSHIFT | UNSHIFT_A => parse_unmod(UNSHIFT, &ac[1..], s),
        LIVE_RELOAD_NUM => parse_live_reload_num(&ac[1..], s),
        LIVE_RELOAD_FILE => parse_live_reload_file(&ac[1..], s),
        CLIPBOARD_SET => parse_clipboard_set(&ac[1..], s),
        CLIPBOARD_CMD_SET => parse_cmd(&ac[1..], s, CmdType::ClipboardSet),
        CLIPBOARD_SAVE => parse_clipboard_save(&ac[1..], s),
        CLIPBOARD_RESTORE => parse_clipboard_restore(&ac[1..], s),
        CLIPBOARD_SAVE_SET => parse_clipboard_save_set(&ac[1..], s),
        CLIPBOARD_SAVE_CMD_SET => parse_cmd(&ac[1..], s, CmdType::ClipboardSaveSet),
        CLIPBOARD_SAVE_SWAP => parse_clipboard_save_swap(&ac[1..], s),
        _ => unreachable!(),
    }
}

fn layer_idx(ac_params: &[SExpr], layers: &LayerIndexes, s: &ParserState) -> Result<usize> {
    if ac_params.len() != 1 {
        bail!(
            "Layer actions expect one item: the layer name, found {} items",
            ac_params.len()
        )
    }
    let layer_name = ac_params[0]
        .atom(s.vars())
        .ok_or_else(|| anyhow_expr!(&ac_params[0], "layer name should be a string not a list",))?;
    match layers.get(layer_name) {
        Some(i) => Ok(*i),
        None => err_expr!(
            &ac_params[0],
            "layer name is not declared in any deflayer: {layer_name}"
        ),
    }
}

/// Parse a list expression with length 2 having format:
///     (name value)
/// The items name and value must both be strings.
/// The name string is validated to ensure it matches the input.
/// The value is parsed into a u8.
#[allow(unused)]
fn parse_named_u8_param(name: &str, name_and_param: &SExpr, s: &ParserState) -> Result<u8> {
    let err = || {
        format!(
            "Expected a list with two items: {name} followed by a number. Example:\n\
             ({name} 2)"
        )
    };
    let Some(list) = name_and_param.list(s.vars()) else {
        bail_expr!(name_and_param, "{}", err());
    };
    if list.len() != 2 {
        bail_expr!(name_and_param, "{}", err());
    }
    let Some(expr_name) = list[0].atom(s.vars()) else {
        bail_expr!(&list[0], "Expected {name}");
    };
    if expr_name != name {
        bail_expr!(&list[0], "Expected {name}");
    }
    parse_u8_with_range(&list[1], s, name, 0, 255)
}

fn parse_u8_with_range(expr: &SExpr, s: &ParserState, label: &str, min: u8, max: u8) -> Result<u8> {
    expr.atom(s.vars())
        .map(str::parse::<u8>)
        .and_then(|u| u.ok())
        .and_then(|u| {
            assert!(min <= max);
            if u >= min && u <= max { Some(u) } else { None }
        })
        .ok_or_else(|| anyhow_expr!(expr, "{label} must be {min}-{max}"))
}

fn parse_u16(expr: &SExpr, s: &ParserState, label: &str) -> Result<u16> {
    expr.atom(s.vars())
        .map(str::parse::<u16>)
        .and_then(|u| u.ok())
        .ok_or_else(|| anyhow_expr!(expr, "{label} must be 0-65535"))
}

fn parse_f32(
    expr: &SExpr,
    s: &ParserState,
    label: &str,
    min: f32,
    max: f32,
) -> Result<OrderedFloat<f32>> {
    expr.atom(s.vars())
        .map(str::parse::<f32>)
        .and_then(|u| {
            u.ok().and_then(|v| {
                if v >= min && v <= max {
                    Some(v.into())
                } else {
                    None
                }
            })
        })
        .ok_or_else(|| anyhow_expr!(expr, "{label} must be {min:.2}-{max:.2}"))
}

// Note on allows:
// - macOS CI is behind on Rust version.
// - Clippy bug in new lint of Rust v1.86.
#[allow(unknown_lints)]
#[allow(clippy::manual_ok_err)]
fn parse_non_zero_u16(expr: &SExpr, s: &ParserState, label: &str) -> Result<u16> {
    expr.atom(s.vars())
        .map(str::parse::<u16>)
        .and_then(|u| match u {
            Ok(u @ 1..) => Some(u),
            _ => None,
        })
        .ok_or_else(|| anyhow_expr!(expr, "{label} must be 1-65535"))
}

fn parse_key_list(expr: &SExpr, s: &ParserState, label: &str) -> Result<Vec<OsCode>> {
    expr.list(s.vars())
        .map(|keys| {
            keys.iter().try_fold(vec![], |mut keys, key| {
                key.atom(s.vars())
                    .map(|a| -> Result<()> {
                        keys.push(str_to_oscode(a).ok_or_else(|| {
                            anyhow_expr!(key, "string of a known key is expected")
                        })?);
                        Ok(())
                    })
                    .ok_or_else(|| {
                        anyhow_expr!(key, "string of a known key is expected, found list instead")
                    })??;
                Ok(keys)
            })
        })
        .ok_or_else(|| anyhow_expr!(expr, "{label} must be a list of keys"))?
}

static KEYMODI: &[(&str, KeyCode)] = &[
    ("S-", KeyCode::LShift),
    ("‹⇧", KeyCode::LShift),
    ("⇧›", KeyCode::RShift),
    ("RS-", KeyCode::RShift),
    ("C-", KeyCode::LCtrl),
    ("‹⎈", KeyCode::LCtrl),
    ("‹⌃", KeyCode::LCtrl),
    ("⎈›", KeyCode::RCtrl),
    ("⌃›", KeyCode::RCtrl),
    ("RC-", KeyCode::RCtrl),
    ("M-", KeyCode::LGui),
    ("‹◆", KeyCode::LGui),
    ("‹⌘", KeyCode::LGui),
    ("‹❖", KeyCode::LGui),
    ("◆›", KeyCode::RGui),
    ("⌘›", KeyCode::RGui),
    ("❖›", KeyCode::RGui),
    ("RM-", KeyCode::RGui),
    ("‹⎇", KeyCode::LAlt),
    ("A-", KeyCode::LAlt),
    ("‹⌥", KeyCode::LAlt),
    ("AG-", KeyCode::RAlt),
    ("RA-", KeyCode::RAlt),
    ("⎇›", KeyCode::RAlt),
    ("⌥›", KeyCode::RAlt),
    ("⎈", KeyCode::LCtrl), // Shorter indicators should be at the end to only get matched after
    // indicators with sides have had a chance
    ("⌥", KeyCode::LAlt),
    ("⎇", KeyCode::LAlt),
    ("◆", KeyCode::LGui),
    ("⌘", KeyCode::LGui),
    ("❖", KeyCode::LGui),
    ("O-", KEY_OVERLAP),
];

/// Parses mod keys like `C-S-`. Returns the `KeyCode`s for the modifiers parsed and the unparsed
/// text after any parsed modifier prefixes.
pub fn parse_mod_prefix(mods: &str) -> Result<(Vec<KeyCode>, &str)> {
    let mut key_stack = Vec::new();
    let mut rem = mods;
    loop {
        let mut found_none = true;
        for (key_s, key_code) in KEYMODI {
            if let Some(rest) = rem.strip_prefix(key_s) {
                if key_stack.contains(key_code) {
                    bail!("Redundant \"{key_code:?}\" in {mods:?}");
                }
                key_stack.push(*key_code);
                rem = rest;
                found_none = false;
            }
        }
        if found_none {
            break;
        }
    }
    Ok((key_stack, rem))
}
