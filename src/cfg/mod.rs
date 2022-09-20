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
use std::collections::{HashMap, HashSet};

use kanata_keyberon::action::*;
use kanata_keyberon::key_code::*;
use kanata_keyberon::layout::*;
use sexpr::SExpr;

use self::sexpr::Spanned;

pub type KanataAction = Action<&'static [&'static CustomAction]>;
pub type KanataLayout = Layout<KEYS_IN_ROW, 2, ACTUAL_NUM_LAYERS, &'static [&'static CustomAction]>;
pub type KeySeqsToFKeys = Trie<Vec<u16>, (u8, u16)>;

pub struct Cfg {
    pub mapped_keys: MappedKeys,
    pub key_outputs: KeyOutputs,
    pub layer_info: Vec<LayerInfo>,
    pub items: HashMap<String, String>,
    pub layout: KanataLayout,
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
pub type KeyOutputs = HashMap<OsCode, HashSet<OsCode>>;

fn add_kc_output(i: OsCode, kc: OsCode, outs: &mut KeyOutputs) {
    let outputs = match outs.entry(i) {
        Entry::Occupied(o) => o.into_mut(),
        Entry::Vacant(v) => v.insert(HashSet::new()),
    };
    outputs.insert(kc);
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
    // TODO: get rid of clone
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

    let klayers = parse_layers(&parsed_state)?;

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
    let mut cfg = HashMap::new();
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
                if cfg.insert(k.t.clone(), v.t.clone()).is_some() {
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

/// Parse mapped keys from an expression starting with defsrc. Returns the key mapping as well as
/// a vec of the indexes in order. The length of the returned vec should be matched by the length
/// of all layer declarations.
fn parse_defsrc(
    expr: &[SExpr],
    defcfg: &HashMap<String, String>,
) -> Result<(MappedKeys, Vec<usize>)> {
    let exprs = check_first_expr(expr.iter(), "defsrc")?;
    let mut mkeys = MappedKeys::new();
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
        for osc in 0..KEYS_IN_ROW as u32 {
            if let Some(osc) = OsCode::from_u32(osc) {
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
    let mut layer_indexes = HashMap::new();
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
            is_cmd_enabled: false,
        }
    }
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
        "rpt" | "repeat" => return Ok(sref(Action::Custom(sref_slice(CustomAction::Repeat)))),
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
    let mut rem = ac;
    let mut key_stack = Vec::new();
    loop {
        if let Some(rest) = rem.strip_prefix("C-") {
            if key_stack.contains(&KeyCode::LCtrl) {
                bail!("Redundant \"C-\" in {}", ac)
            }
            key_stack.push(KeyCode::LCtrl);
            rem = rest;
        } else if let Some(rest) = rem.strip_prefix("S-") {
            if key_stack.contains(&KeyCode::LShift) {
                bail!("Redundant \"S-\" in {}", ac)
            }
            key_stack.push(KeyCode::LShift);
            rem = rest;
        } else if let Some(rest) = rem.strip_prefix("AG-") {
            if key_stack.contains(&KeyCode::RAlt) {
                bail!("Redundant \"AltGr\" in {}", ac)
            }
            key_stack.push(KeyCode::RAlt);
            rem = rest;
        } else if let Some(rest) = rem.strip_prefix("RA-") {
            if key_stack.contains(&KeyCode::RAlt) {
                bail!("Redundant \"AltGr\" in {}", ac)
            }
            key_stack.push(KeyCode::RAlt);
            rem = rest;
        } else if let Some(rest) = rem.strip_prefix("A-") {
            if key_stack.contains(&KeyCode::LAlt) {
                bail!("Redundant \"A-\" in {}", ac)
            }
            key_stack.push(KeyCode::LAlt);
            rem = rest;
        } else if let Some(rest) = rem.strip_prefix("M-") {
            if key_stack.contains(&KeyCode::LGui) {
                bail!("Redundant \"M-\" in {}", ac)
            }
            key_stack.push(KeyCode::LGui);
            rem = rest;
        } else if let Some(oscode) = str_to_oscode(rem) {
            key_stack.push(oscode.into());
            return Ok(sref(Action::MultipleKeyCodes(sref(key_stack).as_ref())));
        } else {
            bail!("Could not parse value: {}", ac)
        }
    }
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
        "unicode" => parse_unicode(&ac[1..]),
        "one-shot" => parse_one_shot(&ac[1..], parsed_state),
        "tap-dance" => parse_tap_dance(&ac[1..], parsed_state),
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
            _ => actions.push(*ac),
        }
    }

    if !custom_actions.is_empty() {
        actions.push(Action::Custom(Box::leak(custom_actions.into_boxed_slice())));
    }

    Ok(sref(Action::MultipleActions(sref(actions))))
}

fn parse_macro(ac_params: &[SExpr], parsed_state: &ParsedState) -> Result<&'static KanataAction> {
    if ac_params.is_empty() {
        bail!("macro expects at least one atom after it")
    }
    let mut events = Vec::new();
    for expr in ac_params {
        if let Ok(delay) = parse_timeout(expr) {
            events.push(SequenceEvent::Delay {
                duration: delay.into(),
            });
            continue;
        }
        match parse_action(expr, parsed_state)? {
            Action::KeyCode(kc) => {
                // Should note that I tried `SequenceEvent::Tap` initially but it seems to be buggy
                // so I changed the code to use individual press and release. The SequenceEvent
                // code is from a PR that (at the time of this writing) hasn't yet been merged into
                // keyberon master and doesn't have tests written for it yet. This seems to work as
                // expected right now though.
                events.push(SequenceEvent::Press(*kc));
                events.push(SequenceEvent::Release(*kc));
            }
            Action::MultipleKeyCodes(kcs) => {
                // chord - press in order then release in the reverse order
                for kc in kcs.iter() {
                    events.push(SequenceEvent::Press(*kc));
                }
                for kc in kcs.iter().rev() {
                    events.push(SequenceEvent::Release(*kc));
                }
            }
            _ => {
                bail!(
                    "Action \"macro\" only accepts delays, keys, and chords. Invalid value {:?}",
                    expr
                )
            }
        }
    }
    Ok(sref(Action::Sequence {
        events: sref(events),
    }))
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
                .try_fold(Vec::new(), |mut v, p| {
                    if let SExpr::Atom(s) = p {
                        v.push(s.t.clone());
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

    Ok(sref(Action::TapDance(sref(TapDance { timeout, actions }))))
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
                *layer_action = OsCode::from_u32(i as u32)
                    .and_then(|osc| match KeyCode::from(osc) {
                        KeyCode::No => None,
                        kc => Some(Action::KeyCode(kc)),
                    })
                    .unwrap_or(Action::Trans);
            }
        }
        for (y, action) in parsed_state.fake_keys.values() {
            let (x, y) = get_fake_key_coords(*y);
            layers_cfg[layer_level][x as usize][y as usize] = **action;
        }
    }
    Ok(layers_cfg)
}

fn parse_sequences(exprs: &[&Vec<SExpr>], parsed_state: &ParsedState) -> Result<KeySeqsToFKeys> {
    const ERR_MSG: &str = "defseq expects two parameters: <fake_key_name> <key_list>";
    let mut sequences = Trie::new();
    for expr in exprs {
        let mut subexprs = check_first_expr(expr.iter(), "defseq")?;
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
            .ok_or_else(|| anyhow!(ERR_MSG))?
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
            bail!("defseq {key_seq:?} has a conflict: it contains an earlier defined sequence");
        }
        if sequences.get_raw_descendant(&keycode_seq).is_some() {
            bail!("defseq {key_seq:?} has a conflict: it is contained within an earlier defined seqence");
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
    Ok(sequences)
}

/// Creates a `KeyOutputs` from `layers::LAYERS`.
fn create_key_outputs(layers: &KanataLayers) -> KeyOutputs {
    let mut outs = KeyOutputs::new();
    for layer in layers.iter() {
        for (i, action) in layer[0].iter().enumerate() {
            let i = match i.try_into() {
                Ok(i) => i,
                Err(_) => continue,
            };
            match action {
                Action::KeyCode(kc) => {
                    add_kc_output(i, kc.into(), &mut outs);
                }
                Action::HoldTap(HoldTapAction { tap, hold, .. }) => {
                    if let Action::KeyCode(kc) = tap {
                        add_kc_output(i, kc.into(), &mut outs);
                    }
                    if let Action::KeyCode(kc) = hold {
                        add_kc_output(i, kc.into(), &mut outs);
                    }
                }
                Action::OneShot(OneShot {
                    action: Action::KeyCode(kc),
                    ..
                }) => {
                    add_kc_output(i, kc.into(), &mut outs);
                }
                Action::TapDance(TapDance { actions, .. }) => {
                    for action in actions.iter() {
                        if let Action::KeyCode(kc) = action {
                            add_kc_output(i, kc.into(), &mut outs);
                        }
                    }
                }
                _ => {} // do nothing for other types
            };
        }
    }
    for hset in outs.values_mut() {
        hset.shrink_to_fit();
    }
    outs.shrink_to_fit();
    outs
}

/// Create a layout from `layers::LAYERS`.
fn create_layout(layers: Box<KanataLayers>) -> KanataLayout {
    Layout::new(Box::leak(layers))
}
