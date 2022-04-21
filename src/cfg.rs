//! TBD: Configuration parser.
//!
//! How the configuration maps to keyberon:
//!
//! If the mapped keys are defined as:
//!
//!     (defsrc
//!         esc 1 2 3 4
//!     )
//!
//! and the layers are:
//!
//!     (deflayer one
//!         esc a s d f
//!     )
//!
//!     (deflayer two
//!         esc a o e u
//!     )
//!
//! Then the keyberon layers will be as follows:
//!
//!     xx means unimportant. See `keys.rs` for reference
//!
//!     layers[0] = { xx, 1, 30, 31, 32, 33, xx... }
//!     layers[1] = { xx, 1, 30, 24, 18, 22, xx... }
//!
//!  Note that this example isn't practical, but `(defsrc esc 1 2 3 4)` is used because these keys
//!  are at the beginning of the array. The column index for layers is the numerical value of
//!  the key from `keys::OsCode`.
//!
//!  If you want to change how the physical key `A` works on a given layer, you would change index
//!  30 (see `keys::OsCode::KEY_A`) of the desired layer to the desired `keyberon::action::Action`.

#![allow(dead_code)]

use crate::default_layers::*;
use crate::keys::*;

use anyhow::{anyhow, bail, Result};
use std::collections::HashMap;

use keyberon::action::*;
use keyberon::key_code::*;
use keyberon::layout::*;

pub struct Cfg {
    /// Mapped keys are the result of the kmonad `defsrc` declaration. Events for keys that are not
    /// mapped by ktrl will be sent directly to the OS without being processed internally.
    ///
    /// TODO: currently not used, `create_mapped_keys` is used instead (hardcoded).
    pub mapped_keys: MappedKeys,
    pub key_outputs: KeyOutputs,
    pub items: HashMap<String, String>,
    pub layout: Layout<256, 1, MAX_LAYERS>,
}

impl Cfg {
    pub fn new_from_file(p: &std::path::Path) -> Result<Self> {
        let (items, mapped_keys, key_outputs, layout) = parse_cfg(p)?;
        Ok(Self {
            items,
            mapped_keys,
            key_outputs,
            layout,
        })
    }
}

/// TODO: replace this with cfg fns
pub fn create_layout() -> Layout<256, 1, 25> {
    // DEFAULT_LAYERS is permanently locked after this.
    Layout::new(sref(*DEFAULT_LAYERS.lock().expect("layers lk poisoned")))
}

pub const MAPPED_KEYS_LEN: usize = 256;
pub type MappedKeys = [bool; MAPPED_KEYS_LEN];
pub type KeyOutputs = [Option<Vec<OsCode>>; MAPPED_KEYS_LEN];

fn add_kc_output(i: usize, kc: OsCode, outs: &mut KeyOutputs) {
    log::info!("Adding {:?} to idx {}", kc, i);
    match outs[i].as_mut() {
        None => {
            outs[i] = Some(vec![kc]);
        }
        Some(v) => {
            v.push(kc);
        }
    }
}

/// TODO: replace this with cfg fns
fn create_key_outputs() -> KeyOutputs {
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
    for layer in DEFAULT_LAYERS.lock().expect("layer lk poisoned").iter() {
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

// This test is my experimentation for parsing lisp
#[test]
fn read_and_parse() {
    parse_cfg(&std::path::PathBuf::from("./cfg_samples/jtroo.kbd")).unwrap();
}

fn parse_cfg(
    p: &std::path::Path,
) -> Result<(
    HashMap<String, String>,
    MappedKeys,
    KeyOutputs,
    Layout<256, 1, MAX_LAYERS>,
)> {
    let cfg = std::fs::read_to_string(p)?;

    let s_exprs = get_root_exprs(&cfg)?;
    let root_exprs: Vec<_> = s_exprs
        .iter()
        .map(|expr| parse_expr(expr).unwrap_or_else(|e| panic!("Parsing error: {}", e)))
        .collect();

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

    let layer_exprs = root_exprs
        .iter()
        .filter(gen_first_atom_filter("deflayer"))
        .collect::<Vec<_>>();
    if layer_exprs.is_empty() {
        bail!("No deflayer expressions exist. At least one layer must be defined.")
    }
    if layer_exprs.len() > MAX_LAYERS {
        bail!("Exceeded the maximum layer count of {}", MAX_LAYERS)
    }
    let layer_idxs = parse_layer_indexes(&layer_exprs, mapping_order.len())?;

    let alias_exprs = root_exprs
        .iter()
        .filter(gen_first_atom_filter("defalias"))
        .collect::<Vec<_>>();
    let aliases = parse_aliases(&alias_exprs, &layer_idxs)?;
    parse_layers(&layer_exprs, &aliases, &layer_idxs, &mapping_order)?;
    Ok((cfg, src, create_key_outputs(), create_layout()))
}

/// Return a closure that filters a root expression by the content of the first element. The
/// closure returns true if the first element is an atom that matches the input `a` and false
/// otherwise.
fn gen_first_atom_filter(a: &str) -> impl FnMut(&&Vec<SExpr>) -> bool {
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

#[derive(Debug)]
/// I know this isn't the classic definition of an S-Expression which uses cons cell and atom, but
/// this is more convenient to work with (I find).
enum SExpr {
    Atom(String),
    List(Vec<SExpr>),
}

// Get the root expressions and strip comments.
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

// Parse an expression string into an SExpr
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

/// Consumes the first element and returns the rest of the iterator,
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

/// Parse configuration entries from an expression starting with defcfg
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

/// Represents an action or an alias that may or may not exist (hasn't been verified).
enum MaybeAction {
    Action(Action),
    Alias(String),
}

type LayerIndexes = HashMap<String, usize>;
type Aliases = HashMap<String, &'static Action>;

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

/// Returns the content of the SExpr if the SExpr is an atom, or returns None otherwise.
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
        loop {
            let alias = match subexprs.next() {
                Some(k) => k,
                None => break,
            };
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

fn sref<T>(v: T) -> &'static T {
    Box::leak(Box::new(v))
}

fn parse_action(expr: &SExpr, aliases: &Aliases, layers: &LayerIndexes) -> Result<&'static Action> {
    match expr {
        SExpr::Atom(a) => parse_action_atom(a, aliases),
        SExpr::List(l) => parse_action_list(l, aliases, layers),
    }
}

fn parse_action_atom(ac: &str, aliases: &Aliases) -> Result<&'static Action> {
    match ac {
        "_" => return Ok(sref(Action::Trans)),
        "XX" => return Ok(sref(Action::NoOp)),
        _ => {},
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
            if key_stack.contains(&KeyCode::LShift) {
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

fn parse_action_list(
    ac: &[SExpr],
    aliases: &Aliases,
    layers: &LayerIndexes,
) -> Result<&'static Action> {
    if ac.is_empty() {
        return Ok(sref(Action::NoOp));
    }
    let ac_type = match &ac[0] {
        SExpr::Atom(a) => a,
        _ => bail!("Action list must start with an atom"),
    };
    match ac_type.as_str() {
        "layer-base" => parse_layer_base(&ac[1..], layers),
        "layer-toggle" => parse_layer_toggle(&ac[1..], layers),
        "tap-hold" => parse_tap_hold(&ac[1..], aliases, layers),
        "multi" => parse_multi(&ac[1..], aliases, layers),
        _ => bail!(
            "Unknown action type: {}. Valid types: layer-base, layer-toggle, tap-hold, multi",
            ac_type
        ),
    }
}

fn parse_layer_base(ac_params: &[SExpr], layers: &LayerIndexes) -> Result<&'static Action> {
    Ok(sref(Action::DefaultLayer(layer_idx(ac_params, layers)?)))
}

fn parse_layer_toggle(ac_params: &[SExpr], layers: &LayerIndexes) -> Result<&'static Action> {
    Ok(sref(Action::Layer(layer_idx(ac_params, layers)?)))
}

fn layer_idx(ac_params: &[SExpr], layers: &LayerIndexes) -> Result<usize> {
    if ac_params.len() != 1 {
        bail!(
            "layer-base expects one atom: the layer name. Incorrect value: {:?}",
            ac_params
        )
    }
    let layer_name = match &ac_params[0] {
        SExpr::Atom(ln) => ln,
        _ => bail!(
            "layer-base name should be an atom, not a list: {:?}",
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
) -> Result<&'static Action> {
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
) -> Result<&'static Action> {
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

/// Mutates DEFAULT_LAYERS using the inputs.
fn parse_layers(
    layers: &[&Vec<SExpr>],
    aliases: &Aliases,
    layer_idxs: &LayerIndexes,
    mapping_order: &[usize],
) -> Result<()> {
    let mut layers_cfg = DEFAULT_LAYERS.lock().expect("layer lk poisoned");
    for (layer_level, layer) in layers.iter().enumerate() {
        // skip deflayer and name
        for (i, ac) in layer.iter().skip(2).enumerate() {
            let ac = parse_action(ac, aliases, layer_idxs)?;
            layers_cfg[layer_level][0][mapping_order[i]] = *ac;
        }
    }
    Ok(())
}

/// Convert a str to an oscode.
///
/// Could be implemented as `TryFrom` but I like it better in this file since this only applies to
/// parsing tho configuration. OsCode is in a different file.
///
/// kmonad str to key mapping found here:
/// https://github.com/kmonad/kmonad/blob/master/src/KMonad/Keyboard/Keycode.hs
///
/// At the time of writing this only contains aliases I use in my configuration.
fn str_to_oscode(s: &str) -> Option<OsCode> {
    Some(match s {
        "grv" => OsCode::KEY_GRAVE,
        "1" => OsCode::KEY_1,
        "2" => OsCode::KEY_2,
        "3" => OsCode::KEY_3,
        "4" => OsCode::KEY_4,
        "5" => OsCode::KEY_5,
        "6" => OsCode::KEY_6,
        "7" => OsCode::KEY_7,
        "8" => OsCode::KEY_8,
        "9" => OsCode::KEY_9,
        "0" => OsCode::KEY_0,
        "+" => OsCode::KEY_KPPLUS,
        "-" => OsCode::KEY_MINUS,
        "=" => OsCode::KEY_EQUAL,
        "bspc" => OsCode::KEY_BACKSPACE,
        "tab" => OsCode::KEY_TAB,
        "q" => OsCode::KEY_Q,
        "w" => OsCode::KEY_W,
        "e" => OsCode::KEY_E,
        "r" => OsCode::KEY_R,
        "t" => OsCode::KEY_T,
        "y" => OsCode::KEY_Y,
        "u" => OsCode::KEY_U,
        "i" => OsCode::KEY_I,
        "o" => OsCode::KEY_O,
        "p" => OsCode::KEY_P,
        "{" => OsCode::KEY_LEFTBRACE,
        "}" => OsCode::KEY_RIGHTBRACE,
        "[" => OsCode::KEY_LEFTBRACE,
        "]" => OsCode::KEY_RIGHTBRACE,
        "\\" => OsCode::KEY_BACKSLASH,
        "caps" => OsCode::KEY_CAPSLOCK,
        "a" => OsCode::KEY_A,
        "s" => OsCode::KEY_S,
        "d" => OsCode::KEY_D,
        "f" => OsCode::KEY_F,
        "g" => OsCode::KEY_G,
        "h" => OsCode::KEY_H,
        "j" => OsCode::KEY_J,
        "k" => OsCode::KEY_K,
        "l" => OsCode::KEY_L,
        ";" => OsCode::KEY_SEMICOLON,
        "'" => OsCode::KEY_APOSTROPHE,
        "ret" => OsCode::KEY_ENTER,
        "lsft" => OsCode::KEY_LEFTSHIFT,
        "z" => OsCode::KEY_Z,
        "x" => OsCode::KEY_X,
        "c" => OsCode::KEY_C,
        "v" => OsCode::KEY_V,
        "b" => OsCode::KEY_B,
        "n" => OsCode::KEY_N,
        "m" => OsCode::KEY_M,
        "," => OsCode::KEY_COMMA,
        "." => OsCode::KEY_DOT,
        "/" => OsCode::KEY_SLASH,
        "esc" => OsCode::KEY_ESC,
        "rsft" => OsCode::KEY_RIGHTSHIFT,
        "lctl" => OsCode::KEY_LEFTCTRL,
        "lmet" => OsCode::KEY_LEFTMETA,
        "lalt" => OsCode::KEY_LEFTALT,
        "spc" => OsCode::KEY_SPACE,
        "ralt" => OsCode::KEY_RIGHTALT,
        "rmet" => OsCode::KEY_RIGHTMETA,
        "rctl" => OsCode::KEY_RIGHTCTRL,
        "del" => OsCode::KEY_DELETE,
        "pgup" => OsCode::KEY_PAGEUP,
        "pgdn" => OsCode::KEY_PAGEDOWN,
        "up" => OsCode::KEY_UP,
        "down" => OsCode::KEY_DOWN,
        "left" => OsCode::KEY_LEFT,
        "rght" => OsCode::KEY_RIGHT,
        "home" => OsCode::KEY_HOME,
        "end" => OsCode::KEY_END,
        _ => return None,
    })
}
