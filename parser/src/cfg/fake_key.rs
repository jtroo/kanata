use super::*;

use crate::{anyhow_expr, bail, bail_expr};

#[allow(unused_variables)]
fn set_virtual_key_reference_lsp_hint(vk_name_expr: &SExpr, s: &ParserState) {
    #[cfg(feature = "lsp")]
    {
        let atom = match vk_name_expr {
            SExpr::Atom(x) => x,
            SExpr::List(_) => unreachable!("should be validated to be atom earlier"),
        };
        s.lsp_hints
            .borrow_mut()
            .reference_locations
            .virtual_key
            .push_from_atom(atom);
    }
}

pub(crate) fn parse_on_press_fake_key_op(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    let (coord, action) = parse_fake_key_op_coord_action(ac_params, s, ON_PRESS_FAKEKEY)?;
    set_virtual_key_reference_lsp_hint(&ac_params[0], s);
    Ok(s.a.sref(Action::Custom(
        s.a.sref(s.a.sref_slice(CustomAction::FakeKey { coord, action })),
    )))
}

pub(crate) fn parse_on_release_fake_key_op(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    let (coord, action) = parse_fake_key_op_coord_action(ac_params, s, ON_RELEASE_FAKEKEY)?;
    set_virtual_key_reference_lsp_hint(&ac_params[0], s);
    Ok(s.a.sref(Action::Custom(s.a.sref(
        s.a.sref_slice(CustomAction::FakeKeyOnRelease { coord, action }),
    ))))
}

pub(crate) fn parse_on_idle_fakekey(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "on-idle-fakekey expects three parameters:\n<fake key name> <(tap|press|release)> <idle time>\n";
    if ac_params.len() != 3 {
        bail!("{ERR_MSG}");
    }
    let y = match s
        .virtual_keys
        .get(ac_params[0].atom(s.vars()).ok_or_else(|| {
            anyhow_expr!(
                &ac_params[0],
                "{ERR_MSG}\nInvalid first parameter: a fake key name cannot be a list",
            )
        })?) {
        Some((y, _)) => *y as u16, // cast should be safe; checked in `parse_fake_keys`
        None => bail_expr!(
            &ac_params[0],
            "{ERR_MSG}\nInvalid first parameter: unknown fake key name {:?}",
            &ac_params[0]
        ),
    };
    let action = ac_params[1]
        .atom(s.vars())
        .and_then(|a| match a {
            "tap" => Some(FakeKeyAction::Tap),
            "press" => Some(FakeKeyAction::Press),
            "release" => Some(FakeKeyAction::Release),
            _ => None,
        })
        .ok_or_else(|| {
            anyhow_expr!(
                &ac_params[1],
                "{ERR_MSG}\nInvalid second parameter, it must be one of: tap, press, release",
            )
        })?;
    let idle_duration = parse_u16(&ac_params[2], s, "idle time").map_err(|mut e| {
        e.msg = format!("{ERR_MSG}\nInvalid third parameter: {}", e.msg);
        e
    })?;
    let (x, y) = get_fake_key_coords(y);
    let coord = Coord { x, y };
    set_virtual_key_reference_lsp_hint(&ac_params[0], s);
    Ok(s.a.sref(Action::Custom(s.a.sref(s.a.sref_slice(
        CustomAction::FakeKeyOnIdle(FakeKeyOnIdle {
            coord,
            action,
            idle_duration,
        }),
    )))))
}

fn parse_fake_key_op_coord_action(
    ac_params: &[SExpr],
    s: &ParserState,
    ac_name: &str,
) -> Result<(Coord, FakeKeyAction)> {
    const ERR_MSG: &str = "expects two parameters: <fake key name> <(tap|press|release|toggle)>";
    if ac_params.len() != 2 {
        bail!("{ac_name} {ERR_MSG}");
    }
    let y = match s
        .virtual_keys
        .get(ac_params[0].atom(s.vars()).ok_or_else(|| {
            anyhow_expr!(
                &ac_params[0],
                "{ac_name} {ERR_MSG}\nInvalid first parameter: a fake key name cannot be a list",
            )
        })?) {
        Some((y, _)) => *y as u16, // cast should be safe; checked in `parse_fake_keys`
        None => bail_expr!(
            &ac_params[0],
            "{ac_name} {ERR_MSG}\nInvalid first parameter: unknown fake key name {:?}",
            &ac_params[0]
        ),
    };
    let action = ac_params[1]
        .atom(s.vars())
        .and_then(|a| match a {
            "tap" => Some(FakeKeyAction::Tap),
            "press" => Some(FakeKeyAction::Press),
            "release" => Some(FakeKeyAction::Release),
            "toggle" => Some(FakeKeyAction::Toggle),
            _ => None,
        })
        .ok_or_else(|| {
            anyhow_expr!(
                &ac_params[1],
                "{ERR_MSG}\nInvalid second parameter, it must be one of: tap, press, release",
            )
        })?;
    let (x, y) = get_fake_key_coords(y);
    set_virtual_key_reference_lsp_hint(&ac_params[0], s);
    Ok((Coord { x, y }, action))
}

pub const NORMAL_KEY_ROW: u8 = 0;
pub const FAKE_KEY_ROW: u8 = 1;

pub(crate) fn get_fake_key_coords<T: Into<usize>>(y: T) -> (u8, u16) {
    let y: usize = y.into();
    (FAKE_KEY_ROW, y as u16)
}

pub(crate) fn parse_fake_key_delay(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    parse_delay(ac_params, false, s)
}

pub(crate) fn parse_on_release_fake_key_delay(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    parse_delay(ac_params, true, s)
}

fn parse_delay(
    ac_params: &[SExpr],
    is_release: bool,
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "delay expects a single number (ms, 0-65535)";
    let delay = ac_params[0]
        .atom(s.vars())
        .map(str::parse::<u16>)
        .ok_or_else(|| anyhow!("{ERR_MSG}"))?
        .map_err(|e| anyhow!("{ERR_MSG}: {e}"))?;
    Ok(s.a
        .sref(Action::Custom(s.a.sref(s.a.sref_slice(match is_release {
            false => CustomAction::Delay(delay),
            true => CustomAction::DelayOnRelease(delay),
        })))))
}

pub(crate) fn parse_vkey_coord(param: &SExpr, s: &ParserState) -> Result<Coord> {
    let name = param
        .atom(s.vars())
        .ok_or_else(|| anyhow_expr!(param, "key-name must not be a list",))?;
    let y = match s.virtual_keys.get(name) {
        Some((y, _)) => *y as u16, // cast should be safe; checked in `parse_fake_keys`
        None => bail_expr!(param, "unknown virtual key name: {name}",),
    };
    let coord = Coord { x: FAKE_KEY_ROW, y };
    set_virtual_key_reference_lsp_hint(param, s);
    Ok(coord)
}

fn parse_vkey_action(param: &SExpr, s: &ParserState) -> Result<FakeKeyAction> {
    let action = param
        .atom(s.vars())
        .and_then(|ac| {
            Some(match ac {
                "press-vkey" | "press-virtualkey" => FakeKeyAction::Press,
                "release-vkey" | "release-virtualkey" => FakeKeyAction::Release,
                "tap-vkey" | "tap-virtualkey" => FakeKeyAction::Tap,
                "toggle-vkey" | "toggle-virtualkey" => FakeKeyAction::Toggle,
                _ => return None,
            })
        })
        .ok_or_else(|| {
            anyhow_expr!(
                param,
                "action must be one of: (press|release|tap|toggle)-virtualkey\n\
                 example: toggle-virtualkey"
            )
        })?;
    Ok(action)
}

pub(crate) fn parse_on_press(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "on-press expects two parameters: <action> <key-name>";
    if ac_params.len() != 2 {
        bail!("{ERR_MSG}");
    }
    let action = parse_vkey_action(&ac_params[0], s)?;
    let coord = parse_vkey_coord(&ac_params[1], s)?;

    Ok(s.a.sref(Action::Custom(
        s.a.sref(s.a.sref_slice(CustomAction::FakeKey { coord, action })),
    )))
}

pub(crate) fn parse_on_release(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "on-release expects two parameters: <action> <key-name>";
    if ac_params.len() != 2 {
        bail!("{ERR_MSG}");
    }
    let action = parse_vkey_action(&ac_params[0], s)?;
    let coord = parse_vkey_coord(&ac_params[1], s)?;

    Ok(s.a.sref(Action::Custom(s.a.sref(
        s.a.sref_slice(CustomAction::FakeKeyOnRelease { coord, action }),
    ))))
}

pub(crate) fn parse_on_idle(ac_params: &[SExpr], s: &ParserState) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "on-idle expects three parameters: <timeout> <action> <key-name>";
    if ac_params.len() != 3 {
        bail!("{ERR_MSG}");
    }
    let idle_duration = parse_non_zero_u16(&ac_params[0], s, "on-idle-timeout")?;
    let action = parse_vkey_action(&ac_params[1], s)?;
    let coord = parse_vkey_coord(&ac_params[2], s)?;

    Ok(s.a.sref(Action::Custom(s.a.sref(s.a.sref_slice(
        CustomAction::FakeKeyOnIdle(FakeKeyOnIdle {
            coord,
            action,
            idle_duration,
        }),
    )))))
}

pub(crate) fn parse_on_physical_idle(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str =
        "on-physical-idle expects three parameters: <timeout> <action> <key-name>";
    if ac_params.len() != 3 {
        bail!("{ERR_MSG}");
    }
    let idle_duration = parse_non_zero_u16(&ac_params[0], s, "on-idle-timeout")?;
    let action = parse_vkey_action(&ac_params[1], s)?;
    let coord = parse_vkey_coord(&ac_params[2], s)?;

    Ok(s.a.sref(Action::Custom(s.a.sref(s.a.sref_slice(
        CustomAction::FakeKeyOnPhysicalIdle(FakeKeyOnIdle {
            coord,
            action,
            idle_duration,
        }),
    )))))
}

pub(crate) fn parse_hold_for_duration(
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "hold-for-duration expects two parameters: <hold-duration> <key-name>";
    if ac_params.len() != 2 {
        bail!("{ERR_MSG}");
    }
    let hold_duration = parse_non_zero_u16(&ac_params[0], s, "hold-duration")?;
    let coord = parse_vkey_coord(&ac_params[1], s)?;

    Ok(s.a.sref(Action::Custom(s.a.sref(s.a.sref_slice(
        CustomAction::FakeKeyHoldForDuration(FakeKeyHoldForDuration {
            coord,
            hold_duration,
        }),
    )))))
}
