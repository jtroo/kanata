use super::*;

use crate::bail;

pub(crate) fn parse_one_shot(
    ac_params: &[SExpr],
    s: &ParserState,
    end_config: OneShotEndConfig,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "one-shot expects a timeout followed by a key or action";
    if ac_params.len() != 2 {
        bail!(ERR_MSG);
    }

    let timeout = parse_non_zero_u16(&ac_params[0], s, "timeout")?;
    let action = parse_action(&ac_params[1], s)?;
    if !matches!(
        action,
        Action::Layer(..) | Action::KeyCode(..) | Action::MultipleKeyCodes(..)
    ) {
        bail!("one-shot is only allowed to contain layer-while-held, a keycode, or a chord");
    }

    Ok(s.a.sref(Action::OneShot(s.a.sref(OneShot {
        timeout,
        action,
        end_config,
    }))))
}

pub(crate) fn parse_one_shot_pause_processing(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "one-shot-pause-processing expects a time";
    if ac_params.len() != 1 {
        bail!(ERR_MSG);
    }
    let timeout = parse_non_zero_u16(&ac_params[0], s, "time (milliseconds)")?;
    Ok(s.a.sref(Action::OneShotIgnoreEventsTicks(timeout)))
}
