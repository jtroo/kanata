use super::*;

use crate::bail;
use crate::bail_expr;

pub(crate) fn parse_tap_hold(
    ac_params: &[SExpr],
    s: &ParserState,
    config: HoldTapConfig<'static>,
) -> Result<&'static KanataAction> {
    if ac_params.len() != 4 {
        bail!(
            r"tap-hold expects 4 items after it, got {}.
Params in order:
<tap-repress-timeout> <hold-timeout> <tap-action> <hold-action>",
            ac_params.len(),
        )
    }
    let tap_repress_timeout = parse_u16(&ac_params[0], s, "tap repress timeout")?;
    let hold_timeout = parse_non_zero_u16(&ac_params[1], s, "hold timeout")?;
    let tap_action = parse_action(&ac_params[2], s)?;
    let hold_action = parse_action(&ac_params[3], s)?;
    if matches!(tap_action, Action::HoldTap { .. }) {
        bail!("tap-hold does not work in the tap-action of tap-hold")
    }
    Ok(s.a.sref(Action::HoldTap(s.a.sref(HoldTapAction {
        config,
        tap_hold_interval: tap_repress_timeout,
        timeout: hold_timeout,
        tap: *tap_action,
        hold: *hold_action,
        timeout_action: *hold_action,
        on_press_reset_timeout_to: None,
    }))))
}

pub(crate) fn parse_tap_hold_timeout(
    ac_params: &[SExpr],
    s: &ParserState,
    config: HoldTapConfig<'static>,
) -> Result<&'static KanataAction> {
    const PARAMS_FOR_RELEASE: &str = "Params in order:\n\
       <tap-repress-timeout> <hold-timeout> <tap-action> <hold-action> <timeout-action> [?reset-timeout-on-press]";
    match config {
        HoldTapConfig::PermissiveHold => {
            if ac_params.len() != 5 && ac_params.len() != 6 {
                bail!(
                    "tap-hold-release-timeout expects at least 5 items after it, got {}.\n\
                    {PARAMS_FOR_RELEASE}",
                    ac_params.len(),
                )
            }
        }
        HoldTapConfig::HoldOnOtherKeyPress => {
            if ac_params.len() != 5 {
                bail!(
                    "tap-hold-press-timeout expects 5 items after it, got {}.\n\
                    Params in order:\n\
                    <tap-repress-timeout> <hold-timeout> <tap-action> <hold-action> <timeout-action>",
                    ac_params.len(),
                )
            }
        }
        _ => unreachable!("other configs not expected"),
    };
    let tap_repress_timeout = parse_u16(&ac_params[0], s, "tap repress timeout")?;
    let hold_timeout = parse_non_zero_u16(&ac_params[1], s, "hold timeout")?;
    let tap_action = parse_action(&ac_params[2], s)?;
    let hold_action = parse_action(&ac_params[3], s)?;
    let timeout_action = parse_action(&ac_params[4], s)?;
    if matches!(tap_action, Action::HoldTap { .. }) {
        bail!("tap-hold does not work in the tap-action of tap-hold")
    }
    let on_press_reset_timeout_to = match config {
        HoldTapConfig::PermissiveHold => match ac_params.len() {
            6 => match ac_params[5].atom(s.vars()) {
                Some("reset-timeout-on-press") => std::num::NonZeroU16::new(hold_timeout),
                _ => bail_expr!(&ac_params[5], "Unexpected parameter.\n{PARAMS_FOR_RELEASE}"),
            },
            5 => None,
            _ => unreachable!("other lengths not expected"),
        },
        HoldTapConfig::HoldOnOtherKeyPress => None,
        _ => unreachable!("other configs not expected"),
    };
    Ok(s.a.sref(Action::HoldTap(s.a.sref(HoldTapAction {
        config,
        tap_hold_interval: tap_repress_timeout,
        timeout: hold_timeout,
        tap: *tap_action,
        hold: *hold_action,
        timeout_action: *timeout_action,
        on_press_reset_timeout_to,
    }))))
}

pub(crate) fn parse_tap_hold_keys(
    ac_params: &[SExpr],
    s: &ParserState,
    custom_name: &str,
    custom_func: TapHoldCustomFunc,
) -> Result<&'static KanataAction> {
    if ac_params.len() != 5 {
        bail!(
            r"{} expects 5 items after it, got {}.
Params in order:
<tap-repress-timeout> <hold-timeout> <tap-action> <hold-action> <tap-trigger-keys>",
            custom_name,
            ac_params.len(),
        )
    }
    let tap_repress_timeout = parse_u16(&ac_params[0], s, "tap repress timeout")?;
    let hold_timeout = parse_non_zero_u16(&ac_params[1], s, "hold timeout")?;
    let tap_action = parse_action(&ac_params[2], s)?;
    let hold_action = parse_action(&ac_params[3], s)?;
    let tap_trigger_keys = parse_key_list(&ac_params[4], s, "tap-trigger-keys")?;
    if matches!(tap_action, Action::HoldTap { .. }) {
        bail!("tap-hold does not work in the tap-action of tap-hold")
    }
    Ok(s.a.sref(Action::HoldTap(s.a.sref(HoldTapAction {
        config: HoldTapConfig::Custom(custom_func(&tap_trigger_keys, &s.a)),
        tap_hold_interval: tap_repress_timeout,
        timeout: hold_timeout,
        tap: *tap_action,
        hold: *hold_action,
        timeout_action: *hold_action,
        on_press_reset_timeout_to: None,
    }))))
}

pub(crate) fn parse_tap_hold_keys_trigger_tap_release(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    if !matches!(ac_params.len(), 6) {
        bail!(
            r"{} expects 6 items after it, got {}.
Params in order:
<tap-repress-timeout> <hold-timeout> <tap-action> <hold-action> <tap-trigger-keys-on-press> <tap-trigger-keys-on-press-then-release>",
            TAP_HOLD_RELEASE_KEYS_TAP_RELEASE,
            ac_params.len(),
        )
    }
    let tap_repress_timeout = parse_u16(&ac_params[0], s, "tap repress timeout")?;
    let hold_timeout = parse_non_zero_u16(&ac_params[1], s, "hold timeout")?;
    let tap_action = parse_action(&ac_params[2], s)?;
    let hold_action = parse_action(&ac_params[3], s)?;
    let tap_trigger_keys_on_press =
        parse_key_list(&ac_params[4], s, "tap-trigger-keys-on-multi-press")?;
    let tap_trigger_keys_on_press_then_release =
        parse_key_list(&ac_params[5], s, "tap-trigger-keys-on-release")?;
    if matches!(tap_action, Action::HoldTap { .. }) {
        bail!("tap-hold does not work in the tap-action of tap-hold")
    }
    Ok(s.a.sref(Action::HoldTap(s.a.sref(HoldTapAction {
        config: HoldTapConfig::Custom(custom_tap_hold_release_trigger_tap_release(
            &tap_trigger_keys_on_press,
            &tap_trigger_keys_on_press_then_release,
            &s.a,
        )),
        tap_hold_interval: tap_repress_timeout,
        timeout: hold_timeout,
        tap: *tap_action,
        hold: *hold_action,
        timeout_action: *hold_action,
        on_press_reset_timeout_to: None,
    }))))
}
