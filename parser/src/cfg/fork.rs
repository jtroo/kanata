use super::*;

use crate::bail;

pub(crate) fn parse_fork(ac_params: &[SExpr], s: &ParserState) -> Result<&'static KanataAction> {
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
