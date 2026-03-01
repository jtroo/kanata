use super::*;

use crate::anyhow_expr;
use crate::bail_expr;

/// Parse mapped keys from an expression starting with defsrc. Returns the key mapping as well as
/// a vec of the indexes in order. The length of the returned vec should be matched by the length
/// of all layer declarations.
pub(crate) fn parse_defsrc(
    expr: &[SExpr],
    defcfg: &CfgOptions,
) -> Result<(MappedKeys, Vec<usize>, MouseInDefsrc)> {
    let exprs = check_first_expr(expr.iter(), "defsrc")?;
    let mut mkeys = MappedKeys::default();
    let mut ordered_codes = Vec::new();
    let mut is_mouse_used = MouseInDefsrc::NoMouse;
    for expr in exprs {
        let s = match expr {
            SExpr::Atom(a) => &a.t,
            _ => bail_expr!(expr, "No lists allowed in defsrc"),
        };
        let oscode = str_to_oscode(s)
            .ok_or_else(|| anyhow_expr!(expr, "Unknown key in defsrc: \"{}\"", s))?;
        is_mouse_used = match (is_mouse_used, oscode) {
            (
                MouseInDefsrc::NoMouse,
                OsCode::BTN_LEFT
                | OsCode::BTN_RIGHT
                | OsCode::BTN_MIDDLE
                | OsCode::BTN_SIDE
                | OsCode::BTN_EXTRA
                | OsCode::MouseWheelUp
                | OsCode::MouseWheelDown
                | OsCode::MouseWheelLeft
                | OsCode::MouseWheelRight,
            ) => MouseInDefsrc::MouseUsed,
            _ => is_mouse_used,
        };

        if mkeys.contains(&oscode) {
            bail_expr!(expr, "Repeat declaration of key in defsrc: \"{}\"", s)
        }
        mkeys.insert(oscode);
        ordered_codes.push(oscode.into());
    }

    let mapped_exceptions = match &defcfg.process_unmapped_keys_exceptions {
        Some(excluded_keys) => {
            for excluded_key in excluded_keys.iter() {
                log::debug!("process unmapped keys exception: {:?}", excluded_key);
                if mkeys.contains(&excluded_key.0) {
                    bail_expr!(
                        &excluded_key.1,
                        "Keys cannot be included in defsrc and also excepted in process-unmapped-keys."
                    );
                }
            }

            excluded_keys
                .iter()
                .map(|excluded_key| excluded_key.0)
                .collect()
        }
        None => vec![],
    };

    log::info!("process unmapped keys: {}", defcfg.process_unmapped_keys);
    if defcfg.process_unmapped_keys {
        for osc in 0..KEYS_IN_ROW as u16 {
            if let Some(osc) = OsCode::from_u16(osc) {
                if osc.is_mouse_code() {
                    // Bugfix #1879:
                    // Auto-including mouse activity in mapped keys
                    // seems strictly incorrect to do, so never do it.
                    // Users can still choose to opt in if they want.
                    // Auto-including mouse activity breaks many scenarios.
                    continue;
                }
                match KeyCode::from(osc) {
                    KeyCode::No => {}
                    _ => {
                        if !mapped_exceptions.contains(&osc) {
                            mkeys.insert(osc);
                        }
                    }
                }
            }
        }
    }

    mkeys.shrink_to_fit();
    Ok((mkeys, ordered_codes, is_mouse_used))
}

pub(crate) fn create_defsrc_layer() -> [KanataAction; KEYS_IN_ROW] {
    let mut layer = [KanataAction::NoOp; KEYS_IN_ROW];

    for (i, ac) in layer.iter_mut().enumerate() {
        *ac = OsCode::from_u16(i as u16)
            .map(|osc| Action::KeyCode(osc.into()))
            .unwrap_or(Action::NoOp);
    }
    // Ensure 0-index is no-op.
    layer[0] = KanataAction::NoOp;
    layer
}
