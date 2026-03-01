use super::*;

use crate::bail;
use crate::err_expr;

pub(crate) fn parse_release_key(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
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

pub(crate) fn parse_release_layer(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    Ok(s.a
        .sref(Action::ReleaseState(ReleasableState::Layer(layer_idx(
            ac_params,
            &s.layer_idxs,
            s,
        )?))))
}
