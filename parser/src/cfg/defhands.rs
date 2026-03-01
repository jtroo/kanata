use super::sexpr::*;
use super::*;
use crate::{anyhow_expr, bail, bail_expr};

pub(super) fn parse_defhands(expr: &[SExpr], s: &ParserState) -> Result<custom_tap_hold::HandMap> {
    use custom_tap_hold::Hand;

    let exprs_iter = check_first_expr(expr.iter(), "defhands")?;
    let mut keys: Vec<u16> = Vec::new();
    let mut hands: Vec<Hand> = Vec::new();
    let mut seen_left = false;
    let mut seen_right = false;

    for group_expr in exprs_iter {
        let group = group_expr
            .list(s.vars())
            .ok_or_else(|| anyhow_expr!(group_expr, "expected (left ...) or (right ...)"))?;
        if group.is_empty() {
            bail_expr!(group_expr, "expected (left ...) or (right ...)");
        }
        let hand_name = group[0]
            .atom(s.vars())
            .ok_or_else(|| anyhow_expr!(&group[0], "expected 'left' or 'right'"))?;
        let hand = match hand_name {
            "left" => {
                if seen_left {
                    bail_expr!(&group[0], "duplicate 'left' group in defhands");
                }
                seen_left = true;
                Hand::Left
            }
            "right" => {
                if seen_right {
                    bail_expr!(&group[0], "duplicate 'right' group in defhands");
                }
                seen_right = true;
                Hand::Right
            }
            _ => bail_expr!(&group[0], "expected 'left' or 'right', got '{}'", hand_name),
        };
        for key_expr in &group[1..] {
            let key_name = key_expr
                .atom(s.vars())
                .ok_or_else(|| anyhow_expr!(key_expr, "expected a key name, found list"))?;
            let osc = str_to_oscode(key_name)
                .ok_or_else(|| anyhow_expr!(key_expr, "unknown key '{}'", key_name))?;
            let code = u16::from(osc);
            if let Some(pos) = keys.iter().position(|&k| k == code) {
                let existing_name = if hands[pos] == Hand::Left {
                    "left"
                } else {
                    "right"
                };
                bail_expr!(
                    key_expr,
                    "Key already assigned to '{}' hand, cannot also be in '{}'",
                    existing_name,
                    hand_name
                );
            }
            keys.push(code);
            hands.push(hand);
        }
    }

    let keys_static = s.a.sref_vec(keys);
    let hands_static = s.a.sref_vec(hands);
    Ok(custom_tap_hold::HandMap {
        keys: keys_static,
        hands: hands_static,
    })
}

pub(super) fn parse_tap_hold_opposite_hand(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    use custom_tap_hold::{DecisionBehavior, custom_tap_hold_opposite_hand};

    const ARITY_MSG: &str = "tap-hold-opposite-hand expects at least 3 items: \
            <timeout> <tap> <hold> [options...]";
    if ac_params.is_empty() {
        bail!(ARITY_MSG);
    }
    if ac_params.len() < 3 {
        bail_expr!(&ac_params[0], "{}", ARITY_MSG);
    }
    let hand_map = s.hand_map.ok_or_else(|| {
        anyhow_expr!(
            &ac_params[0],
            "tap-hold-opposite-hand requires defhands to be defined"
        )
    })?;

    let hold_timeout = parse_non_zero_u16(&ac_params[0], s, "timeout")?;
    let tap_action = parse_action(&ac_params[1], s)?;
    let hold_action = parse_action(&ac_params[2], s)?;
    if matches!(tap_action, Action::HoldTap { .. }) {
        bail_expr!(
            &ac_params[1],
            "tap-hold does not work in the tap-action of tap-hold"
        );
    }

    let mut timeout_behavior = DecisionBehavior::Tap;
    let mut same_hand = DecisionBehavior::Tap;
    let mut neutral_behavior = DecisionBehavior::Ignore;
    let mut unknown_hand = DecisionBehavior::Ignore;
    let mut neutral_keys: Vec<OsCode> = Vec::new();
    let mut seen_options: HashSet<&str> = HashSet::default();

    for option_expr in &ac_params[3..] {
        let Some(option) = option_expr.list(s.vars()) else {
            bail_expr!(
                option_expr,
                "expected option list `(name value)`, e.g. `(timeout hold)`"
            );
        };
        if option.len() != 2 {
            bail_expr!(
                option_expr,
                "option must contain exactly 2 items: `(name value)`"
            );
        }
        let kw = option[0]
            .atom(s.vars())
            .ok_or_else(|| anyhow_expr!(&option[0], "option name must be a string"))?;
        if !seen_options.insert(kw) {
            bail_expr!(
                &option[0],
                "duplicate option '{}' in tap-hold-opposite-hand",
                kw
            );
        }
        match kw {
            "timeout" => {
                timeout_behavior = parse_decision_behavior_tap_hold(&option[1], s)?;
            }
            "same-hand" => {
                same_hand = parse_decision_behavior(&option[1], s)?;
            }
            "neutral" => {
                neutral_behavior = parse_decision_behavior(&option[1], s)?;
            }
            "unknown-hand" => {
                unknown_hand = parse_decision_behavior(&option[1], s)?;
            }
            "neutral-keys" => {
                neutral_keys = parse_key_list(&option[1], s, "neutral-keys")?;
            }
            _ => bail_expr!(
                &option[0],
                "unknown option '{}' for tap-hold-opposite-hand. \
                Valid options: timeout, same-hand, neutral, unknown-hand, neutral-keys",
                kw
            ),
        }
    }

    let timeout_action = match timeout_behavior {
        DecisionBehavior::Tap => tap_action,
        DecisionBehavior::Hold => hold_action,
        DecisionBehavior::Ignore => unreachable!(),
    };

    let neutral_keys_static = s.a.sref_vec(neutral_keys);

    Ok(s.a.sref(Action::HoldTap(s.a.sref(HoldTapAction {
        config: HoldTapConfig::Custom(custom_tap_hold_opposite_hand(
            hand_map,
            same_hand,
            neutral_behavior,
            unknown_hand,
            neutral_keys_static,
            &s.a,
        )),
        tap_hold_interval: 0,
        timeout: hold_timeout,
        tap: *tap_action,
        hold: *hold_action,
        timeout_action: *timeout_action,
        on_press_reset_timeout_to: None,
    }))))
}

fn parse_decision_behavior(
    expr: &SExpr,
    s: &ParserState,
) -> Result<custom_tap_hold::DecisionBehavior> {
    use custom_tap_hold::DecisionBehavior;

    match expr
        .atom(s.vars())
        .ok_or_else(|| anyhow_expr!(expr, "expected tap, hold, or ignore"))?
    {
        "tap" => Ok(DecisionBehavior::Tap),
        "hold" => Ok(DecisionBehavior::Hold),
        "ignore" => Ok(DecisionBehavior::Ignore),
        v => bail_expr!(expr, "expected tap, hold, or ignore; got '{}'", v),
    }
}

fn parse_decision_behavior_tap_hold(
    expr: &SExpr,
    s: &ParserState,
) -> Result<custom_tap_hold::DecisionBehavior> {
    use custom_tap_hold::DecisionBehavior;

    match expr
        .atom(s.vars())
        .ok_or_else(|| anyhow_expr!(expr, "expected tap or hold"))?
    {
        "tap" => Ok(DecisionBehavior::Tap),
        "hold" => Ok(DecisionBehavior::Hold),
        v => bail_expr!(expr, "expected tap or hold for timeout; got '{}'", v),
    }
}
