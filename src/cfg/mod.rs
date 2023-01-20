//! This parses the configuration language to create a `kanata_keyberon::layout::Layout` as well as
//! associated metadata to help with processing.
//!
//! How the configuration maps to keyberon:
//!
//! If the mapped keys are defined as:
//!
//!     (defsrc
//!         esc  1    2    3    4
//!     )
//!
//! and the layers are:
//!
//!     (deflayer one
//!         _   a    s    d    _
//!     )
//!
//!     (deflayer two
//!         _   a    o    e    _
//!     )
//!
//! Then the keyberon layers will be as follows:
//!
//!     xx means unimportant and _ means transparent.
//!
//!     layers[0] = { xx, esc, a, s, d, 4, xx... }
//!     layers[1] = { xx, _  , a, s, d, _, xx... }
//!     layers[2] = { xx, esc, a, o, e, 4, xx... }
//!     layers[3] = { xx, _  , a, s, d, _, xx... }
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
mod sexpr;

use crate::custom_action::*;
use crate::keys::*;
use crate::layers::*;

use anyhow::{anyhow, bail, Result};
use radix_trie::Trie;
use std::collections::hash_map::Entry;

type HashSet<T> = rustc_hash::FxHashSet<T>;
type HashMap<K, V> = rustc_hash::FxHashMap<K, V>;

use kanata_keyberon::action::*;
use kanata_keyberon::key_code::*;
use kanata_keyberon::layout::*;
use sexpr::SExpr;

use self::sexpr::Spanned;

pub type KanataAction = Action<&'static [&'static CustomAction]>;
pub type KanataLayout = Layout<KEYS_IN_ROW, 2, ACTUAL_NUM_LAYERS, &'static [&'static CustomAction]>;
pub type KeySeqsToFKeys = Trie<Vec<u16>, (u8, u16)>;

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
    pub items: HashMap<String, String>,
    /// The keyberon layout state machine struct.
    pub layout: KanataLayout,
    /// Sequences defined in `defseq`.
    pub sequences: KeySeqsToFKeys,
}

impl Cfg {
    pub fn new_from_file(p: &std::path::Path) -> Result<Self> {
        let (items, mapped_keys, layer_info, key_outputs, layout, sequences) = parse_cfg(p)?;
        log::info!("config parsed");
        Ok(Self {
            items,
            mapped_keys,
            layer_info,
            key_outputs,
            layout,
            sequences,
        })
    }
}

pub type MappedKeys = HashSet<OsCode>;
// Note: this uses a Vec instead of a HashSet because ordering matters, e.g. for chords like `S-b`,
// we want to ensure that `b` is checked first, so this will be iterated over in reverse order.
pub type KeyOutputs = Vec<HashMap<OsCode, Vec<OsCode>>>;

fn add_kc_output(osc_slot: OsCode, kc: OsCode, outs: &mut HashMap<OsCode, Vec<OsCode>>) {
    let outputs = match outs.entry(osc_slot) {
        Entry::Occupied(o) => o.into_mut(),
        Entry::Vacant(v) => v.insert(vec![]),
    };
    outputs.push(kc);
}

#[test]
fn parse_simple() {
    parse_cfg(&std::path::PathBuf::from("./cfg_samples/simple.kbd")).unwrap();
}

#[test]
fn parse_minimal() {
    parse_cfg(&std::path::PathBuf::from("./cfg_samples/minimal.kbd")).unwrap();
}

#[test]
fn parse_default() {
    parse_cfg(&std::path::PathBuf::from("./cfg_samples/kanata.kbd")).unwrap();
}

#[test]
fn parse_jtroo() {
    let (_, _, layer_strings, _, _, _) =
        parse_cfg(&std::path::PathBuf::from("./cfg_samples/jtroo.kbd")).unwrap();
    assert_eq!(layer_strings.len(), 16);
}

#[test]
fn parse_f13_f24() {
    parse_cfg(&std::path::PathBuf::from("./cfg_samples/f13_f24.kbd")).unwrap();
}

#[test]
fn parse_transparent_default() {
    let (_, _, layer_strings, layers, _) = parse_cfg_raw(&std::path::PathBuf::from(
        "./cfg_samples/transparent_default.kbd",
    ))
    .unwrap();

    assert_eq!(layer_strings.len(), 4);

    assert_eq!(
        layers[0][0][usize::from(OsCode::KEY_F13)],
        Action::KeyCode(KeyCode::F13)
    );
    assert_eq!(
        layers[0][0][usize::from(OsCode::KEY_F14)],
        Action::DefaultLayer(2)
    );
    assert_eq!(layers[0][0][usize::from(OsCode::KEY_F15)], Action::Layer(3));
    assert_eq!(layers[1][0][usize::from(OsCode::KEY_F13)], Action::Trans);
    assert_eq!(
        layers[1][0][usize::from(OsCode::KEY_F14)],
        Action::DefaultLayer(2)
    );
    assert_eq!(layers[1][0][usize::from(OsCode::KEY_F15)], Action::Layer(3));
    assert_eq!(
        layers[2][0][usize::from(OsCode::KEY_F13)],
        Action::DefaultLayer(0)
    );
    assert_eq!(layers[2][0][usize::from(OsCode::KEY_F14)], Action::Layer(1));
    assert_eq!(
        layers[2][0][usize::from(OsCode::KEY_F15)],
        Action::KeyCode(KeyCode::F15)
    );
    assert_eq!(
        layers[3][0][usize::from(OsCode::KEY_F13)],
        Action::DefaultLayer(0)
    );
    assert_eq!(layers[3][0][usize::from(OsCode::KEY_F14)], Action::Layer(1));
    assert_eq!(layers[3][0][usize::from(OsCode::KEY_F15)], Action::Trans);
}

#[test]
fn parse_all_keys() {
    parse_cfg(&std::path::PathBuf::from(
        "./cfg_samples/all_keys_in_defsrc.kbd",
    ))
    .unwrap();
}

#[test]
fn parse_multiline_comment() {
    parse_cfg(&std::path::PathBuf::from(
        "./test_cfgs/multiline_comment.kbd",
    ))
    .unwrap();
}

#[test]
fn disallow_nested_tap_hold() {
    match parse_cfg(&std::path::PathBuf::from("./test_cfgs/nested_tap_hold.kbd"))
        .map_err(|e| e.to_string())
    {
        Ok(_) => panic!("invalid nested tap-hold in tap action was Ok'd"),
        Err(e) => assert!(e.contains("tap-hold"), "real e: {e}"),
    }
}

#[test]
fn disallow_ancestor_seq() {
    match parse_cfg(&std::path::PathBuf::from("./test_cfgs/ancestor_seq.kbd"))
        .map_err(|e| e.to_string())
    {
        Ok(_) => panic!("invalid ancestor seq was Ok'd"),
        Err(e) => assert!(e.contains("is contained")),
    }
}

#[test]
fn disallow_descendent_seq() {
    match parse_cfg(&std::path::PathBuf::from("./test_cfgs/descendant_seq.kbd"))
        .map_err(|e| e.to_string())
    {
        Ok(_) => panic!("invalid descendant seq was Ok'd"),
        Err(e) => assert!(e.contains("contains")),
    }
}

#[derive(Debug)]
pub struct LayerInfo {
    pub name: String,
    pub cfg_text: String,
}

#[allow(clippy::type_complexity)] // return type is not pub
fn parse_cfg(
    p: &std::path::Path,
) -> Result<(
    HashMap<String, String>,
    MappedKeys,
    Vec<LayerInfo>,
    KeyOutputs,
    KanataLayout,
    KeySeqsToFKeys,
)> {
    let (cfg, src, layer_info, klayers, seqs) = parse_cfg_raw(p)?;

    Ok((
        cfg,
        src,
        layer_info,
        create_key_outputs(&klayers),
        create_layout(klayers),
        seqs,
    ))
}

#[cfg(all(not(feature = "interception_driver"), target_os = "windows"))]
const DEF_LOCAL_KEYS: &str = "deflocalkeys-win";
#[cfg(all(feature = "interception_driver", target_os = "windows"))]
const DEF_LOCAL_KEYS: &str = "deflocalkeys-winercept";
#[cfg(target_os = "linux")]
const DEF_LOCAL_KEYS: &str = "deflocalkeys-linux";

#[allow(clippy::type_complexity)] // return type is not pub
fn parse_cfg_raw(
    p: &std::path::Path,
) -> Result<(
    HashMap<String, String>,
    MappedKeys,
    Vec<LayerInfo>,
    Box<KanataLayers>,
    KeySeqsToFKeys,
)> {
    let text = std::fs::read_to_string(p)?;

    let spanned_root_exprs = sexpr::parse(&text)?;
    let root_exprs: Vec<_> = spanned_root_exprs.iter().map(|t| t.t.clone()).collect();

    let cfg_expr = root_exprs
        .iter()
        .find(gen_first_atom_filter("defcfg"))
        .ok_or_else(|| anyhow!("defcfg is missing from the configuration"))?;
    if root_exprs
        .iter()
        .filter(gen_first_atom_filter("defcfg"))
        .count()
        > 1
    {
        bail!("Only one defcfg is allowed in the configuration")
    }
    let cfg = parse_defcfg(cfg_expr)?;

    if let Some(result) = root_exprs
        .iter()
        .find(gen_first_atom_filter(DEF_LOCAL_KEYS))
        .map(|custom_keys| parse_deflocalkeys(custom_keys))
    {
        result?;
    }
    if root_exprs
        .iter()
        .filter(gen_first_atom_filter(DEF_LOCAL_KEYS))
        .count()
        > 1
    {
        bail!("Only one defcustomkeys is allowed in the configuration")
    }

    let src_expr = root_exprs
        .iter()
        .find(gen_first_atom_filter("defsrc"))
        .ok_or_else(|| anyhow!("defsrc is missing from the configuration"))?;
    if root_exprs
        .iter()
        .filter(gen_first_atom_filter("defsrc"))
        .count()
        > 1
    {
        bail!("Only one defsrc is allowed in the configuration")
    }
    let (src, mapping_order) = parse_defsrc(src_expr, &cfg)?;

    let deflayer_filter = gen_first_atom_filter("deflayer");
    let layer_exprs = root_exprs
        .iter()
        .filter(&deflayer_filter)
        .collect::<Vec<_>>();

    if layer_exprs.is_empty() {
        bail!("No deflayer expressions exist. At least one layer must be defined.")
    }
    if layer_exprs.len() > MAX_LAYERS {
        bail!("Exceeded the maximum layer count of {}", MAX_LAYERS)
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
        .flat_map(|s| {
            // Duplicate the same layer for `layer_strings` because the keyberon layout itself has
            // two versions of each layer.
            std::iter::repeat(s).take(2)
        })
        .collect::<Vec<_>>();

    let layer_strings = spanned_root_exprs
        .iter()
        .filter(|expr| deflayer_filter(&&expr.t))
        .map(|expr| text[expr.span].to_string())
        .flat_map(|s| {
            // Duplicate the same layer for `layer_strings` because the keyberon layout itself has
            // two versions of each layer.
            std::iter::repeat(s).take(2)
        })
        .collect::<Vec<_>>();

    let layer_info: Vec<LayerInfo> = layer_names
        .into_iter()
        .zip(layer_strings)
        .map(|(name, cfg_text)| LayerInfo { name, cfg_text })
        .collect();

    let alias_exprs = root_exprs
        .iter()
        .filter(gen_first_atom_filter("defalias"))
        .collect::<Vec<_>>();
    let defsrc_layer = parse_defsrc_layer(src_expr, &mapping_order);
    let mut parsed_state = ParsedState {
        layer_exprs,
        layer_idxs,
        mapping_order,
        defsrc_layer,
        is_cmd_enabled: {
            #[cfg(feature = "cmd")]
            {
                cfg.get("danger-enable-cmd").map_or(false, |s| {
                    if s == "yes" {
                        log::warn!("DANGER! cmd action is enabled.");
                        true
                    } else {
                        false
                    }
                })
            }
            #[cfg(not(feature = "cmd"))]
            {
                log::info!("NOTE: kanata was compiled to never allow cmd");
                false
            }
        },
        ..Default::default()
    };

    let chords_exprs = root_exprs
        .iter()
        .filter(gen_first_atom_filter("defchords"))
        .collect::<Vec<_>>();
    parse_chord_groups(&chords_exprs, &mut parsed_state)?;

    let fake_keys_exprs = root_exprs
        .iter()
        .filter(gen_first_atom_filter("deffakekeys"))
        .collect::<Vec<_>>();
    parse_fake_keys(&fake_keys_exprs, &mut parsed_state)?;

    let sequence_exprs = root_exprs
        .iter()
        .filter(gen_first_atom_filter("defseq"))
        .collect::<Vec<_>>();
    let sequences = parse_sequences(&sequence_exprs, &parsed_state)?;

    parse_aliases(&alias_exprs, &mut parsed_state)?;

    let mut klayers = parse_layers(&parsed_state)?;

    resolve_chord_groups(&mut klayers, &mut parsed_state)?;

    Ok((cfg, src, layer_info, klayers, sequences))
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

/// Consumes the first element and returns the rest of the iterator. Returns `Ok` if the first
/// element is an atom and equals `expected_first`.
fn check_first_expr<'a>(
    mut exprs: impl Iterator<Item = &'a SExpr>,
    expected_first: &str,
) -> Result<impl Iterator<Item = &'a SExpr>> {
    let first_atom = exprs
        .next()
        .ok_or_else(|| anyhow!("Passed empty list to {expected_first}"))?
        .atom()
        .ok_or_else(|| anyhow!("First entry is expected to be an atom for {expected_first}"))?;
    if first_atom != expected_first {
        bail!("Passed non-{expected_first} expression to {expected_first}: {first_atom}");
    }
    Ok(exprs)
}

/// Parse configuration entries from an expression starting with defcfg.
fn parse_defcfg(expr: &[SExpr]) -> Result<HashMap<String, String>> {
    let mut cfg = HashMap::default();
    let mut exprs = check_first_expr(expr.iter(), "defcfg")?;
    // Read k-v pairs from the configuration
    loop {
        let key = match exprs.next() {
            Some(k) => k,
            None => return Ok(cfg),
        };
        let val = match exprs.next() {
            Some(v) => v,
            None => bail!("Incorrect number of elements found in defcfg; they should be pairs of keys and values."),
        };
        match (&key, &val) {
            (SExpr::Atom(k), SExpr::Atom(v)) => {
                if cfg
                    .insert(
                        k.t.trim_matches('"').to_owned(),
                        v.t.trim_matches('"').to_owned(),
                    )
                    .is_some()
                {
                    bail!("duplicate cfg entries for key {}", k.t);
                }
            }
            (_, _) => {
                bail!(
                    "defcfg should only be composed of atoms. Incorrect (k,v) found: {:?},{:?}",
                    key,
                    val
                );
            }
        }
    }
}

/// Parse custom keys from an expression starting with deflocalkeys. Statefully updates the `keys`
/// module using the custom keys parsed.
fn parse_deflocalkeys(expr: &[SExpr]) -> Result<()> {
    let mut cfg = HashMap::default();
    let mut exprs = check_first_expr(expr.iter(), DEF_LOCAL_KEYS)?;
    clear_custom_str_oscode_mapping();
    // Read k-v pairs from the configuration
    loop {
        let key = match exprs.next() {
            Some(k) => k
                .atom()
                .ok_or_else(|| anyhow!("{DEF_LOCAL_KEYS} contains a list; no lists are allowed"))?,
            None => {
                replace_custom_str_oscode_mapping(&cfg);
                return Ok(());
            }
        };
        if str_to_oscode(key).is_some() {
            bail!("{key} is a default key name in kanata; it cannot be used in {DEF_LOCAL_KEYS}");
        } else if cfg.contains_key(key) {
            bail!(
                "{key} has been defined more than once in {DEF_LOCAL_KEYS}; no duplicates are allowed"
            );
        }
        let osc = match exprs.next() {
            Some(v) => v.atom()
                .ok_or_else(|| anyhow!("{DEF_LOCAL_KEYS} contains a list; no lists are allowed"))
                .and_then(|osc| osc.parse::<u16>().map_err(|_| anyhow!("{DEF_LOCAL_KEYS} unknown number {osc}")))
                .and_then(|osc| OsCode::from_u16(osc).ok_or_else(|| anyhow!("{DEF_LOCAL_KEYS} unknown number {osc}")))?,
            None => bail!("Incorrect number of elements found in {DEF_LOCAL_KEYS}; they should be pairs of keys and numbers."),
        };
        log::debug!("custom mapping: {key} {}", osc.as_u16());
        cfg.insert(key.to_owned(), osc);
    }
}

/// Parse mapped keys from an expression starting with defsrc. Returns the key mapping as well as
/// a vec of the indexes in order. The length of the returned vec should be matched by the length
/// of all layer declarations.
fn parse_defsrc(
    expr: &[SExpr],
    defcfg: &HashMap<String, String>,
) -> Result<(MappedKeys, Vec<usize>)> {
    let exprs = check_first_expr(expr.iter(), "defsrc")?;
    let mut mkeys = MappedKeys::default();
    let mut ordered_codes = Vec::new();
    for expr in exprs {
        let s = match expr {
            SExpr::Atom(a) => &a.t,
            _ => bail!("No lists allowed in defsrc"),
        };
        let oscode = str_to_oscode(s).ok_or_else(|| anyhow!("Unknown key in defsrc: \"{}\"", s))?;
        if mkeys.contains(&oscode) {
            bail!("Repeat declaration of key in defsrc: \"{}\"", s)
        }
        mkeys.insert(oscode);
        ordered_codes.push(oscode.into());
    }

    let process_unmapped_keys = defcfg
        .get("process-unmapped-keys")
        .map(|text| matches!(text.to_lowercase().as_str(), "true" | "yes"))
        .unwrap_or(false);
    log::info!("process unmapped keys: {process_unmapped_keys}");
    if process_unmapped_keys {
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

/// Returns layer names and their indexes into the keyberon layout. This also checks that all
/// layers have the same number of items as the defsrc.
fn parse_layer_indexes(exprs: &[&Vec<SExpr>], expected_len: usize) -> Result<LayerIndexes> {
    let mut layer_indexes = HashMap::default();
    for (i, expr) in exprs.iter().enumerate() {
        let mut subexprs = check_first_expr(expr.iter(), "deflayer")?;
        let layer_name = subexprs
            .next()
            .ok_or_else(|| anyhow!("deflayer requires a name and keys"))?
            .atom()
            .ok_or_else(|| anyhow!("layer name after deflayer must be an atom"))?
            .to_owned();
        let num_actions = subexprs.count();
        if num_actions != expected_len {
            bail!(
                "layer {} has {} items, but requires {} to match defsrc",
                layer_name,
                num_actions,
                expected_len
            )
        }
        layer_indexes.insert(layer_name, i);
    }

    Ok(layer_indexes)
}

#[derive(Debug)]
struct ParsedState<'a> {
    layer_exprs: Vec<&'a Vec<SExpr>>,
    aliases: Aliases,
    layer_idxs: LayerIndexes,
    mapping_order: Vec<usize>,
    fake_keys: HashMap<String, (usize, &'static KanataAction)>,
    chord_groups: HashMap<String, ChordGroup<'a>>,
    defsrc_layer: [KanataAction; KEYS_IN_ROW],
    is_cmd_enabled: bool,
}

impl<'a> Default for ParsedState<'a> {
    fn default() -> Self {
        Self {
            layer_exprs: Default::default(),
            aliases: Default::default(),
            layer_idxs: Default::default(),
            mapping_order: Default::default(),
            defsrc_layer: [KanataAction::Trans; KEYS_IN_ROW],
            fake_keys: Default::default(),
            chord_groups: Default::default(),
            is_cmd_enabled: false,
        }
    }
}

#[derive(Debug, Clone)]
struct ChordGroup<'a> {
    id: u16,
    name: String,
    keys: Vec<String>,
    coords: Vec<((u8, u16), ChordKeys)>,
    chords: HashMap<u32, &'a SExpr>,
    timeout: u16,
}

/// Parse alias->action mappings from multiple exprs starting with defalias.
/// Mutates the input `parsed_state` by storing aliases inside.
fn parse_aliases(exprs: &[&Vec<SExpr>], parsed_state: &mut ParsedState) -> Result<()> {
    for expr in exprs {
        let mut subexprs = check_first_expr(expr.iter(), "defalias")?;
        // Read k-v pairs from the configuration
        while let Some(alias) = subexprs.next() {
            let action = match subexprs.next() {
                Some(v) => v,
                None => bail!("Incorrect number of elements found in defalias; they should be pairs of aliases and actions."),
            };
            let alias = match alias {
                SExpr::Atom(a) => &a.t,
                _ => bail!("Alias keys must be atoms. Invalid alias: {:?}", alias),
            };
            let action = parse_action(action, parsed_state)?;
            if parsed_state.aliases.insert(alias.into(), action).is_some() {
                bail!("Duplicate alias: {}", alias);
            }
        }
    }
    Ok(())
}

/// Returns a `&'static T` by leaking a box.
fn sref<T>(v: T) -> &'static T {
    Box::leak(Box::new(v))
}

/// Returns a `&'static [&'static T]` by leaking a box + boxed array
fn sref_slice<T>(v: T) -> &'static [&'static T] {
    Box::leak(vec![sref(v)].into_boxed_slice())
}

/// Parse a `kanata_keyberon::action::Action` from a `SExpr`.
fn parse_action(expr: &SExpr, parsed_state: &ParsedState) -> Result<&'static KanataAction> {
    match expr {
        SExpr::Atom(a) => parse_action_atom(a, &parsed_state.aliases),
        SExpr::List(l) => parse_action_list(&l.t, parsed_state),
    }
}

/// Parse a `kanata_keyberon::action::Action` from a string.
fn parse_action_atom(ac: &Spanned<String>, aliases: &Aliases) -> Result<&'static KanataAction> {
    let ac = &*ac.t;
    match ac {
        "_" => return Ok(sref(Action::Trans)),
        "XX" => return Ok(sref(Action::NoOp)),
        "lrld" => return Ok(sref(Action::Custom(sref_slice(CustomAction::LiveReload)))),
        "sldr" => {
            return Ok(sref(Action::Custom(sref_slice(
                CustomAction::SequenceLeader,
            ))))
        }
        "mlft" | "mouseleft" => {
            return Ok(sref(Action::Custom(sref_slice(CustomAction::Mouse(
                Btn::Left,
            )))))
        }
        "mrgt" | "mouseright" => {
            return Ok(sref(Action::Custom(sref_slice(CustomAction::Mouse(
                Btn::Right,
            )))))
        }
        "mmid" | "mousemid" => {
            return Ok(sref(Action::Custom(sref_slice(CustomAction::Mouse(
                Btn::Mid,
            )))))
        }
        "mfwd" | "mouseforward" => {
            return Ok(sref(Action::Custom(sref_slice(CustomAction::Mouse(
                Btn::Forward,
            )))))
        }
        "mbck" | "mousebackward" => {
            return Ok(sref(Action::Custom(sref_slice(CustomAction::Mouse(
                Btn::Backward,
            )))))
        }
        "mltp" | "mousetapleft" => {
            return Ok(sref(Action::Custom(sref_slice(CustomAction::MouseTap(
                Btn::Left,
            )))))
        }
        "mrtp" | "mousetapright" => {
            return Ok(sref(Action::Custom(sref_slice(CustomAction::MouseTap(
                Btn::Right,
            )))))
        }
        "mmtp" | "mousetapmid" => {
            return Ok(sref(Action::Custom(sref_slice(CustomAction::MouseTap(
                Btn::Mid,
            )))))
        }
        "mftp" | "mousetapforward" => {
            return Ok(sref(Action::Custom(sref_slice(CustomAction::MouseTap(
                Btn::Forward,
            )))))
        }
        "mbtp" | "mousetapbackward" => {
            return Ok(sref(Action::Custom(sref_slice(CustomAction::MouseTap(
                Btn::Backward,
            )))))
        }
        "rpt" | "repeat" => return Ok(sref(Action::Custom(sref_slice(CustomAction::Repeat)))),
        "dynamic-macro-record-stop" => {
            return Ok(sref(Action::Custom(sref_slice(
                CustomAction::DynamicMacroRecordStop,
            ))))
        }
        _ => {}
    };
    if let Some(oscode) = str_to_oscode(ac) {
        return Ok(sref(k(oscode.into())));
    }
    if let Some(alias) = ac.strip_prefix('@') {
        return match aliases.get(alias) {
            Some(ac) => Ok(*ac),
            None => bail!(
                "Referenced unknown alias {}. Note that order of declarations matter.",
                alias
            ),
        };
    }

    // Parse a sequence like `C-S-v` or `C-A-del`
    let (mut keys, unparsed_str) = parse_mod_prefix(ac)?;
    keys.push(
        str_to_oscode(unparsed_str)
            .ok_or_else(|| anyhow!("Could not parse: {ac:?}"))?
            .into(),
    );
    Ok(sref(Action::MultipleKeyCodes(sref(keys).as_ref())))
}

/// Parse a `kanata_keyberon::action::Action` from a `SExpr::List`.
fn parse_action_list(ac: &[SExpr], parsed_state: &ParsedState) -> Result<&'static KanataAction> {
    if ac.is_empty() {
        return Ok(sref(Action::NoOp));
    }
    let ac_type = match &ac[0] {
        SExpr::Atom(a) => &a.t,
        _ => bail!("Action list must start with an atom"),
    };
    let layers = &parsed_state.layer_idxs;
    match ac_type.as_str() {
        "layer-switch" => parse_layer_base(&ac[1..], layers),
        "layer-toggle" | "layer-while-held" => parse_layer_toggle(&ac[1..], layers),
        "tap-hold" => parse_tap_hold(&ac[1..], parsed_state, HoldTapConfig::Default),
        "tap-hold-press" => parse_tap_hold(&ac[1..], parsed_state, HoldTapConfig::HoldOnOtherKeyPress),
        "tap-hold-release" => parse_tap_hold(&ac[1..], parsed_state, HoldTapConfig::PermissiveHold),
        "multi" => parse_multi(&ac[1..], parsed_state),
        "macro" => parse_macro(&ac[1..], parsed_state),
        "macro-release-cancel" => parse_macro_release_cancel(&ac[1..], parsed_state),
        "unicode" => parse_unicode(&ac[1..]),
        "one-shot" => parse_one_shot(&ac[1..], parsed_state),
        "tap-dance" => parse_tap_dance(&ac[1..], parsed_state, TapDanceConfig::Lazy),
        "tap-dance-eager" => parse_tap_dance(&ac[1..], parsed_state, TapDanceConfig::Eager),
        "chord" => parse_chord(&ac[1..], parsed_state),
        "release-key" => parse_release_key(&ac[1..], parsed_state),
        "release-layer" => parse_release_layer(&ac[1..], parsed_state),
        "on-press-fakekey" => parse_fake_key_op(&ac[1..], parsed_state),
        "on-release-fakekey" => parse_on_release_fake_key_op(&ac[1..], parsed_state),
        "on-press-fakekey-delay" => parse_fake_key_delay(&ac[1..]),
        "on-release-fakekey-delay" => parse_on_release_fake_key_delay(&ac[1..]),
        "mwheel-up" => parse_mwheel(&ac[1..], MWheelDirection::Up),
        "mwheel-down" => parse_mwheel(&ac[1..], MWheelDirection::Down),
        "mwheel-left" => parse_mwheel(&ac[1..], MWheelDirection::Left),
        "mwheel-right" => parse_mwheel(&ac[1..], MWheelDirection::Right),
        "movemouse-up" => parse_move_mouse(&ac[1..], MoveDirection::Up),
        "movemouse-down" => parse_move_mouse(&ac[1..], MoveDirection::Down),
        "movemouse-left" => parse_move_mouse(&ac[1..], MoveDirection::Left),
        "movemouse-right" => parse_move_mouse(&ac[1..], MoveDirection::Right),
        "movemouse-accel-up" => parse_move_mouse_accel(&ac[1..], MoveDirection::Up),
        "movemouse-accel-down" => parse_move_mouse_accel(&ac[1..], MoveDirection::Down),
        "movemouse-accel-left" => parse_move_mouse_accel(&ac[1..], MoveDirection::Left),
        "movemouse-accel-right" => parse_move_mouse_accel(&ac[1..], MoveDirection::Right),
        "dynamic-macro-record" => parse_dynamic_macro_record(&ac[1..]),
        "dynamic-macro-play" => parse_dynamic_macro_play(&ac[1..]),
        "cmd" => parse_cmd(&ac[1..], parsed_state.is_cmd_enabled),
        _ => bail!(
            "Unknown action type: {}. Valid types:\n\tlayer-switch\n\tlayer-toggle | layer-while-held\n\ttap-hold | tap-hold-press | tap-hold-release\n\tmulti\n\tmacro\n\tunicode\n\tone-shot\n\ttap-dance\n\trelease-key | release-layer\n\tmwheel-up | mwheel-down | mwheel-left | mwheel-right\n\ton-press-fakekey | on-release-fakekey\n\ton-press-fakekey-delay | on-release-fakekey-delay\n\tcmd",
            ac_type
        ),
    }
}

fn parse_layer_base(ac_params: &[SExpr], layers: &LayerIndexes) -> Result<&'static KanataAction> {
    Ok(sref(Action::DefaultLayer(
        layer_idx(ac_params, layers)? * 2,
    )))
}

fn parse_layer_toggle(ac_params: &[SExpr], layers: &LayerIndexes) -> Result<&'static KanataAction> {
    Ok(sref(Action::Layer(layer_idx(ac_params, layers)? * 2 + 1)))
}

fn layer_idx(ac_params: &[SExpr], layers: &LayerIndexes) -> Result<usize> {
    if ac_params.len() != 1 {
        bail!(
            "layer actions expect one atom: the layer name. Incorrect value: {:?}",
            ac_params
        )
    }
    let layer_name = match &ac_params[0] {
        SExpr::Atom(ln) => &ln.t,
        _ => bail!(
            "layer name should be an atom, not a list: {:?}",
            ac_params[0]
        ),
    };
    match layers.get(layer_name) {
        Some(i) => Ok(*i),
        None => bail!("layer name {} is not declared in any deflayer", layer_name),
    }
}

fn parse_tap_hold(
    ac_params: &[SExpr],
    parsed_state: &ParsedState,
    config: HoldTapConfig,
) -> Result<&'static KanataAction> {
    if ac_params.len() != 4 {
        bail!("tap-hold expects 4 atoms after it: <tap-timeout> <hold-timeout> <tap-action> <hold-action>, got {}", ac_params.len())
    }
    let tap_timeout =
        parse_timeout(&ac_params[0]).map_err(|e| anyhow!("invalid tap-timeout: {}", e))?;
    let hold_timeout =
        parse_timeout(&ac_params[1]).map_err(|e| anyhow!("invalid tap-timeout: {}", e))?;
    let tap_action = parse_action(&ac_params[2], parsed_state)?;
    let hold_action = parse_action(&ac_params[3], parsed_state)?;
    if matches!(tap_action, Action::HoldTap { .. }) {
        bail!("tap-hold does not work in the tap-action of tap-hold")
    }
    Ok(sref(Action::HoldTap(sref(HoldTapAction {
        config,
        tap_hold_interval: tap_timeout,
        timeout: hold_timeout,
        tap: *tap_action,
        hold: *hold_action,
    }))))
}

fn parse_timeout(a: &SExpr) -> Result<u16> {
    match a {
        SExpr::Atom(a) => a.t.parse().map_err(|e| anyhow!("expected integer: {}", e)),
        _ => bail!("expected atom, not list for integer"),
    }
}

fn parse_multi(ac_params: &[SExpr], parsed_state: &ParsedState) -> Result<&'static KanataAction> {
    if ac_params.is_empty() {
        bail!("multi expects at least one atom after it")
    }
    let mut actions = Vec::new();
    let mut custom_actions: Vec<&'static CustomAction> = Vec::new();
    for expr in ac_params {
        let ac = parse_action(expr, parsed_state)?;
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
        actions.push(Action::Custom(Box::leak(custom_actions.into_boxed_slice())));
    }

    Ok(sref(Action::MultipleActions(sref(actions))))
}

#[test]
fn recursive_multi_is_flattened() {
    macro_rules! atom {
        ($e:expr) => {
            SExpr::Atom(Spanned::new($e.into(), Span::default()))
        };
    }
    macro_rules! list {
        ($e:tt) => {
            SExpr::List(Spanned::new(vec! $e, Span::default()))
        };
    }
    use sexpr::*;
    let params = [
        atom!("a"),
        atom!("mmtp"),
        list!([
            atom!("multi"),
            atom!("b"),
            atom!("mltp"),
            list!([atom!("multi"), atom!("c"), atom!("mrtp"),])
        ]),
    ];
    let parsed_state = ParsedState::default();
    if let KanataAction::MultipleActions(parsed_multi) =
        parse_multi(&params, &parsed_state).unwrap()
    {
        assert_eq!(parsed_multi.len(), 4);
        assert_eq!(parsed_multi[0], Action::KeyCode(KeyCode::A));
        assert_eq!(parsed_multi[1], Action::KeyCode(KeyCode::B));
        assert_eq!(parsed_multi[2], Action::KeyCode(KeyCode::C));
        assert_eq!(
            parsed_multi[3],
            Action::Custom(
                &[
                    &CustomAction::MouseTap(Btn::Mid),
                    &CustomAction::MouseTap(Btn::Left),
                    &CustomAction::MouseTap(Btn::Right),
                ][..]
            )
        );
    } else {
        panic!("multi did not parse into multi");
    }
}

const MACRO_ERR: &str =
    "Action \"macro\" only accepts delays, keys, chords, and chorded sub-macros";

fn parse_macro(ac_params: &[SExpr], parsed_state: &ParsedState) -> Result<&'static KanataAction> {
    if ac_params.is_empty() {
        bail!("macro expects at least one atom after it")
    }
    let mut all_events = Vec::new();
    let mut events;
    let mut params_remainder = ac_params;
    while !params_remainder.is_empty() {
        (events, params_remainder) = parse_macro_item(params_remainder, parsed_state)?;
        all_events.append(&mut events);
    }
    Ok(sref(Action::Sequence {
        events: sref(all_events),
    }))
}

fn parse_macro_release_cancel(
    ac_params: &[SExpr],
    parsed_state: &ParsedState,
) -> Result<&'static KanataAction> {
    let macro_action = parse_macro(ac_params, parsed_state)?;
    Ok(sref(Action::MultipleActions(sref(vec![
        *macro_action,
        Action::Custom(sref_slice(CustomAction::CancelMacroOnRelease)),
    ]))))
}

#[allow(clippy::type_complexity)] // return type is not pub
fn parse_macro_item<'a>(
    acs: &'a [SExpr],
    parsed_state: &ParsedState,
) -> Result<(
    Vec<SequenceEvent<&'static [&'static CustomAction]>>,
    &'a [SExpr],
)> {
    if let Ok(duration) = parse_timeout(&acs[0]) {
        let duration = u32::from(duration);
        return Ok((vec![SequenceEvent::Delay { duration }], &acs[1..]));
    }
    match parse_action(&acs[0], parsed_state) {
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
            // Try to parse a chorded sub-macro, e.g. S-(tab tab tab)
            if acs.len() < 2 {
                bail!("{MACRO_ERR}. Invalid value {acs:?}");
            }
            let held_mods = parse_mods_held_for_submacro(&acs[0])?;
            let mut all_events = vec![];

            // First, press all of the modifiers
            for kc in held_mods.iter().copied() {
                all_events.push(SequenceEvent::Press(kc));
            }

            // Do the submacro
            let submacro = acs[1]
                .list()
                .ok_or_else(|| anyhow!("{MACRO_ERR}. Invalid value: {acs:?}"))?;
            let mut submacro_remainder = submacro;
            let mut events;
            while !submacro_remainder.is_empty() {
                (events, submacro_remainder) = parse_macro_item(submacro_remainder, parsed_state)?;
                all_events.append(&mut events);
            }

            // Lastly, release modifiers
            for kc in held_mods.iter().copied() {
                all_events.push(SequenceEvent::Release(kc));
            }

            Ok((all_events, &acs[2..]))
        }
    }
}

/// Parses mod keys like `C-S-`. There must be no remaining text after the prefixes.
fn parse_mods_held_for_submacro(held_mods: &SExpr) -> Result<Vec<KeyCode>> {
    let mods = held_mods
        .atom()
        .ok_or_else(|| anyhow!("{MACRO_ERR}. Invalid value: {held_mods:?}"))?;
    let (mod_keys, unparsed_str) = parse_mod_prefix(mods)?;
    if !unparsed_str.is_empty() {
        bail!("{MACRO_ERR}. Invalid value: {held_mods:?}");
    }
    Ok(mod_keys)
}

/// Parses mod keys like `C-S-`. There must be no remaining text after the prefixes. Returns the
/// `KeyCode`s for the modifiers parsed and the unparsed text after any parsed modifier prefixes.
fn parse_mod_prefix(mods: &str) -> Result<(Vec<KeyCode>, &str)> {
    let mut key_stack = Vec::new();
    let mut rem = mods;
    loop {
        if let Some(rest) = rem.strip_prefix("C-") {
            if key_stack.contains(&KeyCode::LCtrl) {
                bail!("Redundant \"C-\" in {mods:?}");
            }
            key_stack.push(KeyCode::LCtrl);
            rem = rest;
        } else if let Some(rest) = rem.strip_prefix("S-") {
            if key_stack.contains(&KeyCode::LShift) {
                bail!("Redundant \"S-\" in {mods:?}");
            }
            key_stack.push(KeyCode::LShift);
            rem = rest;
        } else if let Some(rest) = rem.strip_prefix("AG-") {
            if key_stack.contains(&KeyCode::RAlt) {
                bail!("Redundant \"AltGr\" in {mods:?}");
            }
            key_stack.push(KeyCode::RAlt);
            rem = rest;
        } else if let Some(rest) = rem.strip_prefix("RA-") {
            if key_stack.contains(&KeyCode::RAlt) {
                bail!("Redundant \"AltGr\" in {mods:?}");
            }
            key_stack.push(KeyCode::RAlt);
            rem = rest;
        } else if let Some(rest) = rem.strip_prefix("A-") {
            if key_stack.contains(&KeyCode::LAlt) {
                bail!("Redundant \"A-\" in {mods:?}");
            }
            key_stack.push(KeyCode::LAlt);
            rem = rest;
        } else if let Some(rest) = rem.strip_prefix("M-") {
            if key_stack.contains(&KeyCode::LGui) {
                bail!("Redundant \"M-\" in {mods:?}");
            }
            key_stack.push(KeyCode::LGui);
            rem = rest;
        } else {
            break;
        }
    }
    Ok((key_stack, rem))
}

fn parse_unicode(ac_params: &[SExpr]) -> Result<&'static KanataAction> {
    const ERR_STR: &str = "unicode expects exactly one unicode character as an argument";
    if ac_params.len() != 1 {
        bail!(ERR_STR)
    }
    match &ac_params[0] {
        SExpr::Atom(s) => {
            if s.t.chars().count() != 1 {
                bail!(ERR_STR)
            }
            Ok(sref(Action::Custom(sref_slice(CustomAction::Unicode(
                s.t.chars().next().unwrap(),
            )))))
        }
        _ => bail!(ERR_STR),
    }
}

fn parse_cmd(ac_params: &[SExpr], is_cmd_enabled: bool) -> Result<&'static KanataAction> {
    const ERR_STR: &str = "cmd expects one or more strings";
    if !is_cmd_enabled {
        bail!("cmd is not enabled but cmd action is specified somewhere");
    }
    if ac_params.is_empty() {
        bail!(ERR_STR);
    }
    Ok(sref(Action::Custom(sref_slice(CustomAction::Cmd(
        Box::leak(
            ac_params
                .iter()
                .try_fold(vec![], |mut v, p| {
                    if let SExpr::Atom(s) = p {
                        v.push(s.t.trim_matches('"').to_owned());
                        Ok(v)
                    } else {
                        bail!("{}, found a list", ERR_STR);
                    }
                })?
                .into_boxed_slice(),
        ),
    )))))
}

fn parse_one_shot(
    ac_params: &[SExpr],
    parsed_state: &ParsedState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "one-shot expects a timeout (number) followed by an action";
    if ac_params.len() != 2 {
        bail!(ERR_MSG);
    }

    use std::str::FromStr;
    let timeout = match &ac_params[0] {
        SExpr::Atom(s) => match u16::from_str(&s.t) {
            Ok(t) => t,
            Err(e) => {
                log::error!("{}", e);
                bail!(ERR_MSG);
            }
        },
        _ => bail!(ERR_MSG),
    };

    let action = parse_action(&ac_params[1], parsed_state)?;
    if !matches!(
        action,
        Action::Layer(..) | Action::KeyCode(..) | Action::MultipleKeyCodes(..)
    ) {
        dbg!(action);
        bail!("one-shot is only allowed to contain layer-toggle, a keycode, or a chord");
    }

    let end_config = OneShotEndConfig::EndOnFirstPress;
    Ok(sref(Action::OneShot(sref(OneShot {
        timeout,
        action,
        end_config,
    }))))
}

fn parse_tap_dance(
    ac_params: &[SExpr],
    parsed_state: &ParsedState,
    config: TapDanceConfig,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "tap-dance expects a timeout (number) followed by a list of actions";
    if ac_params.len() != 2 {
        bail!(ERR_MSG);
    }

    use std::str::FromStr;
    let timeout = match &ac_params[0] {
        SExpr::Atom(s) => match u16::from_str(&s.t) {
            Ok(t) => t,
            Err(e) => {
                log::error!("{}", e);
                bail!(ERR_MSG);
            }
        },
        _ => bail!(ERR_MSG),
    };
    let actions = match &ac_params[1] {
        SExpr::List(tap_dance_actions) => {
            let mut actions = Vec::new();
            for expr in &tap_dance_actions.t {
                let ac = parse_action(expr, parsed_state)?;
                actions.push(ac);
            }
            sref(actions.into_boxed_slice())
        }
        _ => bail!(ERR_MSG),
    };

    Ok(sref(Action::TapDance(sref(TapDance {
        timeout,
        actions,
        config,
    }))))
}

fn parse_chord(ac_params: &[SExpr], parsed_state: &ParsedState) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "chord expects a chords group name followed by an identifier";
    if ac_params.len() != 2 {
        bail!(ERR_MSG);
    }

    let name = match &ac_params[0] {
        SExpr::Atom(s) => &s.t,
        _ => bail!(ERR_MSG),
    };
    let group = match parsed_state.chord_groups.get(name) {
        Some(t) => t,
        None => bail!("Referenced unknown chord group `{}`.", name),
    };
    let chord_key_index = match &ac_params[1] {
        SExpr::Atom(s) => match group.keys.iter().position(|e| e == &s.t) {
            Some(i) => i,
            None => bail!("Identifier `{}` is not used in chord group `{}`.", &s.t, name,),
        },
        _ => bail!(ERR_MSG),
    };
    let chord_keys = 1 << chord_key_index;

    // We don't yet know at this point what the entire chords group will look like nor at which
    // coords this action will end up. So instead we store a dummy action which will be properly
    // resolved in `resolve_chord_groups`.
    Ok(sref(Action::Chords(sref(ChordsGroup {
        timeout: group.timeout,
        coords: sref([((0, group.id), chord_keys)]),
        chords: sref([]),
    }))))
}

fn parse_release_key(
    ac_params: &[SExpr],
    parsed_state: &ParsedState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "release-key expects exactly one keycode (e.g. lalt)";
    if ac_params.len() != 1 {
        bail!(ERR_MSG);
    }
    let ac = parse_action(&ac_params[0], parsed_state)?;
    match ac {
        Action::KeyCode(kc) => Ok(sref(Action::ReleaseState(ReleasableState::KeyCode(*kc)))),
        _ => bail!("{}, got {:?}", ERR_MSG, ac),
    }
}

fn parse_release_layer(
    ac_params: &[SExpr],
    parsed_state: &ParsedState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "release-key expects exactly one layer name (e.g. arrows)";
    if ac_params.len() != 1 {
        bail!(ERR_MSG);
    }
    Ok(sref(Action::ReleaseState(ReleasableState::Layer(
        layer_idx(ac_params, &parsed_state.layer_idxs)? * 2 + 1,
    ))))
}

fn parse_defsrc_layer(defsrc: &[SExpr], mapping_order: &[usize]) -> [KanataAction; KEYS_IN_ROW] {
    let mut layer = [KanataAction::Trans; KEYS_IN_ROW];

    // These can be default (empty) since the defsrc layer definitely won't use it.
    for (i, ac) in defsrc.iter().skip(1).enumerate() {
        let ac = parse_action(ac, &Default::default()).unwrap();
        layer[mapping_order[i]] = *ac;
    }
    layer
}

fn parse_chord_groups<'a>(
    exprs: &[&'a Vec<SExpr>],
    parsed_state: &mut ParsedState<'a>,
) -> Result<()> {
    const MSG: &str = "Incorrect number of elements found in defchords; should be group name, followed by timeout, followed by keys-action pairs.";
    for expr in exprs {
        let mut subexprs = check_first_expr(expr.iter(), "defchords")?;
        let name = match subexprs.next() {
            Some(SExpr::Atom(a)) => &a.t,
            _ => bail!(MSG),
        };
        let timeout = match subexprs.next() {
            Some(e) => parse_timeout(e)?,
            None => bail!(MSG),
        };
        let id = match parsed_state.chord_groups.len().try_into() {
            Ok(id) => id,
            Err(_) => bail!("Maximum number of chord groups exceeded."),
        };
        let mut group = ChordGroup {
            id,
            name: name.to_owned(),
            keys: Vec::new(),
            coords: Vec::new(),
            chords: HashMap::default(),
            timeout,
        };
        // Read k-v pairs from the configuration
        while let Some(keys_expr) = subexprs.next() {
            let action = match subexprs.next() {
                Some(v) => v,
                None => bail!(MSG),
            };
            let mut keys = match keys_expr {
                SExpr::List(keys) => keys.t.iter().map(|key| match key {
                    SExpr::Atom(a) => Ok(&a.t),
                    _ => bail!("Chord keys must be atoms. Invalid key name: {:?}", key),
                }),
                _ => bail!(
                    "Chord must be a list/set of keys, not a single atom: {:?}",
                    keys_expr
                ),
            };
            let mask = keys.try_fold(0, |mask, key| {
                let key = key?;
                let index = match group.keys.iter().position(|k| k == key) {
                    Some(i) => i,
                    None => {
                        let i = group.keys.len();
                        if i + 1 >= MAX_CHORD_KEYS {
                            bail!("Maximum number of keys in a chords group ({MAX_CHORD_KEYS}) exceeded.");
                        }
                        group.keys.push(key.to_owned());
                        i
                    }
                };
                Ok(mask | (1 << index))
            })?;
            if group.chords.insert(mask, action).is_some() {
                bail!("Duplicate chord in group {}: {:?}", name, keys_expr);
            }
        }
        if parsed_state
            .chord_groups
            .insert(name.to_owned(), group)
            .is_some()
        {
            bail!("Duplicate chords group: {}", name);
        }
    }
    Ok(())
}

fn resolve_chord_groups(layers: &mut KanataLayers, parsed_state: &mut ParsedState) -> Result<()> {
    let mut chord_groups = parsed_state
        .chord_groups
        .values()
        .cloned()
        .collect::<Vec<_>>();
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
            if group.coords.iter().find(|(_, keys)| keys & key_mask != 0) == None {
                bail!("Coord group `{0}` defines unused key `{1}`. Did you forget to bind `(chord {0} {1})`?", group.name, key)
            }
        }

        let chords = group.chords.iter().map(|(mask, action)| {
            Ok((*mask, parse_action(action, parsed_state)?))
        }).collect::<Result<Vec<_>>>()?;

        Ok(sref(ChordsGroup {
            coords: Box::leak(group.coords.into_boxed_slice()),
            chords: Box::leak(chords.into_boxed_slice()),
            timeout: group.timeout,
        }))
    }).collect::<Result<Vec<_>>>()?;

    for layer in layers.iter_mut() {
        for row in layer.iter_mut() {
            for cell in row.iter_mut() {
                if let Some(action) = fill_chords(&chord_groups, cell) {
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
        | Action::KeyCode(_)
        | Action::MultipleKeyCodes(_)
        | Action::Layer(_)
        | Action::DefaultLayer(_)
        | Action::Sequence { .. }
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
    }
}

fn fill_chords(
    chord_groups: &[&'static ChordsGroup<&[&CustomAction]>],
    action: &KanataAction,
) -> Option<KanataAction> {
    match action {
        Action::Chords(ChordsGroup { coords, .. }) => {
            let (_, group_id) = coords
                .iter()
                .next()
                .expect("unresolved chords should have exactly one entry");
            Some(Action::Chords(chord_groups[*group_id as usize]))
        }
        Action::NoOp
        | Action::Trans
        | Action::KeyCode(_)
        | Action::MultipleKeyCodes(_)
        | Action::Layer(_)
        | Action::DefaultLayer(_)
        | Action::Sequence { .. }
        | Action::CancelSequences
        | Action::ReleaseState(_)
        | Action::Custom(_) => None,
        Action::HoldTap(hta @ HoldTapAction { tap, hold, .. }) => {
            let new_tap = fill_chords(chord_groups, tap);
            let new_hold = fill_chords(chord_groups, hold);
            if new_tap.is_some() || new_hold.is_some() {
                Some(Action::HoldTap(sref(HoldTapAction {
                    hold: new_hold.unwrap_or_else(|| hold.clone()),
                    tap: new_tap.unwrap_or_else(|| tap.clone()),
                    ..(*hta).clone()
                })))
            } else {
                None
            }
        }
        Action::OneShot(os @ OneShot { action: ac, .. }) => fill_chords(chord_groups, ac).map(|ac| {
            Action::OneShot(sref(OneShot {
                action: sref(ac),
                ..(*os).clone()
            }))
        }),
        Action::MultipleActions(actions) => {
            let new_actions = actions
                .iter()
                .map(|ac| fill_chords(chord_groups, ac))
                .collect::<Vec<_>>();
            if new_actions.iter().any(|it| it.is_some()) {
                let new_actions = new_actions
                    .iter()
                    .zip(*actions)
                    .map(|(new_ac, ac)| new_ac.unwrap_or_else(|| ac.clone()))
                    .collect::<Vec<_>>();
                Some(Action::MultipleActions(Box::leak(
                    new_actions.into_boxed_slice(),
                )))
            } else {
                None
            }
        }
        Action::TapDance(td @ TapDance { actions, .. }) => {
            let new_actions = actions
                .iter()
                .map(|ac| fill_chords(chord_groups, ac))
                .collect::<Vec<_>>();
            if new_actions.iter().any(|it| it.is_some()) {
                let new_actions = new_actions
                    .iter()
                    .zip(*actions)
                    .map(|(new_ac, ac)| new_ac.map(sref).unwrap_or(*ac))
                    .collect::<Vec<_>>();
                Some(Action::TapDance(sref(TapDance {
                    actions: Box::leak(new_actions.into_boxed_slice()),
                    ..(*td).clone()
                })))
            } else {
                None
            }
        }
    }
}

fn parse_fake_keys(exprs: &[&Vec<SExpr>], parsed_state: &mut ParsedState) -> Result<()> {
    for expr in exprs {
        let mut subexprs = check_first_expr(expr.iter(), "deffakekeys")?;
        // Read k-v pairs from the configuration
        while let Some(key_name) = subexprs.next() {
            let action = match subexprs.next() {
                Some(v) => v,
                None => bail!("Incorrect number of elements found in deffakekeys; they should be pairs of key-names and actions."),
            };
            let key_name = match key_name {
                SExpr::Atom(a) => &a.t,
                _ => bail!(
                    "fake key names must be atoms. Invalid key name: {:?}",
                    key_name
                ),
            };
            let action = parse_action(action, parsed_state)?;
            let idx = parsed_state.fake_keys.len();
            log::trace!("inserting {key_name}->{idx}:{action:?}");
            if parsed_state
                .fake_keys
                .insert(key_name.into(), (idx, action))
                .is_some()
            {
                bail!("Duplicate fake key: {}", key_name);
            }
        }
    }
    if parsed_state.fake_keys.len() > KEYS_IN_ROW {
        bail!(
            "Maximum number of fake keys is {KEYS_IN_ROW}, found {}",
            parsed_state.fake_keys.len()
        );
    }
    Ok(())
}

fn parse_fake_key_op(
    ac_params: &[SExpr],
    parsed_state: &ParsedState,
) -> Result<&'static KanataAction> {
    let (coord, action) = parse_fake_key_op_coord_action(ac_params, parsed_state)?;
    Ok(sref(Action::Custom(sref_slice(CustomAction::FakeKey {
        coord,
        action,
    }))))
}

fn parse_on_release_fake_key_op(
    ac_params: &[SExpr],
    parsed_state: &ParsedState,
) -> Result<&'static KanataAction> {
    let (coord, action) = parse_fake_key_op_coord_action(ac_params, parsed_state)?;
    Ok(sref(Action::Custom(sref_slice(
        CustomAction::FakeKeyOnRelease { coord, action },
    ))))
}

fn parse_fake_key_op_coord_action(
    ac_params: &[SExpr],
    parsed_state: &ParsedState,
) -> Result<(Coord, FakeKeyAction)> {
    const ERR_MSG: &str = "fake-key-op expects two parameters: <fake key name> <operation>\n\tvalid operations: tap, press, release";
    if ac_params.len() != 2 {
        bail!("{ERR_MSG}");
    }
    let y = match parsed_state.fake_keys.get(match &ac_params[0] {
        SExpr::Atom(fake_key_name) => &fake_key_name.t,
        _ => bail!(
            "{ERR_MSG}\n\tinvalid first parameter (list): {:?}",
            &ac_params[0]
        ),
    }) {
        Some((y, _)) => *y as u8, // cast should be safe; checked in `parse_fake_keys`
        None => bail!("unknown fake key name {:?}", &ac_params[0]),
    };
    let action = match &ac_params[1] {
        SExpr::Atom(op) => match op.t.as_str() {
            "tap" => FakeKeyAction::Tap,
            "press" => FakeKeyAction::Press,
            "release" => FakeKeyAction::Release,
            _ => bail!("{ERR_MSG}\n\tinvalid second parameter: {:?}", op),
        },
        _ => bail!(
            "{ERR_MSG}\n\tinvalid second parameter (list): {:?}",
            ac_params[1]
        ),
    };
    let (x, y) = get_fake_key_coords(y);
    Ok((Coord { x, y }, action))
}

fn get_fake_key_coords<T: Into<usize>>(y: T) -> (u8, u16) {
    let y: usize = y.into();
    (1, y as u16)
}

fn parse_fake_key_delay(ac_params: &[SExpr]) -> Result<&'static KanataAction> {
    parse_delay(ac_params, false)
}

fn parse_on_release_fake_key_delay(ac_params: &[SExpr]) -> Result<&'static KanataAction> {
    parse_delay(ac_params, true)
}

fn parse_delay(ac_params: &[SExpr], is_release: bool) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "fakekey-delay expects a single number (ms, 0-65535)";
    let delay = ac_params[0]
        .atom()
        .map(str::parse::<u16>)
        .transpose()
        .map_err(|e| anyhow!("{ERR_MSG}: {e}"))?
        .ok_or_else(|| anyhow!("{ERR_MSG}"))?;
    Ok(sref(Action::Custom(sref_slice(match is_release {
        false => CustomAction::Delay(delay),
        true => CustomAction::DelayOnRelease(delay),
    }))))
}

fn parse_mwheel(ac_params: &[SExpr], direction: MWheelDirection) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "mwheel expects two parameters: <interval (ms)> <distance>";
    if ac_params.len() != 2 {
        bail!("{ERR_MSG}");
    }
    let interval = ac_params[0]
        .atom()
        .map(str::parse::<u16>)
        .transpose()
        .map_err(|e| anyhow!("{ERR_MSG}: {e}"))?
        .and_then(|i| match i {
            0 => None,
            _ => Some(i),
        })
        .ok_or_else(|| anyhow!("{ERR_MSG}: interval should be 1-65535"))?;
    let distance = ac_params[1]
        .atom()
        .map(str::parse::<u16>)
        .transpose()
        .map_err(|e| anyhow!("{ERR_MSG}: {e}"))?
        .and_then(|d| match d {
            1..=30000 => Some(d),
            _ => None,
        })
        .ok_or_else(|| anyhow!("{ERR_MSG}: distance should be 1-30000"))?;
    Ok(sref(Action::Custom(sref_slice(CustomAction::MWheel {
        direction,
        interval,
        distance,
    }))))
}

fn parse_move_mouse(
    ac_params: &[SExpr],
    direction: MoveDirection,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "movemouse expects two parameters: <interval (ms)> <distance (px)>";
    if ac_params.len() != 2 {
        bail!("{ERR_MSG}");
    }
    let interval = ac_params[0]
        .atom()
        .map(str::parse::<u16>)
        .transpose()
        .map_err(|e| anyhow!("{ERR_MSG}: {e}"))?
        .and_then(|i| match i {
            0 => None,
            _ => Some(i),
        })
        .ok_or_else(|| anyhow!("{ERR_MSG}: interval should be 1-65535"))?;
    let distance = ac_params[1]
        .atom()
        .map(str::parse::<u16>)
        .transpose()
        .map_err(|e| anyhow!("{ERR_MSG}: {e}"))?
        .and_then(|d| match d {
            1..=30000 => Some(d),
            _ => None,
        })
        .ok_or_else(|| anyhow!("{ERR_MSG}: distance should be 1-30000"))?;
    Ok(sref(Action::Custom(sref_slice(CustomAction::MoveMouse {
        direction,
        interval,
        distance,
    }))))
}

fn parse_move_mouse_accel(
    ac_params: &[SExpr],
    direction: MoveDirection,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "movemouse-accel expects four parameters: <interval (ms)> <acceleration time (ms)> <min_distance> <max_distance>";
    if ac_params.len() != 4 {
        bail!("{ERR_MSG}");
    }
    let interval = ac_params[0]
        .atom()
        .map(str::parse::<u16>)
        .transpose()
        .map_err(|e| anyhow!("{ERR_MSG}: {e}"))?
        .and_then(|i| match i {
            0 => None,
            _ => Some(i),
        })
        .ok_or_else(|| anyhow!("{ERR_MSG}: interval should be 1-65535"))?;
    let accel_time = ac_params[1]
        .atom()
        .map(str::parse::<u16>)
        .transpose()
        .map_err(|e| anyhow!("{ERR_MSG}: {e}"))?
        .ok_or_else(|| anyhow!("{ERR_MSG}: acceleration time should be 0-65535"))?;
    let min_distance = ac_params[2]
        .atom()
        .map(str::parse::<u16>)
        .transpose()
        .map_err(|e| anyhow!("{ERR_MSG}: {e}"))?
        .and_then(|d| match d {
            1..=30000 => Some(d),
            _ => None,
        })
        .ok_or_else(|| anyhow!("{ERR_MSG}: min_distance should be 1-30000"))?;
    let max_distance = ac_params[3]
        .atom()
        .map(str::parse::<u16>)
        .transpose()
        .map_err(|e| anyhow!("{ERR_MSG}: {e}"))?
        .and_then(|d| match d {
            1..=30000 => Some(d),
            _ => None,
        })
        .ok_or_else(|| anyhow!("{ERR_MSG}: max_distance should be 1-30000"))?;
    if min_distance > max_distance {
        return Err(anyhow!(
            "{ERR_MSG}: min_distance should be less than max_distance"
        ));
    }
    Ok(sref(Action::Custom(sref_slice(
        CustomAction::MoveMouseAccel {
            direction,
            interval,
            accel_time,
            min_distance,
            max_distance,
        },
    ))))
}

fn parse_dynamic_macro_record(ac_params: &[SExpr]) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "dynamic-macro-record expects 1 parameter: <macro ID (number 0-65535)>";
    if ac_params.len() != 1 {
        bail!("{ERR_MSG}");
    }
    let key = ac_params[0]
        .atom()
        .map(str::parse::<u16>)
        .transpose()
        .map_err(|e| anyhow!("{ERR_MSG}: {e}"))?
        .ok_or_else(|| anyhow!("{ERR_MSG}: macro ID should be 1-65535"))?;
    return Ok(sref(Action::Custom(sref_slice(
        CustomAction::DynamicMacroRecord(key),
    ))));
}

fn parse_dynamic_macro_play(ac_params: &[SExpr]) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "dynamic-macro-play expects 1 parameter: <macro ID (number 0-65535)>";
    if ac_params.len() != 1 {
        bail!("{ERR_MSG}");
    }
    let key = ac_params[0]
        .atom()
        .map(str::parse::<u16>)
        .transpose()
        .map_err(|e| anyhow!("{ERR_MSG}: {e}"))?
        .ok_or_else(|| anyhow!("{ERR_MSG}: macro ID should be 1-65535"))?;
    return Ok(sref(Action::Custom(sref_slice(
        CustomAction::DynamicMacroPlay(key),
    ))));
}

/// Mutates `layers::LAYERS` using the inputs.
fn parse_layers(parsed_state: &ParsedState) -> Result<Box<KanataLayers>> {
    let mut layers_cfg = new_layers();
    for (layer_level, layer) in parsed_state.layer_exprs.iter().enumerate() {
        // skip deflayer and name
        for (i, ac) in layer.iter().skip(2).enumerate() {
            let ac = parse_action(ac, parsed_state)?;
            layers_cfg[layer_level * 2][0][parsed_state.mapping_order[i]] = *ac;
            layers_cfg[layer_level * 2 + 1][0][parsed_state.mapping_order[i]] = *ac;
        }
        for (i, (layer_action, defsrc_action)) in layers_cfg[layer_level * 2][0]
            .iter_mut()
            .zip(parsed_state.defsrc_layer)
            .enumerate()
        {
            if *layer_action == Action::Trans {
                *layer_action = defsrc_action;
            }
            // If key is unmapped in defsrc as well, default it to the OsCode for that index if the
            // configuration says to do so.
            if *layer_action == Action::Trans {
                *layer_action = OsCode::from_u16(i as u16)
                    .and_then(|osc| match KeyCode::from(osc) {
                        KeyCode::No => None,
                        kc => Some(Action::KeyCode(kc)),
                    })
                    .unwrap_or(Action::Trans);
            }
        }
        for (y, action) in parsed_state.fake_keys.values() {
            let (x, y) = get_fake_key_coords(*y);
            layers_cfg[layer_level * 2][x as usize][y as usize] = **action;
        }
    }
    Ok(layers_cfg)
}

fn parse_sequences(exprs: &[&Vec<SExpr>], parsed_state: &ParsedState) -> Result<KeySeqsToFKeys> {
    const ERR_MSG: &str = "defseq expects pairs of parameters: <fake_key_name> <key_list>";
    let mut sequences = Trie::new();
    for expr in exprs {
        let mut subexprs = check_first_expr(expr.iter(), "defseq")?.peekable();

        while subexprs.peek().is_some() {
            let fake_key = subexprs
                .next()
                .ok_or_else(|| anyhow!(ERR_MSG))?
                .atom()
                .ok_or_else(|| anyhow!("{ERR_MSG}: got a list for fake_key_name"))?;
            if !parsed_state.fake_keys.contains_key(fake_key) {
                bail!("{ERR_MSG}: {fake_key} is not the name of a fake key");
            }
            let key_seq = subexprs
                .next()
                .ok_or_else(|| anyhow!("{ERR_MSG}: missing key_list for {fake_key}"))?
                .list()
                .ok_or_else(|| anyhow!("{ERR_MSG}: got a non-list for key_list"))?;
            let keycode_seq =
                key_seq
                    .iter()
                    .try_fold::<_, _, Result<Vec<_>>>(vec![], |mut keys, key| {
                        keys.push(
                            str_to_oscode(key.atom().ok_or_else(|| {
                                anyhow!("{ERR_MSG}: invalid key in key_list {key:?}")
                            })?)
                            .map(u16::from) // u16 is sufficient for all keys in the keyberon array
                            .ok_or_else(|| anyhow!("{ERR_MSG}: invalid key in key_list {key:?}"))?,
                        );
                        Ok(keys)
                    })?;
            if sequences.get_ancestor(&keycode_seq).is_some() {
                bail!("defseq {fake_key} {key_seq:?} has a conflict: its sequence contains an earlier defined sequence");
            }
            if sequences.get_raw_descendant(&keycode_seq).is_some() {
                bail!("defseq {fake_key} {key_seq:?} has a conflict: its sequence is contained within an earlier defined seqence");
            }
            sequences.insert(
                keycode_seq,
                parsed_state
                    .fake_keys
                    .get(fake_key)
                    .map(|(y, _)| get_fake_key_coords(*y))
                    .unwrap(),
            );
        }
    }
    Ok(sequences)
}

/// Creates a `KeyOutputs` from `layers::LAYERS`.
fn create_key_outputs(layers: &KanataLayers) -> KeyOutputs {
    let mut outs = KeyOutputs::new();
    for layer in layers.iter() {
        let mut layer_outputs = HashMap::default();
        for (i, action) in layer[0].iter().enumerate() {
            let osc_slot = match i.try_into() {
                Ok(i) => i,
                Err(_) => continue,
            };
            add_key_output_from_action_to_key_pos(osc_slot, action, &mut layer_outputs);
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

fn add_key_output_from_action_to_key_pos(
    osc_slot: OsCode,
    action: &KanataAction,
    outputs: &mut HashMap<OsCode, Vec<OsCode>>,
) {
    match action {
        Action::KeyCode(kc) => {
            add_kc_output(osc_slot, kc.into(), outputs);
        }
        Action::HoldTap(HoldTapAction { tap, hold, .. }) => {
            add_key_output_from_action_to_key_pos(osc_slot, tap, outputs);
            add_key_output_from_action_to_key_pos(osc_slot, hold, outputs);
        }
        Action::OneShot(OneShot { action: ac, .. }) => {
            add_key_output_from_action_to_key_pos(osc_slot, ac, outputs);
        }
        Action::MultipleKeyCodes(kcs) => {
            for kc in kcs.iter() {
                add_kc_output(osc_slot, kc.into(), outputs);
            }
        }
        Action::MultipleActions(actions) => {
            for ac in actions.iter() {
                add_key_output_from_action_to_key_pos(osc_slot, ac, outputs);
            }
        }
        Action::TapDance(TapDance { actions, .. }) => {
            for ac in actions.iter() {
                add_key_output_from_action_to_key_pos(osc_slot, ac, outputs);
            }
        }
        Action::Chords(ChordsGroup { chords, .. }) => {
            for (_, ac) in chords.iter() {
                add_key_output_from_action_to_key_pos(osc_slot, ac, outputs);
            }
        }
        Action::NoOp
        | Action::Trans
        | Action::Layer(_)
        | Action::DefaultLayer(_)
        | Action::Sequence { .. }
        | Action::CancelSequences
        | Action::ReleaseState(_)
        | Action::Custom(_) => {}
    };
}

/// Create a layout from `layers::LAYERS`.
fn create_layout(layers: Box<KanataLayers>) -> KanataLayout {
    Layout::new(Box::leak(layers))
}
