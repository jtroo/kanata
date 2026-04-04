use super::*;

use crate::anyhow_expr;
use crate::bail;
use crate::bail_expr;

pub(crate) fn parse_unmod(
    unmod_type: &str,
    ac_params: &[SExpr],
    s: &ParserState,
) -> Result<&'static KanataAction> {
    const ERR_MSG: &str = "expects expects at least one key name";
    if ac_params.is_empty() {
        bail!("{unmod_type} {ERR_MSG}\nfound {} items", ac_params.len());
    }

    let mut mods = UnmodMods::all();
    let mut params = ac_params;
    // Parse the optional first-list that specifies the mod keys to use.
    if let Some(mod_list) = ac_params[0].list(s.vars()) {
        if unmod_type != UNMOD {
            bail_expr!(
                &ac_params[0],
                "{unmod_type} only expects key names but found a list"
            );
        }
        mods = mod_list
            .iter()
            .try_fold(UnmodMods::empty(), |mod_flags, mod_key| {
                let flag = mod_key
                    .atom(s.vars())
                    .and_then(str_to_oscode)
                    .and_then(|osc| match osc {
                        OsCode::KEY_LEFTSHIFT => Some(UnmodMods::LSft),
                        OsCode::KEY_RIGHTSHIFT => Some(UnmodMods::RSft),
                        OsCode::KEY_LEFTCTRL => Some(UnmodMods::LCtl),
                        OsCode::KEY_RIGHTCTRL => Some(UnmodMods::RCtl),
                        OsCode::KEY_LEFTMETA => Some(UnmodMods::LMet),
                        OsCode::KEY_RIGHTMETA => Some(UnmodMods::RMet),
                        OsCode::KEY_LEFTALT => Some(UnmodMods::LAlt),
                        OsCode::KEY_RIGHTALT => Some(UnmodMods::RAlt),
                        _ => None,
                    })
                    .ok_or_else(|| {
                        anyhow_expr!(
                            mod_key,
                            "{UNMOD} expects modifier key names within the modifier list."
                        )
                    })?;
                if !(mod_flags & flag).is_empty() {
                    bail_expr!(
                        mod_key,
                        "Duplicate key name in modifier key list is not allowed."
                    );
                }
                Ok::<_, ParseError>(mod_flags | flag)
            })?;
        if mods.is_empty() {
            bail_expr!(&ac_params[0], "an empty modifier key list is invalid");
        }
        if ac_params[1..].is_empty() {
            bail!("at least one key is required after the modifier key list");
        }
        params = &ac_params[1..];
    }

    let keys: Vec<KeyCode> = params.iter().try_fold(Vec::new(), |mut keys, param| {
        keys.push(
            param
                .atom(s.vars())
                .and_then(str_to_oscode)
                .ok_or_else(|| {
                    anyhow_expr!(
                        &ac_params[0],
                        "{unmod_type} {ERR_MSG}\nfound invalid key name"
                    )
                })?
                .into(),
        );
        Ok::<_, ParseError>(keys)
    })?;
    let keys = s.a.sref_vec(keys);
    match unmod_type {
        UNMOD => custom(CustomAction::Unmodded { keys, mods }, &s.a),
        UNSHIFT => custom(CustomAction::Unshifted { keys }, &s.a),
        _ => panic!("Unknown unmod type {unmod_type}"),
    }
}
