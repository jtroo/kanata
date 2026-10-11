use super::*;

use crate::anyhow_expr;
use crate::bail_expr;

pub(crate) fn parse_suppress_keys(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    let suppressed_keys: Vec<KeyCode> = ac_params.iter().try_fold(vec![], |mut keys, param| {
        let osc = param
            .atom(s.vars())
            .and_then(str_to_oscode)
            .ok_or_else(|| anyhow_expr!(&param, "Invalid key name."))?;
        // These keys are appended to the output list directly rather than
        // going through an action, so the check `parse_action_atom` makes for
        // every other output position has to be repeated here.
        let is_mouse_movement = osc == OsCode::KEY_766;
        if osc.is_gamepad_code() || is_mouse_movement {
            bail_expr!(param, "{osc} can only be used as an input");
        }
        keys.push(osc.into());
        Ok::<_, ParseError>(keys)
    })?;
    let suppressed_keys = s.a.sref_vec(suppressed_keys);
    custom(CustomAction::Suppressed { suppressed_keys }, &s.a)
}
