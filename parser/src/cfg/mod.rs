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
pub mod sexpr;

pub(crate) mod alloc;
use alloc::*;

mod key_override;
use kanata_keyberon::chord::ChordsV2;
pub use key_override::*;

mod custom_tap_hold;
use custom_tap_hold::*;

pub mod list_actions;
use list_actions::*;

mod defcfg;
pub use defcfg::*;

mod deftemplate;
pub use deftemplate::*;

mod switch;
pub use switch::*;

use crate::custom_action::*;
use crate::keys::*;
use crate::layers::*;

mod error;
pub use error::*;

mod chord;
use chord::*;

mod fake_key;
use fake_key::*;
pub use fake_key::{FAKE_KEY_ROW, NORMAL_KEY_ROW};

mod os_dependent;
use os_dependent::*;

mod is_a_button;
use is_a_button::*;

use crate::trie::Trie;
use anyhow::anyhow;
use std::cell::Cell;
use std::collections::hash_map::Entry;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

type HashSet<T> = rustc_hash::FxHashSet<T>;
type HashMap<K, V> = rustc_hash::FxHashMap<K, V>;

use kanata_keyberon::action::*;
use kanata_keyberon::key_code::*;
use kanata_keyberon::layout::*;
use sexpr::*;

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
    /// Optionally, it could implement caching and a mechanism preventing "file" and "./file" from loading twice.
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

pub type KanataCustom = &'static &'static [&'static CustomAction];
pub type KanataAction = Action<'static, KanataCustom>;
type KLayout = Layout<'static, KEYS_IN_ROW, 2, KanataCustom>;

type TapHoldCustomFunc =
    fn(
        &[OsCode],
        &Allocations,
    ) -> &'static (dyn Fn(QueuedIter) -> (Option<WaitingAction>, bool) + Send + Sync);

pub type BorrowedKLayout<'a> = Layout<'a, KEYS_IN_ROW, 2, &'a &'a [&'a CustomAction]>;
pub type KeySeqsToFKeys = Trie;

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
    pub fn bm(&mut self) -> &mut BorrowedKLayout {
        // shrink the lifetime
        unsafe { std::mem::transmute(&mut self.layout) }
    }

    /// b stands for borrow.
    pub fn b(&self) -> &BorrowedKLayout {
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
    pub switch_max_key_timing: u16,
}

/// Parse a new configuration from a file.
pub fn new_from_file(p: &Path) -> MResult<Cfg> {
    parse_cfg(p)
}

pub fn new_from_str(cfg_text: &str) -> MResult<Cfg> {
    let mut s = ParserState::default();
    let icfg = parse_cfg_raw_string(
        cfg_text,
        &mut s,
        &PathBuf::from("configuration"),
        &mut FileContentProvider {
            get_file_content_fn: &mut |_| Err("include is not supported".into()),
        },
        DEF_LOCAL_KEYS,
        Err("environment variables are not supported".into()),
    )?;
    let (layers, allocations) = icfg.klayers.get();
    let key_outputs = create_key_outputs(&layers, &icfg.overrides, &icfg.chords_v2);
    let switch_max_key_timing = s.switch_max_key_timing.get();
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
    layout.bm().oneshot.on_press_release_delay = icfg.options.rapid_event_delay;
    let mut fake_keys: HashMap<String, usize> = s
        .virtual_keys
        .iter()
        .map(|(k, v)| (k.clone(), v.0))
        .collect();
    fake_keys.shrink_to_fit();
    log::info!("config file is valid");
    Ok(Cfg {
        options: icfg.options,
        mapped_keys: icfg.mapped_keys,
        layer_info: icfg.layer_info,
        key_outputs,
        layout,
        sequences: icfg.sequences,
        overrides: icfg.overrides,
        fake_keys,
        switch_max_key_timing,
    })
}

pub type MappedKeys = HashSet<OsCode>;
// Note: this uses a Vec inside the HashMap instead of a HashSet because ordering matters, e.g. for
// chords like `S-b`, we want to ensure that `b` is checked first because key repeat for `b` is
// useful while it is not useful for shift. The outputs should be iterated over in reverse order.
pub type KeyOutputs = Vec<HashMap<OsCode, Vec<OsCode>>>;

#[derive(Debug)]
pub struct LayerInfo {
    pub name: String,
    pub cfg_text: String,
}

#[allow(clippy::type_complexity)] // return type is not pub
fn parse_cfg(p: &Path) -> MResult<Cfg> {
    let mut s = ParserState::default();
    let icfg = parse_cfg_raw(p, &mut s)?;
    let (layers, allocations) = icfg.klayers.get();
    let key_outputs = create_key_outputs(&layers, &icfg.overrides, &icfg.chords_v2);
    let switch_max_key_timing = s.switch_max_key_timing.get();
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
    layout.bm().oneshot.on_press_release_delay = icfg.options.rapid_event_delay;
    let mut fake_keys: HashMap<String, usize> = s
        .virtual_keys
        .iter()
        .map(|(k, v)| (k.clone(), v.0))
        .collect();
    fake_keys.shrink_to_fit();
    log::info!("config file is valid");
    Ok(Cfg {
        options: icfg.options,
        mapped_keys: icfg.mapped_keys,
        layer_info: icfg.layer_info,
        key_outputs,
        layout,
        sequences: icfg.sequences,
        overrides: icfg.overrides,
        fake_keys,
        switch_max_key_timing,
    })
}

#[cfg(all(
    not(feature = "interception_driver"),
    any(
        not(feature = "win_llhook_read_scancodes"),
        not(feature = "win_sendinput_send_scancodes")
    ),
    target_os = "windows"
))]
const DEF_LOCAL_KEYS: &str = "deflocalkeys-win";
#[cfg(all(
    feature = "win_llhook_read_scancodes",
    feature = "win_sendinput_send_scancodes",
    target_os = "windows"
))]
const DEF_LOCAL_KEYS: &str = "deflocalkeys-winiov2";
#[cfg(all(feature = "interception_driver", target_os = "windows"))]
const DEF_LOCAL_KEYS: &str = "deflocalkeys-wintercept";
#[cfg(target_os = "macos")]
const DEF_LOCAL_KEYS: &str = "deflocalkeys-macos";
#[cfg(any(target_os = "linux", target_os = "unknown"))]
const DEF_LOCAL_KEYS: &str = "deflocalkeys-linux";

#[derive(Debug)]
pub struct IntermediateCfg {
    pub options: CfgOptions,
    pub mapped_keys: MappedKeys,
    pub layer_info: Vec<LayerInfo>,
    pub klayers: KanataLayers,
    pub sequences: KeySeqsToFKeys,
    pub overrides: Overrides,
    pub chords_v2: Option<ChordsV2<'static, KanataCustom>>,
}

// A snapshot of enviroment variables, or an error message with an explanation
// why env vars are not not supported.
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

        let abs_filepath: PathBuf = filepath_relative_to_loaded_kanata_cfg
            .canonicalize()
            .map_err(|e| {
                format!(
                    "Failed to resolve relative path: {}: {}",
                    filepath_relative_to_loaded_kanata_cfg.to_string_lossy(),
                    e
                )
            })?;

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
            let include_file_path = spanned_filepath.t.trim_matches('"');
            let file_content = file_content_provider.get_file_content(Path::new(include_file_path)).map_err(|e| anyhow_span!(spanned_filepath, "{e}"))?;
            let tree = sexpr::parse(&file_content, include_file_path)?;
            acc.extend(tree);

            Ok(acc)
        } else {
            acc.push(spanned_exprs.clone());
            Ok(acc)
        }
    })
}

const DEFLAYER: &str = "deflayer";
const DEFLAYER_MAPPED: &str = "deflayermap";
const DEFLOCALKEYS_VARIANTS: &[&str] = &[
    "deflocalkeys-win",
    "deflocalkeys-winiov2",
    "deflocalkeys-wintercept",
    "deflocalkeys-linux",
    "deflocalkeys-macos",
];

#[derive(Debug, Clone)]
pub struct LspHintInactiveCode {
    pub span: Span,
    pub reason: String,
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
    let mut lsp_hint_inactive_code: Vec<LspHintInactiveCode> = vec![];

    let spanned_root_exprs = sexpr::parse(text, &cfg_path.to_string_lossy())
        .and_then(|xs| expand_includes(xs, file_content_provider))
        .and_then(|xs| {
            filter_platform_specific_cfg(
                xs,
                def_local_keys_variant_to_apply,
                &mut lsp_hint_inactive_code,
            )
        })
        .and_then(expand_templates)?;

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
        if let Some((result, span)) = spanned_root_exprs
            .iter()
            .find(gen_first_atom_filter_spanned(def_local_keys_variant))
            .map(|x| {
                (
                    parse_deflocalkeys(def_local_keys_variant, &x.t),
                    x.span.clone(),
                )
            })
        {
            let mapping = result?;
            if def_local_keys_variant == &def_local_keys_variant_to_apply {
                assert!(
                    local_keys.is_none(),
                    ">1 mutually exclusive deflocalkeys variants were parsed"
                );
                local_keys = Some(mapping);
            } else {
                lsp_hint_inactive_code.push(LspHintInactiveCode {
                    span,
                    reason: format!(
                        "Another localkeys variant is currently active: {def_local_keys_variant_to_apply}"
                    ),
                })
            }
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

    let cfg = root_exprs
        .iter()
        .find(gen_first_atom_filter("defcfg"))
        .map(|cfg| parse_defcfg(cfg))
        .transpose()?
        .unwrap_or_default();
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
    let (mut mapped_keys, mapping_order) = parse_defsrc(src_expr, &cfg)?;

    let deflayer_labels = [DEFLAYER, DEFLAYER_MAPPED];
    let deflayer_spanned_filter = |exprs: &&Spanned<Vec<SExpr>>| -> bool {
        if exprs.t.is_empty() {
            return false;
        }
        if let SExpr::Atom(atom) = &exprs.t[0] {
            deflayer_labels.contains(&atom.t.as_str())
        } else {
            false
        }
    };
    let layer_exprs = spanned_root_exprs
        .iter()
        .filter(deflayer_spanned_filter)
        .cloned()
        .map(|e| match e.t[0].atom(None).unwrap() {
            DEFLAYER => SpannedLayerExprs::DefsrcMapping(e.clone()),
            DEFLAYER_MAPPED => SpannedLayerExprs::CustomMapping(e.clone()),
            _ => unreachable!(),
        })
        .collect::<Vec<_>>();
    if layer_exprs.is_empty() {
        bail!("No deflayer expressions exist. At least one layer must be defined.")
    }

    let layer_idxs = parse_layer_indexes(&layer_exprs, mapping_order.len())?;
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
    let layer_strings = spanned_root_exprs
        .iter()
        .filter(|expr| deflayer_filter(&&expr.t))
        .map(|expr| expr.span.file_content()[expr.span.clone()].to_string())
        .collect::<Vec<_>>();

    let layer_info: Vec<LayerInfo> = layer_names
        .into_iter()
        .zip(layer_strings)
        .map(|(name, cfg_text)| LayerInfo { name, cfg_text })
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
        .cloned()
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
        lsp_hint_inactive_code,
        ..Default::default()
    };

    let var_exprs = root_exprs
        .iter()
        .filter(gen_first_atom_filter("defvar"))
        .collect::<Vec<_>>();
    parse_vars(&var_exprs, s)?;

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

    let mut klayers = parse_layers(s, &mut mapped_keys, &cfg)?;

    resolve_chord_groups(&mut klayers, s)?;

    let override_exprs = root_exprs
        .iter()
        .filter(gen_first_atom_filter("defoverrides"))
        .collect::<Vec<_>>();
    let overrides = match override_exprs.len() {
        0 => Overrides::new(&[]),
        1 => parse_overrides(override_exprs[0], s)?,
        _ => {
            let spanned = spanned_root_exprs
                .iter()
                .filter(gen_first_atom_filter_spanned("defoverrides"))
                .nth(1)
                .expect("> 2 overrides");
            bail_span!(
                spanned,
                "Only one defoverrides allowed, found more. Delete the extras."
            )
        }
    };

    s.trans_forbidden_reason = Some("Transparent action is forbidden within chordsv2");
    let chords_v2_exprs = root_exprs
        .iter()
        .filter(gen_first_atom_filter("defchordsv2-experimental"))
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
                .filter(gen_first_atom_filter_spanned("defchordsv2-experimental"))
                .nth(1)
                .expect("> 2 overrides");
            bail_span!(
                spanned,
                "Only one defchordsv2 allowed, found more.\nDelete the extras."
            )
        }
    };
    s.trans_forbidden_reason = None;
    if chords_v2.is_some() && !cfg.concurrent_tap_hold {
        return Err(anyhow!(
            "With defchordsv2 defined, concurrent-tap-hold in defcfg must be true.\n\
            It is currently false or unspecified."
        )
        .into());
    }
    let layers = s.a.bref_slice(klayers);
    let klayers = unsafe { KanataLayers::new(layers, s.a.clone()) };
    Ok(IntermediateCfg {
        options: cfg,
        mapped_keys,
        layer_info,
        klayers,
        sequences,
        overrides,
        chords_v2,
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
                | "defchordsv2-experimental"
                | "defseq" => Ok(()),
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

/// Parse custom keys from an expression starting with deflocalkeys.
fn parse_deflocalkeys(
    def_local_keys_variant: &str,
    expr: &[SExpr],
) -> Result<HashMap<String, OsCode>> {
    let mut localkeys = HashMap::default();
    let mut exprs = check_first_expr(expr.iter(), def_local_keys_variant)?;
    // Read k-v pairs from the configuration
    while let Some(key_expr) = exprs.next() {
        let key = key_expr.atom(None).ok_or_else(|| {
            anyhow_expr!(key_expr, "No lists are allowed in {def_local_keys_variant}")
        })?;
        if localkeys.contains_key(key) {
            bail_expr!(
                key_expr,
                "Duplicate {key} found in {def_local_keys_variant}"
            );
        }
        let osc = match exprs.next() {
            Some(v) => v
                .atom(None)
                .ok_or_else(|| anyhow_expr!(v, "No lists are allowed in {def_local_keys_variant}"))
                .and_then(|osc| {
                    osc.parse::<u16>().map_err(|_| {
                        anyhow_expr!(v, "Unknown number in {def_local_keys_variant}: {osc}")
                    })
                })
                .and_then(|osc| {
                    OsCode::from_u16(osc).ok_or_else(|| {
                        anyhow_expr!(v, "Unknown number in {def_local_keys_variant}: {osc}")
                    })
                })?,
            None => bail_expr!(key_expr, "Key without a number in {def_local_keys_variant}"),
        };
        log::debug!("custom mapping: {key} {}", osc.as_u16());
        localkeys.insert(key.to_owned(), osc);
    }
    Ok(localkeys)
}

/// Parse mapped keys from an expression starting with defsrc. Returns the key mapping as well as
/// a vec of the indexes in order. The length of the returned vec should be matched by the length
/// of all layer declarations.
fn parse_defsrc(expr: &[SExpr], defcfg: &CfgOptions) -> Result<(MappedKeys, Vec<usize>)> {
    let exprs = check_first_expr(expr.iter(), "defsrc")?;
    let mut mkeys = MappedKeys::default();
    let mut ordered_codes = Vec::new();
    for expr in exprs {
        let s = match expr {
            SExpr::Atom(a) => &a.t,
            _ => bail_expr!(expr, "No lists allowed in defsrc"),
        };
        let oscode = str_to_oscode(s)
            .ok_or_else(|| anyhow_expr!(expr, "Unknown key in defsrc: \"{}\"", s))?;
        if mkeys.contains(&oscode) {
            bail_expr!(expr, "Repeat declaration of key in defsrc: \"{}\"", s)
        }
        mkeys.insert(oscode);
        ordered_codes.push(oscode.into());
    }

    log::info!("process unmapped keys: {}", defcfg.process_unmapped_keys);
    if defcfg.process_unmapped_keys {
        for osc in 0..KEYS_IN_ROW as u16 {
            if let Some(osc) = OsCode::from_u16(osc) {
                match KeyCode::from(osc) {
                    KeyCode::No => {}
                    _ => {
                        mkeys.insert(osc);
                    }
                }
            }
        }
    }

    mkeys.shrink_to_fit();
    Ok((mkeys, ordered_codes))
}

type LayerIndexes = HashMap<String, usize>;
type Aliases = HashMap<String, &'static KanataAction>;

/// Returns layer names and their indexes into the keyberon layout. This also checks that:
/// - All layers have the same number of items as the defsrc,
/// - There are no duplicate layer names
/// - Parentheses weren't used directly or kmonad-style escapes for parentheses weren't used.
fn parse_layer_indexes(exprs: &[SpannedLayerExprs], expected_len: usize) -> Result<LayerIndexes> {
    let mut layer_indexes = HashMap::default();
    for (i, expr_type) in exprs.iter().enumerate() {
        let (mut subexprs, expr, do_element_count_check) = match expr_type {
            SpannedLayerExprs::DefsrcMapping(e) => {
                (check_first_expr(e.t.iter(), DEFLAYER)?, e, true)
            }
            SpannedLayerExprs::CustomMapping(e) => {
                (check_first_expr(e.t.iter(), DEFLAYER_MAPPED)?, e, false)
            }
        };
        let layer_expr = subexprs.next().ok_or_else(|| {
            anyhow_span!(expr, "deflayer requires a name and {expected_len} item(s)")
        })?;
        let layer_name = match expr_type {
            SpannedLayerExprs::DefsrcMapping(_) => layer_expr
                .atom(None)
                .ok_or_else(|| {
                    anyhow_expr!(layer_expr, "layer name after {DEFLAYER} must be a string")
                })?
                .to_owned(),
            SpannedLayerExprs::CustomMapping(_) => {
                let list = layer_expr
                    .list(None)
                    .ok_or_else(|| {
                        anyhow_expr!(
                            layer_expr,
                            "layer name after {DEFLAYER_MAPPED} must be in parentheses"
                        )
                    })?
                    .to_owned();
                if list.len() != 1 {
                    bail_expr!(
                        layer_expr,
                        "layer name after {DEFLAYER_MAPPED} must be a string within one pair of parentheses"
                    );
                }
                list[0].atom(None).ok_or_else(|| anyhow_expr!(layer_expr, "layer name after {DEFLAYER_MAPPED} must be a string within one pair of parentheses"))?.to_owned()
            }
        };
        if layer_indexes.get(&layer_name).is_some() {
            bail_expr!(layer_expr, "duplicate layer name: {}", layer_name);
        }
        // Check if user tried to use parentheses directly - `(` and `)`
        // or escaped them like in kmonad - `\(` and `\)`.
        for subexpr in subexprs {
            if let Some(list) = subexpr.list(None) {
                if list.is_empty() {
                    bail_expr!(
                        subexpr,
                        "You can't put parentheses in deflayer directly, because they are special characters for delimiting lists.\n\
                         To get `(` and `)` in US layout, you should use `S-9` and `S-0` respectively.\n\
                         For more context, see: https://github.com/jtroo/kanata/issues/459"
                    )
                }
                if list.len() == 1
                    && list
                        .first()
                        .is_some_and(|s| s.atom(None).is_some_and(|atom| atom == "\\"))
                {
                    bail_expr!(
                        subexpr,
                        "Escaping shifted characters with `\\` is currently not supported in kanata.\n\
                         To get `(` and `)` in US layout, you should use `S-9` and `S-0` respectively.\n\
                         For more context, see: https://github.com/jtroo/kanata/issues/163"
                    )
                }
            }
        }
        if do_element_count_check {
            let num_actions = expr.t.len() - 2;
            if num_actions != expected_len {
                bail_span!(
                    expr,
                    "Layer {} has {} item(s), but requires {} to match defsrc",
                    layer_name,
                    num_actions,
                    expected_len
                )
            }
        }
        layer_indexes.insert(layer_name, i);
    }

    Ok(layer_indexes)
}

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

#[derive(Debug)]
pub struct ParserState {
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
    switch_max_key_timing: Cell<u16>,
    trans_forbidden_reason: Option<&'static str>,
    pub lsp_hint_inactive_code: Vec<LspHintInactiveCode>,
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
            switch_max_key_timing: Cell::new(0),
            trans_forbidden_reason: None,
            lsp_hint_inactive_code: Default::default(),
            a: unsafe { Allocations::new() },
        }
    }
}

#[derive(Debug, Clone)]
struct ChordGroup {
    id: u16,
    name: String,
    keys: Vec<String>,
    coords: Vec<((u8, u16), ChordKeys)>,
    chords: HashMap<u128, SExpr>,
    timeout: u16,
}

fn parse_vars(exprs: &[&Vec<SExpr>], s: &mut ParserState) -> Result<()> {
    for expr in exprs {
        let mut subexprs = check_first_expr(expr.iter(), "defvar")?;
        // Read k-v pairs from the configuration
        while let Some(var_name_expr) = subexprs.next() {
            let var_name = match var_name_expr {
                SExpr::Atom(a) => &a.t,
                _ => bail_expr!(var_name_expr, "variable name must not be a list"),
            };
            let var_expr = match subexprs.next() {
                Some(v) => match v {
                    SExpr::Atom(_) => v.clone(),
                    SExpr::List(l) => parse_list_var(l, &s.vars),
                },
                None => bail_expr!(var_name_expr, "variable name must have a subsequent value"),
            };
            if s.vars.insert(var_name.into(), var_expr).is_some() {
                bail_expr!(var_name_expr, "duplicate variable name: {}", var_name);
            }
        }
    }
    Ok(())
}

fn parse_list_var(expr: &Spanned<Vec<SExpr>>, vars: &HashMap<String, SExpr>) -> SExpr {
    let ret = match expr.t.first() {
        Some(SExpr::Atom(a)) => match a.t.as_str() {
            "concat" => {
                let mut concat_str = String::new();
                let visitees = &expr.t[1..];
                push_all_atoms(visitees, vars, &mut concat_str);
                SExpr::Atom(Spanned {
                    span: expr.span.clone(),
                    t: concat_str,
                })
            }
            _ => SExpr::List(expr.clone()),
        },
        _ => SExpr::List(expr.clone()),
    };
    ret
}

fn push_all_atoms(exprs: &[SExpr], vars: &HashMap<String, SExpr>, pusheen: &mut String) {
    for expr in exprs {
        if let Some(a) = expr.atom(Some(vars)) {
            pusheen.push_str(a.trim_matches('"'));
        } else if let Some(l) = expr.list(Some(vars)) {
            push_all_atoms(l, vars, pusheen);
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
                        s.lsp_hint_inactive_code.push(LspHintInactiveCode {
                            span: exprs.span.clone(),
                            reason: msg.clone(),
                        });
                        log::info!("{msg}, skipping associated aliases");
                        return Ok(());
                    } else if !values_of_matching_vars.iter().any(|&v| v == env_var_value) {
                        let msg =
                            format!("Env var '{env_var_name}' is set, but value doesn't match");
                        s.lsp_hint_inactive_code.push(LspHintInactiveCode {
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
    Ok(a.sref(Action::Custom(a.sref(a.sref_slice(ca)))))
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
            if let Some(trans_forbidden_reason) = s.trans_forbidden_reason {
                bail_span!(ac_span, "{trans_forbidden_reason}");
            } else {
                return Ok(s.a.sref(Action::Trans));
            }
        }
        "XX" | "✗" | "∅" | "•" => return Ok(s.a.sref(Action::NoOp)),
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
            )
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
            )
        }
        "mwd" | "mousewheeldown" => {
            return custom(
                CustomAction::MWheelNotch {
                    direction: MWheelDirection::Down,
                },
                &s.a,
            )
        }
        "mwl" | "mousewheelleft" => {
            return custom(
                CustomAction::MWheelNotch {
                    direction: MWheelDirection::Left,
                },
                &s.a,
            )
        }
        "mwr" | "mousewheelright" => {
            return custom(
                CustomAction::MWheelNotch {
                    direction: MWheelDirection::Right,
                },
                &s.a,
            )
        }
        "rpt" | "repeat" | "rpt-key" => return custom(CustomAction::Repeat, &s.a),
        "rpt-any" => return Ok(s.a.sref(Action::Repeat)),
        "dynamic-macro-record-stop" => {
            return custom(CustomAction::DynamicMacroRecordStop(0), &s.a)
        }
        _ => {}
    };
    if let Some(oscode) = str_to_oscode(ac) {
        return Ok(s.a.sref(k(oscode.into())));
    }
    if let Some(alias) = ac.strip_prefix('@') {
        return match s.aliases.get(alias) {
            Some(ac) => Ok(*ac),
            None => bail!(
                "Referenced unknown alias {}. Note that order of declarations matter.",
                alias
            ),
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
        TAP_HOLD_PRESS => parse_tap_hold(&ac[1..], s, HoldTapConfig::HoldOnOtherKeyPress),
        TAP_HOLD_RELEASE => parse_tap_hold(&ac[1..], s, HoldTapConfig::PermissiveHold),
        TAP_HOLD_PRESS_TIMEOUT => {
            parse_tap_hold_timeout(&ac[1..], s, HoldTapConfig::HoldOnOtherKeyPress)
        }
        TAP_HOLD_RELEASE_TIMEOUT => {
            parse_tap_hold_timeout(&ac[1..], s, HoldTapConfig::PermissiveHold)
        }
        TAP_HOLD_RELEASE_KEYS => {
            parse_tap_hold_keys(&ac[1..], s, "release", custom_tap_hold_release)
        }
        TAP_HOLD_EXCEPT_KEYS => parse_tap_hold_keys(&ac[1..], s, "except", custom_tap_hold_except),
        MULTI => parse_multi(&ac[1..], s),
        MACRO => parse_macro(&ac[1..], s, RepeatMacro::No),
        MACRO_REPEAT => parse_macro(&ac[1..], s, RepeatMacro::Yes),
        MACRO_RELEASE_CANCEL => parse_macro_release_cancel(&ac[1..], s, RepeatMacro::No),
        MACRO_REPEAT_RELEASE_CANCEL => parse_macro_release_cancel(&ac[1..], s, RepeatMacro::Yes),
        UNICODE => parse_unicode(&ac[1..], s),
        SYM => parse_unicode(&ac[1..], s),
        ONE_SHOT | ONE_SHOT_PRESS => parse_one_shot(&ac[1..], s, OneShotEndConfig::EndOnFirstPress),
        ONE_SHOT_RELEASE => parse_one_shot(&ac[1..], s, OneShotEndConfig::EndOnFirstRelease),
        ONE_SHOT_PRESS_PCANCEL => {
            parse_one_shot(&ac[1..], s, OneShotEndConfig::EndOnFirstPressOrRepress)
        }
        ONE_SHOT_RELEASE_PCANCEL => {
            parse_one_shot(&ac[1..], s, OneShotEndConfig::EndOnFirstReleaseOrRepress)
        }
        TAP_DANCE => parse_tap_dance(&ac[1..], s, TapDanceConfig::Lazy),
        TAP_DANCE_EAGER => parse_tap_dance(&ac[1..], s, TapDanceConfig::Eager),
        CHORD => parse_chord(&ac[1..], s),
        RELEASE_KEY => parse_release_key(&ac[1..], s),
        RELEASE_LAYER => parse_release_layer(&ac[1..], s),
        ON_PRESS_FAKEKEY => parse_on_press_fake_key_op(&ac[1..], s),
        ON_RELEASE_FAKEKEY => parse_on_release_fake_key_op(&ac[1..], s),
        ON_PRESS_FAKEKEY_DELAY => parse_fake_key_delay(&ac[1..], s),
        ON_RELEASE_FAKEKEY_DELAY => parse_on_release_fake_key_delay(&ac[1..], s),
        ON_IDLE_FAKEKEY => parse_on_idle_fakekey(&ac[1..], s),
        ON_PRESS => parse_on_press(&ac[1..], s),
        ON_RELEASE => parse_on_release(&ac[1..], s),
        ON_IDLE => parse_on_idle(&ac[1..], s),
        MWHEEL_UP => parse_mwheel(&ac[1..], MWheelDirection::Up, s),
        MWHEEL_DOWN => parse_mwheel(&ac[1..], MWheelDirection::Down, s),
        MWHEEL_LEFT => parse_mwheel(&ac[1..], MWheelDirection::Left, s),
        MWHEEL_RIGHT => parse_mwheel(&ac[1..], MWheelDirection::Right, s),
        MOVEMOUSE_UP => parse_move_mouse(&ac[1..], MoveDirection::Up, s),
        MOVEMOUSE_DOWN => parse_move_mouse(&ac[1..], MoveDirection::Down, s),
        MOVEMOUSE_LEFT => parse_move_mouse(&ac[1..], MoveDirection::Left, s),
        MOVEMOUSE_RIGHT => parse_move_mouse(&ac[1..], MoveDirection::Right, s),
        MOVEMOUSE_ACCEL_UP => parse_move_mouse_accel(&ac[1..], MoveDirection::Up, s),
        MOVEMOUSE_ACCEL_DOWN => parse_move_mouse_accel(&ac[1..], MoveDirection::Down, s),
        MOVEMOUSE_ACCEL_LEFT => parse_move_mouse_accel(&ac[1..], MoveDirection::Left, s),
        MOVEMOUSE_ACCEL_RIGHT => parse_move_mouse_accel(&ac[1..], MoveDirection::Right, s),
        MOVEMOUSE_SPEED => parse_move_mouse_speed(&ac[1..], s),
        SETMOUSE => parse_set_mouse(&ac[1..], s),
        DYNAMIC_MACRO_RECORD => parse_dynamic_macro_record(&ac[1..], s),
        DYNAMIC_MACRO_PLAY => parse_dynamic_macro_play(&ac[1..], s),
        ARBITRARY_CODE => parse_arbitrary_code(&ac[1..], s),
        CMD => parse_cmd(&ac[1..], s, CmdType::Standard),
        CMD_OUTPUT_KEYS => parse_cmd(&ac[1..], s, CmdType::OutputKeys),
        PUSH_MESSAGE => parse_push_message(&ac[1..], s),
        FORK => parse_fork(&ac[1..], s),
        CAPS_WORD => parse_caps_word(&ac[1..], s),
        CAPS_WORD_CUSTOM => parse_caps_word_custom(&ac[1..], s),
        DYNAMIC_MACRO_RECORD_STOP_TRUNCATE => parse_macro_record_stop_truncate(&ac[1..], s),
        SWITCH => parse_switch(&ac[1..], s),
        SEQUENCE => parse_sequence_start(&ac[1..], s),
        UNMOD => parse_unmod(UNMOD, &ac[1..], s),
        UNSHIFT => parse_unmod(UNSHIFT, &ac[1..], s),
        LIVE_RELOAD_NUM => parse_live_reload_num(&ac[1..], s),
        LIVE_RELOAD_FILE => parse_live_reload_file(&ac[1..], s),
        _ => unreachable!(),
    }
}

fn parse_layer_base(ac_params: &[SExpr], s: &ParserState) -> Result<&'static KanataAction> {
    Ok(s.a.sref(Action::DefaultLayer(layer_idx(ac_params, &s.layer_idxs)?)))
}

fn parse_layer_toggle(ac_params: &[SExpr], s: &ParserState) -> Result<&'static KanataAction> {
    Ok(s.a.sref(Action::Layer(layer_idx(ac_params, &s.layer_idxs)?)))
}

fn layer_idx(ac_params: &[SExpr], layers: &LayerIndexes) -> Result<usize> {
    if ac_params.len() != 1 {
        bail!(
            "Layer actions expect one item: the layer name, found {} items",
            ac_params.len()
        )
    }
    let layer_name = match &ac_params[0] {
        SExpr::Atom(ln) => &ln.t,
        _ => bail_expr!(&ac_params[0], "layer name should be a string not a list",),
    };
    match layers.get(layer_name) {
        Some(i) => Ok(*i),
        None => err_expr!(
            &ac_params[0],
            "layer name is not declared in any deflayer: {layer_name}"
        ),
    }
}

fn parse_tap_hold(
    ac_params: &[SExpr],
    s: &ParserState,
    config: HoldTapConfig<'static>,
) -> Result<&'static KanataAction> {
    if ac_params.len() != 4 {
        bail!(
            r"tap-hold expects 4 items after it, got {}.
Params in order:
<tap-timeout> <hold-timeout> <tap-action> <hold-action>",
            ac_params.len(),
        )
    }
    let tap_timeout = parse_u16(&ac_params[0], s, "tap timeout")?;
    let hold_timeout = parse_non_zero_u16(&ac_params[1], s, "hold timeout")?;
    let tap_action = parse_action(&ac_params[2], s)?;
    let hold_action = parse_action(&ac_params[3], s)?;
    if matches!(tap_action, Action::HoldTap { .. }) {
        bail!("tap-hold does not work in the tap-action of tap-hold")
    }
    Ok(s.a.sref(Action::HoldTap(s.a.sref(HoldTapAction {
        config,
        tap_hold_interval: tap_timeout,
        timeout: hold_timeout,
        tap: *tap_action,
        hold: *hold_action,
        timeout_action: *hold_action,
    }))))
}

fn parse_tap_hold_timeout(
    ac_params: &[SExpr],
    s: &ParserState,
    config: HoldTapConfig<'static>,
) -> Result<&'static KanataAction> {
    if ac_params.len() != 5 {
        bail!(
            r"tap-hold-(press|release)-timeout expects 5 items after it, got {}.
Params in order:
<tap-timeout> <hold-timeout> <tap-action> <hold-action> <timeout-action>",
            ac_params.len(),
        )
    }
    let tap_timeout = parse_u16(&ac_params[0], s, "tap timeout")?;
    let hold_timeout = parse_non_zero_u16(&ac_params[1], s, "hold timeout")?;
    let tap_action = parse_action(&ac_params[2], s)?;
    let hold_action = parse_action(&ac_params[3], s)?;
    let timeout_action = parse_action(&ac_params[4], s)?;
    if matches!(tap_action, Action::HoldTap { .. }) {
        bail!("tap-hold does not work in the tap-action of tap-hold")
    }
    Ok(s.a.sref(Action::HoldTap(s.a.sref(HoldTapAction {
        config,
        tap_hold_interval: tap_timeout,
        timeout: hold_timeout,
        tap: *tap_action,
        hold: *hold_action,
        timeout_action: *timeout_action,
    }))))
}

fn parse_tap_hold_keys(
    ac_params: &[SExpr],
    s: &ParserState,
    custom_name: &str,
    custom_func: TapHoldCustomFunc,
) -> Result<&'static KanataAction> {
    if ac_params.len() != 5 {
        bail!(
            r"tap-hold-{}-keys expects 5 items after it, got {}.
Params in order:
<tap-timeout> <hold-timeout> <tap-action> <hold-action> <tap-trigger-keys>",
            custom_name,
            ac_params.len(),
        )
    }
    let tap_timeout = parse_u16(&ac_params[0], s, "tap timeout")?;
    let hold_timeout = parse_non_zero_u16(&ac_params[1], s, "hold timeout")?;
    let tap_action = parse_action(&ac_params[2], s)?;
    let hold_action = parse_action(&ac_params[3], s)?;
    let tap_trigger_keys = parse_key_list(&ac_params[4], s, "tap-trigger-keys")?;
    if matches!(tap_action, Action::HoldTap { .. }) {
        bail!("tap-hold does not work in the tap-action of tap-hold")
    }
    Ok(s.a.sref(Action::HoldTap(s.a.sref(HoldTapAction {
        config: HoldTapConfig::Custom(custom_func(&tap_trigger_keys, &s.a)),
        tap_hold_interval: tap_timeout,
        timeout: hold_timeout,
        tap: *tap_action,
        hold: *hold_action,
        timeout_action: *hold_action,
    }))))
}

fn parse_u8_with_range(expr: &SExpr, s: &ParserState, label: &str, min: u8, max: u8) -> Result<u8> {
    expr.atom(s.vars())
        .map(str::parse::<u8>)
        .and_then(|u| u.ok())
        .and_then(|u| {
            assert!(min <= max);
            if u >= min && u <= max {
                Some(u)
            } else {
                None
            }
        })
        .ok_or_else(|| anyhow_expr!(expr, "{label} must be {min}-{max}"))
}

fn parse_u16(expr: &SExpr, s: &ParserState, label: &str) -> Result<u16> {
    expr.atom(s.vars())
        .map(str::parse::<u16>)
        .and_then(|u| u.ok())
        .ok_or_else(|| anyhow_expr!(expr, "{label} must be 0-65535"))
}

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

fn parse_multi(ac_params: &[SExpr], s: &ParserState) -> Result<&'static KanataAction> {
    if ac_params.is_empty() {
        bail!("multi expects at least one item after it")
    }
    let mut actions = Vec::new();
    let mut custom_actions: Vec<&'static CustomAction> = Vec::new();
    for expr in ac_params {
        let ac = parse_action(expr, s)?;
        match ac {
            Action::Custom(acs) => {
                for ac in acs.iter() {
                    custom_actions.push(ac);
                }
            }
            // Flatten multi actions
            Action::MultipleActions(acs) => {
                for ac in acs.iter() {
                    match ac {
                        Action::Custom(acs) => {
                            for ac in acs.iter() {
                                custom_actions.push(ac);
                            }
                        }
                        _ => actions.push(*ac),
                    }
                }
            }
            _ => actions.push(*ac),
        }
    }

    if !custom_actions.is_empty() {
        actions.push(Action::Custom(s.a.sref(s.a.sref_vec(custom_actions))));
    }

    if actions
        .iter()
        .filter(|ac| {
            matches!(
                ac,
                Action::TapDance(TapDance {
                    config: TapDanceConfig::Lazy,
                    ..
                }) | Action::HoldTap { .. }
                    | Action::Chords { .. }
            )
        })
        .count()
        > 1
    {
        bail!("Cannot combine multiple tap-hold/tap-dance/chord");
    }

    Ok(s.a.sref(Action::MultipleActions(s.a.sref(s.a.sref_vec(actions)))))
}

const MACRO_ERR: &str = "Action macro only accepts delays, keys, chords, chorded sub-macros, and a subset of special actions.\nThe macro section of the documentation describes this in more detail:\nhttps://github.com/jtroo/kanata/blob/main/docs/config.adoc#macro";
enum RepeatMacro {
    Yes,
    No,
}

fn parse_macro(
    ac_params: &[SExpr],
    s: &ParserState,
    repeat: RepeatMacro,
) -> Result<&'static KanataAction> {
    if ac_params.is_empty() {
        bail!("macro expects at least one item after it")
    }
    let mut all_events = Vec::with_capacity(256);
    let mut params_remainder = ac_params;
    while !params_remainder.is_empty() {
        let mut events;
        (events, params_remainder) = parse_macro_item(params_remainder, s)?;
        all_events.append(&mut events);
    }
    all_events.push(SequenceEvent::Complete);
    all_events.shrink_to_fit();
    match repeat {
        RepeatMacro::No => Ok(s.a.sref(Action::Sequence {
            events: s.a.sref(s.a.sref(s.a.sref_vec(all_events))),
        })),
        RepeatMacro::Yes => Ok(s.a.sref(Action::RepeatableSequence {
            events: s.a.sref(s.a.sref(s.a.sref_vec(all_events))),
        })),
    }
}

fn parse_macro_release_cancel(
    ac_params: &[SExpr],
    s: &ParserState,
    repeat: RepeatMacro,
) -> Result<&'static KanataAction> {
    let macro_action = parse_macro(ac_params, s, repeat)?;
    Ok(s.a.sref(Action::MultipleActions(s.a.sref(s.a.sref_vec(vec![
        *macro_action,
        Action::Custom(s.a.sref(s.a.sref_slice(CustomAction::CancelMacroOnRelease))),
    ])))))
}

#[derive(PartialEq)]
enum MacroNumberParseMode {
    Delay,
    Action,
}

#[allow(clippy::type_complexity)] // return type is not pub
fn parse_macro_item<'a>(
    acs: &'a [SExpr],
    s: &ParserState,
) -> Result<(
    Vec<SequenceEvent<'static, &'static &'static [&'static CustomAction]>>,
    &'a [SExpr],
)> {
    parse_macro_item_impl(acs, s, MacroNumberParseMode::Delay)
}

#[allow(clippy::type_complexity)] // return type is not pub
fn parse_macro_item_impl<'a>(
    acs: &'a [SExpr],
    s: &ParserState,
    num_parse_mode: MacroNumberParseMode,
) -> Result<(
    Vec<SequenceEvent<'static, &'static &'static [&'static CustomAction]>>,
    &'a [SExpr],
)> {
    if num_parse_mode == MacroNumberParseMode::Delay {
        if let Some(a) = acs[0].atom(s.vars()) {
            match parse_non_zero_u16(&acs[0], s, "delay") {
                Ok(duration) => {
                    let duration = u32::from(duration);
                    return Ok((vec![SequenceEvent::Delay { duration }], &acs[1..]));
                }
                Err(e) => {
                    if a.chars().all(|c| c.is_ascii_digit()) {
                        return Err(e);
                    }
                }
            }
        }
    }
    match parse_action(&acs[0], s) {
        Ok(Action::KeyCode(kc)) => {
            // Should note that I tried `SequenceEvent::Tap` initially but it seems to be buggy
            // so I changed the code to use individual press and release. The SequenceEvent
            // code is from a PR that (at the time of this writing) hasn't yet been merged into
            // keyberon master and doesn't have tests written for it yet. This seems to work as
            // expected right now though.
            Ok((
                vec![SequenceEvent::Press(*kc), SequenceEvent::Release(*kc)],
                &acs[1..],
            ))
        }
        Ok(Action::MultipleKeyCodes(kcs)) => {
            // chord - press in order then release in the reverse order
            let mut events = vec![];
            for kc in kcs.iter() {
                events.push(SequenceEvent::Press(*kc));
            }
            for kc in kcs.iter().rev() {
                events.push(SequenceEvent::Release(*kc));
            }
            Ok((events, &acs[1..]))
        }
        Ok(Action::Custom(custom)) => Ok((vec![SequenceEvent::Custom(custom)], &acs[1..])),
        _ => {
            if let Some(submacro) = acs[0].list(s.vars()) {
                // If it's just a list that's not parsable as a usable action, try parsing the
                // content.
                let mut submacro_remainder = submacro;
                let mut all_events = vec![];
                while !submacro_remainder.is_empty() {
                    let mut events;
                    (events, submacro_remainder) = parse_macro_item(submacro_remainder, s)?;
                    all_events.append(&mut events);
                }
                return Ok((all_events, &acs[1..]));
            }

            let (held_mods, unparsed_str) = parse_mods_held_for_submacro(&acs[0], s)?;
            let mut all_events = vec![];

            // First, press all of the modifiers
            for kc in held_mods.iter().copied() {
                all_events.push(SequenceEvent::Press(kc));
            }

            let mut rem_start = 1;
            let maybe_list_var = SExpr::Atom(Spanned::new(unparsed_str.into(), acs[0].span()));
            let submacro = match maybe_list_var.list(s.vars()) {
                Some(l) => l,
                None => {
                    // Ensure that the unparsed text is empty since otherwise it means there is
                    // invalid text there
                    if !unparsed_str.is_empty() {
                        bail_expr!(&acs[0], "{MACRO_ERR}")
                    }
                    // Check for a follow-up list
                    rem_start = 2;
                    if acs.len() < 2 {
                        bail_expr!(&acs[0], "{MACRO_ERR}")
                    }
                    acs[1]
                        .list(s.vars())
                        .ok_or_else(|| anyhow_expr!(&acs[1], "{MACRO_ERR}"))?
                }
            };
            let mut submacro_remainder = submacro;
            let mut events;
            while !submacro_remainder.is_empty() {
                (events, submacro_remainder) = parse_macro_item(submacro_remainder, s)?;
                all_events.append(&mut events);
            }

            // Lastly, release modifiers
            for kc in held_mods.iter().copied() {
                all_events.push(SequenceEvent::Release(kc));
            }

            Ok((all_events, &acs[rem_start..]))
        }
    }
}

/// Parses mod keys like `C-S-`. Returns the `KeyCode`s for the modifiers parsed and the unparsed
/// text after any parsed modifier prefixes.
fn parse_mods_held_for_submacro<'a>(
    held_mods: &'a SExpr,
    s: &'a ParserState,
) -> Result<(Vec<KeyCode>, &'a str)> {
    let mods = held_mods
        .atom(s.vars())
        .ok_or_else(|| anyhow_expr!(held_mods, "{MACRO_ERR}"))?;
    let (mod_keys, unparsed_str) = parse_mod_prefix(mods)?;
    if mod_keys.is_empty() {
        bail_expr!(held_mods, "{MACRO_ERR}");
    }
    Ok((mod_keys, unparsed_str))
}

static KEYMODI: [(&str, KeyCode); 31] = [
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
    ("⎈", KeyCode::LCtrl), // Shorter indicators should be at the end to only get matched after indicators with sides have had a chance
    ("⌥", KeyCode::LAlt),
    ("⎇", KeyCode::LAlt),
    ("◆", KeyCode::LGui),
    ("⌘", KeyCode::LGui),
    ("❖", KeyCode::LGui),
];

/// Parses mod keys like `C-S-`. Returns the `KeyCode`s for the modifiers parsed and the unparsed
/// text after any parsed modifier prefixes.
pub fn parse_mod_prefix(mods: &str) -> Result<(Vec<KeyCode>, &str)> {
    let mut key_stack = Vec::new();
    let mut rem = mods;
    loop {
        let mut found_none = true;
        for (key_s, key_code) in &KEYMODI {
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

fn parse_unicode(ac_params: &[SExpr], s: &ParserState) -> Result<&'static KanataAction> {
    const ERR_STR: &str = "unicode expects exactly one (not combos looking like one) unicode character as an argument";
    if ac_params.len() != 1 {
        bail!(ERR_STR)
    }
    ac_params[0]
        .atom(s.vars())
        .map(|a| {
            if a.chars().count() != 1 {
                bail_expr!(&ac_params[0], "{ERR_STR}")
            }
            Ok(s.a.sref(Action::Custom(s.a.sref(s.a.sref_slice(
                CustomAction::Unicode(a.chars().next().expect("1 char")),
            )))))
        })
        .ok_or_else(|| anyhow_expr!(&ac_params[0], "{ERR_STR}"))?
}

enum CmdType {
    Standard,
    OutputKeys,
}

fn parse_cmd(
    ac_params: &[SExpr],
    s: &ParserState,
    cmd_type: CmdType,
) -> Result<&'static KanataAction> {
    const ERR_STR: &str = "cmd expects at least one string";
    if !s.is_cmd_enabled {
        bail!("cmd is not enabled, but cmd is in the configuration");
    }
    let mut cmd = vec![];
    collect_strings(ac_params, &mut cmd, s);
    if cmd.is_empty() {
        bail!(ERR_STR);
    }
    Ok(s.a
        .sref(Action::Custom(s.a.sref(s.a.sref_slice(match cmd_type {
            CmdType::Standard => CustomAction::Cmd(cmd),
            CmdType::OutputKeys => CustomAction::CmdOutputKeys(cmd),
        })))))
}

/// Recurse through all levels of list nesting and collect into a flat list of strings.
/// Recursion is DFS, which matches left-to-right reading of the strings as they appear,
/// if everything was on a single line.
fn collect_strings(params: &[SExpr], strings: &mut Vec<String>, s: &ParserState) {
    for param in params {
        if let Some(a) = param.atom(s.vars()) {
            strings.push(a.trim_matches('"').to_owned());
        } else {
            // unwrap: this must be a list, since it's not an atom.
            let l = param.list(s.vars()).unwrap();
            collect_strings(l, strings, s);
        }
    }
}

#[test]
fn test_collect_strings() {
    let params = r#"(gah (squish "squash" (splish splosh) "bah mah") dah)"#;
    let params = sexpr::parse(params, "noexist").unwrap();
    let mut strings = vec![];
    collect_strings(&params[0].t, &mut strings, &ParserState::default());
    assert_eq!(
        &strings,
        &["gah", "squish", "squash", "splish", "splosh", "bah mah", "dah"]
    );
}

fn parse_push_message(ac_params: &[SExpr], s: &ParserState) -> Result<&'static KanataAction> {
    if ac_params.is_empty() {
        bail!(
             "{PUSH_MESSAGE} expects at least one item, an item can be a list or an atom, found 0, none"
        );
    }
    let message = to_simple_expr(ac_params, s);
    custom(CustomAction::PushMessage(message), &s.a)
}

fn to_simple_expr(params: &[SExpr], s: &ParserState) -> Vec<SimpleSExpr> {
    let mut result: Vec<SimpleSExpr> = Vec::new();
    for param in params {
        if let Some(a) = param.atom(s.vars()) {
            result.push(SimpleSExpr::Atom(a.trim_matches('"').to_owned()));
        } else {
            // unwrap: this must be a list, since it's not an atom.
            let sexps = param.list(s.vars()).unwrap();
            let value = to_simple_expr(sexps, s);
            let list = SimpleSExpr::List(value);
            result.push(list);
        }
    }
    result
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SimpleSExpr {
    Atom(String),
    List(Vec<SimpleSExpr>),
}

fn parse_one_shot(
    ac_params: &[SExpr],
    s: &ParserState,
    end_config: OneShotEndConfig,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "one-shot expects a timeout followed by a key or action";
    if ac_params.len() != 2 {
        bail!(ERR_MSG);
    }

    let timeout = parse_non_zero_u16(&ac_params[0], s, "timeout")?;
    let action = parse_action(&ac_params[1], s)?;
    if !matches!(
        action,
        Action::Layer(..) | Action::KeyCode(..) | Action::MultipleKeyCodes(..)
    ) {
        bail!("one-shot is only allowed to contain layer-while-held, a keycode, or a chord");
    }

    Ok(s.a.sref(Action::OneShot(s.a.sref(OneShot {
        timeout,
        action,
        end_config,
    }))))
}

fn parse_tap_dance(
    ac_params: &[SExpr],
    s: &ParserState,
    config: TapDanceConfig,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "tap-dance expects a timeout (number) followed by a list of actions";
    if ac_params.len() != 2 {
        bail!(ERR_MSG);
    }

    let timeout = parse_non_zero_u16(&ac_params[0], s, "timeout")?;
    let actions = ac_params[1]
        .list(s.vars())
        .map(|tap_dance_actions| -> Result<Vec<&'static KanataAction>> {
            let mut actions = Vec::new();
            for expr in tap_dance_actions {
                let ac = parse_action(expr, s)?;
                actions.push(ac);
            }
            Ok(actions)
        })
        .ok_or_else(|| anyhow_expr!(&ac_params[1], "{ERR_MSG}: expected a list"))??;

    Ok(s.a.sref(Action::TapDance(s.a.sref(TapDance {
        timeout,
        actions: s.a.sref_vec(actions),
        config,
    }))))
}

fn parse_chord(ac_params: &[SExpr], s: &ParserState) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "Action chord expects a chords group name followed by an identifier";
    if ac_params.len() != 2 {
        bail!(ERR_MSG);
    }

    let name = ac_params[0]
        .atom(s.vars())
        .ok_or_else(|| anyhow_expr!(&ac_params[0], "{ERR_MSG}"))?;
    let group = match s.chord_groups.get(name) {
        Some(t) => t,
        None => bail_expr!(&ac_params[0], "Referenced unknown chord group: {}.", name),
    };
    let chord_key_index = ac_params[1]
        .atom(s.vars())
        .map(|s| match group.keys.iter().position(|e| e == s) {
            Some(i) => Ok(i),
            None => err_expr!(
                &ac_params[1],
                r#"Identifier "{}" is not used in chord group "{}"."#,
                &s,
                name,
            ),
        })
        .ok_or_else(|| anyhow_expr!(&ac_params[0], "{ERR_MSG}"))??;
    let chord_keys: u128 = 1 << chord_key_index;

    // We don't yet know at this point what the entire chords group will look like nor at which
    // coords this action will end up. So instead we store a dummy action which will be properly
    // resolved in `resolve_chord_groups`.
    Ok(s.a.sref(Action::Chords(s.a.sref(ChordsGroup {
        timeout: group.timeout,
        coords: s.a.sref_vec(vec![((0, group.id), chord_keys)]),
        chords: s.a.sref_vec(vec![]),
    }))))
}

fn parse_release_key(ac_params: &[SExpr], s: &ParserState) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "release-key expects exactly one keycode (e.g. lalt)";
    if ac_params.len() != 1 {
        bail!("{ERR_MSG}: found {} items", ac_params.len());
    }
    let ac = parse_action(&ac_params[0], s)?;
    match ac {
        Action::KeyCode(kc) => {
            Ok(s.a.sref(Action::ReleaseState(ReleasableState::KeyCode(*kc))))
        }
        _ => err_expr!(&ac_params[0], "{}", ERR_MSG),
    }
}

fn parse_release_layer(ac_params: &[SExpr], s: &ParserState) -> Result<&'static KanataAction> {
    Ok(s.a
        .sref(Action::ReleaseState(ReleasableState::Layer(layer_idx(
            ac_params,
            &s.layer_idxs,
        )?))))
}

fn create_defsrc_layer() -> [KanataAction; KEYS_IN_ROW] {
    let mut layer = [KanataAction::NoOp; KEYS_IN_ROW];

    for (i, ac) in layer.iter_mut().enumerate() {
        *ac = OsCode::from_u16(i as u16)
            .map(|osc| Action::KeyCode(osc.into()))
            .unwrap_or(Action::NoOp);
    }
    layer
}

fn parse_chord_groups(exprs: &[&Spanned<Vec<SExpr>>], s: &mut ParserState) -> Result<()> {
    const MSG: &str = "Incorrect number of elements found in defchords.\nThere should be the group name, followed by timeout, followed by keys-action pairs";
    for expr in exprs {
        let mut subexprs = check_first_expr(expr.t.iter(), "defchords")?;
        let name = subexprs
            .next()
            .and_then(|e| e.atom(s.vars()))
            .ok_or_else(|| anyhow_span!(expr, "{MSG}"))?
            .to_owned();
        let timeout = match subexprs.next() {
            Some(e) => parse_non_zero_u16(e, s, "timeout")?,
            None => bail_span!(expr, "{MSG}"),
        };
        let id = match s.chord_groups.len().try_into() {
            Ok(id) => id,
            Err(_) => bail_span!(expr, "Maximum number of chord groups exceeded."),
        };
        let mut group = ChordGroup {
            id,
            name: name.clone(),
            keys: Vec::new(),
            coords: Vec::new(),
            chords: HashMap::default(),
            timeout,
        };
        // Read k-v pairs from the configuration
        while let Some(keys_expr) = subexprs.next() {
            let action = match subexprs.next() {
                Some(v) => v,
                None => bail_expr!(
                    keys_expr,
                    "Key list found without action - add an action for this chord"
                ),
            };
            let mut keys = keys_expr
                .list(s.vars())
                .map(|keys| {
                    keys.iter().map(|key| {
                        key.atom(s.vars()).ok_or_else(|| {
                            anyhow_expr!(
                                key,
                                "Chord keys cannot be lists. Invalid key name: {:?}",
                                key
                            )
                        })
                    })
                })
                .ok_or_else(|| anyhow_expr!(keys_expr, "Chord must be a list/set of keys"))?;
            let mask: u128 = keys.try_fold(0, |mask, key| {
                let key = key?;
                let index = match group.keys.iter().position(|k| k == key) {
                    Some(i) => i,
                    None => {
                        let i = group.keys.len();
                        if i + 1 > MAX_CHORD_KEYS {
                            bail_expr!(keys_expr, "Maximum number of keys in a chords group ({MAX_CHORD_KEYS}) exceeded - found {}", i + 1);
                        }
                        group.keys.push(key.to_owned());
                        i
                    }
                };
                Ok(mask | (1 << index))
            })?;
            if group.chords.insert(mask, action.clone()).is_some() {
                bail_expr!(keys_expr, "Duplicate chord in group {name}");
            }
        }
        if s.chord_groups.insert(name.to_owned(), group).is_some() {
            bail_span!(expr, "Duplicate chords group: {}", name);
        }
    }
    Ok(())
}

fn resolve_chord_groups(layers: &mut IntermediateLayers, s: &ParserState) -> Result<()> {
    let mut chord_groups = s.chord_groups.values().cloned().collect::<Vec<_>>();
    chord_groups.sort_by_key(|group| group.id);

    for layer in layers.iter() {
        for (i, row) in layer.iter().enumerate() {
            for (j, cell) in row.iter().enumerate() {
                find_chords_coords(&mut chord_groups, (i as u8, j as u16), cell);
            }
        }
    }

    let chord_groups = chord_groups.into_iter().map(|group| {
        // Check that all keys in the chord group have been assigned to some coordinate
        for (key_index, key) in group.keys.iter().enumerate() {
            let key_mask = 1 << key_index;
            if !group.coords.iter().any(|(_, keys)| keys & key_mask != 0) {
                bail!("coord group `{0}` defines unused key `{1}`, did you forget to bind `(chord {0} {1})`?", group.name, key)
            }
        }

        let chords = group.chords.iter().map(|(mask, action)| {
            Ok((*mask, parse_action(action, s)?))
        }).collect::<Result<Vec<_>>>()?;

        Ok(s.a.sref(ChordsGroup {
            coords: s.a.sref_vec(group.coords),
            chords: s.a.sref_vec(chords),
            timeout: group.timeout,
        }))
    }).collect::<Result<Vec<_>>>()?;

    for layer in layers.iter_mut() {
        for row in layer.iter_mut() {
            for cell in row.iter_mut() {
                if let Some(action) = fill_chords(&chord_groups, cell, s) {
                    *cell = action;
                }
            }
        }
    }

    Ok(())
}

fn find_chords_coords(chord_groups: &mut [ChordGroup], coord: (u8, u16), action: &KanataAction) {
    match action {
        Action::Chords(ChordsGroup { coords, .. }) => {
            for ((_, group_id), chord_keys) in coords.iter() {
                let group = &mut chord_groups[*group_id as usize];
                group.coords.push((coord, *chord_keys));
            }
        }
        Action::NoOp
        | Action::Trans
        | Action::Repeat
        | Action::KeyCode(_)
        | Action::MultipleKeyCodes(_)
        | Action::Layer(_)
        | Action::DefaultLayer(_)
        | Action::Sequence { .. }
        | Action::RepeatableSequence { .. }
        | Action::CancelSequences
        | Action::ReleaseState(_)
        | Action::Custom(_) => {}
        Action::HoldTap(HoldTapAction { tap, hold, .. }) => {
            find_chords_coords(chord_groups, coord, tap);
            find_chords_coords(chord_groups, coord, hold);
        }
        Action::OneShot(OneShot { action: ac, .. }) => {
            find_chords_coords(chord_groups, coord, ac);
        }
        Action::MultipleActions(actions) => {
            for ac in actions.iter() {
                find_chords_coords(chord_groups, coord, ac);
            }
        }
        Action::TapDance(TapDance { actions, .. }) => {
            for ac in actions.iter() {
                find_chords_coords(chord_groups, coord, ac);
            }
        }
        Action::Fork(ForkConfig { left, right, .. }) => {
            find_chords_coords(chord_groups, coord, left);
            find_chords_coords(chord_groups, coord, right);
        }
        Action::Switch(Switch { cases }) => {
            for case in cases.iter() {
                find_chords_coords(chord_groups, coord, case.1);
            }
        }
    }
}

fn fill_chords(
    chord_groups: &[&'static ChordsGroup<&&[&CustomAction]>],
    action: &KanataAction,
    s: &ParserState,
) -> Option<KanataAction> {
    match action {
        Action::Chords(ChordsGroup { coords, .. }) => {
            let ((_, group_id), _) = coords
                .iter()
                .next()
                .expect("unresolved chords should have exactly one entry");
            Some(Action::Chords(chord_groups[*group_id as usize]))
        }
        Action::NoOp
        | Action::Trans
        | Action::Repeat
        | Action::KeyCode(_)
        | Action::MultipleKeyCodes(_)
        | Action::Layer(_)
        | Action::DefaultLayer(_)
        | Action::Sequence { .. }
        | Action::RepeatableSequence { .. }
        | Action::CancelSequences
        | Action::ReleaseState(_)
        | Action::Custom(_) => None,
        Action::HoldTap(&hta @ HoldTapAction { tap, hold, .. }) => {
            let new_tap = fill_chords(chord_groups, &tap, s);
            let new_hold = fill_chords(chord_groups, &hold, s);
            if new_tap.is_some() || new_hold.is_some() {
                Some(Action::HoldTap(s.a.sref(HoldTapAction {
                    hold: new_hold.unwrap_or(hold),
                    tap: new_tap.unwrap_or(tap),
                    ..hta
                })))
            } else {
                None
            }
        }
        Action::OneShot(&os @ OneShot { action: ac, .. }) => {
            fill_chords(chord_groups, ac, s).map(|ac| {
                Action::OneShot(s.a.sref(OneShot {
                    action: s.a.sref(ac),
                    ..os
                }))
            })
        }
        Action::MultipleActions(actions) => {
            let new_actions = actions
                .iter()
                .map(|ac| fill_chords(chord_groups, ac, s))
                .collect::<Vec<_>>();
            if new_actions.iter().any(|it| it.is_some()) {
                let new_actions = new_actions
                    .iter()
                    .zip(**actions)
                    .map(|(new_ac, ac)| new_ac.unwrap_or(*ac))
                    .collect::<Vec<_>>();
                Some(Action::MultipleActions(s.a.sref(s.a.sref_vec(new_actions))))
            } else {
                None
            }
        }
        Action::TapDance(&td @ TapDance { actions, .. }) => {
            let new_actions = actions
                .iter()
                .map(|ac| fill_chords(chord_groups, ac, s))
                .collect::<Vec<_>>();
            if new_actions.iter().any(|it| it.is_some()) {
                let new_actions = new_actions
                    .iter()
                    .zip(actions)
                    .map(|(new_ac, ac)| new_ac.map(|v| s.a.sref(v)).unwrap_or(*ac))
                    .collect::<Vec<_>>();
                Some(Action::TapDance(s.a.sref(TapDance {
                    actions: s.a.sref_vec(new_actions),
                    ..td
                })))
            } else {
                None
            }
        }
        Action::Fork(&fcfg @ ForkConfig { left, right, .. }) => {
            let new_left = fill_chords(chord_groups, &left, s);
            let new_right = fill_chords(chord_groups, &right, s);
            if new_left.is_some() || new_right.is_some() {
                Some(Action::Fork(s.a.sref(ForkConfig {
                    left: new_left.unwrap_or(left),
                    right: new_right.unwrap_or(right),
                    ..fcfg
                })))
            } else {
                None
            }
        }
        Action::Switch(Switch { cases }) => {
            let mut new_cases = vec![];
            for case in cases.iter() {
                new_cases.push((
                    case.0,
                    fill_chords(chord_groups, case.1, s)
                        .map(|ac| s.a.sref(ac))
                        .unwrap_or(case.1),
                    case.2,
                ));
            }
            Some(Action::Switch(s.a.sref(Switch {
                cases: s.a.sref_vec(new_cases),
            })))
        }
    }
}

fn parse_fake_keys(exprs: &[&Vec<SExpr>], s: &mut ParserState) -> Result<()> {
    for expr in exprs {
        let mut subexprs = check_first_expr(expr.iter(), "deffakekeys")?;
        // Read k-v pairs from the configuration
        while let Some(key_name_expr) = subexprs.next() {
            let key_name = key_name_expr
                .atom(s.vars())
                .ok_or_else(|| anyhow_expr!(key_name_expr, "Fake key name must not be a list."))?
                .to_owned();
            let action = match subexprs.next() {
                Some(v) => v,
                None => bail_expr!(
                    key_name_expr,
                    "Fake key name has no action - you should add an action."
                ),
            };
            let action = parse_action(action, s)?;
            let idx = s.virtual_keys.len();
            log::trace!("inserting {key_name}->{idx}:{action:?}");
            if s.virtual_keys
                .insert(key_name.clone(), (idx, action))
                .is_some()
            {
                bail_expr!(key_name_expr, "Duplicate fake key: {}", key_name);
            }
        }
    }
    if s.virtual_keys.len() > KEYS_IN_ROW {
        bail!(
            "Maximum number of fake keys is {KEYS_IN_ROW}, found {}",
            s.virtual_keys.len()
        );
    }
    Ok(())
}

fn parse_virtual_keys(exprs: &[&Vec<SExpr>], s: &mut ParserState) -> Result<()> {
    for expr in exprs {
        let mut subexprs = check_first_expr(expr.iter(), "defvirtualkeys")?;
        // Read k-v pairs from the configuration
        while let Some(key_name_expr) = subexprs.next() {
            let key_name = key_name_expr
                .atom(s.vars())
                .ok_or_else(|| anyhow_expr!(key_name_expr, "Virtual key name must not be a list."))?
                .to_owned();
            let action = match subexprs.next() {
                Some(v) => v,
                None => bail_expr!(
                    key_name_expr,
                    "Virtual key name has no action - you must add an action."
                ),
            };
            let action = parse_action(action, s)?;
            let idx = s.virtual_keys.len();
            log::trace!("inserting {key_name}->{idx}:{action:?}");
            if s.virtual_keys
                .insert(key_name.clone(), (idx, action))
                .is_some()
            {
                bail_expr!(key_name_expr, "Duplicate virtual key: {}", key_name);
            }
        }
    }
    if s.virtual_keys.len() > KEYS_IN_ROW {
        bail!(
            "Maximum number of virtual keys is {KEYS_IN_ROW}, found {}",
            s.virtual_keys.len()
        );
    }
    Ok(())
}

fn parse_distance(expr: &SExpr, s: &ParserState, label: &str) -> Result<u16> {
    expr.atom(s.vars())
        .map(str::parse::<u16>)
        .and_then(|d| match d {
            Ok(dist @ 1..=30000) => Some(dist),
            _ => None,
        })
        .ok_or_else(|| anyhow_expr!(expr, "{label} must be 1-30000"))
}

fn parse_mwheel(
    ac_params: &[SExpr],
    direction: MWheelDirection,
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "mwheel expects 2 parameters: <interval (ms)> <distance>";
    if ac_params.len() != 2 {
        bail!("{ERR_MSG}, found {}", ac_params.len());
    }
    let interval = parse_non_zero_u16(&ac_params[0], s, "interval")?;
    let distance = parse_distance(&ac_params[1], s, "distance")?;
    Ok(s.a.sref(Action::Custom(s.a.sref(s.a.sref_slice(
        CustomAction::MWheel {
            direction,
            interval,
            distance,
        },
    )))))
}

fn parse_move_mouse(
    ac_params: &[SExpr],
    direction: MoveDirection,
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "movemouse expects 2 parameters: <interval (ms)> <distance (px)>";
    if ac_params.len() != 2 {
        bail!("{ERR_MSG}, found {}", ac_params.len());
    }
    let interval = parse_non_zero_u16(&ac_params[0], s, "interval")?;
    let distance = parse_distance(&ac_params[1], s, "distance")?;
    Ok(s.a.sref(Action::Custom(s.a.sref(s.a.sref_slice(
        CustomAction::MoveMouse {
            direction,
            interval,
            distance,
        },
    )))))
}

fn parse_move_mouse_accel(
    ac_params: &[SExpr],
    direction: MoveDirection,
    s: &ParserState,
) -> Result<&'static KanataAction> {
    if ac_params.len() != 4 {
        bail!("movemouse-accel expects four parameters, found {}\n<interval (ms)> <acceleration time (ms)> <min_distance> <max_distance>", ac_params.len());
    }
    let interval = parse_non_zero_u16(&ac_params[0], s, "interval")?;
    let accel_time = parse_non_zero_u16(&ac_params[1], s, "acceleration time")?;
    let min_distance = parse_distance(&ac_params[2], s, "min distance")?;
    let max_distance = parse_distance(&ac_params[3], s, "max distance")?;
    if min_distance > max_distance {
        bail!("min distance should be less than max distance")
    }
    Ok(s.a.sref(Action::Custom(s.a.sref(s.a.sref_slice(
        CustomAction::MoveMouseAccel {
            direction,
            interval,
            accel_time,
            min_distance,
            max_distance,
        },
    )))))
}

fn parse_move_mouse_speed(ac_params: &[SExpr], s: &ParserState) -> Result<&'static KanataAction> {
    if ac_params.len() != 1 {
        bail!(
            "movemouse-speed expects one parameter, found {}\n<speed scaling % (1-65535)>",
            ac_params.len()
        );
    }
    let speed = parse_non_zero_u16(&ac_params[0], s, "speed scaling %")?;
    Ok(s.a.sref(Action::Custom(
        s.a.sref(s.a.sref_slice(CustomAction::MoveMouseSpeed { speed })),
    )))
}

fn parse_set_mouse(ac_params: &[SExpr], s: &ParserState) -> Result<&'static KanataAction> {
    if ac_params.len() != 2 {
        bail!(
            "movemouse-accel expects two parameters, found {}: <x> <y>",
            ac_params.len()
        );
    }
    let x = parse_u16(&ac_params[0], s, "x")?;
    let y = parse_u16(&ac_params[1], s, "y")?;
    Ok(s.a.sref(Action::Custom(
        s.a.sref(s.a.sref_slice(CustomAction::SetMouse { x, y })),
    )))
}

fn parse_dynamic_macro_record(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "dynamic-macro-record expects 1 parameter: <macro ID (0-65535)>";
    if ac_params.len() != 1 {
        bail!("{ERR_MSG}, found {}", ac_params.len());
    }
    let key = parse_u16(&ac_params[0], s, "macro ID")?;
    Ok(s.a.sref(Action::Custom(
        s.a.sref(s.a.sref_slice(CustomAction::DynamicMacroRecord(key))),
    )))
}

fn parse_dynamic_macro_play(ac_params: &[SExpr], s: &ParserState) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "dynamic-macro-play expects 1 parameter: <macro ID (number 0-65535)>";
    if ac_params.len() != 1 {
        bail!("{ERR_MSG}, found {}", ac_params.len());
    }
    let key = parse_u16(&ac_params[0], s, "macro ID")?;
    Ok(s.a.sref(Action::Custom(
        s.a.sref(s.a.sref_slice(CustomAction::DynamicMacroPlay(key))),
    )))
}

fn parse_live_reload_num(ac_params: &[SExpr], s: &ParserState) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "expects 1 parameter: <config argument position (1-65535)>";
    if ac_params.len() != 1 {
        bail!("{LIVE_RELOAD_NUM} {ERR_MSG}, found {}", ac_params.len());
    }
    let num = parse_non_zero_u16(&ac_params[0], s, "config argument position")?;
    Ok(s.a.sref(Action::Custom(
        // Note: for user-friendliness (hopefully), begin at 1 for parsing.
        // But for use as an index when stored as data, subtract 1 for 0-based indexing.
        s.a.sref(s.a.sref_slice(CustomAction::LiveReloadNum(num - 1))),
    )))
}

fn parse_live_reload_file(ac_params: &[SExpr], s: &ParserState) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "expects 1 parameter: <config argument (exact path)>";
    if ac_params.len() != 1 {
        bail!("{LIVE_RELOAD_FILE} {ERR_MSG}, found {}", ac_params.len());
    }
    let expr = &ac_params[0];
    let spanned_filepath = match expr {
        SExpr::Atom(filepath) => filepath,
        SExpr::List(_) => {
            bail_expr!(&expr, "Filepath cannot be a list")
        }
    };
    let lrld_file_path = spanned_filepath.t.trim_matches('"');
    Ok(s.a.sref(Action::Custom(s.a.sref(s.a.sref_slice(
        CustomAction::LiveReloadFile(lrld_file_path.to_string()),
    )))))
}

fn parse_layers(
    s: &ParserState,
    mapped_keys: &mut MappedKeys,
    defcfg: &CfgOptions,
) -> Result<IntermediateLayers> {
    let mut layers_cfg = new_layers(s.layer_exprs.len());
    if s.layer_exprs.len() > MAX_LAYERS {
        bail!("Maximum number of layers ({}) exceeded.", MAX_LAYERS);
    }
    let mut defsrc_layer = s.defsrc_layer;
    for (layer_level, layer) in s.layer_exprs.iter().enumerate() {
        match layer {
            // The skip is done to skip the the `deflayer` and layer name tokens.
            LayerExprs::DefsrcMapping(layer) => {
                // Parse actions in the layer and place them appropriately according
                // to defsrc mapping order.
                for (i, ac) in layer.iter().skip(2).enumerate() {
                    let ac = parse_action(ac, s)?;
                    layers_cfg[layer_level][0][s.mapping_order[i]] = *ac;
                }
            }
            LayerExprs::CustomMapping(layer) => {
                // Parse actions as input output pairs
                let mut pairs = layer[2..].chunks_exact(2);
                let mut layer_mapped_keys = HashSet::default();
                let mut defsrc_anykey_used = false;
                let mut unmapped_anykey_used = false;
                let mut both_anykey_used = false;
                for triplet in pairs.by_ref() {
                    let input = &triplet[0];
                    let action = &triplet[1];

                    // TODO: remove me some time after April 2024 to reduce code bloat somewhat.
                    const MAPSTRS: &[&str] = &[":", "->", ">>", "maps-to", "→", "🞂"];
                    const MAPSTR_ERR: &str = "Seems you are using a retired configuration style.\n\
                            You should remove all mapping strings from deflayermap;\n\
                            deflayermap now uses pairs instead of triples.";
                    if input.atom(s.vars()).is_some_and(|x| MAPSTRS.contains(&x)) {
                        bail_expr!(input, "{MAPSTR_ERR}");
                    }
                    if action.atom(s.vars()).is_some_and(|x| MAPSTRS.contains(&x)) {
                        bail_expr!(action, "{MAPSTR_ERR}");
                    }

                    let action = parse_action(action, s)?;
                    if input.atom(s.vars()).is_some_and(|x| x == "_") {
                        if defsrc_anykey_used {
                            bail_expr!(input, "must have only one use of _ within a layer")
                        }
                        if both_anykey_used {
                            bail_expr!(input, "must either use _ or ___ within a layer, not both")
                        }
                        for i in 0..s.mapping_order.len() {
                            if layers_cfg[layer_level][0][s.mapping_order[i]] == Action::Trans {
                                layers_cfg[layer_level][0][s.mapping_order[i]] = *action;
                            }
                        }
                        defsrc_anykey_used = true;
                    } else if input.atom(s.vars()).is_some_and(|x| x == "__") {
                        if unmapped_anykey_used {
                            bail_expr!(input, "must have only one use of __ within a layer")
                        }
                        if !defcfg.process_unmapped_keys {
                            bail_expr!(
                                input,
                                "must set process-unmapped-keys to yes to use __ to map unmapped keys"
                            );
                        }
                        if both_anykey_used {
                            bail_expr!(input, "must either use __ or ___ within a layer, not both")
                        }
                        for i in 0..layers_cfg[0][0].len() {
                            if layers_cfg[layer_level][0][i] == Action::Trans
                                && !s.mapping_order.contains(&i)
                            {
                                layers_cfg[layer_level][0][i] = *action;
                            }
                        }
                        unmapped_anykey_used = true;
                    } else if input.atom(s.vars()).is_some_and(|x| x == "___") {
                        if both_anykey_used {
                            bail_expr!(input, "must have only one use of ___ within a layer")
                        }
                        if defsrc_anykey_used {
                            bail_expr!(input, "must either use _ or ___ within a layer, not both")
                        }
                        if unmapped_anykey_used {
                            bail_expr!(input, "must either use __ or ___ within a layer, not both")
                        }
                        if !defcfg.process_unmapped_keys {
                            bail_expr!(
                                input,
                                "must set process-unmapped-keys to yes to use ___ to also map unmapped keys"
                            );
                        }
                        for i in 0..layers_cfg[0][0].len() {
                            if layers_cfg[layer_level][0][i] == Action::Trans {
                                layers_cfg[layer_level][0][i] = *action;
                            }
                        }
                        both_anykey_used = true;
                    } else {
                        let input_key = input
                            .atom(s.vars())
                            .and_then(str_to_oscode)
                            .ok_or_else(|| anyhow_expr!(input, "input must be a key name"))?;
                        mapped_keys.insert(input_key);
                        if !layer_mapped_keys.insert(input_key) {
                            bail_expr!(input, "input key must not be repeated within a layer")
                        }
                        layers_cfg[layer_level][0][usize::from(input_key)] = *action;
                    }
                }
                let rem = pairs.remainder();
                match rem.len() {
                    0 => {}
                    1 => {
                        bail_expr!(
                            &rem[0],
                            "an input must be followed by a map string and an action"
                        );
                    }
                    2 => {
                        bail_expr!(&rem[1], "map string must be followed by an action");
                    }
                    _ => unreachable!(),
                }
            }
        }
        for (osc, layer_action) in layers_cfg[layer_level][0].iter_mut().enumerate() {
            if s.block_unmapped_keys && *layer_action == Action::Trans && !is_a_button(osc as u16) {
                *layer_action = Action::NoOp;
            }
        }

        // Set fake keys on every layer.
        for (y, action) in s.virtual_keys.values() {
            let (x, y) = get_fake_key_coords(*y);
            layers_cfg[layer_level][x as usize][y as usize] = **action;
        }

        // If the user has configured delegation to the first (default) layer for transparent keys,
        // (as opposed to delegation to defsrc), replace the defsrc actions with the actions from
        // the first layer.
        if layer_level == 0 && s.delegate_to_first_layer {
            for (defsrc_ac, default_layer_ac) in defsrc_layer.iter_mut().zip(layers_cfg[0][0]) {
                if default_layer_ac != Action::Trans {
                    *defsrc_ac = default_layer_ac;
                }
            }
        }
    }
    Ok(layers_cfg)
}

const SEQ_ERR: &str = "defseq expects pairs of parameters: <virtual_key_name> <key_list>";

fn parse_sequences(exprs: &[&Vec<SExpr>], s: &ParserState) -> Result<KeySeqsToFKeys> {
    let mut sequences = Trie::new();
    for expr in exprs {
        let mut subexprs = check_first_expr(expr.iter(), "defseq")?.peekable();

        while let Some(vkey_expr) = subexprs.next() {
            let vkey = vkey_expr.atom(s.vars()).ok_or_else(|| {
                anyhow_expr!(vkey_expr, "{SEQ_ERR}\nvirtual_key_name must not be a list")
            })?;
            if !s.virtual_keys.contains_key(vkey) {
                bail_expr!(
                    vkey_expr,
                    "{SEQ_ERR}\nThe referenced key does not exist: {vkey}"
                );
            }
            let key_seq_expr = subexprs
                .next()
                .ok_or_else(|| anyhow_expr!(vkey_expr, "{SEQ_ERR}\nMissing key_list for {vkey}"))?;
            let key_seq = key_seq_expr.list(s.vars()).ok_or_else(|| {
                anyhow_expr!(key_seq_expr, "{SEQ_ERR}\nGot a non-list for key_list")
            })?;
            if key_seq.is_empty() {
                bail_expr!(key_seq_expr, "{SEQ_ERR}\nkey_list cannot be empty");
            }
            let keycode_seq = parse_sequence_keys(key_seq, s)?;
            if sequences.ancestor_exists(&keycode_seq) {
                bail_expr!(
                    key_seq_expr,
                    "Sequence has a conflict: its sequence contains an earlier defined sequence"
                );
            }
            if sequences.descendant_exists(&keycode_seq) {
                bail_expr!(key_seq_expr, "Sequence has a conflict: its sequence is contained within an earlier defined seqence");
            }
            sequences.insert(
                keycode_seq,
                s.virtual_keys
                    .get(vkey)
                    .map(|(y, _)| get_fake_key_coords(*y))
                    .expect("vk exists, checked earlier"),
            );
        }
    }
    Ok(sequences)
}

fn parse_sequence_keys(exprs: &[SExpr], s: &ParserState) -> Result<Vec<u16>> {
    use crate::sequences::*;
    use SequenceEvent::*;

    // Reuse macro parsing but do some other processing since sequences don't support everything
    // that can go in a macro, and also change error messages of course.
    let mut exprs_remaining = exprs;
    let mut all_keys = Vec::new();
    while !exprs_remaining.is_empty() {
        let (mut keys, exprs_remaining_tmp) =
            match parse_macro_item_impl(exprs_remaining, s, MacroNumberParseMode::Action) {
                Ok(res) => {
                    if res.0.iter().any(|k| !matches!(k, Press(..) | Release(..))) {
                        // Determine the bad expression depending on how many expressions were consumed
                        // by parse_macro_item_impl.
                        let bad_expr = if exprs_remaining.len() - res.1.len() == 1 {
                            &exprs_remaining[0]
                        } else {
                            // This error message will have an imprecise span since it will take the
                            // whole chorded list instead of the single element inside that's not a
                            // standard key. Oh well, should still be helpful. I'm too lazy to write
                            // the code to find the exact expr to use right now.
                            &exprs_remaining[1]
                        };
                        bail_expr!(bad_expr, "{SEQ_ERR}\nFound invalid key/chord in key_list");
                    }

                    // The keys are currenty in the form of SequenceEvent::{Press, Release}. This is
                    // not what we want.
                    //
                    // The trivial and incorrect way to parse this would be to just take all of the
                    // presses. However, we need to transform chorded keys/lists like S-a or S-(a b) to
                    // have the upper bits set, to be able to differentiate (S-a b) from (S-(a b)).
                    //
                    // The order of presses and releases reveals whether or not a key is chorded with
                    // some modifier. When a chord starts, there are multiple presses in a row, whereas
                    // non-chords will always be a press followed by a release. Likewise, a chord
                    // ending is marked by multiple releases in a row.
                    let mut mods_currently_held = vec![];
                    let mut key_actions = res.0.iter().peekable();
                    let mut seq = vec![];
                    let mut do_release_mod = false;
                    while let Some(action) = key_actions.next() {
                        match action {
                            Press(pressed) => {
                                if matches!(key_actions.peek(), Some(Press(..))) {
                                    // press->press: current press is mod
                                    mods_currently_held.push(*pressed);
                                }
                                let mut seq_num = u16::from(OsCode::from(pressed));
                                for modk in mods_currently_held.iter().copied() {
                                    seq_num |= mod_mask_for_keycode(modk);
                                }
                                seq.push(seq_num);
                            }
                            Release(released) => {
                                if do_release_mod {
                                    mods_currently_held.remove(
                                        mods_currently_held
                                            .iter()
                                            .position(|modk| modk == released)
                                            .expect("had to be pressed to be released"),
                                    );
                                }
                                // release->release: next release is mod
                                do_release_mod = matches!(key_actions.peek(), Some(Release(..)));
                            }
                            _ => unreachable!("should be filtered out"),
                        }
                    }

                    (seq, res.1)
                }
                Err(mut e) => {
                    e.msg = format!("{SEQ_ERR}\nFound invalid key/chord in key_list");
                    return Err(e);
                }
            };
        all_keys.append(&mut keys);
        exprs_remaining = exprs_remaining_tmp;
    }
    Ok(all_keys)
}

fn parse_arbitrary_code(ac_params: &[SExpr], s: &ParserState) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "arbitrary code expects one parameter: <code: 0-767>";
    if ac_params.len() != 1 {
        bail!("{ERR_MSG}");
    }
    let code = ac_params[0]
        .atom(s.vars())
        .map(str::parse::<u16>)
        .and_then(|c| match c {
            Ok(code @ 0..=767) => Some(code),
            _ => None,
        })
        .ok_or_else(|| anyhow!("{ERR_MSG}: got {:?}", ac_params[0]))?;
    Ok(s.a.sref(Action::Custom(
        s.a.sref(s.a.sref_slice(CustomAction::SendArbitraryCode(code))),
    )))
}

fn parse_overrides(exprs: &[SExpr], s: &ParserState) -> Result<Overrides> {
    const ERR_MSG: &str =
        "defoverrides expects pairs of parameters: <input key list> <output key list>";
    let mut subexprs = check_first_expr(exprs.iter(), "defoverrides")?;

    let mut overrides = Vec::<Override>::new();
    while let Some(in_keys_expr) = subexprs.next() {
        let in_keys = in_keys_expr
            .list(s.vars())
            .ok_or_else(|| anyhow_expr!(in_keys_expr, "Input keys must be a list"))?;
        let out_keys_expr = subexprs
            .next()
            .ok_or_else(|| anyhow_expr!(in_keys_expr, "Missing output keys for input keys"))?;
        let out_keys = out_keys_expr
            .list(s.vars())
            .ok_or_else(|| anyhow_expr!(out_keys_expr, "Output keys must be a list"))?;
        let in_keys =
            in_keys
                .iter()
                .try_fold(vec![], |mut keys, key_expr| -> Result<Vec<OsCode>> {
                    let key = key_expr
                        .atom(s.vars())
                        .and_then(str_to_oscode)
                        .ok_or_else(|| {
                            anyhow_expr!(key_expr, "Unknown input key name, must use known keys")
                        })?;
                    keys.push(key);
                    Ok(keys)
                })?;
        let out_keys =
            out_keys
                .iter()
                .try_fold(vec![], |mut keys, key_expr| -> Result<Vec<OsCode>> {
                    let key = key_expr
                        .atom(s.vars())
                        .and_then(str_to_oscode)
                        .ok_or_else(|| {
                            anyhow_expr!(key_expr, "Unknown output key name, must use known keys")
                        })?;
                    keys.push(key);
                    Ok(keys)
                })?;
        overrides
            .push(Override::try_new(&in_keys, &out_keys).map_err(|e| anyhow!("{ERR_MSG}: {e}"))?);
    }
    log::debug!("All overrides:\n{overrides:#?}");
    Ok(Overrides::new(&overrides))
}

fn parse_fork(ac_params: &[SExpr], s: &ParserState) -> Result<&'static KanataAction> {
    const ERR_STR: &str =
        "fork expects 3 params: <left-action> <right-action> <right-trigger-keys>";
    if ac_params.len() != 3 {
        bail!("{ERR_STR}\nFound {} params instead of 3", ac_params.len());
    }
    let left = *parse_action(&ac_params[0], s)?;
    let right = *parse_action(&ac_params[1], s)?;
    let right_triggers = s.a.sref_vec(
        parse_key_list(&ac_params[2], s, "right-trigger-keys")?
            .into_iter()
            .map(KeyCode::from)
            .collect::<Vec<_>>(),
    );
    Ok(s.a.sref(Action::Fork(s.a.sref(ForkConfig {
        left,
        right,
        right_triggers,
    }))))
}

fn parse_caps_word(ac_params: &[SExpr], s: &ParserState) -> Result<&'static KanataAction> {
    const ERR_STR: &str = "caps-word expects 1 param: <timeout>";
    if ac_params.len() != 1 {
        bail!("{ERR_STR}\nFound {} params instead of 1", ac_params.len());
    }
    let timeout = parse_non_zero_u16(&ac_params[0], s, "timeout")?;
    Ok(s.a.sref(Action::Custom(s.a.sref(s.a.sref_slice(
        CustomAction::CapsWord(CapsWordCfg {
            keys_to_capitalize: &[
                KeyCode::A,
                KeyCode::B,
                KeyCode::C,
                KeyCode::D,
                KeyCode::E,
                KeyCode::F,
                KeyCode::G,
                KeyCode::H,
                KeyCode::I,
                KeyCode::J,
                KeyCode::K,
                KeyCode::L,
                KeyCode::M,
                KeyCode::N,
                KeyCode::O,
                KeyCode::P,
                KeyCode::Q,
                KeyCode::R,
                KeyCode::S,
                KeyCode::T,
                KeyCode::U,
                KeyCode::V,
                KeyCode::W,
                KeyCode::X,
                KeyCode::Y,
                KeyCode::Z,
                KeyCode::Minus,
            ],
            keys_nonterminal: &[
                KeyCode::Kb0,
                KeyCode::Kb1,
                KeyCode::Kb2,
                KeyCode::Kb3,
                KeyCode::Kb4,
                KeyCode::Kb5,
                KeyCode::Kb6,
                KeyCode::Kb7,
                KeyCode::Kb8,
                KeyCode::Kb9,
                KeyCode::Kp0,
                KeyCode::Kp1,
                KeyCode::Kp2,
                KeyCode::Kp3,
                KeyCode::Kp4,
                KeyCode::Kp5,
                KeyCode::Kp6,
                KeyCode::Kp7,
                KeyCode::Kp8,
                KeyCode::Kp9,
                KeyCode::BSpace,
                KeyCode::Delete,
                KeyCode::Up,
                KeyCode::Down,
                KeyCode::Left,
                KeyCode::Right,
            ],
            timeout,
        }),
    )))))
}

fn parse_caps_word_custom(ac_params: &[SExpr], s: &ParserState) -> Result<&'static KanataAction> {
    const ERR_STR: &str = "caps-word-custom expects 3 param: <timeout> <keys-to-capitalize> <extra-non-terminal-keys>";
    if ac_params.len() != 3 {
        bail!("{ERR_STR}\nFound {} params instead of 3", ac_params.len());
    }
    let timeout = parse_non_zero_u16(&ac_params[0], s, "timeout")?;
    Ok(s.a.sref(Action::Custom(
        s.a.sref(
            s.a.sref_slice(CustomAction::CapsWord(CapsWordCfg {
                keys_to_capitalize: s.a.sref_vec(
                    parse_key_list(&ac_params[1], s, "keys-to-capitalize")?
                        .into_iter()
                        .map(KeyCode::from)
                        .collect(),
                ),
                keys_nonterminal: s.a.sref_vec(
                    parse_key_list(&ac_params[2], s, "extra-non-terminal-keys")?
                        .into_iter()
                        .map(KeyCode::from)
                        .collect(),
                ),
                timeout,
            })),
        ),
    )))
}

fn parse_macro_record_stop_truncate(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_STR: &str =
        "dynamic-macro-record-stop-truncate expects 1 param: <num-keys-to-truncate>";
    if ac_params.len() != 1 {
        bail!("{ERR_STR}\nFound {} params instead of 1", ac_params.len());
    }
    let num_to_truncate = parse_u16(&ac_params[0], s, "num-keys-to-truncate")?;
    Ok(s.a.sref(Action::Custom(s.a.sref(
        s.a.sref_slice(CustomAction::DynamicMacroRecordStop(num_to_truncate)),
    ))))
}

fn parse_sequence_start(ac_params: &[SExpr], s: &ParserState) -> Result<&'static KanataAction> {
    const ERR_MSG: &str =
        "sequence expects one or two params: <timeout-override> <?input-mode-override>";
    if !matches!(ac_params.len(), 1 | 2) {
        bail!("{ERR_MSG}\nfound {} items", ac_params.len());
    }
    let timeout = parse_non_zero_u16(&ac_params[0], s, "timeout-override")?;
    let input_mode = if ac_params.len() > 1 {
        if let Some(Ok(input_mode)) = ac_params[1]
            .atom(s.vars())
            .map(SequenceInputMode::try_from_str)
        {
            input_mode
        } else {
            bail_expr!(&ac_params[1], "{ERR_MSG}\n{}", SequenceInputMode::err_msg());
        }
    } else {
        s.default_sequence_input_mode
    };
    Ok(s.a.sref(Action::Custom(s.a.sref(
        s.a.sref_slice(CustomAction::SequenceLeader(timeout, input_mode)),
    ))))
}

fn parse_unmod(
    unmod_type: &str,
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "expects expects at least one key name";
    if ac_params.is_empty() {
        bail!("{unmod_type} {ERR_MSG}\nfound {} items", ac_params.len());
    }
    let mut keys: Vec<KeyCode> = ac_params.iter().try_fold(Vec::new(), |mut keys, param| {
        keys.push(
            param
                .atom(s.vars())
                .and_then(str_to_oscode)
                .ok_or_else(|| anyhow_expr!(&ac_params[0], "{unmod_type} {ERR_MSG}"))?
                .into(),
        );
        Ok::<_, ParseError>(keys)
    })?;
    keys.shrink_to_fit();
    match unmod_type {
        UNMOD => Ok(s.a.sref(Action::Custom(
            s.a.sref(s.a.sref_slice(CustomAction::Unmodded { keys })),
        ))),
        UNSHIFT => Ok(s.a.sref(Action::Custom(
            s.a.sref(s.a.sref_slice(CustomAction::Unshifted { keys })),
        ))),
        _ => panic!("Unknown unmod type {unmod_type}"),
    }
}

/// Creates a `KeyOutputs` from `layers::LAYERS`.
fn create_key_outputs(
    layers: &KLayers,
    overrides: &Overrides,
    chords_v2: &Option<ChordsV2<'static, KanataCustom>>,
) -> KeyOutputs {
    let mut outs = KeyOutputs::new();
    for (layer_idx, layer) in layers.iter().enumerate() {
        let mut layer_outputs = HashMap::default();
        for (i, action) in layer[0].iter().enumerate() {
            let osc_slot = match i.try_into() {
                Ok(i) => i,
                Err(_) => continue,
            };
            add_key_output_from_action_to_key_pos(osc_slot, action, &mut layer_outputs, overrides);
            add_chordsv2_output_for_key_pos(
                osc_slot,
                layer_idx,
                chords_v2,
                &mut layer_outputs,
                overrides,
            );
        }
        outs.push(layer_outputs);
    }
    for layer_outs in outs.iter_mut() {
        for keys_out in layer_outs.values_mut() {
            keys_out.shrink_to_fit();
        }
        layer_outs.shrink_to_fit();
    }
    outs.shrink_to_fit();
    outs
}

fn add_chordsv2_output_for_key_pos(
    osc_slot: OsCode,
    layer_idx: usize,
    chords_v2: &Option<ChordsV2<'static, KanataCustom>>,
    outputs: &mut HashMap<OsCode, Vec<OsCode>>,
    overrides: &Overrides,
) {
    assert!(layer_idx <= usize::from(u16::MAX));
    let Some(chords_v2) = chords_v2.as_ref() else {
        return;
    };
    let Some(chords_for_key) = chords_v2.chords().mapping.get(&u16::from(osc_slot)) else {
        return;
    };
    for chord in chords_for_key.chords.iter() {
        if !chord.disabled_layers.contains(&(layer_idx as u16)) {
            add_key_output_from_action_to_key_pos(osc_slot, chord.action, outputs, overrides);
        }
    }
}

fn add_key_output_from_action_to_key_pos(
    osc_slot: OsCode,
    action: &KanataAction,
    outputs: &mut HashMap<OsCode, Vec<OsCode>>,
    overrides: &Overrides,
) {
    match action {
        Action::KeyCode(kc) => {
            add_kc_output(osc_slot, kc.into(), outputs, overrides);
        }
        Action::HoldTap(HoldTapAction {
            tap,
            hold,
            timeout_action,
            ..
        }) => {
            add_key_output_from_action_to_key_pos(osc_slot, tap, outputs, overrides);
            add_key_output_from_action_to_key_pos(osc_slot, hold, outputs, overrides);
            add_key_output_from_action_to_key_pos(osc_slot, timeout_action, outputs, overrides);
        }
        Action::OneShot(OneShot { action: ac, .. }) => {
            add_key_output_from_action_to_key_pos(osc_slot, ac, outputs, overrides);
        }
        Action::MultipleKeyCodes(kcs) => {
            for kc in kcs.iter() {
                add_kc_output(osc_slot, kc.into(), outputs, overrides);
            }
        }
        Action::MultipleActions(actions) => {
            for ac in actions.iter() {
                add_key_output_from_action_to_key_pos(osc_slot, ac, outputs, overrides);
            }
        }
        Action::TapDance(TapDance { actions, .. }) => {
            for ac in actions.iter() {
                add_key_output_from_action_to_key_pos(osc_slot, ac, outputs, overrides);
            }
        }
        Action::Fork(ForkConfig { left, right, .. }) => {
            add_key_output_from_action_to_key_pos(osc_slot, left, outputs, overrides);
            add_key_output_from_action_to_key_pos(osc_slot, right, outputs, overrides);
        }
        Action::Chords(ChordsGroup { chords, .. }) => {
            for (_, ac) in chords.iter() {
                add_key_output_from_action_to_key_pos(osc_slot, ac, outputs, overrides);
            }
        }
        Action::Switch(Switch { cases }) => {
            for case in cases.iter() {
                add_key_output_from_action_to_key_pos(osc_slot, case.1, outputs, overrides);
            }
        }
        Action::Custom(cacs) => {
            for ac in cacs.iter() {
                match ac {
                    CustomAction::Unmodded { keys } | CustomAction::Unshifted { keys } => {
                        for k in keys.iter() {
                            add_kc_output(osc_slot, k.into(), outputs, overrides);
                        }
                    }
                    _ => {}
                }
            }
        }
        Action::NoOp
        | Action::Trans
        | Action::Repeat
        | Action::Layer(_)
        | Action::DefaultLayer(_)
        | Action::Sequence { .. }
        | Action::RepeatableSequence { .. }
        | Action::CancelSequences
        | Action::ReleaseState(_) => {}
    };
}

fn add_kc_output(
    osc_slot: OsCode,
    osc: OsCode,
    outs: &mut HashMap<OsCode, Vec<OsCode>>,
    overrides: &Overrides,
) {
    let outputs = match outs.entry(osc_slot) {
        Entry::Occupied(o) => o.into_mut(),
        Entry::Vacant(v) => v.insert(vec![]),
    };
    if !outputs.contains(&osc) {
        outputs.push(osc);
    }
    for ov_osc in overrides
        .output_non_mods_for_input_non_mod(osc)
        .iter()
        .copied()
    {
        if !outputs.contains(&ov_osc) {
            outputs.push(ov_osc);
        }
    }
}
