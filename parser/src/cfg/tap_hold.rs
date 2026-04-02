use super::*;

use crate::anyhow_expr;
use crate::bail;
use crate::bail_expr;

/// Options that can be specified as trailing `(keyword value)` lists on any tap-hold action.
#[derive(Default)]
pub(crate) struct TapHoldOptions {
    pub(crate) require_prior_idle: Option<u16>,
}

/// Parse the value of a `(require-prior-idle <ms>)` option list.
/// Validates that the list has exactly 2 items and the value is a u16.
pub(crate) fn parse_require_prior_idle_option(
    option: &[SExpr],
    option_expr: &SExpr,
    s: &ParserState,
) -> Result<u16> {
    if option.len() != 2 {
        bail_expr!(
            option_expr,
            "require-prior-idle option expects exactly 2 items: \
            `(require-prior-idle <ms>)`"
        );
    }
    let prior_idle = parse_u16(&option[1], s, "require-prior-idle")?;
    s.max_key_timing_check
        .set(std::cmp::max(prior_idle, s.max_key_timing_check.get()));
    Ok(prior_idle)
}

/// Parse trailing `(keyword value)` option lists from tap-hold action parameters.
/// Returns the parsed options. Errors on unknown or duplicate options.
pub(crate) fn parse_tap_hold_options(
    option_exprs: &[SExpr],
    s: &ParserState,
) -> Result<TapHoldOptions> {
    let mut opts = TapHoldOptions::default();
    let mut seen_options: HashSet<&str> = HashSet::default();

    for option_expr in option_exprs {
        let Some(option) = option_expr.list(s.vars()) else {
            bail_expr!(
                option_expr,
                "expected option list, e.g. `(require-prior-idle 150)`"
            );
        };
        if option.is_empty() {
            bail_expr!(option_expr, "option list cannot be empty");
        }
        let kw = option[0]
            .atom(s.vars())
            .ok_or_else(|| anyhow_expr!(&option[0], "option name must be a string"))?;
        if !seen_options.insert(kw) {
            bail_expr!(&option[0], "duplicate option '{}'", kw);
        }
        match kw {
            "require-prior-idle" => {
                opts.require_prior_idle =
                    Some(parse_require_prior_idle_option(option, option_expr, s)?);
            }
            _ => bail_expr!(
                &option[0],
                "unknown tap-hold option '{}'. \
                Valid options: require-prior-idle",
                kw
            ),
        }
    }
    Ok(opts)
}

const TAP_HOLD_OPTION_KEYWORDS: &[&str] = &["require-prior-idle"];
const TAP_HOLD_KEYS_OPTION_KEYWORDS: &[&str] = &[
    "require-prior-idle",
    "tap-on-press",
    "tap-on-press-release",
    "hold-on-press",
];

/// Count how many trailing expressions are tap-hold option lists.
/// An option list is a list whose first element is a known option keyword.
/// Stops at the first non-option expression (scanning from the end).
fn count_trailing_options(ac_params: &[SExpr], s: &ParserState) -> usize {
    let mut count = 0;
    for expr in ac_params.iter().rev() {
        if let Some(list) = expr.list(s.vars()) {
            if let Some(kw) = list.first().and_then(|e| e.atom(s.vars())) {
                if TAP_HOLD_OPTION_KEYWORDS.contains(&kw) {
                    count += 1;
                    continue;
                }
            }
        }
        break;
    }
    count
}

pub(crate) fn parse_tap_hold(
    ac_params: &[SExpr],
    s: &ParserState,
    config: HoldTapConfig<'static>,
) -> Result<&'static KanataAction> {
    let n_opts = count_trailing_options(ac_params, s);
    let n_positional = ac_params.len() - n_opts;
    if n_positional != 4 {
        bail!(
            r"tap-hold expects 4 items after it, got {}.
Params in order:
<tap-repress-timeout> <hold-timeout> <tap-action> <hold-action>",
            n_positional,
        )
    }
    let tap_repress_timeout = parse_u16(&ac_params[0], s, "tap repress timeout")?;
    let hold_timeout = parse_non_zero_u16(&ac_params[1], s, "hold timeout")?;
    let tap_action = parse_action(&ac_params[2], s)?;
    let hold_action = parse_action(&ac_params[3], s)?;
    if matches!(tap_action, Action::HoldTap { .. }) {
        bail!("tap-hold does not work in the tap-action of tap-hold")
    }
    let opts = parse_tap_hold_options(&ac_params[n_positional..], s)?;
    Ok(s.a.sref(Action::HoldTap(s.a.sref(HoldTapAction {
        config,
        tap_hold_interval: tap_repress_timeout,
        timeout: hold_timeout,
        tap: *tap_action,
        hold: *hold_action,
        timeout_action: *hold_action,
        on_press_reset_timeout_to: None,
        require_prior_idle: opts.require_prior_idle,
    }))))
}

pub(crate) fn parse_tap_hold_timeout(
    ac_params: &[SExpr],
    s: &ParserState,
    config: HoldTapConfig<'static>,
) -> Result<&'static KanataAction> {
    const PARAMS_FOR_RELEASE: &str = "Params in order:\n\
       <tap-repress-timeout> <hold-timeout> <tap-action> <hold-action> <timeout-action> [?reset-timeout-on-press]";
    let n_opts = count_trailing_options(ac_params, s);
    let n_positional = ac_params.len() - n_opts;
    match config {
        HoldTapConfig::PermissiveHold => {
            if n_positional != 5 && n_positional != 6 {
                bail!(
                    "tap-hold-release-timeout expects at least 5 items after it, got {}.\n\
                    {PARAMS_FOR_RELEASE}",
                    n_positional,
                )
            }
        }
        HoldTapConfig::HoldOnOtherKeyPress => {
            if n_positional != 5 {
                bail!(
                    "tap-hold-press-timeout expects 5 items after it, got {}.\n\
                    Params in order:\n\
                    <tap-repress-timeout> <hold-timeout> <tap-action> <hold-action> <timeout-action>",
                    n_positional,
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
        HoldTapConfig::PermissiveHold => match n_positional {
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
    let opts = parse_tap_hold_options(&ac_params[n_positional..], s)?;
    Ok(s.a.sref(Action::HoldTap(s.a.sref(HoldTapAction {
        config,
        tap_hold_interval: tap_repress_timeout,
        timeout: hold_timeout,
        tap: *tap_action,
        hold: *hold_action,
        timeout_action: *timeout_action,
        on_press_reset_timeout_to,
        require_prior_idle: opts.require_prior_idle,
    }))))
}

pub(crate) fn parse_tap_hold_order(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    let n_opts = count_trailing_options(ac_params, s);
    let n_positional = ac_params.len() - n_opts;
    if n_positional != 4 {
        bail!(
            r"tap-hold-order expects 4 items after it, got {}.
Params in order:
<tap-repress-timeout> <buffer-ms> <tap-action> <hold-action>",
            n_positional,
        )
    }
    let tap_repress_timeout = parse_u16(&ac_params[0], s, "tap repress timeout")?;
    let buffer = parse_u16(&ac_params[1], s, "buffer")?;
    let tap_action = parse_action(&ac_params[2], s)?;
    let hold_action = parse_action(&ac_params[3], s)?;
    if matches!(tap_action, Action::HoldTap { .. }) {
        bail!("tap-hold does not work in the tap-action of tap-hold")
    }
    let opts = parse_tap_hold_options(&ac_params[n_positional..], s)?;
    Ok(s.a.sref(Action::HoldTap(s.a.sref(HoldTapAction {
        config: HoldTapConfig::Order { buffer },
        tap_hold_interval: tap_repress_timeout,
        timeout: u16::MAX, // Resolution is purely event-driven, not timeout-based.
        tap: *tap_action,
        hold: *hold_action,
        timeout_action: *tap_action,
        on_press_reset_timeout_to: None,
        require_prior_idle: opts.require_prior_idle,
    }))))
}

pub(crate) fn parse_tap_hold_keys(
    ac_params: &[SExpr],
    s: &ParserState,
    custom_name: &str,
    custom_func: TapHoldCustomFunc,
) -> Result<&'static KanataAction> {
    let n_opts = count_trailing_options(ac_params, s);
    let n_positional = ac_params.len() - n_opts;
    if n_positional != 5 {
        bail!(
            r"{} expects 5 items after it, got {}.
Params in order:
<tap-repress-timeout> <hold-timeout> <tap-action> <hold-action> <tap-trigger-keys>",
            custom_name,
            n_positional,
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
    let opts = parse_tap_hold_options(&ac_params[n_positional..], s)?;
    Ok(s.a.sref(Action::HoldTap(s.a.sref(HoldTapAction {
        config: HoldTapConfig::Custom(custom_func(&tap_trigger_keys, &s.a)),
        tap_hold_interval: tap_repress_timeout,
        timeout: hold_timeout,
        tap: *tap_action,
        hold: *hold_action,
        timeout_action: *hold_action,
        on_press_reset_timeout_to: None,
        require_prior_idle: opts.require_prior_idle,
    }))))
}

pub(crate) fn parse_tap_hold_keys_trigger_tap_release(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    let n_opts = count_trailing_options(ac_params, s);
    let n_positional = ac_params.len() - n_opts;
    if n_positional != 6 {
        bail!(
            r"{} expects 6 items after it, got {}.
Params in order:
<tap-repress-timeout> <hold-timeout> <tap-action> <hold-action> <tap-trigger-keys-on-press> <tap-trigger-keys-on-press-then-release>",
            TAP_HOLD_RELEASE_KEYS_TAP_RELEASE,
            n_positional,
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
    let opts = parse_tap_hold_options(&ac_params[n_positional..], s)?;
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
        require_prior_idle: opts.require_prior_idle,
    }))))
}

pub(crate) fn parse_tap_hold_keys_named_lists(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    // Count trailing options using the extended keyword set.
    let n_opts = {
        let mut count = 0;
        for expr in ac_params.iter().rev() {
            if let Some(list) = expr.list(s.vars()) {
                if let Some(kw) = list.first().and_then(|e| e.atom(s.vars())) {
                    if TAP_HOLD_KEYS_OPTION_KEYWORDS.contains(&kw) {
                        count += 1;
                        continue;
                    }
                }
            }
            break;
        }
        count
    };
    let n_positional = ac_params.len() - n_opts;
    if n_positional != 4 {
        bail!(
            r"{} expects 4 items after it, got {}.
Params in order:
<tap-repress-timeout> <hold-timeout> <tap-action> <hold-action>
Followed by optional lists:
(tap-on-press <keys...>) (tap-on-press-release <keys...>) (hold-on-press <keys...>)
(require-prior-idle <ms>)",
            TAP_HOLD_KEYS,
            n_positional,
        )
    }
    let tap_repress_timeout = parse_u16(&ac_params[0], s, "tap repress timeout")?;
    let hold_timeout = parse_non_zero_u16(&ac_params[1], s, "hold timeout")?;
    let tap_action = parse_action(&ac_params[2], s)?;
    let hold_action = parse_action(&ac_params[3], s)?;
    if matches!(tap_action, Action::HoldTap { .. }) {
        bail!("tap-hold does not work in the tap-action of tap-hold")
    }

    let mut require_prior_idle = None;
    let mut tap_on_press: Vec<OsCode> = vec![];
    let mut tap_on_press_release: Vec<OsCode> = vec![];
    let mut hold_on_press: Vec<OsCode> = vec![];
    let mut seen_options: HashSet<&str> = HashSet::default();
    let mut seen_keys: HashSet<OsCode> = HashSet::default();

    for option_expr in &ac_params[n_positional..] {
        let Some(option) = option_expr.list(s.vars()) else {
            bail_expr!(
                option_expr,
                "expected option list, e.g. `(tap-on-press a b c)`"
            );
        };
        if option.is_empty() {
            bail_expr!(option_expr, "option list cannot be empty");
        }
        let kw = option[0]
            .atom(s.vars())
            .ok_or_else(|| anyhow_expr!(&option[0], "option name must be a string"))?;
        if !seen_options.insert(kw) {
            bail_expr!(&option[0], "duplicate option '{}'", kw);
        }
        match kw {
            "require-prior-idle" => {
                require_prior_idle = Some(parse_require_prior_idle_option(option, option_expr, s)?);
            }
            "tap-on-press" => {
                tap_on_press =
                    parse_key_list_from_option(option, option_expr, s, kw, &mut seen_keys)?;
            }
            "tap-on-press-release" => {
                tap_on_press_release =
                    parse_key_list_from_option(option, option_expr, s, kw, &mut seen_keys)?;
            }
            "hold-on-press" => {
                hold_on_press =
                    parse_key_list_from_option(option, option_expr, s, kw, &mut seen_keys)?;
            }
            _ => bail_expr!(
                &option[0],
                "unknown tap-hold-keys option '{}'. \
                Valid options: tap-on-press, tap-on-press-release, hold-on-press, require-prior-idle",
                kw
            ),
        }
    }

    Ok(s.a.sref(Action::HoldTap(s.a.sref(HoldTapAction {
        config: HoldTapConfig::Custom(custom_tap_hold_keys(
            &tap_on_press,
            &tap_on_press_release,
            &hold_on_press,
            &s.a,
        )),
        tap_hold_interval: tap_repress_timeout,
        timeout: hold_timeout,
        tap: *tap_action,
        hold: *hold_action,
        timeout_action: *hold_action,
        on_press_reset_timeout_to: None,
        require_prior_idle,
    }))))
}

/// Parse keys from a named option list like `(tap-on-press a b c)`.
/// The first element is the keyword, remaining elements are key names.
fn parse_key_list_from_option(
    option: &[SExpr],
    option_expr: &SExpr,
    s: &ParserState,
    kw: &str,
    seen_keys: &mut HashSet<OsCode>,
) -> Result<Vec<OsCode>> {
    if option.len() < 2 {
        bail_expr!(
            option_expr,
            "{} expects at least one key, e.g. `({} a b c)`",
            kw,
            kw
        );
    }
    let mut keys = Vec::new();
    for key in &option[1..] {
        let a = key.atom(s.vars()).ok_or_else(|| {
            anyhow_expr!(key, "string of a known key is expected, found list instead")
        })?;
        let osc = str_to_oscode(a)
            .ok_or_else(|| anyhow_expr!(key, "string of a known key is expected"))?;
        if !seen_keys.insert(osc) {
            bail_expr!(
                key,
                "key '{}' is already used in another tap-hold-keys option list",
                a
            );
        }
        keys.push(osc);
    }
    Ok(keys)
}
