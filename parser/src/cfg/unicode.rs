use super::*;

use crate::anyhow_expr;
use crate::bail;
use crate::bail_expr;

pub(crate) fn parse_unicode(ac_params: &[SExpr], s: &ParserState) -> Result<&'static KanataAction> {
    const ERR_STR: &str = "unicode expects exactly one (not combos looking like one) unicode character as an argument\nor a unicode hex number, prefixed by U+. Example: U+1F686.";
    if ac_params.len() != 1 {
        bail!(ERR_STR)
    }
    ac_params[0]
        .atom(s.vars())
        .map(|a| {
            let a = a.trim_atom_quotes();
            let unicode_char = match a.chars().count() {
                0 => bail_expr!(&ac_params[0], "{ERR_STR}"),
                1 => a.chars().next().expect("1 char"),
                _ => {
                    let normalized = a.to_uppercase();
                    let Some(hexnum) = normalized.strip_prefix("U+") else {
                        bail_expr!(&ac_params[0], "{ERR_STR}.\nMust begin with U+")
                    };
                    let Ok(u_val) = u32::from_str_radix(hexnum, 16) else {
                        bail_expr!(&ac_params[0], "{ERR_STR}.\nInvalid number after U+")
                    };
                    match char::from_u32(u_val) {
                        Some(v) => v,
                        None => bail_expr!(&ac_params[0], "{ERR_STR}.\nInvalid char."),
                    }
                }
            };
            custom(CustomAction::Unicode(unicode_char), &s.a)
        })
        .ok_or_else(|| anyhow_expr!(&ac_params[0], "{ERR_STR}"))?
}
