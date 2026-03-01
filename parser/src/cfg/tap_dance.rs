use super::*;

use crate::anyhow_expr;
use crate::bail;

pub(crate) fn parse_tap_dance(
    ac_params: &[SExpr],
    s: &ParserState,
    config: TapDanceConfig,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "tap-dance expects a timeout (number) followed by a list of actions";
    if ac_params.len() != 2 {
        bail!(ERR_MSG);
    }

    let timeout = parse_non_zero_u16(&ac_params[0], s, "timeout")?;
    let actions = ac_params[1]
        .list(s.vars())
        .map(|tap_dance_actions| -> Result<Vec<&'static KanataAction>> {
            let mut actions = Vec::new();
            for expr in tap_dance_actions {
                let ac = parse_action(expr, s)?;
                actions.push(ac);
            }
            Ok(actions)
        })
        .ok_or_else(|| anyhow_expr!(&ac_params[1], "{ERR_MSG}: expected a list"))??;

    Ok(s.a.sref(Action::TapDance(s.a.sref(TapDance {
        timeout,
        actions: s.a.sref_vec(actions),
        config,
    }))))
}
