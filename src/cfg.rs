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

use crate::custom_action::*;
use crate::keys::*;
use crate::layers::*;

use anyhow::{anyhow, bail, Result};
use std::collections::HashMap;

use kanata_keyberon::action::*;
use kanata_keyberon::key_code::*;
use kanata_keyberon::layout::*;

pub type KanataAction = Action<CustomAction>;
pub type KanataLayout = Layout<256, 1, ACTUAL_NUM_LAYERS, CustomAction>;

pub struct Cfg {
    pub mapped_keys: MappedKeys,
    pub key_outputs: KeyOutputs,
    pub layer_strings: Vec<String>,
    pub items: HashMap<String, String>,
    pub layout: KanataLayout,
}

impl Cfg {
    pub fn new_from_file(p: &std::path::Path) -> Result<Self> {
        let (items, mapped_keys, layer_strings, key_outputs, layout) = parse_cfg(p)?;
        Ok(Self {
            items,
            mapped_keys,
            layer_strings,
            key_outputs,
            layout,
        })
    }
}

/// Length of the MappedKeys array.
pub const MAPPED_KEYS_LEN: usize = 256;

/// Used as a silly `HashSet<OsCode>` to know which `OsCode`s are used in defsrc. I should probably
/// just use a HashSet for this.
pub type MappedKeys = [bool; MAPPED_KEYS_LEN];

/// Used as a silly `HashMap<Oscode, Vec<OsCode>>` to know which `OsCode`s are potential outputs
/// for a given physical key location. I should probably just use a HashMap for this.
pub type KeyOutputs = [Option<Vec<OsCode>>; MAPPED_KEYS_LEN];

fn add_kc_output(i: usize, kc: OsCode, outs: &mut KeyOutputs) {
    match outs[i].as_mut() {
        None => {
            outs[i] = Some(vec![kc]);
        }
        Some(v) => {
            if !v.contains(&kc) {
                v.push(kc);
            }
        }
    }
}

#[test]
fn parse_simple() {
    parse_cfg(&std::path::PathBuf::from("./cfg_samples/simple.kbd")).unwrap();
}

#[test]
fn parse_default() {
    parse_cfg(&std::path::PathBuf::from("./cfg_samples/kanata.kbd")).unwrap();
}

#[test]
fn parse_jtroo() {
    let (_, _, layer_strings, _, _) =
        parse_cfg(&std::path::PathBuf::from("./cfg_samples/jtroo.kbd")).unwrap();
    assert_eq!(layer_strings.len(), 16);
}

#[test]
fn parse_f13_f24() {
    parse_cfg(&std::path::PathBuf::from("./cfg_samples/f13_f24.kbd")).unwrap();
}

#[test]
fn parse_transparent_default() {
    let (_, _, layer_strings, layers) = parse_cfg_raw(&std::path::PathBuf::from(
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

#[allow(clippy::type_complexity)] // return type is not pub
fn parse_cfg(
    p: &std::path::Path,
) -> Result<(
    HashMap<String, String>,
    MappedKeys,
    Vec<String>,
    KeyOutputs,
    KanataLayout,
)> {
    let (cfg, src, layer_strings, klayers) = parse_cfg_raw(p)?;

    Ok((
        cfg,
        src,
        layer_strings,
        create_key_outputs(&klayers),
        create_layout(klayers),
    ))
}

#[allow(clippy::type_complexity)] // return type is not pub
fn parse_cfg_raw(
    p: &std::path::Path,
) -> Result<(
    HashMap<String, String>,
    MappedKeys,
    Vec<String>,
    KanataLayers,
)> {
    let cfg = std::fs::read_to_string(p)?;

    let root_expr_strs = get_root_exprs(&cfg)?;
    let mut root_exprs = Vec::new();
    for expr in root_expr_strs.iter() {
        root_exprs.push(parse_expr(expr)?);
    }

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
    let (src, mapping_order) = parse_defsrc(src_expr)?;

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

    let layer_strings = root_expr_strs
        .into_iter()
        .zip(root_exprs.iter())
        .filter(|(_, expr)| deflayer_filter(expr))
        .flat_map(|(s, _)| {
            // Duplicate the same layer for `layer_strings` because the keyberon layout itself has
            // two versions of each layer.
            std::iter::repeat(s).take(2)
        })
        .collect::<Vec<_>>();

    let alias_exprs = root_exprs
        .iter()
        .filter(gen_first_atom_filter("defalias"))
        .collect::<Vec<_>>();
    let aliases = parse_aliases(&alias_exprs, &layer_idxs)?;

    let defsrc_layer = parse_defsrc_layer(src_expr, &mapping_order);
    let klayers = parse_layers(
        &layer_exprs,
        &aliases,
        &layer_idxs,
        &mapping_order,
        &defsrc_layer,
    )?;

    Ok((cfg, src, layer_strings, klayers))
}

/// Return a closure that filters a root expression by the content of the first element. The
/// closure returns true if the first element is an atom that matches the input `a` and false
/// otherwise.
fn gen_first_atom_filter(a: &str) -> impl Fn(&&Vec<SExpr>) -> bool {
    let a = a.to_owned();
    move |expr: &&Vec<SExpr>| {
        if expr.is_empty() {
            return false;
        }
        if let SExpr::Atom(atom) = &expr[0] {
            atom == &a
        } else {
            false
        }
    }
}

#[derive(Clone, Debug)]
/// I know this isn't the classic definition of an S-Expression which uses cons cell and atom, but
/// this is more convenient to work with (I find).
enum SExpr {
    Atom(String),
    List(Vec<SExpr>),
}

/// Get the root expressions and strip comments.
fn get_root_exprs(cfg: &str) -> Result<Vec<String>> {
    let mut open_paren_count = 0;
    let mut close_paren_count = 0;
    let mut s_exprs = Vec::new();
    let mut cur_expr = String::new();
    for line in cfg.lines() {
        // remove comments
        let line = line.split(";;").next().unwrap();
        for c in line.chars() {
            if c == '(' {
                open_paren_count += 1;
            } else if c == ')' {
                close_paren_count += 1;
            }
        }
        if open_paren_count == 0 {
            continue;
        }
        cur_expr.push_str(line);
        cur_expr.push('\n');
        if open_paren_count == close_paren_count {
            open_paren_count = 0;
            close_paren_count = 0;
            s_exprs.push(cur_expr.trim().to_owned());
            cur_expr.clear();
        }
    }
    if !cur_expr.is_empty() {
        bail!("Unclosed root expression:\n{}", cur_expr)
    }
    Ok(s_exprs)
}

/// Parse an expression string into an SExpr
fn parse_expr(expr: &str) -> Result<Vec<SExpr>> {
    if !expr.starts_with('(') {
        bail!("Expression in cfg does not start with '(':\n{}", expr)
    }
    if !expr.ends_with(')') {
        bail!("Expression in cfg does not end with ')':\n{}", expr)
    }
    let expr = expr.strip_prefix('(').unwrap_or(expr);
    let expr = expr.strip_suffix(')').unwrap_or(expr);

    let mut ret = Vec::new();
    let mut tokens = expr.split_whitespace();
    loop {
        let token = match tokens.next() {
            None => break,
            Some(t) => t,
        };
        if token.contains('(') {
            // seek to matching close paren and recurse
            let mut paren_stack_size = token.chars().filter(|c| *c == '(').count();
            paren_stack_size -= token.chars().filter(|c| *c == ')').count();
            let mut subexpr = String::new();
            subexpr.push_str(token);
            while paren_stack_size > 0 {
                let token = match tokens.next() {
                    None => bail!(
                        "Sub expression does not close:\n{}\nwhole expr:\n{}",
                        subexpr,
                        expr
                    ),
                    Some(t) => t,
                };
                paren_stack_size += token.chars().filter(|c| *c == '(').count();
                paren_stack_size -= token.chars().filter(|c| *c == ')').count();
                subexpr.push(' ');
                subexpr.push_str(token);
            }
            ret.push(SExpr::List(parse_expr(&subexpr)?))
        } else if token.contains(')') {
            bail!(
                "Unexpected closing paren in token {} in expr:\n{}",
                token,
                expr
            )
        } else {
            ret.push(SExpr::Atom(token.to_owned()));
        }
    }
    Ok(ret)
}

/// Consumes the first element and returns the rest of the iterator. Returns `Ok` if the first
/// element is an atom and equals `expected_first`.
fn check_first_expr<'a>(
    mut exprs: impl Iterator<Item = &'a SExpr>,
    expected_first: &str,
) -> Result<impl Iterator<Item = &'a SExpr>> {
    if let Some(first) = exprs.next() {
        match first {
            SExpr::Atom(a) => {
                if a != expected_first {
                    bail!(
                        "Passed non-{} expression to parse_defcfg: {}",
                        expected_first,
                        a
                    );
                }
            }
            SExpr::List(_) => {
                bail!(
                    "First entry is expected to be an atom for {}",
                    expected_first
                );
            }
        };
    } else {
        bail!("Passed empty list to check_first_expr")
    };
    Ok(exprs)
}

/// Parse configuration entries from an expression starting with defcfg.
fn parse_defcfg(expr: &[SExpr]) -> Result<HashMap<String, String>> {
    let mut cfg = HashMap::new();
    let mut exprs = match check_first_expr(expr.iter(), "defcfg") {
        Ok(s) => s,
        Err(e) => bail!(e),
    };

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
                if cfg.insert(k.clone(), v.clone()).is_some() {
                    bail!("duplicate cfg entries for key {}", k);
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
fn parse_defsrc(expr: &[SExpr]) -> Result<(MappedKeys, Vec<usize>)> {
    // Validate first expression, which should be defsrc
    let exprs = match check_first_expr(expr.iter(), "defsrc") {
        Ok(s) => s,
        Err(e) => bail!(e),
    };

    let mut mkeys = [false; 256];
    let mut ordered_codes = Vec::new();
    for expr in exprs {
        let s = match expr {
            SExpr::Atom(a) => a,
            _ => bail!("No lists allowed in defsrc"),
        };
        let oscode: usize = match str_to_oscode(s) {
            Some(c) => c.into(),
            None => bail!("Unknown key in defsrc: \"{}\"", s),
        };
        if oscode >= MAPPED_KEYS_LEN {
            bail!("Cannot use key \"{}\"", s)
        }
        if mkeys[oscode] {
            bail!("Repeat declaration of key in defsrc: \"{}\"", s)
        }
        mkeys[oscode] = true;
        ordered_codes.push(oscode);
    }
    Ok((mkeys, ordered_codes))
}

type LayerIndexes = HashMap<String, usize>;
type Aliases = HashMap<String, &'static KanataAction>;

/// Returns layer names and their indexes into the keyberon layout. This also checks that all
/// layers have the same number of items as the defsrc.
fn parse_layer_indexes(exprs: &[&Vec<SExpr>], expected_len: usize) -> Result<LayerIndexes> {
    let mut layer_indexes = HashMap::new();
    for (i, expr) in exprs.iter().enumerate() {
        let mut subexprs = match check_first_expr(expr.iter(), "deflayer") {
            Ok(s) => s,
            Err(e) => bail!(e),
        };
        let layer_name = get_atom(
            subexprs
                .next()
                .ok_or_else(|| anyhow!("deflayer requires a name and keys"))?,
        )
        .ok_or_else(|| anyhow!("layer name after deflayer must be an atom"))?;
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

/// Returns the content of an `SExpr::Atom` or returns `None` for `SExpr::List`.
fn get_atom(a: &SExpr) -> Option<String> {
    match a {
        SExpr::Atom(a) => Some(a.clone()),
        _ => None,
    }
}

/// Parse alias->action mappings from multiple exprs starting with defalias.
fn parse_aliases(exprs: &[&Vec<SExpr>], layers: &HashMap<String, usize>) -> Result<Aliases> {
    let mut aliases = HashMap::new();
    for expr in exprs {
        let mut subexprs = match check_first_expr(expr.iter(), "defalias") {
            Ok(s) => s,
            Err(e) => bail!(e),
        };

        // Read k-v pairs from the configuration
        while let Some(alias) = subexprs.next() {
            let action = match subexprs.next() {
                Some(v) => v,
                None => bail!("Incorrect number of elements found in defcfg; they should be pairs of aliases and actions."),
            };
            let alias = match alias {
                SExpr::Atom(a) => a,
                _ => bail!("Alias keys must be atoms. Invalid alias: {:?}", alias),
            };
            let action = parse_action(action, &aliases, layers)?;
            if aliases.insert(alias.into(), action).is_some() {
                bail!("Duplicate alias: {}", alias);
            }
        }
    }
    Ok(aliases)
}

/// Returns a `&'static T` by leaking a box.
fn sref<T>(v: T) -> &'static T {
    Box::leak(Box::new(v))
}

/// Parse a `kanata_keyberon::action::Action` from a `SExpr`.
fn parse_action(
    expr: &SExpr,
    aliases: &Aliases,
    layers: &LayerIndexes,
) -> Result<&'static KanataAction> {
    match expr {
        SExpr::Atom(a) => parse_action_atom(a, aliases),
        SExpr::List(l) => parse_action_list(l, aliases, layers),
    }
}

/// Parse a `kanata_keyberon::action::Action` from a string.
fn parse_action_atom(ac: &str, aliases: &Aliases) -> Result<&'static KanataAction> {
    match ac {
        "_" => return Ok(sref(Action::Trans)),
        "XX" => return Ok(sref(Action::NoOp)),
        "lrld" => return Ok(sref(Action::Custom(CustomAction::LiveReload))),
        "mlft" => return Ok(sref(Action::Custom(CustomAction::Mouse(Btn::Left)))),
        "mrgt" => return Ok(sref(Action::Custom(CustomAction::Mouse(Btn::Right)))),
        "mmid" => return Ok(sref(Action::Custom(CustomAction::Mouse(Btn::Mid)))),
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
        } else if let Some(rest) = rem.strip_prefix("A-") {
            if key_stack.contains(&KeyCode::LAlt) {
                bail!("Redundant \"A-\" in {}", ac)
            }
            key_stack.push(KeyCode::LAlt);
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
fn parse_action_list(
    ac: &[SExpr],
    aliases: &Aliases,
    layers: &LayerIndexes,
) -> Result<&'static KanataAction> {
    if ac.is_empty() {
        return Ok(sref(Action::NoOp));
    }
    let ac_type = match &ac[0] {
        SExpr::Atom(a) => a,
        _ => bail!("Action list must start with an atom"),
    };
    match ac_type.as_str() {
        "layer-switch" => parse_layer_base(&ac[1..], layers),
        "layer-toggle" => parse_layer_toggle(&ac[1..], layers),
        "tap-hold" => parse_tap_hold(&ac[1..], aliases, layers),
        "multi" => parse_multi(&ac[1..], aliases, layers),
        "macro" => parse_macro(&ac[1..], aliases, layers),
        "unicode" => parse_unicode(&ac[1..]),
        _ => bail!(
            "Unknown action type: {}. Valid types:\n\tlayer-switch\n\tlayer-toggle\n\ttap-hold\n\tmulti\n\tmacro\n\tunicode",
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
        SExpr::Atom(ln) => ln,
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
    aliases: &Aliases,
    layers: &LayerIndexes,
) -> Result<&'static KanataAction> {
    if ac_params.len() != 4 {
        bail!("tap-hold expects 4 atoms after it: <tap-timeout> <hold-timeout> <tap-action> <hold-action>, got {}", ac_params.len())
    }
    let tap_timeout =
        parse_timeout(&ac_params[0]).map_err(|e| anyhow!("invalid tap-timeout: {}", e))?;
    let hold_timeout =
        parse_timeout(&ac_params[1]).map_err(|e| anyhow!("invalid tap-timeout: {}", e))?;
    let tap_action = parse_action(&ac_params[2], aliases, layers)?;
    let hold_action = parse_action(&ac_params[3], aliases, layers)?;
    Ok(sref(Action::HoldTap {
        config: HoldTapConfig::Default,
        tap_hold_interval: tap_timeout,
        timeout: hold_timeout,
        tap: tap_action,
        hold: hold_action,
    }))
}

fn parse_timeout(a: &SExpr) -> Result<u16> {
    match a {
        SExpr::Atom(a) => a.parse().map_err(|e| anyhow!("expected integer: {}", e)),
        _ => bail!("expected atom, not list for integer"),
    }
}

fn parse_multi(
    ac_params: &[SExpr],
    aliases: &Aliases,
    layers: &LayerIndexes,
) -> Result<&'static KanataAction> {
    if ac_params.is_empty() {
        bail!("multi expects at least one atom after it")
    }
    let mut actions = Vec::new();
    for expr in ac_params {
        let ac = parse_action(expr, aliases, layers)?;
        actions.push(*ac);
    }
    Ok(sref(Action::MultipleActions(sref(actions))))
}

fn parse_macro(
    ac_params: &[SExpr],
    aliases: &Aliases,
    layers: &LayerIndexes,
) -> Result<&'static KanataAction> {
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
        match parse_action(expr, aliases, layers)? {
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
            if s.chars().count() != 1 {
                bail!(ERR_STR)
            }
            Ok(sref(Action::Custom(CustomAction::Unicode(
                s.chars().next().unwrap(),
            ))))
        }
        _ => bail!(ERR_STR),
    }
}

fn parse_defsrc_layer(defsrc: &[SExpr], mapping_order: &[usize]) -> [KanataAction; 256] {
    let mut layer = empty_layer!();

    // These can be default (empty) since the defsrc layer definitely won't use it.
    let aliases = Default::default();
    let layer_idxs = Default::default();

    for (i, ac) in defsrc.iter().skip(1).enumerate() {
        let ac = parse_action(ac, &aliases, &layer_idxs).unwrap();
        layer[mapping_order[i]] = *ac;
    }
    layer
}

/// Mutates `layers::LAYERS` using the inputs.
fn parse_layers(
    layers: &[&Vec<SExpr>],
    aliases: &Aliases,
    layer_idxs: &LayerIndexes,
    mapping_order: &[usize],
    defsrc_layer: &[KanataAction],
) -> Result<KanataLayers> {
    let mut layers_cfg = new_layers();
    for (layer_level, layer) in layers.iter().enumerate() {
        // skip deflayer and name
        for (i, ac) in layer.iter().skip(2).enumerate() {
            let ac = parse_action(ac, aliases, layer_idxs)?;
            layers_cfg[layer_level * 2][0][mapping_order[i]] = *ac;
            layers_cfg[layer_level * 2 + 1][0][mapping_order[i]] = *ac;
        }
        for (layer_action, defsrc_action) in
            layers_cfg[layer_level * 2][0].iter_mut().zip(defsrc_layer)
        {
            if *layer_action == Action::Trans {
                *layer_action = *defsrc_action;
            }
        }
    }
    Ok(layers_cfg)
}

/// Creates a `KeyOutputs` from `layers::LAYERS`.
fn create_key_outputs(layers: &KanataLayers) -> KeyOutputs {
    // Option<Vec<..>> is not Copy, so need to manually write out all of the None values :(
    let mut outs = [
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        None,
    ];
    for layer in layers.iter() {
        for (i, action) in layer[0].iter().enumerate() {
            match action {
                Action::KeyCode(kc) => {
                    add_kc_output(i, kc.into(), &mut outs);
                }
                Action::HoldTap {
                    tap,
                    hold,
                    timeout: _,
                    config: _,
                    tap_hold_interval: _,
                } => {
                    if let Action::KeyCode(kc) = tap {
                        add_kc_output(i, kc.into(), &mut outs);
                    }
                    if let Action::KeyCode(kc) = hold {
                        add_kc_output(i, kc.into(), &mut outs);
                    }
                }
                _ => {} // do nothing for other types
            };
        }
    }
    outs
}

/// Create a layout from `layers::LAYERS`.
fn create_layout(layers: KanataLayers) -> KanataLayout {
    Layout::new(sref(layers))
}
