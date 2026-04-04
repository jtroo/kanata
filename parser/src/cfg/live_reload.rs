use super::*;

use crate::bail;
use crate::bail_expr;

pub(crate) fn parse_live_reload_num(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "expects 1 parameter: <config argument position (1-65535)>";
    if ac_params.len() != 1 {
        bail!("{LIVE_RELOAD_NUM} {ERR_MSG}, found {}", ac_params.len());
    }
    let num = parse_non_zero_u16(&ac_params[0], s, "config argument position")?;
    // Note: for user-friendliness (hopefully), begin at 1 for parsing.
    // But for use as an index when stored as data, subtract 1 for 0-based indexing.
    custom(CustomAction::LiveReloadNum(num - 1), &s.a)
}

pub(crate) fn parse_live_reload_file(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
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
    let lrld_file_path = spanned_filepath.t.trim_atom_quotes();
    custom(
        CustomAction::LiveReloadFile(s.a.sref_str(lrld_file_path.to_string())),
        &s.a,
    )
}
