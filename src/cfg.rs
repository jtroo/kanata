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
//!  `DEFAULT_LAYERS` is currently set up similarly to the examples above, so you can look there
//!  for an example.

#![allow(dead_code)]

use crate::default_layers::*;
use crate::keys::*;

use anyhow::{anyhow, bail, Result};
use std::collections::{HashMap, HashSet};

use keyberon::action::*;
use keyberon::layout::*;

pub struct Cfg {
    /// Mapped keys are the result of the kmonad `defsrc` declaration. Events for keys that are not
    /// mapped by ktrl will be sent directly to the OS without being processed internally.
    ///
    /// TODO: currently not used, `create_mapped_keys` is used instead (hardcoded).
    pub mapped_keys: HashSet<OsCode>,
}

impl Cfg {
    pub fn new() -> Self {
        let mut mapped_keys = HashSet::new();
        mapped_keys.insert(OsCode::KEY_A); // FIXME: parse from cfg
        Self { mapped_keys }
    }
}

/// TODO: replace this with cfg fns
pub fn create_layout() -> Layout<256, 1, 25> {
    Layout::new(&DEFAULT_LAYERS)
}

pub const MAPPED_KEYS_LEN: usize = 256;
pub type MappedKeys = [bool; MAPPED_KEYS_LEN];

/// TODO: replace this with cfg fns
pub fn create_mapped_keys() -> MappedKeys {
    let mut map = [false; MAPPED_KEYS_LEN];
    map[OsCode::KEY_ESC as usize] = true;
    map[OsCode::KEY_1 as usize] = true;
    map[OsCode::KEY_2 as usize] = true;
    map[OsCode::KEY_3 as usize] = true;
    map[OsCode::KEY_4 as usize] = true;
    map
}

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
pub fn create_key_outputs() -> KeyOutputs {
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
    for layer in DEFAULT_LAYERS.iter() {
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
    parse_cfg().unwrap();
}

fn parse_cfg() -> Result<()> {
    let cfg = std::fs::read_to_string("./cfg_samples/jtroo.kbd")?;

    let s_exprs = get_root_exprs(&cfg)?;

    let root_exprs: Vec<_> = s_exprs
        .iter()
        .map(|expr| parse_expr(expr).unwrap_or_else(|e| panic!("Parsing error: {}", e)))
        .collect();

    let cfg_filter = |expr: &&Vec<SExpr>| {
        if expr.is_empty() {
            return false;
        }
        if let SExpr::Atom(atom) = &expr[0] {
            atom == "defcfg"
        } else {
            false
        }
    };
    let cfg_expr = root_exprs
        .iter()
        .find(cfg_filter)
        .ok_or_else(|| anyhow!("defcfg is missing from the configuration"))?;
    if root_exprs.iter().filter(cfg_filter).count() > 1 {
        bail!("Only one defcfg is allowed in the configuration")
    }
    let cfg = parse_defcfg(cfg_expr).unwrap();
    dbg!(cfg);

    let src_filter = |expr: &&Vec<SExpr>| {
        if expr.is_empty() {
            return false;
        }
        if let SExpr::Atom(atom) = &expr[0] {
            atom == "defsrc"
        } else {
            false
        }
    };
    let src_expr = root_exprs
        .iter()
        .find(src_filter)
        .ok_or_else(|| anyhow!("defsrc is missing from the configuration"))?;
    if root_exprs.iter().filter(src_filter).count() > 1 {
        bail!("Only one defcfg is allowed in the configuration")
    }
    let src = parse_defsrc(src_expr).unwrap();
    dbg!(src);

    Ok(())
}

#[derive(Debug)]
enum SExpr {
    List(Vec<SExpr>),
    Atom(String),
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

// Parse a configuration from a defcfg expr
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

// Consumes the first element and returns the rest of the iterator
fn check_first_expr<'a>(
    mut exprs: impl Iterator<Item = &'a SExpr>,
    expected_first: &str,
) -> Result<impl Iterator<Item = &'a SExpr>> {
    if let Some(first) = exprs.next() {
        match first {
            SExpr::Atom(a) => {
                if a != expected_first {
                    bail!("Passed non-defcfg expression to parse_defcfg: {}", a);
                }
            }
            SExpr::List(_) => {
                bail!("First entry should not be a list for parse_defcfg");
            }
        };
    } else {
        bail!("Passed empty list to parse_defcfg")
    };
    Ok(exprs)
}

/// Parse a defsrc and return the mapped keys.
fn parse_defsrc(expr: &[SExpr]) -> Result<MappedKeys> {
    // Validate first expression, which should be defsrc
    let exprs = match check_first_expr(expr.iter(), "defsrc") {
        Ok(s) => s,
        Err(e) => bail!(e),
    };

    let mut mkeys = [false; 256];
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
        mkeys[oscode] = true;
    }
    Ok(mkeys)
}

/// Convert a str to an oscode.
///
/// Could be implemented as `TryFrom` but I like it better in this file since this only applies to
/// parsing tho configuration. OsCode is in a different file.
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
        "rsft" => OsCode::KEY_RIGHTSHIFT,
        "lctl" => OsCode::KEY_LEFTCTRL,
        "lmet" => OsCode::KEY_LEFTMETA,
        "lalt" => OsCode::KEY_LEFTALT,
        "spc" => OsCode::KEY_SPACE,
        "ralt" => OsCode::KEY_RIGHTALT,
        "rmet" => OsCode::KEY_RIGHTMETA,
        "rctl" => OsCode::KEY_RIGHTCTRL,
        _ => return None,
    })
}
