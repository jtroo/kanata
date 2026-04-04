use super::*;

use crate::bail;

pub(crate) fn parse_multi(ac_params: &[SExpr], s: &ParserState) -> Result<&'static KanataAction> {
    if ac_params.is_empty() {
        bail!("multi expects at least one item after it")
    }
    s.multi_action_nest_count
        .replace(s.multi_action_nest_count.get().saturating_add(1));
    let mut actions = Vec::new();
    for expr in ac_params {
        let ac = parse_action(expr, s)?;
        match ac {
            // Flatten multi actions
            Action::MultipleActions(acs) => {
                for ac in acs.iter() {
                    actions.push(*ac);
                }
            }
            _ => actions.push(*ac),
        }
    }

    // Transform all but the last Mouse actions into MouseTap.
    // Need to transform mouse actions to preserve old v<=1.11.0 mouse behaviour where an action like:
    //     (multi mlft mlft)
    // should result in an event sequence like:
    //     click-release-click ... held until key release ... release
    //
    // See test `multi_mouse_button_does_multi_click_release_single_hold`.
    for ca in actions
        .iter_mut()
        .rev()
        .filter(|ac| matches!(ac, Action::Custom(CustomAction::Mouse(..))))
        .skip(1)
    {
        *ca = match ca {
            Action::Custom(CustomAction::Mouse(btn)) => {
                Action::Custom(s.a.sref(CustomAction::MouseTap(*btn)))
            }
            _ => *ca,
        };
    }

    if actions
        .iter()
        .filter(|ac| {
            matches!(
                ac,
                Action::TapDance(TapDance {
                    config: TapDanceConfig::Lazy,
                    ..
                }) | Action::HoldTap { .. }
                    | Action::Chords { .. }
            )
        })
        .count()
        > 1
    {
        bail!("Cannot combine multiple tap-hold/tap-dance/chord");
    }

    s.multi_action_nest_count
        .replace(s.multi_action_nest_count.get().saturating_sub(1));
    Ok(s.a.sref(Action::MultipleActions(s.a.sref(s.a.sref_vec(actions)))))
}
